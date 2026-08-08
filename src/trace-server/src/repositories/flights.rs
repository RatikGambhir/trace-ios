//! `flights`, keyed on the natural (airline, number, departure) triple.

use sqlx::PgConnection;

use crate::{
    core::error::{check_violation, ApiError},
    core::sql_builder::{Insert, Select},
    models::entities::flight::Flight,
    models::requests::flight::ValidatedFlight,
};

/// Every column of `flights`, in one place — the insert returns exactly this
/// set, the idempotent read-back selects it, and `Flight` decodes it.
const FLIGHT_COLUMNS: &[&str] = &[
    "id",
    "airline_id",
    "flight_number",
    "origin_airport_id",
    "destination_airport_id",
    "distance_miles",
    "scheduled_departure_at",
    "scheduled_arrival_at",
    "actual_departure_at",
    "actual_arrival_at",
    "status",
    "departure_terminal",
    "departure_gate",
    "arrival_terminal",
    "arrival_gate",
    "aircraft_type",
    "aircraft_registration",
    "created_at",
    "updated_at",
];

/// The triple that makes a flight the same flight: `flights_unique_instance`.
const NATURAL_KEY: &[&str] = &["airline_id", "flight_number", "scheduled_departure_at"];

/// The outcome of [`insert`]: either the row we just wrote, or the one an
/// identical earlier request already wrote.
pub enum FlightUpsert {
    Created(Flight),
    AlreadyExists(Flight),
}

impl FlightUpsert {
    /// The row, either way. Callers who only need the flight — a journey
    /// segment, say — do not care which request created it.
    pub fn into_row(self) -> Flight {
        match self {
            FlightUpsert::Created(row) | FlightUpsert::AlreadyExists(row) => row,
        }
    }
}

/// Insert the flight, or return the existing row for the same
/// `(airline, flight_number, scheduled_departure_at)`.
///
/// That triple is [`NATURAL_KEY`], which makes it the natural idempotency key:
/// replaying a request is a read, not a duplicate and not a 409.
pub async fn insert(
    conn: &mut PgConnection,
    flight: &ValidatedFlight,
    airline_id: i64,
    origin_airport_id: i64,
    destination_airport_id: i64,
    distance_miles: Option<i32>,
) -> Result<FlightUpsert, ApiError> {
    let inserted: Option<Flight> = Insert::into("flights")
        .set("airline_id", airline_id)
        .set("flight_number", &flight.flight_number)
        .set("origin_airport_id", origin_airport_id)
        .set("destination_airport_id", destination_airport_id)
        .set("distance_miles", distance_miles)
        .set("scheduled_departure_at", flight.scheduled_departure_at)
        .set("scheduled_arrival_at", flight.scheduled_arrival_at)
        .set("actual_departure_at", flight.actual_departure_at)
        .set("actual_arrival_at", flight.actual_arrival_at)
        .set("status", &flight.status)
        .set("departure_terminal", &flight.departure_terminal)
        .set("departure_gate", &flight.departure_gate)
        .set("arrival_terminal", &flight.arrival_terminal)
        .set("arrival_gate", &flight.arrival_gate)
        .set("aircraft_type", &flight.aircraft_type)
        .set("aircraft_registration", &flight.aircraft_registration)
        .on_conflict_do_nothing(NATURAL_KEY)
        .returning(FLIGHT_COLUMNS)
        .fetch_optional(conn)
        .await
        .map_err(check_violation)?;

    if let Some(flight) = inserted {
        return Ok(FlightUpsert::Created(flight));
    }

    Select::from("flights")
        .columns(FLIGHT_COLUMNS)
        .where_eq("airline_id", airline_id)
        .where_eq("flight_number", &flight.flight_number)
        .where_eq("scheduled_departure_at", flight.scheduled_departure_at)
        .fetch_optional(conn)
        .await?
        .map(FlightUpsert::AlreadyExists)
        .ok_or_else(|| {
            ApiError::Conflict("this flight is being created by another request; retry".to_string())
        })
}
