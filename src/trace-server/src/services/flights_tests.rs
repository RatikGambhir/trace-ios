use super::*;

use crate::models::entities::flight::ResolvedAirport;

#[test]
fn distance_needs_both_sets_of_coordinates() {
    let jfk = ResolvedAirport {
        id: 1,
        latitude: Some(40.639751),
        longitude: Some(-73.778925),
    };
    let lhr = ResolvedAirport {
        id: 2,
        latitude: Some(51.470020),
        longitude: Some(-0.454295),
    };
    let unmapped = ResolvedAirport {
        id: 3,
        latitude: None,
        longitude: None,
    };

    assert_eq!(distance_between(&jfk, &lhr), Some(3443));
    assert_eq!(distance_between(&jfk, &unmapped), None);
    assert_eq!(distance_between(&unmapped, &lhr), None);
}
