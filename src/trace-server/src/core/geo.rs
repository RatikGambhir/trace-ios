/// Mean Earth radius in statute miles (IUGG mean radius, 6371.0088 km).
const EARTH_RADIUS_MILES: f64 = 3958.7613;

/// A point on the globe, in decimal degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coordinates {
    pub latitude: f64,
    pub longitude: f64,
}

/// Great-circle distance between two points, in statute miles.
///
/// Uses the haversine formula with `atan2`, which stays accurate for short
/// distances where the law-of-cosines form loses precision.
pub fn great_circle_miles(from: Coordinates, to: Coordinates) -> f64 {
    let lat1 = from.latitude.to_radians();
    let lat2 = to.latitude.to_radians();
    let delta_lat = (to.latitude - from.latitude).to_radians();
    let delta_lon = (to.longitude - from.longitude).to_radians();

    let a =
        (delta_lat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (delta_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).max(0.0).sqrt());

    EARTH_RADIUS_MILES * c
}

#[cfg(test)]
#[path = "geo_tests.rs"]
mod tests;
