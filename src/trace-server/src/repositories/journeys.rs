//! `journeys`, `journey_legs`, and the per-mode `journey_flights` and
//! `journey_drives` tables.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{
    core::error::{segment_constraint_error, ApiError},
    core::sql_builder::{Insert, Select},
    models::entities::journey::{DriveRow, Journey, JourneyTotals, LegRow},
    models::requests::journey::{
        ValidatedBooking, ValidatedDrive, ValidatedJourney, ValidatedSegment,
    },
    models::responses::journey::{
        DriveResponse, FlightResponse, JourneyResponse, PlaceResponse, SegmentResponse,
        VehicleResponse,
    },
};

// ---------------------------------------------------------------------------

/// The journey row, and whether this request is the one that created it.
pub enum JourneyUpsert {
    Created(Uuid),
    AlreadyExists(Uuid),
}

/// Insert the journey, or return the one an identical earlier request created.
///
/// This runs *first*, before any segment or reference row, so a replay costs one
/// statement and creates nothing. Without an `idempotency_key` every call is a
/// new journey — two trips can legitimately share a title and dates, so there is
/// no natural key to fall back on.
pub async fn insert(
    conn: &mut PgConnection,
    journey: &ValidatedJourney,
) -> Result<JourneyUpsert, ApiError> {
    if let Some(key) = journey.idempotency_key.as_deref() {
        if let Some(id) = select_by_key(conn, journey.user_id, key).await? {
            return Ok(JourneyUpsert::AlreadyExists(id));
        }
    }

    let inserted: Option<Uuid> = Insert::into("journeys")
        .set("user_id", journey.user_id)
        .set("idempotency_key", &journey.idempotency_key)
        .set("title", &journey.title)
        .set("description", &journey.description)
        .set("started_at", journey.started_at)
        .set("ended_at", journey.ended_at)
        .set("status", &journey.status)
        .set("visibility", &journey.visibility)
        .set_cast("metadata", &journey.metadata, "jsonb")
        .on_conflict_do_nothing_where(
            &["user_id", "idempotency_key"],
            "idempotency_key IS NOT NULL",
        )
        .returning(&["id"])
        .fetch_optional_scalar(conn)
        .await?;

    if let Some(id) = inserted {
        return Ok(JourneyUpsert::Created(id));
    }

    // The insert conflicted, so a concurrent request got there first.
    let key = journey
        .idempotency_key
        .as_deref()
        .expect("a conflict is only possible when an idempotency key was sent");

    select_by_key(conn, journey.user_id, key)
        .await?
        .map(JourneyUpsert::AlreadyExists)
        .ok_or_else(|| {
            ApiError::Conflict(
                "this journey is being created by another request; retry".to_string(),
            )
        })
}

async fn select_by_key(
    conn: &mut PgConnection,
    user_id: Uuid,
    key: &str,
) -> Result<Option<Uuid>, ApiError> {
    Select::from("journeys")
        .columns(&["id"])
        .where_eq("user_id", user_id)
        .where_eq("idempotency_key", key)
        .fetch_optional_scalar(conn)
        .await
        .map_err(ApiError::Database)
}

// ---------------------------------------------------------------------------

/// Assemble a journey with its segments, their endpoints, and their per-mode
/// details. Four queries regardless of how many segments there are.
pub async fn load(conn: &mut PgConnection, journey_id: Uuid) -> Result<JourneyResponse, ApiError> {
    let journey: Journey = Select::from("journeys")
        .columns(&[
            "id",
            "user_id",
            "idempotency_key",
            "title",
            "description",
            "started_at",
            "ended_at",
            "status",
            "visibility",
            "metadata",
            "created_at",
            "updated_at",
        ])
        .where_eq("id", journey_id)
        .fetch_optional(conn)
        .await?
        .ok_or(ApiError::NotFound)?;

    let totals: JourneyTotals = Select::from("journey_totals")
        .columns(&[
            "segment_count",
            "mode_count",
            "total_distance_miles",
            "total_duration_minutes",
            "first_departure_at",
            "last_arrival_at",
        ])
        .where_eq("journey_id", journey_id)
        .fetch_one(conn)
        .await?;

    let rows: Vec<LegRow> = Select::from("journey_legs")
        .columns(&[
            "id",
            "position",
            "mode",
            "origin_place_id",
            "destination_place_id",
            "started_at",
            "ended_at",
            "duration_minutes",
            "distance_miles",
            "notes",
            "metadata",
        ])
        .where_eq("journey_id", journey_id)
        .order_by("position")
        .fetch_all(conn)
        .await?;

    let leg_ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let place_ids: Vec<i64> = rows
        .iter()
        .flat_map(|row| [row.origin_place_id, row.destination_place_id])
        .flatten()
        .collect();

    let places = load_places(conn, &place_ids).await?;
    let mut flights = load_flights(conn, &leg_ids).await?;
    let mut drives = load_drives(conn, &leg_ids).await?;

    let segments = rows
        .into_iter()
        .map(|row| SegmentResponse {
            origin: row.origin_place_id.and_then(|id| places.get(&id).cloned()),
            destination: row
                .destination_place_id
                .and_then(|id| places.get(&id).cloned()),
            flight: flights.remove(&row.id),
            drive: drives.remove(&row.id),
            id: row.id,
            position: row.position,
            mode: row.mode,
            started_at: row.started_at,
            ended_at: row.ended_at,
            duration_minutes: row.duration_minutes,
            distance_miles: row.distance_miles,
            notes: row.notes,
            metadata: row.metadata,
        })
        .collect();

    Ok(JourneyResponse {
        journey,
        totals,
        segments,
    })
}

