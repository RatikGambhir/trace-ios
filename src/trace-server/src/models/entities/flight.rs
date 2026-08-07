//! Rows of `flights`, and the shape an airport takes once resolved.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

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

/// An airport row after `resolve_airport` — the id the flight references, plus
/// the coordinates the distance is computed from.
#[derive(Debug, FromRow)]
pub struct ResolvedAirport {
    pub id: i64,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}
