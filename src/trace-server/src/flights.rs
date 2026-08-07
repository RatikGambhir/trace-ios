//! Flights: the shared, real-world leg. Types, validation, and the database
//! work behind a flight segment of a journey.
//!
//! A flight is not owned by a user — AA100 on a given day is one row in
//! `flights` however many people were aboard. So this module resolves a flight
//! to its canonical row and leaves everything personal (seat, cabin, booking
//! reference) to `segment_flights`, over in [`crate::journeys`], which is what
//! `POST /api/v1/journeys` drives.
//!
//! Three rules shape it:
//!
//! * **Atomic** — airports, airline, and flight join whatever transaction the
//!   caller opened, so a failure part-way through leaves no half-created
//!   reference rows.
//! * **Idempotent** — resolving the same flight twice returns the row that is
//!   already stored instead of erroring or duplicating it.
//! * **Derived distance** — `distance_miles` is computed from the stored airport
//!   coordinates, never taken from the caller.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use crate::{
    error::ApiError,
    geo::{great_circle_miles, Coordinates},
};

/// The statuses allowed by the `flights_status_check` constraint.
const FLIGHT_STATUSES: [&str; 8] = [
    "scheduled",
    "boarding",
    "departed",
    "in_air",
    "landed",
    "delayed",
    "cancelled",
    "diverted",
];

const DEFAULT_STATUS: &str = "scheduled";

// Column widths from `0002_create_flights_schema.sql`. Checking them here turns
// what would be a 500 from the driver into a 422 that names the field.
const MAX_FLIGHT_NUMBER_LEN: usize = 10;
const MAX_NAME_LEN: usize = 150;
const MAX_CITY_LEN: usize = 100;
const MAX_TIMEZONE_LEN: usize = 50;
const MAX_TERMINAL_OR_GATE_LEN: usize = 10;
const MAX_AIRCRAFT_TYPE_LEN: usize = 50;
const MAX_AIRCRAFT_REGISTRATION_LEN: usize = 20;

/// Every column of `flights`, in one place — the insert and the idempotent
/// read-back both return exactly this set, and `Flight` decodes it.
const FLIGHT_COLUMNS: &str = "id, airline_id, flight_number, origin_airport_id, \
     destination_airport_id, distance_miles, scheduled_departure_at, scheduled_arrival_at, \
     actual_departure_at, actual_arrival_at, status, departure_terminal, departure_gate, \
     arrival_terminal, arrival_gate, aircraft_type, aircraft_registration, created_at, updated_at";

/// An airport as the caller describes it. Only `iata_code` is required: it is
/// the natural key, and everything else is needed just for the insert that
/// happens when the airport is new to us.
#[derive(Debug, Deserialize)]
pub struct AirportInput {
    pub iata_code: String,
    #[serde(default)]
    pub icao_code: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub city: Option<String>,
    #[serde(default)]
    pub country_code: Option<String>,
    #[serde(default)]
    pub latitude: Option<f64>,
    #[serde(default)]
    pub longitude: Option<f64>,
    #[serde(default)]
    pub timezone: Option<String>,
}

