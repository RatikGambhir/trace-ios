//! Rows of `journeys` and `journey_segments`, plus the flat shapes the
//! segment queries decode into before they are assembled into a response.

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, FromRow)]
pub struct Journey {
    pub id: Uuid,
    pub user_id: Uuid,
    pub idempotency_key: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub status: String,
    pub visibility: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Straight off the `journey_totals` view — one roll-up across every mode.
#[derive(Debug, Serialize, FromRow)]
pub struct JourneyTotals {
    pub segment_count: i64,
    pub mode_count: i64,
    pub total_distance_miles: i64,
    pub total_duration_minutes: i64,
    pub first_departure_at: Option<DateTime<Utc>>,
    pub last_arrival_at: Option<DateTime<Utc>>,
}

/// Flat row for the drive join; split into `DriveResponse` + `VehicleResponse` after.
#[derive(Debug, FromRow)]
pub struct DriveRow {
    pub segment_id: Uuid,
    pub role: Option<String>,
    pub route_polyline: Option<String>,
    pub vehicle_id: Option<i64>,
    pub nickname: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub year: Option<i16>,
}

#[derive(Debug, FromRow)]
pub struct SegmentRow {
    pub id: Uuid,
    pub position: i32,
    pub mode: String,
    pub origin_place_id: Option<i64>,
    pub destination_place_id: Option<i64>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_minutes: Option<i32>,
    pub distance_miles: Option<i32>,
    pub notes: Option<String>,
    pub metadata: Value,
}
