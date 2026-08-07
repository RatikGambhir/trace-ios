mod error;
mod flights;
mod geo;
mod handlers;
mod models;
mod state;

use std::{sync::Arc, time::Duration};

use anyhow::Context;
use axum::{
    routing::{get, post},
    Router,
};
use sqlx::postgres::PgPoolOptions;
use tokio::{net::TcpListener, signal};
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::state::AppState;

const DEFAULT_PORT: u16 = 8080;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "trace_server=info,tower_http=info,axum::rejection=trace".into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Railway injects both of these: DATABASE_URL via a ${{Postgres.DATABASE_URL}}
    // reference, PORT by the platform.
    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    let port: u16 = match std::env::var("PORT") {
        Ok(value) => value.parse().context("PORT must be a valid port number")?,
        Err(_) => DEFAULT_PORT,
    };

    let db = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url)
        .await
        .context("failed to connect to the database")?;

    let state = Arc::new(AppState { db });

    let listener = TcpListener::bind(("0.0.0.0", port))
        .await
        .with_context(|| format!("failed to bind 0.0.0.0:{port}"))?;

    tracing::info!("listening on 0.0.0.0:{port}");

    axum::serve(listener, app(state))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;

    Ok(())
}

/// Build the router. Kept separate from `main` so tests can drive it directly
/// with `oneshot` instead of binding a port.
fn app(state: Arc<AppState>) -> Router {
    let api = Router::new()
        .route("/users", post(handlers::create_user))
        .route("/users/{id}", get(handlers::get_user))
        .route("/flights", post(handlers::create_flight));

    Router::new()
        .route("/health", get(handlers::health))
        .route("/ready", get(handlers::ready))
        .nest("/api/v1", api)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive()),
        )
        .with_state(state)
}

/// Finish in-flight requests when the platform sends SIGTERM (or on Ctrl-C locally).
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutdown signal received");
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    /// A pool that is never actually connected. Enough to build the router, and
    /// enough for the routes below, which answer before touching the database.
    fn test_state() -> Arc<AppState> {
        Arc::new(AppState {
            db: PgPoolOptions::new()
                .connect_lazy("postgres://postgres@localhost/postgres")
                .expect("the test connection string parses"),
        })
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let response = app(test_state())
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["status"], "healthy");
    }

    #[tokio::test]
    async fn create_user_rejects_an_invalid_payload_before_hitting_the_database() {
        let response = app(test_state())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/users")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"first_name":"","last_name":"Lovelace","password":"short"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["error"], "validation_failed");
        assert_eq!(body["details"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn create_flight_rejects_an_invalid_payload_before_opening_a_transaction() {
        let response = app(test_state())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/flights")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "airline": {"iata_code": "AA", "name": "American Airlines"},
                            "flight_number": "100",
                            "origin": {"iata_code": "JFK", "name": "Kennedy"},
                            "destination": {"iata_code": "JFK", "name": "Kennedy"},
                            "scheduled_departure_at": "2026-08-10T22:00:00Z",
                            "scheduled_arrival_at": "2026-08-10T21:00:00Z"
                        }"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["error"], "validation_failed");
        // Same origin and destination, and an arrival before the departure.
        assert_eq!(body["details"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn unknown_routes_are_not_found() {
        let response = app(test_state())
            .oneshot(Request::builder().uri("/nope").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
