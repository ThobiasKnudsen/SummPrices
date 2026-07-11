//! Hosted vision-LLM extractor over any OpenAI-compatible `/chat/completions`
//! endpoint: OpenRouter or Mistral (dev/prod), or a self-hosted vLLM/Ollama.
//! Set VLM_URL, VLM_MODEL, and VLM_API_KEY (bearer). No key → keyless (local).
use base64::Engine;
use serde::Deserialize;

use crate::enums::{ItemType, PriceType};
use crate::errors::AppError;

use super::{dec2, parse_purchase_at, ExtractedLineItem, ExtractedReceipt, ReceiptExtractor};

pub struct HostedVlmExtractor {
    client: reqwest::Client,
    url: String,
    model: String,
    api_key: Option<String>,
    engine: String,
}

impl HostedVlmExtractor {
    pub fn new(url: &str, model: &str, api_key: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            url: url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            api_key,
            engine: model.to_string(),
        }
    }
}

const PROMPT: &str = r#"You are a receipt-parsing engine. Extract the receipt in the image into JSON with EXACTLY this shape (no extra keys, no prose, no markdown):
{
  "store": { "name": string|null, "org_no": string|null, "address": string|null, "city": string|null, "postal_code": string|null, "country_code": string|null },
  "purchase_at": string|null,          // the printed local date/time, e.g. "2026-01-15T13:30" or "2026-01-15 13:30"
  "currency": string,                  // the currency ACTUALLY printed: "NOK","SEK","DKK","EUR","CHF",...
  "receipt_number": string|null,       // bong/receipt number if printed
  "payment": { "method": string|null },// "card" | "cash" | "vipps" | ... (NEVER card numbers)
  "subtotal": number|null,
  "total": number|null,
  "mva_lines": [ { "rate": number, "base": number, "vat": number } ],  // the VAT/MVA breakdown table
  "line_items": [
    {
      "description": string,
      "product_code": string|null,     // EAN/barcode if printed
      "quantity": number|null,
      "unit": string|null,             // "stk","kg","l"
      "shelf_unit_price": number|null, // price before discount, if shown
      "unit_price": number|null,       // net price actually paid per unit
      "discount_amount": number|null,
      "line_total": number|null,       // signed amount for the line (see rules)
      "item_type": "product"|"deposit"|"discount"|"fee"|"rounding"|"correction"|"unknown",
      "price_type": "shelf"|"promo"|"member"|"coupon"|"net_only",
      "mva_rate": number|null          // e.g. 25, 15, 12
    }
  ],
  "confidence": number,                // 0..1: how sure you are the extraction is COMPLETE and CORRECT
  "notes": string|null                 // brief note on any problem (blurry, cropped, unreadable lines) or null
}
Rules:
- Extract EVERY printed line item. Do not skip, merge, or summarize lines.
- ROW ALIGNMENT: each item's amount is printed on the SAME horizontal row as its description.
  Read the receipt row by row and pair each description with the number on its OWN line — never
  borrow the price from the row above or below it. Amounts usually line up in a right-hand column.
- PER-LINE CHECK: when both quantity and unit_price are present, quantity × unit_price must equal
  line_total for that row. If it doesn't, you have grabbed the wrong number — re-read that row.
- Store name: transcribe the header EXACTLY as printed. Never guess or substitute a different
  company; if the name is unreadable, set store.name to null rather than inventing one.
- Numbers: a comma is the decimal separator (49,90 = 49.90). Never invent digits you cannot read.
- Currency: use what is printed (CHF for Swiss receipts, EUR, SEK, DKK, ...). Do NOT default to NOK.
- Store: fill address / city / postal_code / country_code from the header when present.
- Signs: products, deposits and fees are POSITIVE line_total; discounts, corrections and rounding are NEGATIVE line_total.
  "pant" = deposit; "Rabatt"/"Trumf"/"Mixrabatt" = discount; "øreavrunding"/"avrunding"/rounding = rounding.
- Corrections/voids: lines like "KORR", "KORR. TID.", "KORR. SIST", "Korreksjon", "Retur", "Storno", "Void",
  "Annuller" REVERSE a previously scanned item. Emit BOTH the original item line AND a separate line with
  item_type "correction" and a negative line_total. Never delete the original and never move its price onto a
  different item — the two lines must cancel out.
- Self-check the arithmetic before answering: the sum of ALL line_total values must equal "total"
  (allowing only for a rounding line). If it does not, re-read the receipt and lower "confidence".
- If the image is blurry, cropped, or partly unreadable, extract what you can, set a LOW confidence, and
  describe the problem in "notes".
Output only the JSON object."#;