/// An airline as the caller describes it, on the same terms as `AirportInput`.
#[derive(Debug, Deserialize)]
pub struct AirlineInput {
    pub iata_code: String,
    #[serde(default)]
    pub icao_code: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

/// A flight as the caller describes it, nested inside a journey segment.
///
/// `distance_miles` is deliberately absent — the server derives it.
#[derive(Debug, Deserialize)]
pub struct FlightInput {
    pub airline: AirlineInput,
    pub flight_number: String,
    pub origin: AirportInput,
    pub destination: AirportInput,
    pub scheduled_departure_at: DateTime<Utc>,
    pub scheduled_arrival_at: DateTime<Utc>,
    #[serde(default)]
    pub actual_departure_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub actual_arrival_at: Option<DateTime<Utc>>,
    /// Optional; the column defaults to `scheduled`.
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub departure_terminal: Option<String>,
    #[serde(default)]
    pub departure_gate: Option<String>,
    #[serde(default)]
    pub arrival_terminal: Option<String>,
    #[serde(default)]
    pub arrival_gate: Option<String>,
    #[serde(default)]
    pub aircraft_type: Option<String>,
    #[serde(default)]
    pub aircraft_registration: Option<String>,
}

/// A row of `flights`, as returned to the caller.
#[derive(Debug, Serialize, FromRow)]
pub struct Flight {
    pub id: Uuid,
    pub airline_id: i64,
    pub flight_number: String,
    pub origin_airport_id: i64,
    pub destination_airport_id: i64,
    /// `NULL` when either airport has no coordinates on file.
    pub distance_miles: Option<i32>,
    pub scheduled_departure_at: DateTime<Utc>,
    pub scheduled_arrival_at: DateTime<Utc>,
    pub actual_departure_at: Option<DateTime<Utc>>,
    pub actual_arrival_at: Option<DateTime<Utc>>,
    pub status: String,
    pub departure_terminal: Option<String>,
    pub departure_gate: Option<String>,
    pub arrival_terminal: Option<String>,
    pub arrival_gate: Option<String>,
    pub aircraft_type: Option<String>,
    pub aircraft_registration: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A trimmed, range-checked `AirportInput`.
pub struct ValidatedAirport {
    pub iata_code: String,
    pub icao_code: Option<String>,
    pub name: Option<String>,
    pub city: Option<String>,
    pub country_code: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub timezone: Option<String>,
}

/// A trimmed, range-checked `AirlineInput`.
pub struct ValidatedAirline {
    pub iata_code: String,
    pub icao_code: Option<String>,
    pub name: Option<String>,
}

/// A trimmed, range-checked `FlightInput`.
pub struct ValidatedFlight {
    pub airline: ValidatedAirline,
    pub flight_number: String,
    pub origin: ValidatedAirport,
    pub destination: ValidatedAirport,
    pub scheduled_departure_at: DateTime<Utc>,
    pub scheduled_arrival_at: DateTime<Utc>,
    pub actual_departure_at: Option<DateTime<Utc>>,
    pub actual_arrival_at: Option<DateTime<Utc>>,
    pub status: String,
    pub departure_terminal: Option<String>,
    pub departure_gate: Option<String>,
    pub arrival_terminal: Option<String>,
    pub arrival_gate: Option<String>,
    pub aircraft_type: Option<String>,
    pub aircraft_registration: Option<String>,
}

/// An airport row after `resolve_airport` — the id the flight references, plus
/// the coordinates the distance is computed from.
#[derive(Debug, FromRow)]
pub struct ResolvedAirport {
    pub id: i64,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

impl ResolvedAirport {
    fn coordinates(&self) -> Option<Coordinates> {
        Some(Coordinates {
            latitude: self.latitude?,
            longitude: self.longitude?,
        })
    }
}

/// Great-circle distance between two resolved airports, rounded to whole miles.
/// `None` when either side has no coordinates on file — the column is nullable
/// precisely so that a sparsely populated airport does not block the flight.
pub fn distance_between(origin: &ResolvedAirport, destination: &ResolvedAirport) -> Option<i32> {
    let from = origin.coordinates()?;
    let to = destination.coordinates()?;

    Some(great_circle_miles(from, to).round() as i32)
}

/// Join a nesting prefix to a field name. A flight now arrives inside a journey
/// segment, so its errors have to say *which* segment: `segments[2].flight.origin`
/// rather than a bare `origin`.
pub fn field(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}.{name}")
    }
}

impl FlightInput {
    /// Check and normalise, collecting into a caller-owned error list so a
    /// journey can report problems across every segment at once.
    ///
    /// `prefix` names where this flight sits in the request body — pass `""`
    /// when it stands alone.
    pub fn validate_into(self, prefix: &str, errors: &mut Vec<String>) -> ValidatedFlight {
        let airline = self.airline.validate(prefix, errors);
        let origin = self.origin.validate(&field(prefix, "origin"), errors);
        let destination = self
            .destination
            .validate(&field(prefix, "destination"), errors);

        let flight_number = self.flight_number.trim().to_ascii_uppercase();
        let flight_number_field = field(prefix, "flight_number");
        if flight_number.is_empty() {
            errors.push(format!("{flight_number_field} must not be empty"));
        } else if flight_number.chars().count() > MAX_FLIGHT_NUMBER_LEN {
            errors.push(format!(
                "{flight_number_field} must be at most {MAX_FLIGHT_NUMBER_LEN} characters"
            ));
        } else if !flight_number.chars().all(|c| c.is_ascii_alphanumeric()) {
            errors.push(format!("{flight_number_field} must be alphanumeric"));
        }

        let status = self
            .status
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_STATUS.to_string());
        if !FLIGHT_STATUSES.contains(&status.as_str()) {
            errors.push(format!(
                "{} must be one of: {}",
                field(prefix, "status"),
                FLIGHT_STATUSES.join(", ")
            ));
        }