async fn load_places(
    conn: &mut PgConnection,
    place_ids: &[i64],
) -> Result<HashMap<i64, PlaceResponse>, ApiError> {
    if place_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let places: Vec<PlaceResponse> = Select::from("places p")
        .columns(&[
            "p.id",
            "p.name",
            "p.kind",
            "a.iata_code",
            "p.city",
            "p.country_code",
            "p.latitude::float8 AS latitude",
            "p.longitude::float8 AS longitude",
            "p.timezone",
        ])
        .left_join("airports a", "a.id = p.airport_id")
        .where_any_of("p.id", place_ids)
        .fetch_all(conn)
        .await?;

    Ok(places.into_iter().map(|place| (place.id, place)).collect())
}

async fn load_flights(
    conn: &mut PgConnection,
    leg_ids: &[Uuid],
) -> Result<HashMap<Uuid, FlightResponse>, ApiError> {
    if leg_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows: Vec<FlightResponse> = Select::from("journey_flights jf")
        .columns(&[
            "jf.leg_id",
            "jf.flight_id",
            "f.airline_id",
            "f.flight_number",
            "f.origin_airport_id",
            "f.destination_airport_id",
            "f.distance_miles",
            "f.scheduled_departure_at",
            "f.scheduled_arrival_at",
            "f.status",
            "jf.seat",
            "jf.cabin",
            "jf.booking_reference",
            "jf.ticket_number",
        ])
        .join("flights f", "f.id = jf.flight_id")
        .where_any_of("jf.leg_id", leg_ids)
        .fetch_all(conn)
        .await?;

    Ok(rows.into_iter().map(|row| (row.leg_id, row)).collect())
}

async fn load_drives(
    conn: &mut PgConnection,
    leg_ids: &[Uuid],
) -> Result<HashMap<Uuid, DriveResponse>, ApiError> {
    if leg_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows: Vec<DriveRow> = Select::from("journey_drives jd")
        .columns(&[
            "jd.leg_id",
            "jd.role",
            "jd.route_polyline",
            "v.id AS vehicle_id",
            "v.nickname",
            "v.make",
            "v.model",
            "v.year",
        ])
        .left_join("vehicles v", "v.id = jd.vehicle_id")
        .where_any_of("jd.leg_id", leg_ids)
        .fetch_all(conn)
        .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.leg_id,
                DriveResponse {
                    vehicle: row.vehicle_id.map(|id| VehicleResponse {
                        id,
                        nickname: row.nickname,
                        make: row.make,
                        model: row.model,
                        year: row.year,
                    }),
                    role: row.role,
                    route_polyline: row.route_polyline,
                },
            )
        })
        .collect())
}

/// The parent-row values a segment insert needs, after the service has derived
/// whatever it could from the segment's mode.
pub struct LegWrite<'a> {
    pub journey_id: Uuid,
    pub segment: &'a ValidatedSegment,
    pub origin_place_id: Option<i64>,
    pub destination_place_id: Option<i64>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_minutes: Option<i32>,
    pub distance_miles: Option<i32>,
}

/// Insert the `journey_legs` row and return its id.
pub async fn insert_leg(conn: &mut PgConnection, write: &LegWrite<'_>) -> Result<Uuid, ApiError> {
    let prefix = format!("segments[{}]", write.segment.position - 1);

    Insert::into("journey_legs")
        .set("journey_id", write.journey_id)
        .set("position", write.segment.position)
        .set("mode", &write.segment.mode)
        .set("origin_place_id", write.origin_place_id)
        .set("destination_place_id", write.destination_place_id)
        .set("started_at", write.started_at)
        .set("ended_at", write.ended_at)
        .set("duration_minutes", write.duration_minutes)
        .set("distance_miles", write.distance_miles)
        .set("notes", &write.segment.notes)
        .set_cast("metadata", &write.segment.metadata, "jsonb")
        .returning(&["id"])
        .fetch_one_scalar(conn)
        .await
        .map_err(|err| segment_constraint_error(err, &prefix))
}

/// Attach the traveller's booking to a flight segment.
pub async fn insert_flight(
    conn: &mut PgConnection,
    leg_id: Uuid,
    flight_id: Uuid,
    booking: &ValidatedBooking,
) -> Result<(), ApiError> {
    Insert::into("journey_flights")
        .set("leg_id", leg_id)
        .set("flight_id", flight_id)
        .set("seat", &booking.seat)
        .set("cabin", &booking.cabin)
        .set("booking_reference", &booking.booking_reference)
        .set("ticket_number", &booking.ticket_number)
        .execute(conn)
        .await?;

    Ok(())
}

/// Attach the vehicle and route to a drive segment.
pub async fn insert_drive(
    conn: &mut PgConnection,
    leg_id: Uuid,
    vehicle_id: Option<i64>,
    drive: &ValidatedDrive,
) -> Result<(), ApiError> {
    Insert::into("journey_drives")
        .set("leg_id", leg_id)
        .set("vehicle_id", vehicle_id)
        .set("role", &drive.role)
        .set("route_polyline", &drive.route_polyline)
        .execute(conn)
        .await?;

    Ok(())
}
