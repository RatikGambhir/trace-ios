use super::*;

fn airport(iata: &str, latitude: f64, longitude: f64) -> AirportRequest {
    AirportRequest {
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

fn request() -> FlightRequest {
    FlightRequest {
        airline: AirlineRequest {
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
fn errors(request: FlightRequest) -> Vec<String> {
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
