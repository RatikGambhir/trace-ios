//! `POST /api/v1/journeys`: a user's trip, its ordered segments, and everything
//! those segments reference.
//!
//! This is the write path for the whole schema. One request can create a
//! journey, its segments, the places at either end of each one, the airports,
//! airlines and flights behind a flight segment, and the vehicle behind a drive
//! — and it holds the same three rules the flight work did:
//!
//! * **Atomic** — all of it in one transaction. A failure on segment three
//!   leaves no journey, no segments, and none of the reference rows the earlier
//!   segments created.
//! * **Idempotent** — opt-in, via `idempotency_key`. The journey row is written
//!   *first*, so a replay conflicts immediately and returns the stored trip
//!   without touching anything downstream.
//! * **Derived where derivable** — a flight segment takes its endpoints,
//!   distance, and duration from the flight it references rather than making the
//!   caller repeat them.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use crate::{
    error::ApiError,
    flights::{
        self, field, insert_flight, resolve_airline, resolve_airport, AirportInput, FlightInput,
        FlightUpsert, ValidatedAirport, ValidatedFlight,
    },
};

/// The modes allowed by the `journey_segments` mode CHECK.
const SEGMENT_MODES: [&str; 8] = [
    "flight", "drive", "train", "bus", "ferry", "walk", "bike", "other",
];

const JOURNEY_STATUSES: [&str; 4] = ["planned", "active", "completed", "cancelled"];
const VISIBILITIES: [&str; 3] = ["private", "friends", "public"];
const CABINS: [&str; 4] = ["economy", "premium_economy", "business", "first"];
const DRIVE_ROLES: [&str; 2] = ["driver", "passenger"];
const PLACE_KINDS: [&str; 7] = [
    "airport", "station", "port", "address", "city", "landmark", "other",
];

const DEFAULT_JOURNEY_STATUS: &str = "planned";
const DEFAULT_VISIBILITY: &str = "private";
const DEFAULT_PLACE_KIND: &str = "address";

// Column widths from 0004/0005.
const MAX_IDEMPOTENCY_KEY_LEN: usize = 64;
const MAX_TITLE_LEN: usize = 200;
const MAX_PLACE_NAME_LEN: usize = 200;
const MAX_CITY_LEN: usize = 100;
const MAX_TIMEZONE_LEN: usize = 50;
const MAX_SEAT_LEN: usize = 10;
const MAX_BOOKING_REF_LEN: usize = 20;
const MAX_VEHICLE_NAME_LEN: usize = 100;
const MAX_VEHICLE_PART_LEN: usize = 50;
const MAX_LICENSE_PLATE_LEN: usize = 20;

/// Guards against a single request trying to write an unbounded number of rows.
const MAX_SEGMENTS: usize = 100;

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

/// Body of `POST /api/v1/journeys`.
#[derive(Debug, Deserialize)]
pub struct CreateJourneyRequest {
    pub user_id: Uuid,
    /// Opt-in idempotency. Replaying a request with a key this user has already
    /// used returns the stored journey instead of creating a second one.
    #[serde(default)]
    pub idempotency_key: Option<String>,

    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub ended_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub visibility: Option<String>,
    #[serde(default)]
    pub metadata: Option<Value>,

    /// Ordered. `position` is the index in this array, so the caller never has
    /// to keep two orderings in step.
    #[serde(default)]
    pub segments: Vec<SegmentInput>,
}

/// One leg of the journey.
#[derive(Debug, Deserialize)]
pub struct SegmentInput {
    pub mode: String,

    /// Omitted for `flight` segments — a flight already knows its airports.
    #[serde(default)]
    pub origin: Option<PlaceInput>,
    #[serde(default)]
    pub destination: Option<PlaceInput>,

    #[serde(default)]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub ended_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub duration_minutes: Option<i32>,
    /// Road distance for a drive, which is not the great-circle distance, so it
    /// is taken from the caller. Derived for `flight` segments.
    #[serde(default)]
    pub distance_miles: Option<i32>,

    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub metadata: Option<Value>,

    /// Required for `mode: "flight"`, rejected otherwise.
    #[serde(default)]
    pub flight: Option<FlightInput>,
    /// The traveller's own booking on that flight.
    #[serde(default)]
    pub booking: Option<BookingInput>,
    /// Optional for `mode: "drive"`, rejected otherwise.
    #[serde(default)]
    pub drive: Option<DriveInput>,
}

/// Where a segment starts or ends. Three forms: a place already stored, an
/// airport (resolved or created, then read back through its mirrored place), or
/// a new place described inline.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlaceInput {
    Saved { id: i64 },
    Airport(AirportInput),
    Custom(CustomPlaceInput),
}

