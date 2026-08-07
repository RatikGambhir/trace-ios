//! `airports`, and the get-or-create behind an airport reference.

use sqlx::PgConnection;

use crate::{
    core::error::{unique_violation, ApiError},
    models::entities::flight::ResolvedAirport,
    models::requests::flight::ValidatedAirport,
};

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

    let inserted = sqlx::query_as::<_, ResolvedAirport>(
        r#"
        INSERT INTO airports (
            iata_code, icao_code, name, city, country_code, latitude, longitude, timezone
        )
        VALUES ($1, $2, $3, $4, $5, $6::float8::numeric, $7::float8::numeric, $8)
        ON CONFLICT (iata_code) DO NOTHING
        RETURNING id, latitude::float8 AS latitude, longitude::float8 AS longitude
        "#,
    )
    .bind(&airport.iata_code)
    .bind(&airport.icao_code)
    .bind(name)
    .bind(&airport.city)
    .bind(&airport.country_code)
    .bind(airport.latitude)
    .bind(airport.longitude)
    .bind(&airport.timezone)
    .fetch_optional(&mut *conn)
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
    // DECIMAL(9,6) has no native Rust mapping in this build of sqlx, and the
    // coordinates only ever feed a float computation — cast on the way out.
    sqlx::query_as::<_, ResolvedAirport>(
        r#"
        SELECT id, latitude::float8 AS latitude, longitude::float8 AS longitude
        FROM airports
        WHERE iata_code = $1
        "#,
    )
    .bind(iata_code)
    .fetch_optional(conn)
    .await
    .map_err(ApiError::Database)
}
