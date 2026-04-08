use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ItemWithContext {
    pub id: Uuid,
    pub receipt_id: Uuid,
    pub description: String,
    pub quantity: Option<f32>,
    pub unit_price: Option<Decimal>,
    pub line_total: Option<Decimal>,
    pub product_code: Option<String>,
    pub store_name: Option<String>,
    pub purchase_date: Option<NaiveDate>,
}

#[derive(Debug, Serialize)]
pub struct ItemListResponse {
    pub items: Vec<ItemWithContext>,
    pub total_count: i64,
}

#[derive(Debug, Deserialize)]
pub struct ItemListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub q: Option<String>,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub store: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateItemRequest {
    pub description: Option<String>,
    pub quantity: Option<f32>,
    pub unit_price: Option<Decimal>,
    pub line_total: Option<Decimal>,
}