#[derive(Debug, Deserialize)]
pub struct CustomPlaceInput {
    pub name: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
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

/// What is personal about a flight, as opposed to the flight itself.
#[derive(Debug, Deserialize)]
pub struct BookingInput {
    #[serde(default)]
    pub seat: Option<String>,
    #[serde(default)]
    pub cabin: Option<String>,
    #[serde(default)]
    pub booking_reference: Option<String>,
    #[serde(default)]
    pub ticket_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DriveInput {
    #[serde(default)]
    pub vehicle: Option<VehicleInput>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub route_polyline: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VehicleInput {
    Saved { id: i64 },
    New(NewVehicleInput),
}

#[derive(Debug, Deserialize)]
pub struct NewVehicleInput {
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub make: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub year: Option<i32>,
    #[serde(default)]
    pub license_plate: Option<String>,
}

// ---------------------------------------------------------------------------
// Validated types
// ---------------------------------------------------------------------------

pub struct ValidatedJourney {
    pub user_id: Uuid,
    pub idempotency_key: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub status: String,
    pub visibility: String,
    pub metadata: Value,
    pub segments: Vec<ValidatedSegment>,
}

pub struct ValidatedSegment {
    pub position: i32,
    pub mode: String,
    pub origin: Option<ValidatedPlace>,
    pub destination: Option<ValidatedPlace>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_minutes: Option<i32>,
    pub distance_miles: Option<i32>,
    pub notes: Option<String>,
    pub metadata: Value,
    pub detail: SegmentDetail,
}

pub enum SegmentDetail {
    Flight {
        flight: Box<ValidatedFlight>,
        booking: ValidatedBooking,
    },
    Drive(ValidatedDrive),
    /// Modes with nothing extra to say — the parent row is the whole segment.
    Bare,
}

pub enum ValidatedPlace {
    Saved(i64),
    Airport(Box<ValidatedAirport>),
    Custom(Box<ValidatedCustomPlace>),
}

pub struct ValidatedCustomPlace {
    pub name: String,
    pub kind: String,
    pub address: Option<String>,
    pub city: Option<String>,
    pub country_code: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub timezone: Option<String>,
}

#[derive(Default)]
pub struct ValidatedBooking {
    pub seat: Option<String>,
    pub cabin: Option<String>,
    pub booking_reference: Option<String>,
    pub ticket_number: Option<String>,
}

pub struct ValidatedDrive {
    pub vehicle: Option<ValidatedVehicle>,
    pub role: Option<String>,
    pub route_polyline: Option<String>,
}

pub enum ValidatedVehicle {
    Saved(i64),
    New(Box<ValidatedNewVehicle>),
}

pub struct ValidatedNewVehicle {
    pub nickname: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub year: Option<i16>,
    pub license_plate: Option<String>,
}

// ---------------------------------------------------------------------------
// Response types
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

#[derive(Debug, Serialize)]
pub struct JourneyResponse {
    #[serde(flatten)]
    pub journey: Journey,
    pub totals: JourneyTotals,
    pub segments: Vec<SegmentView>,
}

#[derive(Debug, Serialize)]
pub struct SegmentView {
    pub id: Uuid,
    pub position: i32,
    pub mode: String,
    pub origin: Option<PlaceView>,
    pub destination: Option<PlaceView>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_minutes: Option<i32>,
    pub distance_miles: Option<i32>,
    pub notes: Option<String>,
    pub metadata: Value,
    pub flight: Option<FlightView>,
    pub drive: Option<DriveView>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PlaceView {
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
pub struct FlightView {
    #[sqlx(rename = "segment_id")]
    #[serde(skip_serializing)]
    pub segment_id: Uuid,
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
pub struct DriveView {
    pub vehicle: Option<VehicleView>,
    pub role: Option<String>,
    pub route_polyline: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct VehicleView {
    pub id: i64,
    pub nickname: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub year: Option<i16>,
}

/// Flat row for the drive join; split into `DriveView` + `VehicleView` after.
#[derive(Debug, FromRow)]
struct DriveRow {
    segment_id: Uuid,
    role: Option<String>,
    route_polyline: Option<String>,
    vehicle_id: Option<i64>,
    nickname: Option<String>,
    make: Option<String>,
    model: Option<String>,
    year: Option<i16>,
}

#[derive(Debug, FromRow)]
struct SegmentRow {
    id: Uuid,
    position: i32,
    mode: String,
    origin_place_id: Option<i64>,
    destination_place_id: Option<i64>,
    started_at: Option<DateTime<Utc>>,
    ended_at: Option<DateTime<Utc>>,
    duration_minutes: Option<i32>,
    distance_miles: Option<i32>,
    notes: Option<String>,
    metadata: Value,
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

impl CreateJourneyRequest {
    pub fn validate(self) -> Result<ValidatedJourney, ApiError> {
        let mut errors = Vec::new();

        let title = self.title.trim().to_string();
        if title.is_empty() {
            errors.push("title must not be empty".to_string());
        } else if title.chars().count() > MAX_TITLE_LEN {
            errors.push(format!("title must be at most {MAX_TITLE_LEN} characters"));
        }

        let idempotency_key = optional_text(
            self.idempotency_key,
            "idempotency_key",
            MAX_IDEMPOTENCY_KEY_LEN,
            &mut errors,
        );

        let status = one_of(
            self.status,
            "status",
            &JOURNEY_STATUSES,
            DEFAULT_JOURNEY_STATUS,
            &mut errors,
        );
        let visibility = one_of(
            self.visibility,
            "visibility",
            &VISIBILITIES,
            DEFAULT_VISIBILITY,
            &mut errors,
        );

        if let (Some(start), Some(end)) = (self.started_at, self.ended_at) {
            if end < start {
                errors.push("ended_at must not be before started_at".to_string());
            }
        }

        let metadata = object_or_default(self.metadata, "metadata", &mut errors);

        if self.segments.len() > MAX_SEGMENTS {
            errors.push(format!(
                "segments must contain at most {MAX_SEGMENTS} entries"
            ));
        }

        // Position is the array index, so the ordering the caller sends is the
        // ordering that is stored — there is no second field to keep in step.
        let segments = self
            .segments
            .into_iter()
            .take(MAX_SEGMENTS)
            .enumerate()
            .map(|(index, segment)| segment.validate(index, &mut errors))
            .collect();

        if !errors.is_empty() {
            return Err(ApiError::Validation(errors));
        }

        Ok(ValidatedJourney {
            user_id: self.user_id,
            idempotency_key,
            title,
            description: self
                .description
                .map(|d| d.trim().to_string())
                .filter(|d| !d.is_empty()),
            started_at: self.started_at,
            ended_at: self.ended_at,
            status,
            visibility,
            metadata,
            segments,
        })
    }
}

impl SegmentInput {
    fn validate(self, index: usize, errors: &mut Vec<String>) -> ValidatedSegment {
        let prefix = format!("segments[{index}]");

        let mode = self.mode.trim().to_ascii_lowercase();
        if !SEGMENT_MODES.contains(&mode.as_str()) {
            errors.push(format!(
                "{}.mode must be one of: {}",
                prefix,
                SEGMENT_MODES.join(", ")
            ));
        }

        // Each mode admits exactly one detail object. Saying so here means the
        // composite (segment_id, mode) foreign key never has to reject anything.
        let is_flight = mode == "flight";
        let is_drive = mode == "drive";

        if !is_flight && self.flight.is_some() {
            errors.push(format!(
                "{prefix}.flight is only valid when mode is \"flight\""
            ));
        }
        if !is_flight && self.booking.is_some() {
            errors.push(format!(
                "{prefix}.booking is only valid when mode is \"flight\""
            ));
        }
        if !is_drive && self.drive.is_some() {
            errors.push(format!(
                "{prefix}.drive is only valid when mode is \"drive\""
            ));
        }
        if is_flight && self.flight.is_none() {
            errors.push(format!(
                "{prefix}.flight is required when mode is \"flight\""
            ));
        }

        // A flight already knows its airports; repeating them invites a
        // contradiction nobody can resolve.
        if is_flight && (self.origin.is_some() || self.destination.is_some()) {
            errors.push(format!(
                "{prefix}.origin and {prefix}.destination are derived from the flight; omit them"
            ));
        }

        let origin = self
            .origin
            .map(|place| place.validate(&field(&prefix, "origin"), errors));
        let destination = self
            .destination
            .map(|place| place.validate(&field(&prefix, "destination"), errors));

        if let (Some(start), Some(end)) = (self.started_at, self.ended_at) {
            if end < start {
                errors.push(format!(
                    "{prefix}.ended_at must not be before {prefix}.started_at"
                ));
            }
        }

        if let Some(duration) = self.duration_minutes {
            if duration < 0 {
                errors.push(format!("{prefix}.duration_minutes must not be negative"));
            }
        }
        if let Some(distance) = self.distance_miles {
            if distance < 0 {
                errors.push(format!("{prefix}.distance_miles must not be negative"));
            }
        }

        let detail = match (is_flight, is_drive) {
            (true, _) => match self.flight {
                Some(flight) => SegmentDetail::Flight {
                    flight: Box::new(flight.validate_into(&field(&prefix, "flight"), errors)),
                    booking: self
                        .booking
                        .map(|booking| booking.validate(&prefix, errors))
                        .unwrap_or_default(),
                },
                // Already reported above; keep collecting other segments.
                None => SegmentDetail::Bare,
            },
            (_, true) => SegmentDetail::Drive(
                self.drive
                    .map(|drive| drive.validate(&prefix, errors))
                    .unwrap_or(ValidatedDrive {
                        vehicle: None,
                        role: None,
                        route_polyline: None,
                    }),
            ),
            _ => SegmentDetail::Bare,
        };

        // Absent duration with both timestamps is a subtraction, not a question.
        let duration_minutes = self.duration_minutes.or_else(|| {
            let (start, end) = (self.started_at?, self.ended_at?);
            i32::try_from((end - start).num_minutes()).ok()
        });

        ValidatedSegment {
            position: index as i32 + 1,
            mode,
            origin,
            destination,
            started_at: self.started_at,
            ended_at: self.ended_at,
            duration_minutes,
            distance_miles: self.distance_miles,
            notes: self
                .notes
                .map(|n| n.trim().to_string())
                .filter(|n| !n.is_empty()),
            metadata: object_or_default(self.metadata, &field(&prefix, "metadata"), errors),
            detail,
        }
    }
}

impl PlaceInput {
    fn validate(self, prefix: &str, errors: &mut Vec<String>) -> ValidatedPlace {
        match self {
            PlaceInput::Saved { id } => {
                if id <= 0 {
                    errors.push(format!("{prefix}.id must be a positive place id"));
                }
                ValidatedPlace::Saved(id)
            }
            PlaceInput::Airport(airport) => {
                // Reuses the airport rules from the flight path, so an airport
                // is described the same way wherever it appears.
                let mut collected = Vec::new();
                let validated = airport.validate(prefix, &mut collected);
                errors.append(&mut collected);
                ValidatedPlace::Airport(Box::new(validated))
            }
            PlaceInput::Custom(place) => {
                ValidatedPlace::Custom(Box::new(place.validate(prefix, errors)))
            }
        }
    }
}

impl CustomPlaceInput {
    fn validate(self, prefix: &str, errors: &mut Vec<String>) -> ValidatedCustomPlace {
        let name = self.name.trim().to_string();
        if name.is_empty() {
            errors.push(format!("{prefix}.name must not be empty"));
        } else if name.chars().count() > MAX_PLACE_NAME_LEN {
            errors.push(format!(
                "{prefix}.name must be at most {MAX_PLACE_NAME_LEN} characters"
            ));
        }

        let kind = one_of(
            self.kind,
            &field(prefix, "kind"),
            &PLACE_KINDS,
            DEFAULT_PLACE_KIND,
            errors,
        );
        // `places_airport_kind_check` reserves this kind for the rows the
        // airport trigger owns.
        if kind == "airport" {
            errors.push(format!(
                "{prefix}.kind cannot be \"airport\"; use {{\"type\": \"airport\"}} instead"
            ));
        }

        match (self.latitude, self.longitude) {
            (Some(latitude), Some(longitude)) => {
                if !(-90.0..=90.0).contains(&latitude) {
                    errors.push(format!("{prefix}.latitude must be between -90 and 90"));
                }
                if !(-180.0..=180.0).contains(&longitude) {
                    errors.push(format!("{prefix}.longitude must be between -180 and 180"));
                }
            }
            (None, None) => {}
            _ => errors.push(format!(
                "{prefix}.latitude and {prefix}.longitude must be given together"
            )),
        }

        ValidatedCustomPlace {
            name,
            kind,
            address: self
                .address
                .map(|a| a.trim().to_string())
                .filter(|a| !a.is_empty()),
            city: optional_text(self.city, &field(prefix, "city"), MAX_CITY_LEN, errors),
            country_code: self
                .country_code
                .as_deref()
                .map(|code| code.trim())
                .filter(|code| !code.is_empty())
                .map(|code| {
                    let code = code.to_ascii_uppercase();
                    if code.chars().count() != 2 || !code.chars().all(|c| c.is_ascii_alphabetic()) {
                        errors.push(format!("{prefix}.country_code must be 2 letters"));
                    }
                    code
                }),
            latitude: self.latitude,
            longitude: self.longitude,
            timezone: optional_text(
                self.timezone,
                &field(prefix, "timezone"),
                MAX_TIMEZONE_LEN,
                errors,
            ),
        }
    }
}

impl BookingInput {
    fn validate(self, prefix: &str, errors: &mut Vec<String>) -> ValidatedBooking {
        let prefix = field(prefix, "booking");

        ValidatedBooking {
            seat: optional_text(self.seat, &field(&prefix, "seat"), MAX_SEAT_LEN, errors),
            cabin: self
                .cabin
                .as_deref()
                .map(|cabin| cabin.trim().to_ascii_lowercase())
                .filter(|cabin| !cabin.is_empty())
                .inspect(|cabin| {
                    if !CABINS.contains(&cabin.as_str()) {
                        errors.push(format!(
                            "{}.cabin must be one of: {}",
                            prefix,
                            CABINS.join(", ")
                        ));
                    }
                }),
            booking_reference: optional_text(
                self.booking_reference,
                &field(&prefix, "booking_reference"),
                MAX_BOOKING_REF_LEN,
                errors,
            ),
            ticket_number: optional_text(
                self.ticket_number,
                &field(&prefix, "ticket_number"),
                MAX_BOOKING_REF_LEN,
                errors,
            ),
        }
    }
}

impl DriveInput {
    fn validate(self, prefix: &str, errors: &mut Vec<String>) -> ValidatedDrive {
        let prefix = field(prefix, "drive");

        ValidatedDrive {
            vehicle: self
                .vehicle
                .map(|vehicle| vehicle.validate(&field(&prefix, "vehicle"), errors)),
            role: self
                .role
                .as_deref()
                .map(|role| role.trim().to_ascii_lowercase())
                .filter(|role| !role.is_empty())
                .inspect(|role| {
                    if !DRIVE_ROLES.contains(&role.as_str()) {
                        errors.push(format!(
                            "{}.role must be one of: {}",
                            prefix,
                            DRIVE_ROLES.join(", ")
                        ));
                    }
                }),
            route_polyline: self.route_polyline.filter(|p| !p.trim().is_empty()),
        }
    }
}

impl VehicleInput {
    fn validate(self, prefix: &str, errors: &mut Vec<String>) -> ValidatedVehicle {
        match self {
            VehicleInput::Saved { id } => {
                if id <= 0 {
                    errors.push(format!("{prefix}.id must be a positive vehicle id"));
                }
                ValidatedVehicle::Saved(id)
            }
            VehicleInput::New(vehicle) => {
                let nickname = optional_text(
                    vehicle.nickname,
                    &field(prefix, "nickname"),
                    MAX_VEHICLE_NAME_LEN,
                    errors,
                );
                let make = optional_text(
                    vehicle.make,
                    &field(prefix, "make"),
                    MAX_VEHICLE_PART_LEN,
                    errors,
                );
                let model = optional_text(
                    vehicle.model,
                    &field(prefix, "model"),
                    MAX_VEHICLE_PART_LEN,
                    errors,
                );

                // Mirrors `vehicles_identifiable_check`.
                if nickname.is_none() && make.is_none() && model.is_none() {
                    errors.push(format!(
                        "{prefix} needs at least one of nickname, make, or model"
                    ));
                }

                let year = vehicle.year.and_then(|year| {
                    if !(1885..=2100).contains(&year) {
                        errors.push(format!("{prefix}.year must be between 1885 and 2100"));
                        None
                    } else {
                        i16::try_from(year).ok()
                    }
                });

                ValidatedVehicle::New(Box::new(ValidatedNewVehicle {
                    nickname,
                    make,
                    model,
                    year,
                    license_plate: optional_text(
                        vehicle.license_plate,
                        &field(prefix, "license_plate"),
                        MAX_LICENSE_PLATE_LEN,
                        errors,
                    ),
                }))
            }
        }
    }
}

/// Normalise an optional enum-ish string against its allowed set, falling back
/// to `default` when absent.
fn one_of(
    value: Option<String>,
    field: &str,
    allowed: &[&str],
    default: &str,
    errors: &mut Vec<String>,
) -> String {
    let value = value
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string());

    if !allowed.contains(&value.as_str()) {
        errors.push(format!("{field} must be one of: {}", allowed.join(", ")));
    }

    value
}

/// Metadata has to be a JSON object — an array or a bare scalar in a column
/// meant for key/value extras is almost always a client bug.
fn object_or_default(value: Option<Value>, field: &str, errors: &mut Vec<String>) -> Value {
    match value {
        None | Some(Value::Null) => json!({}),
        Some(value) if value.is_object() => value,
        Some(_) => {
            errors.push(format!("{field} must be a JSON object"));
            json!({})
        }
    }
}

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

// ---------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------

/// The journey row, and whether this request is the one that created it.
pub enum JourneyUpsert {
    Created(Uuid),
    AlreadyExists(Uuid),
}

/// Reject an unknown `user_id` as a validation failure rather than letting the
/// foreign key surface as an opaque 500.
pub async fn ensure_user_exists(conn: &mut PgConnection, user_id: Uuid) -> Result<(), ApiError> {
    let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(conn)
        .await?;

    match exists {
        Some(_) => Ok(()),
        None => Err(ApiError::Validation(vec![format!(
            "user_id {user_id} does not exist"
        )])),
    }
}

/// Insert the journey, or return the one an identical earlier request created.
///
/// This runs *first*, before any segment or reference row, so a replay costs one
/// statement and creates nothing. Without an `idempotency_key` every call is a
/// new journey — two trips can legitimately share a title and dates, so there is
/// no natural key to fall back on.
pub async fn insert_journey(
    conn: &mut PgConnection,
    journey: &ValidatedJourney,
) -> Result<JourneyUpsert, ApiError> {
    if let Some(key) = journey.idempotency_key.as_deref() {
        if let Some(id) = select_journey_by_key(conn, journey.user_id, key).await? {
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

    select_journey_by_key(conn, journey.user_id, key)
        .await?
        .map(JourneyUpsert::AlreadyExists)
        .ok_or_else(|| {
            ApiError::Conflict(
                "this journey is being created by another request; retry".to_string(),
            )
        })
}

async fn select_journey_by_key(
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

/// Write one segment and everything it references.
pub async fn insert_segment(
    conn: &mut PgConnection,
    cache: &mut PlaceCache,
    journey_id: Uuid,
    user_id: Uuid,
    segment: &ValidatedSegment,
) -> Result<(), ApiError> {
    let prefix = format!("segments[{}]", segment.position - 1);

    // A flight segment's endpoints, distance and duration all come from the
    // flight, so resolve it before writing the parent row.
    let mut origin_place_id = None;
    let mut destination_place_id = None;
    let mut distance_miles = segment.distance_miles;
    let mut duration_minutes = segment.duration_minutes;
    let mut started_at = segment.started_at;
    let mut ended_at = segment.ended_at;
    let mut resolved_flight = None;

    if let SegmentDetail::Flight { flight, .. } = &segment.detail {
        let flight_prefix = field(&prefix, "flight");
        let origin =
            resolve_airport(conn, &flight.origin, &field(&flight_prefix, "origin")).await?;
        let destination = resolve_airport(
            conn,
            &flight.destination,
            &field(&flight_prefix, "destination"),
        )
        .await?;
        let airline_id = resolve_airline(conn, &flight.airline).await?;

        let flight_distance = flights::distance_between(&origin, &destination);
        let row = match insert_flight(
            conn,
            flight,
            airline_id,
            origin.id,
            destination.id,
            flight_distance,
        )
        .await?
        {
            FlightUpsert::Created(row) | FlightUpsert::AlreadyExists(row) => row,
        };

        let origin_place = place_for_airport(conn, origin.id).await?;
        let destination_place = place_for_airport(conn, destination.id).await?;
        cache
            .0
            .insert(format!("airport:{}", flight.origin.iata_code), origin_place);
        cache.0.insert(
            format!("airport:{}", flight.destination.iata_code),
            destination_place,
        );

        origin_place_id = Some(origin_place);
        destination_place_id = Some(destination_place);
        distance_miles = segment.distance_miles.or(row.distance_miles);
        started_at = started_at.or(Some(row.scheduled_departure_at));
        ended_at = ended_at.or(Some(row.scheduled_arrival_at));
        duration_minutes = duration_minutes.or_else(|| {
            i32::try_from((row.scheduled_arrival_at - row.scheduled_departure_at).num_minutes())
                .ok()
        });
        resolved_flight = Some(row.id);
    } else {
        if let Some(place) = &segment.origin {
            origin_place_id =
                Some(resolve_place(conn, cache, place, &field(&prefix, "origin")).await?);
        }
        if let Some(place) = &segment.destination {
            destination_place_id =
                Some(resolve_place(conn, cache, place, &field(&prefix, "destination")).await?);
        }
    }

    let (segment_id,): (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO journey_segments (
            journey_id, position, mode, origin_place_id, destination_place_id,
            started_at, ended_at, duration_minutes, distance_miles, notes, metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::jsonb)
        RETURNING id
        "#,
    )
    .bind(journey_id)
    .bind(segment.position)
    .bind(&segment.mode)
    .bind(origin_place_id)
    .bind(destination_place_id)
    .bind(started_at)
    .bind(ended_at)
    .bind(duration_minutes)
    .bind(distance_miles)
    .bind(&segment.notes)
    .bind(&segment.metadata)
    .fetch_one(&mut *conn)
    .await
    .map_err(|err| segment_constraint_error(err, &prefix))?;

    match &segment.detail {
        SegmentDetail::Flight { booking, .. } => {
            let flight_id = resolved_flight
                .expect("a flight segment always resolves its flight before the parent insert");

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
            .execute(&mut *conn)
            .await?;
        }
        SegmentDetail::Drive(drive) => {
            let vehicle_id = match &drive.vehicle {
                Some(vehicle) => Some(
                    resolve_vehicle(conn, user_id, vehicle, &field(&prefix, "drive.vehicle"))
                        .await?,
                ),
                None => None,
            };

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
            .execute(&mut *conn)
            .await?;
        }
        SegmentDetail::Bare => {}
    }

    Ok(())
}

/// Places resolved so far in one request.
///
/// A journey is a chain: leg N ends where leg N+1 begins, and the caller
/// describes that waypoint twice. Without this, each description would insert
/// its own row and the trip would end up with two "Cotswolds cottage" places
/// that are the same building.
#[derive(Default)]
pub struct PlaceCache(HashMap<String, i64>);

impl ValidatedPlace {
    /// What makes two descriptions in one request the same place.
    ///
    /// A name at a coordinate is an identity; `kind` and `city` are description,
    /// and a caller who spells the arrival out in full and the departure in
    /// shorthand still means one building. Where the fuller description comes
    /// first, that is the one stored.
    fn cache_key(&self) -> String {
        match self {
            ValidatedPlace::Saved(id) => format!("saved:{id}"),
            ValidatedPlace::Airport(airport) => format!("airport:{}", airport.iata_code),
            ValidatedPlace::Custom(place) => match (place.latitude, place.longitude) {
                (Some(latitude), Some(longitude)) => {
                    format!("custom:{}|{latitude}|{longitude}", place.name)
                }
                // Nothing to pin it to but the words, so use all of them.
                _ => format!(
                    "custom:{}|{}|{}",
                    place.name,
                    place.kind,
                    place.city.as_deref().unwrap_or_default()
                ),
            },
        }
    }
}

/// Resolve a place descriptor to a `places.id`, creating the row when the
/// descriptor is a new place or a new airport.
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

    let id = resolve_place_uncached(conn, place, field).await?;
    cache.0.insert(key, id);

    Ok(id)
}

async fn resolve_place_uncached(
    conn: &mut PgConnection,
    place: &ValidatedPlace,
    field: &str,
) -> Result<i64, ApiError> {
    match place {
        ValidatedPlace::Saved(id) => {
            let row: Option<(i64,)> = sqlx::query_as("SELECT id FROM places WHERE id = $1")
                .bind(id)
                .fetch_optional(&mut *conn)
                .await?;

            row.map(|(id,)| id).ok_or_else(|| {
                ApiError::Validation(vec![format!("{field}.id {id} is not a known place")])
            })
        }
        ValidatedPlace::Airport(airport) => {
            // Creating the airport is enough: `airports_sync_place` mirrors it
            // into `places` inside this same transaction.
            let resolved = resolve_airport(&mut *conn, airport, field).await?;
            place_for_airport(conn, resolved.id).await
        }
        ValidatedPlace::Custom(place) => {
            let (id,): (i64,) = sqlx::query_as(
                r#"
                INSERT INTO places (
                    name, kind, address, city, country_code, latitude, longitude, timezone
                )
                VALUES ($1, $2, $3, $4, $5, $6::float8::numeric, $7::float8::numeric, $8)
                RETURNING id
                "#,
            )
            .bind(&place.name)
            .bind(&place.kind)
            .bind(&place.address)
            .bind(&place.city)
            .bind(&place.country_code)
            .bind(place.latitude)
            .bind(place.longitude)
            .bind(&place.timezone)
            .fetch_one(&mut *conn)
            .await?;

            Ok(id)
        }
    }
}

/// Every airport has exactly one mirrored place, guaranteed by the
/// `airports_sync_place` trigger from 0004.
async fn place_for_airport(conn: &mut PgConnection, airport_id: i64) -> Result<i64, ApiError> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT id FROM places WHERE airport_id = $1")
        .bind(airport_id)
        .fetch_optional(conn)
        .await?;

    row.map(|(id,)| id).ok_or_else(|| {
        ApiError::Internal(anyhow::anyhow!(
            "airport {airport_id} has no mirrored place; is the airports_sync_place trigger installed?"
        ))
    })
}

/// Get-or-create the vehicle. A saved id must belong to this user; a new
/// vehicle with a nickname is keyed on it, so naming the same car across two
/// journeys reuses one row instead of piling up duplicates.
async fn resolve_vehicle(
    conn: &mut PgConnection,
    user_id: Uuid,
    vehicle: &ValidatedVehicle,
    field: &str,
) -> Result<i64, ApiError> {
    match vehicle {
        ValidatedVehicle::Saved(id) => {
            let row: Option<(i64,)> =
                sqlx::query_as("SELECT id FROM vehicles WHERE id = $1 AND user_id = $2")
                    .bind(id)
                    .bind(user_id)
                    .fetch_optional(&mut *conn)
                    .await?;

            row.map(|(id,)| id).ok_or_else(|| {
                ApiError::Validation(vec![format!(
                    "{field}.id {id} is not a vehicle belonging to this user"
                )])
            })
        }
        ValidatedVehicle::New(vehicle) => {
            if let Some(nickname) = vehicle.nickname.as_deref() {
                if let Some(id) = select_vehicle_by_nickname(conn, user_id, nickname).await? {
                    return Ok(id);
                }
            }

            let inserted: Option<(i64,)> = sqlx::query_as(
                r#"
                INSERT INTO vehicles (user_id, nickname, make, model, year, license_plate)
                VALUES ($1, $2, $3, $4, $5, $6)
                ON CONFLICT (user_id, nickname) WHERE nickname IS NOT NULL
                DO NOTHING
                RETURNING id
                "#,
            )
            .bind(user_id)
            .bind(&vehicle.nickname)
            .bind(&vehicle.make)
            .bind(&vehicle.model)
            .bind(vehicle.year)
            .bind(&vehicle.license_plate)
            .fetch_optional(&mut *conn)
            .await?;

            if let Some((id,)) = inserted {
                return Ok(id);
            }

            let nickname = vehicle
                .nickname
                .as_deref()
                .expect("a conflict is only possible when a nickname was sent");

            select_vehicle_by_nickname(conn, user_id, nickname)
                .await?
                .ok_or_else(|| {
                    ApiError::Conflict(format!(
                        "vehicle {nickname} is being created by another request; retry"
                    ))
                })
        }
    }
}

async fn select_vehicle_by_nickname(
    conn: &mut PgConnection,
    user_id: Uuid,
    nickname: &str,
) -> Result<Option<i64>, ApiError> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM vehicles WHERE user_id = $1 AND nickname = $2")
            .bind(user_id)
            .bind(nickname)
            .fetch_optional(conn)
            .await?;

    Ok(row.map(|(id,)| id))
}

/// Name the segment when a `journey_segments` constraint rejects the row, so the
/// caller learns which leg was wrong rather than just which constraint fired.
fn segment_constraint_error(err: sqlx::Error, prefix: &str) -> ApiError {
    if let sqlx::Error::Database(ref db_err) = err {
        if matches!(db_err.code().as_deref(), Some("23514") | Some("23503")) {
            if let Some(constraint) = db_err.constraint() {
                return ApiError::Validation(vec![format!("{prefix} violates {constraint}")]);
            }
        }
    }

    ApiError::Database(err)
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

/// Assemble a journey with its segments, their endpoints, and their per-mode
/// details. Four queries regardless of how many segments there are.
pub async fn load_journey(
    conn: &mut PgConnection,
    journey_id: Uuid,
) -> Result<JourneyResponse, ApiError> {
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
        .map(|row| SegmentView {
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
) -> Result<HashMap<i64, PlaceView>, ApiError> {
    if place_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let places = sqlx::query_as::<_, PlaceView>(
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
) -> Result<HashMap<Uuid, FlightView>, ApiError> {
    if segment_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query_as::<_, FlightView>(
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
) -> Result<HashMap<Uuid, DriveView>, ApiError> {
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
                DriveView {
                    vehicle: row.vehicle_id.map(|id| VehicleView {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn journey(segments: Value) -> CreateJourneyRequest {
        serde_json::from_value(json!({
            "user_id": "00000000-0000-0000-0000-000000000001",
            "title": "England, August 2026",
            "segments": segments,
        }))
        .expect("the fixture deserialises")
    }

    fn errors(request: CreateJourneyRequest) -> Vec<String> {
        match request.validate() {
            Err(ApiError::Validation(errors)) => errors,
            Err(other) => panic!("expected validation errors, got {other:?}"),
            Ok(_) => panic!("expected validation to fail"),
        }
    }

    fn flight_segment() -> Value {
        json!({
            "mode": "flight",
            "flight": {
                "airline": {"iata_code": "AA", "name": "American Airlines"},
                "flight_number": "100",
                "origin": {"iata_code": "JFK", "name": "Kennedy"},
                "destination": {"iata_code": "LHR", "name": "Heathrow"},
                "scheduled_departure_at": "2026-08-10T22:00:00Z",
                "scheduled_arrival_at": "2026-08-11T10:00:00Z"
            },
            "booking": {"seat": "32A", "cabin": "economy"}
        })
    }

    #[test]
    fn positions_come_from_the_array_order() {
        let request = journey(json!([
            {"mode": "walk"},
            flight_segment(),
            {"mode": "drive"}
        ]));

        let journey = request.validate().expect("should be valid");
        let positions: Vec<i32> = journey.segments.iter().map(|s| s.position).collect();

        assert_eq!(positions, vec![1, 2, 3]);
        assert_eq!(journey.status, "planned");
        assert_eq!(journey.visibility, "private");
        assert_eq!(journey.metadata, json!({}));
    }

    #[test]
    fn a_flight_segment_requires_its_flight() {
        let errors = errors(journey(json!([{"mode": "flight"}])));

        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("segments[0].flight is required"));
    }

    #[test]
    fn detail_objects_have_to_match_the_mode() {
        let errors = errors(journey(json!([
            {"mode": "drive", "flight": {
                "airline": {"iata_code": "AA"}, "flight_number": "1",
                "origin": {"iata_code": "JFK"}, "destination": {"iata_code": "LHR"},
                "scheduled_departure_at": "2026-08-10T22:00:00Z",
                "scheduled_arrival_at": "2026-08-11T10:00:00Z"
            }},
            {"mode": "walk", "drive": {"role": "driver"}}
        ])));

        assert_eq!(errors.len(), 2);
        assert!(errors
            .iter()
            .any(|e| e.contains("segments[0].flight is only valid")));
        assert!(errors
            .iter()
            .any(|e| e.contains("segments[1].drive is only valid")));
    }

    #[test]
    fn a_flight_segment_must_not_restate_its_endpoints() {
        let mut segment = flight_segment();
        segment["origin"] = json!({"type": "saved", "id": 1});

        let errors = errors(journey(json!([segment])));
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("derived from the flight"));
    }

    #[test]
    fn errors_name_the_segment_they_came_from() {
        let errors = errors(journey(json!([
            {"mode": "walk"},
            {"mode": "teleport"},
            {"mode": "drive", "drive": {"role": "pilot"}}
        ])));

        assert_eq!(errors.len(), 2);
        assert!(errors.iter().any(|e| e.starts_with("segments[1].mode")));
        assert!(errors
            .iter()
            .any(|e| e.starts_with("segments[2].drive.role")));
    }

    #[test]
    fn nested_flight_errors_carry_the_segment_prefix() {
        let mut segment = flight_segment();
        segment["flight"]["origin"]["iata_code"] = json!("JF");
        segment["booking"]["cabin"] = json!("steerage");

        let errors = errors(journey(json!([{"mode": "walk"}, segment])));

        assert!(errors
            .iter()
            .any(|e| e.starts_with("segments[1].flight.origin.iata_code")));
        assert!(errors
            .iter()
            .any(|e| e.starts_with("segments[1].booking.cabin")));
    }

    #[test]
    fn duration_falls_out_of_the_timestamps() {
        let request = journey(json!([{
            "mode": "drive",
            "started_at": "2026-08-10T18:00:00Z",
            "ended_at": "2026-08-10T19:10:00Z"
        }]));

        let journey = request.validate().expect("should be valid");
        assert_eq!(journey.segments[0].duration_minutes, Some(70));
    }

    #[test]
    fn a_custom_place_cannot_claim_to_be_an_airport() {
        let errors = errors(journey(json!([{
            "mode": "walk",
            "origin": {"type": "custom", "name": "Not really", "kind": "airport"}
        }])));

        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("cannot be \"airport\""));
    }

    #[test]
    fn a_new_vehicle_needs_something_to_call_it() {
        let errors = errors(journey(json!([{
            "mode": "drive",
            "drive": {"vehicle": {"type": "new", "year": 2019}}
        }])));

        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("nickname, make, or model"));
    }

    #[test]
    fn metadata_must_be_an_object() {
        let errors = errors(journey(json!([{"mode": "walk", "metadata": ["nope"]}])));

        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("segments[0].metadata must be a JSON object"));
    }

    #[test]
    fn one_waypoint_described_twice_is_one_place() {
        let request = journey(json!([
            {
                "mode": "drive",
                "destination": {"type": "custom", "name": "Cotswolds cottage",
                                "city": "Stow-on-the-Wold", "latitude": 51.929, "longitude": -1.722}
            },
            {
                // The same building, named in shorthand as the next leg's start.
                "mode": "walk",
                "origin": {"type": "custom", "name": "Cotswolds cottage",
                           "latitude": 51.929, "longitude": -1.722},
                "destination": {"type": "custom", "name": "The Porch House",
                                "latitude": 51.9295, "longitude": -1.7235}
            }
        ]));

        let journey = request.validate().expect("should be valid");
        let arrival = journey.segments[0].destination.as_ref().unwrap();
        let departure = journey.segments[1].origin.as_ref().unwrap();
        let elsewhere = journey.segments[1].destination.as_ref().unwrap();

        assert_eq!(arrival.cache_key(), departure.cache_key());
        assert_ne!(arrival.cache_key(), elsewhere.cache_key());
    }

    #[test]
    fn every_segment_reports_at_once() {
        let errors = errors(journey(json!([
            {"mode": "nope"},
            {"mode": "nope"},
            {"mode": "nope"}
        ])));

        assert_eq!(errors.len(), 3);
    }
}