#[derive(Deserialize, Default)]
struct VlmStore {
    name: Option<String>,
    org_no: Option<String>,
    address: Option<String>,
    city: Option<String>,
    postal_code: Option<String>,
    country_code: Option<String>,
}

#[derive(Deserialize, Default)]
struct VlmPayment {
    method: Option<String>,
}

#[derive(Deserialize)]
struct VlmMva {
    #[allow(dead_code)]
    rate: Option<f64>,
    #[allow(dead_code)]
    base: Option<f64>,
    vat: Option<f64>,
}

#[derive(Deserialize)]
struct VlmOut {
    #[serde(default)]
    store: VlmStore,
    purchase_at: Option<String>,
    currency: Option<String>,
    receipt_number: Option<String>,
    #[serde(default)]
    payment: VlmPayment,
    subtotal: Option<f64>,
    total: Option<f64>,
    #[serde(default)]
    mva_lines: Vec<VlmMva>,
    #[serde(default)]
    line_items: Vec<VlmItem>,
    confidence: Option<f64>,
    notes: Option<String>,
}

#[derive(Deserialize)]
struct VlmItem {
    description: Option<String>,
    product_code: Option<String>,
    quantity: Option<f64>,
    unit: Option<String>,
    shelf_unit_price: Option<f64>,
    unit_price: Option<f64>,
    discount_amount: Option<f64>,
    line_total: Option<f64>,
    item_type: Option<String>,
    price_type: Option<String>,
    mva_rate: Option<f64>,
}

fn item_type_of(s: &Option<String>) -> ItemType {
    match s.as_deref() {
        Some("deposit") => ItemType::Deposit,
        Some("discount") => ItemType::Discount,
        Some("fee") => ItemType::Fee,
        Some("rounding") => ItemType::Rounding,
        Some("correction") => ItemType::Correction,
        Some("unknown") => ItemType::Unknown,
        _ => ItemType::Product,
    }
}

fn price_type_of(s: &Option<String>) -> PriceType {
    match s.as_deref() {
        Some("shelf") => PriceType::Shelf,
        Some("promo") => PriceType::Promo,
        Some("member") => PriceType::Member,
        Some("coupon") => PriceType::Coupon,
        _ => PriceType::NetOnly,
    }
}

/// Best-effort currency for a country code, so a receipt whose currency the model
/// left blank isn't silently mislabeled NOK (the app supports NOK/SEK/DKK/EUR/CHF/…).
fn currency_for_country(cc: &str) -> Option<&'static str> {
    match cc.trim().to_uppercase().as_str() {
        "NO" => Some("NOK"),
        "SE" => Some("SEK"),
        "DK" => Some("DKK"),
        "CH" | "LI" => Some("CHF"),
        "GB" | "UK" => Some("GBP"),
        "US" => Some("USD"),
        "IS" => Some("ISK"),
        "PL" => Some("PLN"),
        "CZ" => Some("CZK"),
        // Eurozone
        "DE" | "FR" | "IT" | "ES" | "NL" | "BE" | "AT" | "FI" | "IE" | "PT" | "GR" | "LU" | "SK"
        | "SI" | "EE" | "LV" | "LT" | "MT" | "CY" | "HR" => Some("EUR"),
        _ => None,
    }
}

/// Extract the outermost JSON object from a model response that may wrap it in
/// markdown fences (```json … ```) or prose.
fn extract_json_object(s: &str) -> &str {
    match (s.find('{'), s.rfind('}')) {
        (Some(a), Some(b)) if b >= a => &s[a..=b],
        _ => s.trim(),
    }
}

fn empty_pdf_result(engine: &str) -> ExtractedReceipt {
    ExtractedReceipt {
        store_name_raw: None,
        org_no: None,
        store_address: None,
        store_city: None,
        store_postal_code: None,
        store_country_code: None,
        receipt_number: None,
        payment_method: None,
        purchase_at: None,
        currency: "NOK".to_string(),
        subtotal: None,
        mva_total: None,
        total: None,
        line_items: vec![],
        confidence: Some(0.0),
        notes: Some("PDF extraction not yet supported by the hosted VLM.".to_string()),
        engine: engine.to_string(),
        raw: serde_json::json!({ "note": "pdf extraction not yet supported by hosted VLM" }),
    }
}

