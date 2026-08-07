//! `airlines`, on the same get-or-create terms as airports.

use sqlx::PgConnection;

use crate::{
    core::error::{unique_violation, ApiError},
    models::requests::flight::ValidatedAirline,
};

/// [`resolve_airport`].
pub async fn resolve(conn: &mut PgConnection, airline: &ValidatedAirline) -> Result<i64, ApiError> {
    if let Some(id) = select(conn, &airline.iata_code).await? {
        return Ok(id);
    }

    let name = airline.name.as_deref().ok_or_else(|| {
        ApiError::Validation(vec![format!(
            "airline.name is required: airline {} is not in the database yet",
            airline.iata_code
        )])
    })?;

    let inserted: Option<(i64,)> = sqlx::query_as(
        r#"
        INSERT INTO airlines (iata_code, icao_code, name)
        VALUES ($1, $2, $3)
        ON CONFLICT (iata_code) DO NOTHING
        RETURNING id
        "#,
    )
    .bind(&airline.iata_code)
    .bind(&airline.icao_code)
    .bind(name)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|err| {
        unique_violation(err, |constraint| match constraint {
            "airlines_icao_code_key" => {
                Some("airline.icao_code already belongs to a different airline".to_string())
            }
            _ => None,
        })
    })?;

    if let Some((id,)) = inserted {
        tracing::info!(iata_code = %airline.iata_code, "inserted airline");
        return Ok(id);
    }

    select(conn, &airline.iata_code).await?.ok_or_else(|| {
        ApiError::Conflict(format!(
            "airline {} is being created by another request; retry",
            airline.iata_code
        ))
    })
}

async fn select(conn: &mut PgConnection, iata_code: &str) -> Result<Option<i64>, ApiError> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT id FROM airlines WHERE iata_code = $1")
        .bind(iata_code)
        .fetch_optional(conn)
        .await?;

    Ok(row.map(|(id,)| id))
}
