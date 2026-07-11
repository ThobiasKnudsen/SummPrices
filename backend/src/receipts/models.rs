use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::enums::{ExtractionStatus, ItemType};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Receipt {
    pub id: Uuid,
    pub user_id: Uuid,
    pub store_name_raw: Option<String>,
    pub store_address: Option<String>,
    pub store_city: Option<String>,
    pub store_postal_code: Option<String>,
    pub store_country_code: Option<String>,
    pub purchase_at: Option<DateTime<Utc>>,
    pub subtotal: Option<Decimal>,
    pub mva_total: Option<Decimal>,
    pub total: Option<Decimal>,
    pub currency: String,
    pub extraction_status: ExtractionStatus,
    pub extraction_conf: Option<f32>,
    pub needs_review: bool,
    pub review_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip)]
    pub original_asset_key: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ReceiptSummary {
    pub id: Uuid,
    pub store_name_raw: Option<String>,
    pub purchase_at: Option<DateTime<Utc>>,
    pub total: Option<Decimal>,
    pub currency: String,
    pub extraction_status: ExtractionStatus,
    pub extraction_conf: Option<f32>,
    pub needs_review: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Transaction {
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
}

#[derive(Debug, Serialize)]
pub struct ReceiptWithTransactions {
    #[serde(flatten)]
    pub receipt: Receipt,
    pub transactions: Vec<Transaction>,
    pub image_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReceiptListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub store: Option<String>,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReceiptListResponse {
    pub receipts: Vec<ReceiptSummary>,
    pub total_count: i64,
}

#[derive(Debug, Deserialize)]
pub struct UpdateReceiptRequest {
    pub store_name: Option<String>,
    pub purchase_at: Option<DateTime<Utc>>,
    pub total: Option<Decimal>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ExtractionStatusResponse {
    pub extraction_status: ExtractionStatus,
    pub extraction_conf: Option<f32>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ReprocessRequest {
    /// Optional model override for this scan only (debug model picker).
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReprocessAllResponse {
    pub queued: i64,
}

#[derive(Debug, Serialize)]
pub struct DebugModelsResponse {
    pub current: String,
    pub options: Vec<String>,
}