#[async_trait::async_trait]
impl ReceiptExtractor for HostedVlmExtractor {
    async fn extract(&self, bytes: &[u8], mime: &str) -> Result<ExtractedReceipt, AppError> {
        // PDFs can't be sent to a vision model directly; text-layer parsing is deferred.
        if mime == "application/pdf" {
            return Ok(empty_pdf_result(&self.engine));
        }

        let data_uri = format!(
            "data:{};base64,{}",
            mime,
            base64::engine::general_purpose::STANDARD.encode(bytes)
        );
        let body = serde_json::json!({
            "model": self.model,
            "temperature": 0,
            "response_format": { "type": "json_object" },
            "messages": [
                { "role": "system", "content": "You output only valid JSON." },
                { "role": "user", "content": [
                    { "type": "text", "text": PROMPT },
                    { "type": "image_url", "image_url": { "url": data_uri } }
                ]}
            ]
        });

        // Send with a few retries on transient rate-limits (429) / 5xx.
        let endpoint = format!("{}/chat/completions", self.url);
        let mut resp = None;
        let mut last_err = String::new();
        for attempt in 1..=3u32 {
            let mut req = self.client.post(&endpoint).json(&body);
            if let Some(key) = &self.api_key {
                // Bearer auth for OpenRouter/Mistral; referer/title are OpenRouter
                // attribution headers, ignored elsewhere.
                req = req
                    .bearer_auth(key)
                    .header("HTTP-Referer", "https://sumprices.app")
                    .header("X-Title", "SumPrices");
            }
            match req.send().await {
                Ok(r) if r.status().is_success() => {
                    resp = Some(r);
                    break;
                }
                Ok(r) => {
                    let status = r.status();
                    let retryable = status.as_u16() == 429 || status.is_server_error();
                    let text = r.text().await.unwrap_or_default();
                    last_err = format!("VLM returned {status}: {text}");
                    if retryable && attempt < 3 {
                        tokio::time::sleep(std::time::Duration::from_millis(800 * attempt as u64))
                            .await;
                        continue;
                    }
                    return Err(AppError::Internal(last_err));
                }
                Err(e) => {
                    last_err = format!("VLM request failed: {e}");
                    if attempt < 3 {
                        tokio::time::sleep(std::time::Duration::from_millis(800 * attempt as u64))
                            .await;
                        continue;
                    }
                    return Err(AppError::Internal(last_err));
                }
            }
        }
        let resp = resp.ok_or_else(|| AppError::Internal(last_err))?;

        let val: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("VLM parse failed: {e}")))?;
        let content = val["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| AppError::Internal("VLM response missing content".to_string()))?;
        // Tolerate markdown fences / prose around the JSON object.
        let json_str = extract_json_object(content);
        let out: VlmOut = serde_json::from_str(json_str)
            .map_err(|e| AppError::Internal(format!("VLM JSON invalid: {e}")))?;
        // Store the model's receipt JSON (store address, mva_lines, payment, …) for audit/reprocess.
        let raw: serde_json::Value = serde_json::from_str(json_str).unwrap_or(serde_json::Value::Null);

        let mva_total = if out.mva_lines.is_empty() {
            None
        } else {
            let sum: f64 = out.mva_lines.iter().filter_map(|m| m.vat).sum();
            dec2(Some(sum))
        };

        let line_items = out
            .line_items
            .into_iter()
            .filter_map(|it| {
                let desc = it.description?;
                Some(ExtractedLineItem {
                    description_clean: Some(desc.clone()),
                    description_raw: desc,
                    product_code: it.product_code,
                    item_type: item_type_of(&it.item_type),
                    quantity: super::dec3(it.quantity),
                    unit: it.unit,
                    shelf_unit_price: dec2(it.shelf_unit_price),
                    unit_price: dec2(it.unit_price),
                    discount_amount: dec2(it.discount_amount),
                    line_total: dec2(it.line_total),
                    price_type: price_type_of(&it.price_type),
                    mva_rate: dec2(it.mva_rate),
                })
            })
            .collect();

        // Prefer the printed currency; if the model left it blank, infer from the
        // store country before falling back to NOK (avoids mislabeling CHF/EUR as NOK).
        let currency = out
            .currency
            .map(|c| c.trim().to_uppercase())
            .filter(|c| !c.is_empty())
            .or_else(|| {
                out.store
                    .country_code
                    .as_deref()
                    .and_then(currency_for_country)
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "NOK".to_string());

        Ok(ExtractedReceipt {
            store_name_raw: out.store.name,
            org_no: out.store.org_no,
            store_address: out.store.address,
            store_city: out.store.city,
            store_postal_code: out.store.postal_code,
            store_country_code: out.store.country_code.map(|c| c.trim().to_uppercase()),
            receipt_number: out.receipt_number,
            payment_method: out.payment.method,
            purchase_at: parse_purchase_at(out.purchase_at.as_deref()),
            currency,
            subtotal: dec2(out.subtotal),
            mva_total,
            total: dec2(out.total),
            line_items,
            confidence: out.confidence.map(|c| c.clamp(0.0, 1.0) as f32),
            notes: out.notes.filter(|n| !n.trim().is_empty()),
            engine: self.engine.clone(),
            raw,
        })
    }

    fn engine_id(&self) -> &str {
        &self.engine
    }
}