        if self.scheduled_arrival_at <= self.scheduled_departure_at {
            errors.push(format!(
                "{} must be after {}",
                field(prefix, "scheduled_arrival_at"),
                field(prefix, "scheduled_departure_at")
            ));
        }

        if let (Some(departure), Some(arrival)) = (self.actual_departure_at, self.actual_arrival_at)
        {
            if arrival <= departure {
                errors.push(format!(
                    "{} must be after {}",
                    field(prefix, "actual_arrival_at"),
                    field(prefix, "actual_departure_at")
                ));
            }
        }

        // Mirrors `flights_origin_destination_check`; caught here so the caller
        // gets a named field rather than a constraint name.
        if origin.iata_code == destination.iata_code {
            errors.push(format!(
                "{} and {} must be different airports",
                field(prefix, "origin"),
                field(prefix, "destination")
            ));
        }

        let departure_terminal = optional_text(
            self.departure_terminal,
            &field(prefix, "departure_terminal"),
            MAX_TERMINAL_OR_GATE_LEN,
            errors,
        );
        let departure_gate = optional_text(
            self.departure_gate,
            &field(prefix, "departure_gate"),
            MAX_TERMINAL_OR_GATE_LEN,
            errors,
        );
        let arrival_terminal = optional_text(
            self.arrival_terminal,
            &field(prefix, "arrival_terminal"),
            MAX_TERMINAL_OR_GATE_LEN,
            errors,
        );
        let arrival_gate = optional_text(
            self.arrival_gate,
            &field(prefix, "arrival_gate"),
            MAX_TERMINAL_OR_GATE_LEN,
            errors,
        );
        let aircraft_type = optional_text(
            self.aircraft_type,
            &field(prefix, "aircraft_type"),
            MAX_AIRCRAFT_TYPE_LEN,
            errors,
        );
        let aircraft_registration = optional_text(
            self.aircraft_registration,
            &field(prefix, "aircraft_registration"),
            MAX_AIRCRAFT_REGISTRATION_LEN,
            errors,
        );

        ValidatedFlight {
            airline,
            flight_number,
            origin,
            destination,
            scheduled_departure_at: self.scheduled_departure_at,
            scheduled_arrival_at: self.scheduled_arrival_at,
            actual_departure_at: self.actual_departure_at,
            actual_arrival_at: self.actual_arrival_at,
            status,
            departure_terminal,
            departure_gate,
            arrival_terminal,
            arrival_gate,
            aircraft_type,
            aircraft_registration,
        }
    }
}

impl AirportInput {
    pub(crate) fn validate(self, field: &str, errors: &mut Vec<String>) -> ValidatedAirport {
        let iata_code = fixed_code(&self.iata_code, &format!("{field}.iata_code"), 3, errors);
        let icao_code = self
            .icao_code
            .as_deref()
            .map(|code| code.trim())
            .filter(|code| !code.is_empty())
            .map(|code| fixed_code(code, &format!("{field}.icao_code"), 4, errors));

        // Latitude and longitude are useless apart — one without the other
        // yields no distance, so require the pair.
        match (self.latitude, self.longitude) {
            (Some(latitude), Some(longitude)) => {
                if !(-90.0..=90.0).contains(&latitude) {
                    errors.push(format!("{field}.latitude must be between -90 and 90"));
                }
                if !(-180.0..=180.0).contains(&longitude) {
                    errors.push(format!("{field}.longitude must be between -180 and 180"));
                }
            }
            (None, None) => {}
            _ => errors.push(format!(
                "{field}.latitude and {field}.longitude must be given together"
            )),
        }

        ValidatedAirport {
            iata_code,
            icao_code,
            name: optional_text(self.name, &format!("{field}.name"), MAX_NAME_LEN, errors),
            city: optional_text(self.city, &format!("{field}.city"), MAX_CITY_LEN, errors),
            country_code: self
                .country_code
                .as_deref()
                .map(|code| code.trim())
                .filter(|code| !code.is_empty())
                .map(|code| fixed_code(code, &format!("{field}.country_code"), 2, errors)),
            latitude: self.latitude,
            longitude: self.longitude,
            timezone: optional_text(
                self.timezone,
                &format!("{field}.timezone"),
                MAX_TIMEZONE_LEN,
                errors,
            ),
        }
    }
}

