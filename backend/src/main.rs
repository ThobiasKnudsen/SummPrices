mod auth;
mod config;
mod db;
mod errors;
mod receipts;
mod storage;

use std::sync::Arc;

use axum::Router;
use axum::routing::{delete, get, post, put};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::receipts::ocr::TabscannerClient;
use crate::storage::s3::Storage;

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub storage: Storage,
    pub ocr_client: Arc<TabscannerClient>,
}

impl axum::extract::FromRef<AppState> for sqlx::PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}

impl axum::extract::FromRef<AppState> for Storage {
    fn from_ref(state: &AppState) -> Self {
        state.storage.clone()
    }
}

impl axum::extract::FromRef<AppState> for Arc<TabscannerClient> {
    fn from_ref(state: &AppState) -> Self {
        state.ocr_client.clone()
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    dotenvy::dotenv().ok();
    let config = config::Config::from_env();

    let pool = db::create_pool(&config.database_url).await;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    let storage = Storage::new(&config);
    let ocr_client = Arc::new(TabscannerClient::new(&config.tabscanner_api_key));

    let state = AppState {
        pool,
        storage,
        ocr_client,
    };

    let app = Router::new()
        .route("/health", get(health))
        // Auth
        .route("/api/auth/register", post(auth::handlers::register))
        .route("/api/auth/login", post(auth::handlers::login))
        .route("/api/auth/me", get(auth::handlers::me))
        // Receipts
        .route("/api/receipts", post(receipts::handlers::upload))
        .route("/api/receipts", get(receipts::handlers::list))
        .route("/api/receipts/{id}", get(receipts::handlers::get_one))
        .route("/api/receipts/{id}", put(receipts::handlers::update))
        .route("/api/receipts/{id}", delete(receipts::handlers::delete))
        .route(
            "/api/receipts/{id}/status",
            get(receipts::handlers::ocr_status),
        )
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind to port 3000");

    tracing::info!("Server running on http://0.0.0.0:3000");
    axum::serve(listener, app).await.expect("Server error");
}

async fn health() -> &'static str {
    "ok"
}
