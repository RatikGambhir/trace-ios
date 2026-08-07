//! Journeys: a user's trip and its ordered segments.
//!
//! Request types, the rows they become, and the validation between the two.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

use crate::{
    core::error::ApiError,
    core::validation::{field, object_or_default, one_of, optional_text},
    models::flight::{AirportInput, FlightInput, ValidatedAirport, ValidatedFlight},
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

impl ValidatedPlace {
    /// What makes two descriptions in one request the same place.
    ///
    /// A name at a coordinate is an identity; `kind` and `city` are description,
    /// and a caller who spells the arrival out in full and the departure in
    /// shorthand still means one building. Where the fuller description comes
    /// first, that is the one stored.
    pub fn cache_key(&self) -> String {
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

#[cfg(test)]
#[path = "journey_tests.rs"]
mod tests;
