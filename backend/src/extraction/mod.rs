//! Receipt extraction behind a swappable trait (DESIGN §6). The whole pipeline
//! depends only on `ExtractedReceipt`, so the engine (mock / hosted VLM) is a config detail.
use std::str::FromStr;
use std::sync::Arc;

use chrono::{DateTime, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Europe::Oslo;
use rust_decimal::Decimal;

use crate::config::Config;
use crate::enums::{ItemType, PriceType};
use crate::errors::AppError;

pub mod hosted_vlm;
pub mod mock;

#[derive(Debug, Clone)]
pub struct ExtractedLineItem {
    pub description_raw: String,
    pub description_clean: Option<String>,
    pub product_code: Option<String>, // EAN/barcode when printed
    pub item_type: ItemType,
    pub quantity: Option<Decimal>,
    pub unit: Option<String>,
    pub shelf_unit_price: Option<Decimal>,
    pub unit_price: Option<Decimal>,
    pub discount_amount: Option<Decimal>,
    pub line_total: Option<Decimal>,
    pub price_type: PriceType,
    pub mva_rate: Option<Decimal>,
}

#[derive(Debug, Clone)]
pub struct ExtractedReceipt {
    pub store_name_raw: Option<String>,
    pub org_no: Option<String>,
    pub store_address: Option<String>,
    pub store_city: Option<String>,
    pub store_postal_code: Option<String>,
    pub store_country_code: Option<String>,
    pub receipt_number: Option<String>,
    pub payment_method: Option<String>, // card | cash | vipps | ... (never card digits)
    pub purchase_at: Option<DateTime<Utc>>,
    pub currency: String,
    pub subtotal: Option<Decimal>,
    pub mva_total: Option<Decimal>,
    pub total: Option<Decimal>,
    pub line_items: Vec<ExtractedLineItem>,
    pub confidence: Option<f32>, // model self-reported completeness/correctness, 0..1
    pub notes: Option<String>,   // model-reported problems (blurry, cropped, unreadable lines)
    pub engine: String,
    pub raw: serde_json::Value, // the model's receipt JSON (mva_lines, payment, … also live here)
}

#[async_trait::async_trait]
pub trait ReceiptExtractor: Send + Sync {
    async fn extract(&self, bytes: &[u8], mime: &str) -> Result<ExtractedReceipt, AppError>;
    #[allow(dead_code)]
    fn engine_id(&self) -> &str;
}

pub fn build_from_env(config: &Config) -> Arc<dyn ReceiptExtractor> {
    match config.extractor.as_str() {
        "hosted" => Arc::new(hosted_vlm::HostedVlmExtractor::new(
            &config.vlm_url,
            &config.vlm_model,
            config.vlm_api_key.clone(),
        )),
        _ => Arc::new(mock::MockExtractor::new()),
    }
}

/// Build a hosted extractor for a specific model, reusing the configured endpoint + key.
/// Used by the debug model picker to rescan a receipt with an on-the-fly model choice.
pub fn build_hosted_model(config: &Config, model: &str) -> Arc<dyn ReceiptExtractor> {
    Arc::new(hosted_vlm::HostedVlmExtractor::new(
        &config.vlm_url,
        model,
        config.vlm_api_key.clone(),
    ))
}

/// f64 -> Decimal at 2 decimal places (money).
pub(crate) fn dec2(v: Option<f64>) -> Option<Decimal> {
    v.and_then(|f| Decimal::from_str(&format!("{f:.2}")).ok())
}

/// f64 -> Decimal at 3 decimal places (quantities, incl. weights).
pub(crate) fn dec3(v: Option<f64>) -> Option<Decimal> {
    v.and_then(|f| Decimal::from_str(&format!("{f:.3}")).ok())
}

/// Parse a receipt's local wall-clock string into a universal instant, interpreting
/// naive (zone-less) times as Europe/Oslo (DESIGN §7.4 fallback).
pub(crate) fn parse_purchase_at(s: Option<&str>) -> Option<DateTime<Utc>> {
    let s = s?.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    for fmt in [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%dT%H:%M",
    ] {
        if let Ok(ndt) = NaiveDateTime::parse_from_str(s, fmt) {
            return oslo_to_utc(ndt);
        }
    }
    if let Ok(nd) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return nd.and_hms_opt(0, 0, 0).and_then(oslo_to_utc);
    }
    None
}

fn oslo_to_utc(ndt: NaiveDateTime) -> Option<DateTime<Utc>> {
    match Oslo.from_local_datetime(&ndt) {
        LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => Some(dt.with_timezone(&Utc)),
        LocalResult::None => None,
    }
}