impl AirlineInput {
    fn validate(self, prefix: &str, errors: &mut Vec<String>) -> ValidatedAirline {
        let prefix = field(prefix, "airline");

        ValidatedAirline {
            iata_code: fixed_code(&self.iata_code, &field(&prefix, "iata_code"), 2, errors),
            icao_code: self
                .icao_code
                .as_deref()
                .map(|code| code.trim())
                .filter(|code| !code.is_empty())
                .map(|code| fixed_code(code, &field(&prefix, "icao_code"), 3, errors)),
            name: optional_text(self.name, &field(&prefix, "name"), MAX_NAME_LEN, errors),
        }
    }
}

/// Normalise a fixed-width alphanumeric code (IATA, ICAO, ISO country) and
/// record a message if it is the wrong shape. Returns the normalised value
/// either way so validation can carry on and report every problem at once.
fn fixed_code(value: &str, field: &str, len: usize, errors: &mut Vec<String>) -> String {
    let code = value.trim().to_ascii_uppercase();

    if code.chars().count() != len || !code.chars().all(|c| c.is_ascii_alphanumeric()) {
        errors.push(format!("{field} must be {len} alphanumeric characters"));
    }

    code
}

/// Trim an optional string, treat blank as absent, and length-check the rest.
fn optional_text(
    value: Option<String>,
    field: &str,
    max: usize,
    errors: &mut Vec<String>,
) -> Option<String> {
    let value = value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;

    if value.chars().count() > max {
        errors.push(format!("{field} must be at most {max} characters"));
    }

    Some(value)
}

/// Find the airport by IATA code, inserting it if we have never seen it.
///
/// Three steps, in order of likelihood: read (the common case, and no write),
/// insert-if-absent, then read again for the request that lost the race. The
/// insert is `ON CONFLICT DO NOTHING`, so two concurrent callers cannot both
/// create the row, and neither gets an error.
///
/// When the airport already exists its stored details win — this call never
/// overwrites them from the request body, so it stays a pure get-or-create.
pub async fn resolve_airport(
    conn: &mut PgConnection,
    airport: &ValidatedAirport,
    field: &str,
) -> Result<ResolvedAirport, ApiError> {
    if let Some(existing) = select_airport(conn, &airport.iata_code).await? {
        return Ok(existing);
    }

    // Unknown airport, so this is an insert — and the columns the insert needs
    // are no longer optional.
    let name = airport.name.as_deref().ok_or_else(|| {
        ApiError::Validation(vec![format!(
            "{field}.name is required: airport {} is not in the database yet",
            airport.iata_code
        )])
    })?;

    let inserted = sqlx::query_as::<_, ResolvedAirport>(
        r#"
        INSERT INTO airports (
            iata_code, icao_code, name, city, country_code, latitude, longitude, timezone
        )
        VALUES ($1, $2, $3, $4, $5, $6::float8::numeric, $7::float8::numeric, $8)
        ON CONFLICT (iata_code) DO NOTHING
        RETURNING id, latitude::float8 AS latitude, longitude::float8 AS longitude
        "#,
    )
    .bind(&airport.iata_code)
    .bind(&airport.icao_code)
    .bind(name)
    .bind(&airport.city)
    .bind(&airport.country_code)
    .bind(airport.latitude)
    .bind(airport.longitude)
    .bind(&airport.timezone)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|err| {
        unique_violation(err, |constraint| match constraint {
            "airports_icao_code_key" => Some(format!(
                "{field}.icao_code already belongs to a different airport"
            )),
            _ => None,
        })
    })?;

    if let Some(inserted) = inserted {
        tracing::info!(iata_code = %airport.iata_code, "inserted airport");
        return Ok(inserted);
    }

    // The insert conflicted, so someone else created it: read theirs.
    select_airport(conn, &airport.iata_code)
        .await?
        .ok_or_else(|| {
            ApiError::Conflict(format!(
                "airport {} is being created by another request; retry",
                airport.iata_code
            ))
        })
}

