use super::*;

use serde_json::json;

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
