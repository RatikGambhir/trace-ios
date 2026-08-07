use sqlx::PgPool;

/// Shared application state. Handlers receive this as `State<Arc<AppState>>`,
/// so cloning across requests is just an `Arc` bump.
pub struct AppState {
    pub db: PgPool,
}