async fn select_airport(
    conn: &mut PgConnection,
    iata_code: &str,
) -> Result<Option<ResolvedAirport>, ApiError> {
    // DECIMAL(9,6) has no native Rust mapping in this build of sqlx, and the
    // coordinates only ever feed a float computation — cast on the way out.
    sqlx::query_as::<_, ResolvedAirport>(
        r#"
        SELECT id, latitude::float8 AS latitude, longitude::float8 AS longitude
        FROM airports
        WHERE iata_code = $1
        "#,
    )
    .bind(iata_code)
    .fetch_optional(conn)
    .await
    .map_err(ApiError::Database)
}

/// Get-or-create the airline by IATA code, on the same terms as
/// [`resolve_airport`].
pub async fn resolve_airline(
    conn: &mut PgConnection,
    airline: &ValidatedAirline,
) -> Result<i64, ApiError> {
    if let Some(id) = select_airline(conn, &airline.iata_code).await? {
        return Ok(id);
    }

    let name = airline.name.as_deref().ok_or_else(|| {
        ApiError::Validation(vec![format!(
            "airline.name is required: airline {} is not in the database yet",
            airline.iata_code
        )])
    })?;

    let inserted: Option<(i64,)> = sqlx::query_as(
        r#"
        INSERT INTO airlines (iata_code, icao_code, name)
        VALUES ($1, $2, $3)
        ON CONFLICT (iata_code) DO NOTHING
        RETURNING id
        "#,
    )
    .bind(&airline.iata_code)
    .bind(&airline.icao_code)
    .bind(name)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|err| {
        unique_violation(err, |constraint| match constraint {
            "airlines_icao_code_key" => {
                Some("airline.icao_code already belongs to a different airline".to_string())
            }
            _ => None,
        })
    })?;

    if let Some((id,)) = inserted {
        tracing::info!(iata_code = %airline.iata_code, "inserted airline");
        return Ok(id);
    }

    select_airline(conn, &airline.iata_code)
        .await?
        .ok_or_else(|| {
            ApiError::Conflict(format!(
                "airline {} is being created by another request; retry",
                airline.iata_code
            ))
        })
}

async fn select_airline(conn: &mut PgConnection, iata_code: &str) -> Result<Option<i64>, ApiError> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT id FROM airlines WHERE iata_code = $1")
        .bind(iata_code)
        .fetch_optional(conn)
        .await?;

    Ok(row.map(|(id,)| id))
}

/// The outcome of [`insert_flight`]: either the row we just wrote, or the one
/// an identical earlier request already wrote.
pub enum FlightUpsert {
    Created(Flight),
    AlreadyExists(Flight),
}

