//! Rust mirrors of the PostgreSQL ENUM types (DESIGN §7.0).
//! `sqlx::Type` maps them to the native PG enums; serde emits snake_case for the API.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "receipt_source", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ReceiptSource {
    CameraPhoto,
    ImageUpload,
    PdfUpload,
    EreceiptApi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "extraction_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ExtractionStatus {
    Pending,
    Queued,
    Processing,
    Done,
    Failed,
    NeedsReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "item_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ItemType {
    Product,
    Deposit,
    Discount,
    Fee,
    Rounding,
    /// A cashier void/reversal of a previously-scanned line (e.g. Norwegian "KORR.").
    Correction,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "price_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum PriceType {
    Shelf,
    Promo,
    Member,
    Coupon,
    NetOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "fraud_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum FraudStatus {
    Ok,
    Suspected,
    Confirmed,
    Dismissed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "ledger_reason", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum LedgerReason {
    ScanReward,
    PriceQuery,
    SignupBonus,
    Referral,
    Adjustment,
    Reversal,
}
