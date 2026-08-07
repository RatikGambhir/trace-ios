//! Route table and the middleware stack.
//!
//! Kept apart from `main` so tests can drive `app()` directly with `oneshot`
//! instead of binding a port.

use std::{sync::Arc, time::Duration};

use axum::{
    http::StatusCode,
    routing::{get, post},
    Router,
};
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, timeout::TimeoutLayer, trace::TraceLayer};

use crate::{core::state::AppState, handlers};

/// Ceiling on a single request, so one stuck transaction cannot pin a pool
/// connection for the life of the process.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

pub fn app(state: Arc<AppState>) -> Router {
    let api = Router::new()
        .route("/users", post(handlers::users::create))
        .route("/users/{id}", get(handlers::users::get))
        .route("/journeys", post(handlers::journeys::create))
        .route("/journeys/{id}", get(handlers::journeys::get));

    Router::new()
        .route("/health", get(handlers::health::health))
        .route("/ready", get(handlers::health::ready))
        .nest("/api/v1", api)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                // A journey POST can touch a lot of rows in one transaction;
                // cap it so one slow request cannot hold a pool connection open
                // indefinitely.
                .layer(TimeoutLayer::with_status_code(
                    StatusCode::SERVICE_UNAVAILABLE,
                    REQUEST_TIMEOUT,
                ))
                .layer(CorsLayer::permissive()),
        )
        .with_state(state)
}

#[cfg(test)]
#[path = "router_tests.rs"]
mod tests;
