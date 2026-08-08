//! `airlines`, on the same get-or-create terms as airports.

use sqlx::PgConnection;

use crate::{
    core::error::{unique_violation, ApiError},
    core::sql_builder::{Insert, Select},
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

    let inserted: Option<i64> = Insert::into("airlines")
        .set("iata_code", &airline.iata_code)
        .set("icao_code", &airline.icao_code)
        .set("name", name)
        .on_conflict_do_nothing(&["iata_code"])
        .returning(&["id"])
        .fetch_optional_scalar(conn)
        .await
        .map_err(|err| {
            unique_violation(err, |constraint| match constraint {
                "airlines_icao_code_key" => {
                    Some("airline.icao_code already belongs to a different airline".to_string())
                }
                _ => None,
            })
        })?;

    if let Some(id) = inserted {
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
    Select::from("airlines")
        .columns(&["id"])
        .where_eq("iata_code", iata_code)
        .fetch_optional_scalar(conn)
        .await
        .map_err(ApiError::Database)
}
