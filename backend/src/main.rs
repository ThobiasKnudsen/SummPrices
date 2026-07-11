use axum::Router;
use axum::routing::{delete, get, post, put};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use kvitteringsapp_backend::storage::s3::Storage;
use kvitteringsapp_backend::{
    analytics, auth, config, db, extraction, receipts, transactions, AppState,
};

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

    if let Err(e) = Storage::ensure_bucket(&config).await {
        tracing::warn!("bucket ensure failed: {e}");
    }
    let storage = Storage::new(&config);
    let extractor = extraction::build_from_env(&config);

    let state = AppState {
        pool,
        storage,
        extractor,
        config,
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
        .route("/api/receipts/{id}/status", get(receipts::handlers::status))
        .route(
            "/api/receipts/{id}/reprocess",
            post(receipts::handlers::reprocess),
        )
        // Debug: selectable extraction models + bulk rescan for the in-app model picker.
        .route("/api/debug/models", get(receipts::handlers::debug_models))
        .route(
            "/api/debug/reprocess-all",
            post(receipts::handlers::reprocess_all),
        )
        // Transactions
        .route("/api/transactions", get(transactions::handlers::list))
        .route("/api/transactions/{id}", put(transactions::handlers::update))
        .route("/api/transactions/{id}", delete(transactions::handlers::delete))
        // Analytics
        .route("/api/analytics/spending", get(analytics::handlers::spending))
        .route("/api/analytics/by-store", get(analytics::handlers::by_store))
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind to {bind_addr}: {e}"));

    tracing::info!("Server running on http://{bind_addr}");
    axum::serve(listener, app).await.expect("Server error");
}

async fn health() -> &'static str {
    "ok"
}
