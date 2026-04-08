use reqwest::Client;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::str::FromStr;
use uuid::Uuid;

use crate::errors::AppError;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TabscannerProcessResponse {
    token: Option<String>,
    status: Option<String>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TabscannerResultResponse {
    status: String,
    result: Option<TabscannerResult>,
    message: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TabscannerResult {
    establishment: Option<String>,
    date: Option<String>,
    total: Option<f64>,
    subtotal: Option<f64>,
    #[serde(default)]
    line_items: Vec<TabscannerLineItem>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct TabscannerLineItem {
    desc_clean: Option<String>,
    desc: Option<String>,
    qty: Option<f64>,
    price: Option<f64>,
    line_total: Option<f64>,
    product_code: Option<String>,
}

pub struct TabscannerClient {
    client: Client,
    api_key: String,
}

impl TabscannerClient {
    pub fn new(api_key: &str) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.to_string(),
        }
    }

    /// Submit an image to Tabscanner for processing. Returns a token to poll for results.
    pub async fn process(&self, image_data: &[u8], content_type: &str) -> Result<String, AppError> {
        let file_part = reqwest::multipart::Part::bytes(image_data.to_vec())
            .file_name("receipt.jpg")
            .mime_str(content_type)
            .map_err(|e| AppError::Internal(format!("Invalid content type: {e}")))?;

        let form = reqwest::multipart::Form::new()
            .part("file", file_part)
            .text("documentType", "receipt")
            .text("region", "NO");

        let response = self
            .client
            .post("https://api.tabscanner.com/api/2/process")
            .header("apikey", &self.api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Tabscanner request failed: {e}")))?;

        let body: TabscannerProcessResponse = response
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Tabscanner parse failed: {e}")))?;

        body.token.ok_or_else(|| {
            AppError::Internal(format!(
                "Tabscanner process failed: {}",
                body.message.unwrap_or_else(|| "unknown error".into())
            ))
        })
    }

    /// Poll Tabscanner for OCR results. Returns None if still processing.
    pub(crate) async fn get_result(
        &self,
        token: &str,
    ) -> Result<Option<TabscannerResult>, AppError> {
        let response = self
            .client
            .get(format!("https://api.tabscanner.com/api/result/{token}"))
            .header("apikey", &self.api_key)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Tabscanner request failed: {e}")))?;

        let body: TabscannerResultResponse = response
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Tabscanner parse failed: {e}")))?;

        match body.status.as_str() {
            "done" => Ok(body.result),
            "pending" => Ok(None),
            _ => Err(AppError::Internal(format!(
                "Tabscanner error: {}",
                body.message.unwrap_or_else(|| "unknown".into())
            ))),
        }
    }
}

/// Submit a receipt image to Tabscanner and store the token.
pub async fn submit_for_ocr(
    pool: &PgPool,
    receipt_id: Uuid,
    image_data: &[u8],
    content_type: &str,
    client: &TabscannerClient,
) -> Result<(), AppError> {
    let token = client.process(image_data, content_type).await?;

    sqlx::query("UPDATE receipts SET ocr_token = $1, ocr_status = 'processing' WHERE id = $2")
        .bind(&token)
        .bind(receipt_id)
        .execute(pool)
        .await?;

    Ok(())
}

/// Check Tabscanner for results and store them if done.
/// Returns the new ocr_status.
pub async fn poll_and_store(
    pool: &PgPool,
    receipt_id: Uuid,
    user_id: Uuid,
    ocr_token: &str,
    client: &TabscannerClient,
) -> Result<String, AppError> {
    let result = client.get_result(ocr_token).await?;

    let result = match result {
        Some(r) => r,
        None => return Ok("processing".to_string()),
    };

    // Store the full raw result as JSON before moving fields out
    let ocr_raw =
        serde_json::to_value(&result).unwrap_or(serde_json::Value::Null);

    // Parse receipt-level fields
    let store_name = result.establishment;
    let purchase_date = result
        .date
        .as_deref()
        .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
    let total = result
        .total
        .and_then(|t| Decimal::from_str(&format!("{t:.2}")).ok());
    let subtotal = result
        .subtotal
        .and_then(|t| Decimal::from_str(&format!("{t:.2}")).ok());

    // Update receipt
    sqlx::query(
        "UPDATE receipts SET
            store_name = COALESCE($2, store_name),
            purchase_date = COALESCE($3, purchase_date),
            total = COALESCE($4, total),
            subtotal = COALESCE($5, subtotal),
            ocr_raw = $6,
            ocr_status = 'done',
            updated_at = now()
         WHERE id = $1",
    )
    .bind(receipt_id)
    .bind(&store_name)
    .bind(purchase_date)
    .bind(total)
    .bind(subtotal)
    .bind(&ocr_raw)
    .execute(pool)
    .await?;

    // Insert items
    for item in &result.line_items {
        let description = item
            .desc_clean
            .as_deref()
            .or(item.desc.as_deref())
            .unwrap_or("Unknown item");
        let quantity = item.qty.map(|q| q as f32);
        let unit_price = item
            .price
            .and_then(|p| Decimal::from_str(&format!("{p:.2}")).ok());
        let line_total = item
            .line_total
            .and_then(|t| Decimal::from_str(&format!("{t:.2}")).ok());

        sqlx::query(
            "INSERT INTO items (receipt_id, user_id, description, quantity, unit_price, line_total, product_code)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(receipt_id)
        .bind(user_id)
        .bind(description)
        .bind(quantity)
        .bind(unit_price)
        .bind(line_total)
        .bind(&item.product_code)
        .execute(pool)
        .await?;
    }

    Ok("done".to_string())
}
