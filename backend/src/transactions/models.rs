use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::enums::ItemType;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TransactionWithContext {
    pub id: i64,
    pub receipt_id: Uuid,
    pub description_raw: String,
    pub description_clean: Option<String>,
    pub item_type: ItemType,
    pub quantity: Option<Decimal>,
    pub unit: Option<String>,
    pub unit_price: Option<Decimal>,
    pub line_total: Option<Decimal>,
    pub mva_rate: Option<Decimal>,
    pub store_name_raw: Option<String>,
    pub currency: String,
    pub purchase_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct TransactionListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub q: Option<String>,
    pub store: Option<String>,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
}

#[derive(Debug, Serialize)]
pub struct TransactionListResponse {
    pub transactions: Vec<TransactionWithContext>,
    pub total_count: i64,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTransactionRequest {
    pub description_clean: Option<String>,
    pub quantity: Option<Decimal>,
    pub unit_price: Option<Decimal>,
    pub line_total: Option<Decimal>,
}
