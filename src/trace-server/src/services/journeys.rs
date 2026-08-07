//! Recording a trip: the transaction, the idempotency decision, and everything
//! the server derives rather than accepts.
//!
//! Three rules shape this module:
//!
//! * **Atomic** — journey, segments, places, airports, airlines, flights, and
//!   vehicles all go in one transaction. A failure on the last segment leaves
//!   none of the reference rows the earlier ones created.
//! * **Idempotent** — opt-in, via `idempotency_key`. The journey row is written
//!   *first*, so a replay conflicts before anything downstream is touched.
//! * **Derived where derivable** — a flight segment takes its endpoints,
//!   distance, and duration from the flight it references.

use std::collections::HashMap;

use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::{
    core::{error::ApiError, validation::field},
    models::requests::journey::{
        InsertJourneyRequest, SegmentDetail, ValidatedJourney, ValidatedPlace, ValidatedSegment,
        ValidatedVehicle,
    },
    models::responses::journey::JourneyResponse,
    repositories::{journeys, places, users, vehicles},
    services::flights,
};

/// Places resolved so far in one request.
///
/// A journey is a chain: leg N ends where leg N+1 begins, and the caller
/// describes that waypoint twice. Without this, each description would insert
/// its own row and the trip would end up with two "Cotswolds cottage" places
/// that are the same building.
#[derive(Default)]
pub struct PlaceCache(HashMap<String, i64>);

/// Record a trip and everything it is made of.
///
/// Returns the stored journey and whether this call is the one that created it —
/// the handler turns that into `201` or `200`.
pub async fn create(
    pool: &PgPool,
    request: InsertJourneyRequest,
) -> Result<(bool, JourneyResponse), ApiError> {
    let journey = request.validate()?;

    // Dropping `tx` without committing rolls back, so every `?` below leaves the
    // database untouched.
    let mut tx = pool.begin().await?;

    if !users::exists(&mut tx, journey.user_id).await? {
        return Err(ApiError::Validation(vec![format!(
            "user_id {} does not exist",
            journey.user_id
        )]));
    }

    // The journey row goes in first. A replay conflicts here, before any segment
    // or reference row is touched, so it costs one statement and creates
    // nothing.
    let journey_id = match journeys::insert(&mut tx, &journey).await? {
        journeys::JourneyUpsert::AlreadyExists(id) => {
            let response = journeys::load(&mut tx, id).await?;
            tx.commit().await?;

            tracing::info!(journey_id = %id, "journey already recorded");
            return Ok((false, response));
        }
        journeys::JourneyUpsert::Created(id) => id,
    };

    // Shared across the segments so a waypoint two legs both name — the airport
    // one leg ends at and the next begins from — resolves to a single place.
    let mut cache = PlaceCache::default();

    for segment in &journey.segments {
        create_segment(&mut tx, &mut cache, journey_id, &journey, segment).await?;
    }

    let response = journeys::load(&mut tx, journey_id).await?;

    tx.commit().await?;

    tracing::info!(
        journey_id = %journey_id,
        segments = response.segments.len(),
        distance_miles = response.totals.total_distance_miles,
        "created journey"
    );

    Ok((true, response))
}

/// Read a trip back with its segments and their per-mode details.
pub async fn get(pool: &PgPool, id: Uuid) -> Result<JourneyResponse, ApiError> {
    let mut conn = pool.acquire().await?;

    journeys::load(&mut conn, id).await
}

