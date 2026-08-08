//! Resolving a flight to its canonical row, and the distance that falls out of
//! doing so.
//!
//! A flight is shared: AA100 on a given day is one `flights` row however many
//! people were aboard. Resolving one means get-or-creating both airports and the
//! airline first, then the flight itself — all on the caller's connection, so it
//! joins the caller's transaction.

use sqlx::PgConnection;

use crate::{
    core::{
        error::ApiError,
        geo::{great_circle_miles, Coordinates},
        validation::field,
    },
    models::entities::flight::{Flight, ResolvedAirport},
    models::requests::flight::ValidatedFlight,
    repositories::{airlines, airports, flights},
};

/// A flight and the airports it turned out to connect.
pub struct ResolvedFlight {
    pub flight: Flight,
    pub origin: ResolvedAirport,
    pub destination: ResolvedAirport,
}

/// Great-circle distance between two resolved airports, rounded to whole miles.
///
/// `None` when either side has no coordinates on file — the column is nullable
/// precisely so that a sparsely populated airport does not block the flight.
pub fn distance_between(origin: &ResolvedAirport, destination: &ResolvedAirport) -> Option<i32> {
    let from = coordinates(origin)?;
    let to = coordinates(destination)?;

    Some(great_circle_miles(from, to).round() as i32)
}

fn coordinates(airport: &ResolvedAirport) -> Option<Coordinates> {
    Some(Coordinates {
        latitude: airport.latitude?,
        longitude: airport.longitude?,
    })
}

/// Get-or-create the airports, the airline, and the flight, deriving
/// `distance_miles` from the airports' stored coordinates on the way.
///
/// `prefix` names where this flight sits in the request, so a failure to resolve
/// an airport can say which one.
pub async fn resolve(
    conn: &mut PgConnection,
    flight: &ValidatedFlight,
    prefix: &str,
) -> Result<ResolvedFlight, ApiError> {
    let origin = airports::resolve(conn, &flight.origin, &field(prefix, "origin")).await?;
    let destination =
        airports::resolve(conn, &flight.destination, &field(prefix, "destination")).await?;
    let airline_id = airlines::resolve(conn, &flight.airline).await?;

    // Derived here, never taken from the caller.
    let distance_miles = distance_between(&origin, &destination);

    let row = flights::insert(
        conn,
        flight,
        airline_id,
        origin.id,
        destination.id,
        distance_miles,
    )
    .await?
    .into_row();

    Ok(ResolvedFlight {
        flight: row,
        origin,
        destination,
    })
}

#[cfg(test)]
#[path = "flights_tests.rs"]
mod tests;
