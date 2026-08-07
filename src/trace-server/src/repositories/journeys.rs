//! `journeys`, `journey_segments`, and the per-mode `segment_*` tables.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{
    core::error::{segment_constraint_error, ApiError},
    models::entities::journey::{DriveRow, Journey, JourneyTotals, SegmentRow},
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

    let inserted: Option<(Uuid,)> = sqlx::query_as(
        r#"
        INSERT INTO journeys (
            user_id, idempotency_key, title, description,
            started_at, ended_at, status, visibility, metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9::jsonb)
        ON CONFLICT (user_id, idempotency_key) WHERE idempotency_key IS NOT NULL
        DO NOTHING
        RETURNING id
        "#,
    )
    .bind(journey.user_id)
    .bind(&journey.idempotency_key)
    .bind(&journey.title)
    .bind(&journey.description)
    .bind(journey.started_at)
    .bind(journey.ended_at)
    .bind(&journey.status)
    .bind(&journey.visibility)
    .bind(&journey.metadata)
    .fetch_optional(&mut *conn)
    .await?;

    if let Some((id,)) = inserted {
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
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM journeys WHERE user_id = $1 AND idempotency_key = $2")
            .bind(user_id)
            .bind(key)
            .fetch_optional(conn)
            .await?;

    Ok(row.map(|(id,)| id))
}

// ---------------------------------------------------------------------------

/// Assemble a journey with its segments, their endpoints, and their per-mode
/// details. Four queries regardless of how many segments there are.
pub async fn load(conn: &mut PgConnection, journey_id: Uuid) -> Result<JourneyResponse, ApiError> {
    let journey = sqlx::query_as::<_, Journey>(
        r#"
        SELECT id, user_id, idempotency_key, title, description, started_at, ended_at,
               status, visibility, metadata, created_at, updated_at
        FROM journeys
        WHERE id = $1
        "#,
    )
    .bind(journey_id)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(ApiError::NotFound)?;

    let totals = sqlx::query_as::<_, JourneyTotals>(
        r#"
        SELECT segment_count, mode_count, total_distance_miles, total_duration_minutes,
               first_departure_at, last_arrival_at
        FROM journey_totals
        WHERE journey_id = $1
        "#,
    )
    .bind(journey_id)
    .fetch_one(&mut *conn)
    .await?;

    let rows = sqlx::query_as::<_, SegmentRow>(
        r#"
        SELECT id, position, mode, origin_place_id, destination_place_id,
               started_at, ended_at, duration_minutes, distance_miles, notes, metadata
        FROM journey_segments
        WHERE journey_id = $1
        ORDER BY position
        "#,
    )
    .bind(journey_id)
    .fetch_all(&mut *conn)
    .await?;

    let segment_ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let place_ids: Vec<i64> = rows
        .iter()
        .flat_map(|row| [row.origin_place_id, row.destination_place_id])
        .flatten()
        .collect();

    let places = load_places(conn, &place_ids).await?;
    let mut flights = load_segment_flights(conn, &segment_ids).await?;
    let mut drives = load_segment_drives(conn, &segment_ids).await?;

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

    let places = sqlx::query_as::<_, PlaceResponse>(
        r#"
        SELECT p.id, p.name, p.kind, a.iata_code, p.city, p.country_code,
               p.latitude::float8 AS latitude, p.longitude::float8 AS longitude, p.timezone
        FROM places p
        LEFT JOIN airports a ON a.id = p.airport_id
        WHERE p.id = ANY($1)
        "#,
    )
    .bind(place_ids)
    .fetch_all(conn)
    .await?;

    Ok(places.into_iter().map(|place| (place.id, place)).collect())
}

async fn load_segment_flights(
    conn: &mut PgConnection,
    segment_ids: &[Uuid],
) -> Result<HashMap<Uuid, FlightResponse>, ApiError> {
    if segment_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query_as::<_, FlightResponse>(
        r#"
        SELECT sf.segment_id, sf.flight_id, f.airline_id, f.flight_number,
               f.origin_airport_id, f.destination_airport_id, f.distance_miles,
               f.scheduled_departure_at, f.scheduled_arrival_at, f.status,
               sf.seat, sf.cabin, sf.booking_reference, sf.ticket_number
        FROM segment_flights sf
        JOIN flights f ON f.id = sf.flight_id
        WHERE sf.segment_id = ANY($1)
        "#,
    )
    .bind(segment_ids)
    .fetch_all(conn)
    .await?;

    Ok(rows.into_iter().map(|row| (row.segment_id, row)).collect())
}

async fn load_segment_drives(
    conn: &mut PgConnection,
    segment_ids: &[Uuid],
) -> Result<HashMap<Uuid, DriveResponse>, ApiError> {
    if segment_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query_as::<_, DriveRow>(
        r#"
        SELECT sd.segment_id, sd.role, sd.route_polyline,
               v.id AS vehicle_id, v.nickname, v.make, v.model, v.year
        FROM segment_drives sd
        LEFT JOIN vehicles v ON v.id = sd.vehicle_id
        WHERE sd.segment_id = ANY($1)
        "#,
    )
    .bind(segment_ids)
    .fetch_all(conn)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.segment_id,
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
pub struct SegmentWrite<'a> {
    pub journey_id: Uuid,
    pub segment: &'a ValidatedSegment,
    pub origin_place_id: Option<i64>,
    pub destination_place_id: Option<i64>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_minutes: Option<i32>,
    pub distance_miles: Option<i32>,
}

/// Insert the `journey_segments` row and return its id.
pub async fn insert_segment(
    conn: &mut PgConnection,
    write: &SegmentWrite<'_>,
) -> Result<Uuid, ApiError> {
    let prefix = format!("segments[{}]", write.segment.position - 1);

    let (id,): (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO journey_segments (
            journey_id, position, mode, origin_place_id, destination_place_id,
            started_at, ended_at, duration_minutes, distance_miles, notes, metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::jsonb)
        RETURNING id
        "#,
    )
    .bind(write.journey_id)
    .bind(write.segment.position)
    .bind(&write.segment.mode)
    .bind(write.origin_place_id)
    .bind(write.destination_place_id)
    .bind(write.started_at)
    .bind(write.ended_at)
    .bind(write.duration_minutes)
    .bind(write.distance_miles)
    .bind(&write.segment.notes)
    .bind(&write.segment.metadata)
    .fetch_one(conn)
    .await
    .map_err(|err| segment_constraint_error(err, &prefix))?;

    Ok(id)
}

/// Attach the traveller's booking to a flight segment.
pub async fn insert_segment_flight(
    conn: &mut PgConnection,
    segment_id: Uuid,
    flight_id: Uuid,
    booking: &ValidatedBooking,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        INSERT INTO segment_flights (
            segment_id, flight_id, seat, cabin, booking_reference, ticket_number
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(segment_id)
    .bind(flight_id)
    .bind(&booking.seat)
    .bind(&booking.cabin)
    .bind(&booking.booking_reference)
    .bind(&booking.ticket_number)
    .execute(conn)
    .await?;

    Ok(())
}

/// Attach the vehicle and route to a drive segment.
pub async fn insert_segment_drive(
    conn: &mut PgConnection,
    segment_id: Uuid,
    vehicle_id: Option<i64>,
    drive: &ValidatedDrive,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        INSERT INTO segment_drives (segment_id, vehicle_id, role, route_polyline)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(segment_id)
    .bind(vehicle_id)
    .bind(&drive.role)
    .bind(&drive.route_polyline)
    .execute(conn)
    .await?;

    Ok(())
}