/// Insert the flight, or return the existing row for the same
/// `(airline, flight_number, scheduled_departure_at)`.
///
/// That triple is `flights_unique_instance`, which makes it the natural
/// idempotency key: replaying a request is a read, not a duplicate and not a
/// 409.
pub async fn insert_flight(
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

/// Turn a unique violation into a caller-visible message when `describe`
/// recognises the constraint, and leave every other error alone.
fn unique_violation(err: sqlx::Error, describe: impl Fn(&str) -> Option<String>) -> ApiError {
    if let sqlx::Error::Database(ref db_err) = err {
        if db_err.code().as_deref() == Some("23505") {
            if let Some(message) = db_err.constraint().and_then(&describe) {
                return ApiError::Conflict(message);
            }
        }
    }

    ApiError::Database(err)
}

/// Report a check-constraint violation as a validation failure. Everything the
/// `flights` constraints cover is also checked in `validate`, so reaching this
/// means the row was rejected for a reason the request alone did not show —
/// name the constraint rather than swallowing it into a 500.
fn check_violation(err: sqlx::Error) -> ApiError {
    if let sqlx::Error::Database(ref db_err) = err {
        if db_err.code().as_deref() == Some("23514") {
            if let Some(constraint) = db_err.constraint() {
                return ApiError::Validation(vec![format!("the flight violates {constraint}")]);
            }
        }
    }

    ApiError::Database(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn airport(iata: &str, latitude: f64, longitude: f64) -> AirportInput {
        AirportInput {
            iata_code: iata.to_string(),
            icao_code: None,
            name: Some(format!("{iata} International")),
            city: None,
            country_code: None,
            latitude: Some(latitude),
            longitude: Some(longitude),
            timezone: None,
        }
    }

    fn request() -> FlightInput {
        FlightInput {
            airline: AirlineInput {
                iata_code: "aa".to_string(),
                icao_code: Some("aal".to_string()),
                name: Some("American Airlines".to_string()),
            },
            flight_number: " 100 ".to_string(),
            origin: airport("jfk", 40.639751, -73.778925),
            destination: airport("lhr", 51.470020, -0.454295),
            scheduled_departure_at: "2026-08-10T22:00:00Z".parse().unwrap(),
            scheduled_arrival_at: "2026-08-11T10:00:00Z".parse().unwrap(),
            actual_departure_at: None,
            actual_arrival_at: None,
            status: None,
            departure_terminal: Some("  8 ".to_string()),
            departure_gate: None,
            arrival_terminal: None,
            arrival_gate: None,
            aircraft_type: None,
            aircraft_registration: Some("".to_string()),
        }
    }

    /// Validate standalone (no journey prefix) and demand failure.
    fn errors(request: FlightInput) -> Vec<String> {
        let mut errors = Vec::new();
        request.validate_into("", &mut errors);
        assert!(!errors.is_empty(), "expected validation to fail");
        errors
    }

    #[test]
    fn normalises_codes_and_defaults_the_status() {
        let mut errors = Vec::new();
        let flight = request().validate_into("", &mut errors);
        assert!(errors.is_empty(), "expected no errors, got {errors:?}");

        assert_eq!(flight.airline.iata_code, "AA");
        assert_eq!(flight.airline.icao_code.as_deref(), Some("AAL"));
        assert_eq!(flight.flight_number, "100");
        assert_eq!(flight.origin.iata_code, "JFK");
        assert_eq!(flight.destination.iata_code, "LHR");
        assert_eq!(flight.status, "scheduled");
        assert_eq!(flight.departure_terminal.as_deref(), Some("8"));
        // Blank optional strings are absent, not empty.
        assert_eq!(flight.aircraft_registration, None);
    }

    #[test]
    fn rejects_a_malformed_iata_code() {
        let mut req = request();
        req.origin.iata_code = "JFKX".to_string();

        let errors = errors(req);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("origin.iata_code"));
    }

    #[test]
    fn rejects_a_flight_that_lands_before_it_leaves() {
        let mut req = request();
        req.scheduled_arrival_at = req.scheduled_departure_at;

        let errors = errors(req);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("scheduled_arrival_at"));
    }

    #[test]
    fn rejects_a_flight_to_the_airport_it_leaves_from() {
        let mut req = request();
        req.destination = airport("jfk", 40.639751, -73.778925);

        let errors = errors(req);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("different airports"));
    }

    #[test]
    fn rejects_an_unknown_status() {
        let mut req = request();
        req.status = Some("taxiing".to_string());

        let errors = errors(req);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("status must be one of"));
    }

    #[test]
    fn rejects_half_a_coordinate_pair_and_an_out_of_range_one() {
        let mut req = request();
        req.origin.longitude = None;
        req.destination.latitude = Some(120.0);

        let errors = errors(req);
        assert_eq!(errors.len(), 2);
        assert!(errors.iter().any(|e| e.contains("must be given together")));
        assert!(errors
            .iter()
            .any(|e| e.contains("destination.latitude must be between")));
    }

    #[test]
    fn reports_every_problem_at_once() {
        let mut req = request();
        req.flight_number = "  ".to_string();
        req.airline.iata_code = "A".to_string();
        req.status = Some("taxiing".to_string());

        assert_eq!(errors(req).len(), 3);
    }

    #[test]
    fn distance_needs_both_sets_of_coordinates() {
        let jfk = ResolvedAirport {
            id: 1,
            latitude: Some(40.639751),
            longitude: Some(-73.778925),
        };
        let lhr = ResolvedAirport {
            id: 2,
            latitude: Some(51.470020),
            longitude: Some(-0.454295),
        };
        let unmapped = ResolvedAirport {
            id: 3,
            latitude: None,
            longitude: None,
        };

        assert_eq!(distance_between(&jfk, &lhr), Some(3443));
        assert_eq!(distance_between(&jfk, &unmapped), None);
        assert_eq!(distance_between(&unmapped, &lhr), None);
    }
}
