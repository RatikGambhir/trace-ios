//! `airports`, and the get-or-create behind an airport reference.

use sqlx::PgConnection;

use crate::{
    core::error::{unique_violation, ApiError},
    core::sql_builder::{Insert, Select},
    models::entities::flight::ResolvedAirport,
    models::requests::flight::ValidatedAirport,
};

/// What a [`ResolvedAirport`] is made of, for the read and the insert alike.
///
/// DECIMAL(9,6) has no native Rust mapping in this build of sqlx, and the
/// coordinates only ever feed a float computation — cast on the way out.
const RESOLVED_COLUMNS: &[&str] = &[
    "id",
    "latitude::float8 AS latitude",
    "longitude::float8 AS longitude",
];

/// Find the airport by IATA code, inserting it if we have never seen it.
///
/// Three steps, in order of likelihood: read (the common case, and no write),
/// insert-if-absent, then read again for the request that lost the race. The
/// insert is `ON CONFLICT DO NOTHING`, so two concurrent callers cannot both
/// create the row, and neither gets an error.
///
/// When the airport already exists its stored details win — this call never
/// overwrites them from the request body, so it stays a pure get-or-create.
pub async fn resolve(
    conn: &mut PgConnection,
    airport: &ValidatedAirport,
    field: &str,
) -> Result<ResolvedAirport, ApiError> {
    if let Some(existing) = select(conn, &airport.iata_code).await? {
        return Ok(existing);
    }

    // Unknown airport, so this is an insert — and the columns the insert needs
    // are no longer optional.
    let name = airport.name.as_deref().ok_or_else(|| {
        ApiError::Validation(vec![format!(
            "{field}.name is required: airport {} is not in the database yet",
            airport.iata_code
        )])
    })?;

    let inserted: Option<ResolvedAirport> = Insert::into("airports")
        .set("iata_code", &airport.iata_code)
        .set("icao_code", &airport.icao_code)
        .set("name", name)
        .set("city", &airport.city)
        .set("country_code", &airport.country_code)
        .set_cast("latitude", airport.latitude, "float8::numeric")
        .set_cast("longitude", airport.longitude, "float8::numeric")
        .set("timezone", &airport.timezone)
        .on_conflict_do_nothing(&["iata_code"])
        .returning(RESOLVED_COLUMNS)
        .fetch_optional(conn)
        .await
        .map_err(|err| {
            unique_violation(err, |constraint| match constraint {
                "airports_icao_code_key" => Some(format!(
                    "{field}.icao_code already belongs to a different airport"
                )),
                _ => None,
            })
        })?;

    if let Some(inserted) = inserted {
        tracing::info!(iata_code = %airport.iata_code, "inserted airport");
        return Ok(inserted);
    }

    // The insert conflicted, so someone else created it: read theirs.
    select(conn, &airport.iata_code).await?.ok_or_else(|| {
        ApiError::Conflict(format!(
            "airport {} is being created by another request; retry",
            airport.iata_code
        ))
    })
}

async fn select(
    conn: &mut PgConnection,
    iata_code: &str,
) -> Result<Option<ResolvedAirport>, ApiError> {
    Select::from("airports")
        .columns(RESOLVED_COLUMNS)
        .where_eq("iata_code", iata_code)
        .fetch_optional(conn)
        .await
        .map_err(ApiError::Database)
}
