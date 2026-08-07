//! What `POST` and `GET` on a journey return.

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

use crate::models::entities::journey::{Journey, JourneyTotals};

#[derive(Debug, Serialize)]
pub struct JourneyResponse {
    #[serde(flatten)]
    pub journey: Journey,
    pub totals: JourneyTotals,
    pub segments: Vec<SegmentResponse>,
}

#[derive(Debug, Serialize)]
pub struct SegmentResponse {
    pub id: Uuid,
    pub position: i32,
    pub mode: String,
    pub origin: Option<PlaceResponse>,
    pub destination: Option<PlaceResponse>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_minutes: Option<i32>,
    pub distance_miles: Option<i32>,
    pub notes: Option<String>,
    pub metadata: Value,
    pub flight: Option<FlightResponse>,
    pub drive: Option<DriveResponse>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PlaceResponse {
    pub id: i64,
    pub name: String,
    pub kind: String,
    /// Present only for airport places.
    pub iata_code: Option<String>,
    pub city: Option<String>,
    pub country_code: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub timezone: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct FlightResponse {
    #[sqlx(rename = "leg_id")]
    #[serde(skip_serializing)]
    pub leg_id: Uuid,
    pub flight_id: Uuid,
    pub airline_id: i64,
    pub flight_number: String,
    pub origin_airport_id: i64,
    pub destination_airport_id: i64,
    pub distance_miles: Option<i32>,
    pub scheduled_departure_at: DateTime<Utc>,
    pub scheduled_arrival_at: DateTime<Utc>,
    pub status: String,
    pub seat: Option<String>,
    pub cabin: Option<String>,
    pub booking_reference: Option<String>,
    pub ticket_number: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DriveResponse {
    pub vehicle: Option<VehicleResponse>,
    pub role: Option<String>,
    pub route_polyline: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct VehicleResponse {
    pub id: i64,
    pub nickname: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub year: Option<i16>,
}

/// `POST /api/v1/journeys` — the trip it recorded.
pub type InsertJourneyResponse = JourneyResponse;

/// `GET /api/v1/journeys/{id}` — the same shape, read back.
///
/// Aliases rather than separate structs: both endpoints return one
/// representation of a journey, and duplicating it would only create somewhere
/// for the two to drift apart.
pub type GetJourneyResponse = JourneyResponse;
