//! Flights: the shared, real-world leg.
//!
//! A flight is not owned by a user — AA100 on a given day is one row in
//! `flights` however many people were aboard. These are the request and row
//! types plus their validation; the personal half of a flight (seat, cabin,
//! booking reference) lives with the journey segment that references it.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::core::validation::{field, fixed_code, optional_text};

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

#[cfg(test)]
#[path = "flight_tests.rs"]
mod tests;
