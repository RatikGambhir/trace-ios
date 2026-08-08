//! Configuration, wiring, and serving. Everything else lives in a layer:
//!
//! ```text
//! handlers/      HTTP in, status code out
//! services/      transactions, idempotency, derived values
//! repositories/  SQL, on a connection the caller owns
//! models/        request and row types, and their validation
//! core/          error shape, geometry, crypto, validation helpers, state
//! ```

mod core;
mod handlers;
mod models;
mod repositories;
mod router;
mod services;

use std::{sync::Arc, time::Duration};

use anyhow::Context;
use sqlx::postgres::PgPoolOptions;
use tokio::{net::TcpListener, signal};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::{core::state::AppState, router::app};

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
