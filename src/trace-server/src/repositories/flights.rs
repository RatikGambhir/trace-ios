//! `flights`, keyed on the natural (airline, number, departure) triple.

use sqlx::PgConnection;

use crate::{
    core::error::{check_violation, ApiError},
    models::flight::{Flight, ValidatedFlight},
};

/// Every column of `flights`, in one place — the insert and the idempotent
/// read-back both return exactly this set, and `Flight` decodes it.
const FLIGHT_COLUMNS: &str = "id, airline_id, flight_number, origin_airport_id, \
     destination_airport_id, distance_miles, scheduled_departure_at, scheduled_arrival_at, \
     actual_departure_at, actual_arrival_at, status, departure_terminal, departure_gate, \
     arrival_terminal, arrival_gate, aircraft_type, aircraft_registration, created_at, updated_at";

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
/// That triple is `flights_unique_instance`, which makes it the natural
/// idempotency key: replaying a request is a read, not a duplicate and not a
/// 409.
pub async fn insert(
    conn: &mut PgConnection,
    flight: &ValidatedFlight,
    airline_id: i64,
    origin_airport_id: i64,
    destination_airport_id: i64,
    distance_miles: Option<i32>,
) -> Result<FlightUpsert, ApiError> {
    // `FLIGHT_COLUMNS` is a compile-time constant, never caller input; the
    // values all travel as bind parameters.
    let insert = format!(
        r#"
        INSERT INTO flights (
            airline_id, flight_number, origin_airport_id, destination_airport_id,
            distance_miles, scheduled_departure_at, scheduled_arrival_at,
            actual_departure_at, actual_arrival_at, status,
            departure_terminal, departure_gate, arrival_terminal, arrival_gate,
            aircraft_type, aircraft_registration
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
        ON CONFLICT (airline_id, flight_number, scheduled_departure_at) DO NOTHING
        RETURNING {FLIGHT_COLUMNS}
        "#
    );

    let inserted = sqlx::query_as::<_, Flight>(&insert)
        .bind(airline_id)
        .bind(&flight.flight_number)
        .bind(origin_airport_id)
        .bind(destination_airport_id)
        .bind(distance_miles)
        .bind(flight.scheduled_departure_at)
        .bind(flight.scheduled_arrival_at)
        .bind(flight.actual_departure_at)
        .bind(flight.actual_arrival_at)
        .bind(&flight.status)
        .bind(&flight.departure_terminal)
        .bind(&flight.departure_gate)
        .bind(&flight.arrival_terminal)
        .bind(&flight.arrival_gate)
        .bind(&flight.aircraft_type)
        .bind(&flight.aircraft_registration)
        .fetch_optional(&mut *conn)
        .await
        .map_err(check_violation)?;

    if let Some(flight) = inserted {
        return Ok(FlightUpsert::Created(flight));
    }

    let select = format!(
        r#"
        SELECT {FLIGHT_COLUMNS}
        FROM flights
        WHERE airline_id = $1 AND flight_number = $2 AND scheduled_departure_at = $3
        "#
    );

    sqlx::query_as::<_, Flight>(&select)
        .bind(airline_id)
        .bind(&flight.flight_number)
        .bind(flight.scheduled_departure_at)
        .fetch_optional(&mut *conn)
        .await?
        .map(FlightUpsert::AlreadyExists)
        .ok_or_else(|| {
            ApiError::Conflict("this flight is being created by another request; retry".to_string())
        })
}