/// Write one segment and everything it references.
async fn create_segment(
    conn: &mut PgConnection,
    cache: &mut PlaceCache,
    journey_id: Uuid,
    journey: &ValidatedJourney,
    segment: &ValidatedSegment,
) -> Result<(), ApiError> {
    let prefix = format!("segments[{}]", segment.position - 1);

    let mut write = journeys::LegWrite {
        journey_id,
        segment,
        origin_place_id: None,
        destination_place_id: None,
        started_at: segment.started_at,
        ended_at: segment.ended_at,
        duration_minutes: segment.duration_minutes,
        distance_miles: segment.distance_miles,
    };
    let mut resolved_flight = None;

    if let SegmentDetail::Flight { flight, .. } = &segment.detail {
        // A flight segment's endpoints, distance, and duration all come from the
        // flight, so resolve it before writing the parent row.
        let resolved = flights::resolve(conn, flight, &field(&prefix, "flight")).await?;

        let origin_place = places::for_airport(conn, resolved.origin.id).await?;
        let destination_place = places::for_airport(conn, resolved.destination.id).await?;

        // Seed the cache so a later leg naming the same airport reuses the row.
        cache
            .0
            .insert(format!("airport:{}", flight.origin.iata_code), origin_place);
        cache.0.insert(
            format!("airport:{}", flight.destination.iata_code),
            destination_place,
        );

        write.origin_place_id = Some(origin_place);
        write.destination_place_id = Some(destination_place);
        write.distance_miles = segment.distance_miles.or(resolved.flight.distance_miles);
        write.started_at = write
            .started_at
            .or(Some(resolved.flight.scheduled_departure_at));
        write.ended_at = write
            .ended_at
            .or(Some(resolved.flight.scheduled_arrival_at));
        write.duration_minutes = write.duration_minutes.or_else(|| {
            let block =
                resolved.flight.scheduled_arrival_at - resolved.flight.scheduled_departure_at;
            i32::try_from(block.num_minutes()).ok()
        });

        resolved_flight = Some(resolved.flight.id);
    } else {
        if let Some(place) = &segment.origin {
            write.origin_place_id =
                Some(resolve_place(conn, cache, place, &field(&prefix, "origin")).await?);
        }
        if let Some(place) = &segment.destination {
            write.destination_place_id =
                Some(resolve_place(conn, cache, place, &field(&prefix, "destination")).await?);
        }
    }

    let leg_id = journeys::insert_leg(conn, &write).await?;

    match &segment.detail {
        SegmentDetail::Flight { booking, .. } => {
            let flight_id = resolved_flight
                .expect("a flight segment always resolves its flight before the parent insert");

            journeys::insert_flight(conn, leg_id, flight_id, booking).await?;
        }
        SegmentDetail::Drive(drive) => {
            let vehicle_id = match &drive.vehicle {
                Some(vehicle) => Some(
                    resolve_vehicle(
                        conn,
                        journey.user_id,
                        vehicle,
                        &field(&prefix, "drive.vehicle"),
                    )
                    .await?,
                ),
                None => None,
            };

            journeys::insert_drive(conn, leg_id, vehicle_id, drive).await?;
        }
        SegmentDetail::Bare => {}
    }

    Ok(())
}

/// Resolve a place descriptor to a `places.id`, creating the row when the
/// descriptor is a new place or a new airport, and reusing anything this request
/// has already resolved.
async fn resolve_place(
    conn: &mut PgConnection,
    cache: &mut PlaceCache,
    place: &ValidatedPlace,
    field: &str,
) -> Result<i64, ApiError> {
    let key = place.cache_key();
    if let Some(id) = cache.0.get(&key) {
        return Ok(*id);
    }

    let id = match place {
        ValidatedPlace::Saved(id) => places::find(conn, *id).await?.ok_or_else(|| {
            ApiError::Validation(vec![format!("{field}.id {id} is not a known place")])
        })?,
        ValidatedPlace::Airport(airport) => {
            // Creating the airport is enough: `airports_sync_place` mirrors it
            // into `places` inside this same transaction.
            let resolved = crate::repositories::airports::resolve(conn, airport, field).await?;
            places::for_airport(conn, resolved.id).await?
        }
        ValidatedPlace::Custom(place) => places::insert(conn, place).await?,
    };

    cache.0.insert(key, id);

    Ok(id)
}

/// Get-or-create the vehicle. A saved id must belong to this user; a new vehicle
/// with a nickname is keyed on it, so naming the same car across two journeys
/// reuses one row instead of piling up duplicates.
async fn resolve_vehicle(
    conn: &mut PgConnection,
    user_id: Uuid,
    vehicle: &ValidatedVehicle,
    field: &str,
) -> Result<i64, ApiError> {
    match vehicle {
        ValidatedVehicle::Saved(id) => vehicles::find_for_user(conn, user_id, *id)
            .await?
            .ok_or_else(|| {
                ApiError::Validation(vec![format!(
                    "{field}.id {id} is not a vehicle belonging to this user"
                )])
            }),
        ValidatedVehicle::New(vehicle) => {
            if let Some(nickname) = vehicle.nickname.as_deref() {
                if let Some(id) = vehicles::find_by_nickname(conn, user_id, nickname).await? {
                    return Ok(id);
                }
            }

            if let Some(id) = vehicles::insert(conn, user_id, vehicle).await? {
                return Ok(id);
            }

            let nickname = vehicle
                .nickname
                .as_deref()
                .expect("a conflict is only possible when a nickname was sent");

            vehicles::find_by_nickname(conn, user_id, nickname)
                .await?
                .ok_or_else(|| {
                    ApiError::Conflict(format!(
                        "vehicle {nickname} is being created by another request; retry"
                    ))
                })
        }
    }
}
