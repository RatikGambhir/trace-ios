use super::*;

fn at(latitude: f64, longitude: f64) -> Coordinates {
    Coordinates {
        latitude,
        longitude,
    }
}

/// Published great-circle distances, to the nearest few miles.
#[test]
fn matches_known_airport_pairs() {
    let jfk = at(40.639751, -73.778925);
    let lhr = at(51.470020, -0.454295);
    let lax = at(33.942536, -118.408075);
    let sfo = at(37.618972, -122.374889);

    assert!((great_circle_miles(jfk, lhr) - 3451.0).abs() < 10.0);
    assert!((great_circle_miles(jfk, lax) - 2475.0).abs() < 10.0);
    assert!((great_circle_miles(lax, sfo) - 337.0).abs() < 10.0);
}

#[test]
fn is_symmetric_and_zero_for_the_same_point() {
    let jfk = at(40.639751, -73.778925);
    let lhr = at(51.470020, -0.454295);

    assert_eq!(great_circle_miles(jfk, jfk), 0.0);
    assert!((great_circle_miles(jfk, lhr) - great_circle_miles(lhr, jfk)).abs() < 1e-9);
}

#[test]
fn handles_the_antimeridian_and_antipodes() {
    // Either side of the date line: a short hop, not most of the way round.
    let west = at(0.0, 179.5);
    let east = at(0.0, -179.5);
    assert!((great_circle_miles(west, east) - 69.0).abs() < 1.0);

    // Antipodal points are half the circumference apart, and `a` lands on
    // 1.0 there — the `.max(0.0)` keeps the square root finite.
    let north = at(90.0, 0.0);
    let south = at(-90.0, 0.0);
    assert!((great_circle_miles(north, south) - 12437.0).abs() < 5.0);
}
