//! SumPrices backend library. `main.rs` is a thin binary over this; integration
//! tests (`tests/`) import it directly.
use std::sync::Arc;

use crate::extraction::ReceiptExtractor;
use crate::storage::s3::Storage;

pub mod analytics;
pub mod auth;
pub mod config;
pub mod db;
pub mod enums;
pub mod errors;
pub mod extraction;
pub mod receipts;
pub mod storage;
pub mod transactions;

/// Git commit that built this binary (embedded by build.rs). Stored on parsed
/// receipts so the ingest harness can detect parser-code changes and re-parse.
pub const GIT_COMMIT: &str = env!("GIT_COMMIT");

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub storage: Storage,
    pub extractor: Arc<dyn ReceiptExtractor>,
    pub config: crate::config::Config,
}

impl axum::extract::FromRef<AppState> for crate::config::Config {
    fn from_ref(state: &AppState) -> Self {
        state.config.clone()
    }
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

impl axum::extract::FromRef<AppState> for Arc<dyn ReceiptExtractor> {
    fn from_ref(state: &AppState) -> Self {
        state.extractor.clone()
    }
}
