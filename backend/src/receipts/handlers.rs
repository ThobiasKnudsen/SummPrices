use std::sync::Arc;

use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::middleware::AuthUser;
use crate::config::Config;
use crate::enums::ReceiptSource;
use crate::errors::AppError;
use crate::extraction::{build_hosted_model, ReceiptExtractor};
use crate::storage::s3::Storage;
use crate::GIT_COMMIT;

use super::models::*;
use super::pipeline;

const RECEIPT_COLS: &str = "id, user_id, store_name_raw, store_address, store_city, store_postal_code, \
     store_country_code, purchase_at, subtotal, mva_total, total, currency, extraction_status, \
     extraction_conf, needs_review, review_reason, created_at, updated_at, original_asset_key";

async fn fetch_receipt(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<Receipt, AppError> {
    sqlx::query_as::<_, Receipt>(&format!(
        "SELECT {RECEIPT_COLS} FROM receipts WHERE id = $1 AND user_id = $2"
    ))
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)
}

pub async fn upload(
    auth: AuthUser,
    State(pool): State<PgPool>,
    State(storage): State<Storage>,
    State(extractor): State<Arc<dyn ReceiptExtractor>>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<Receipt>), AppError> {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut content_type = "application/octet-stream".to_string();
    let mut source = ReceiptSource::ImageUpload;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid multipart: {e}")))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "image" || name == "pdf" {
            if let Some(ct) = field.content_type() {
                content_type = ct.to_string();
            }
            source = if name == "pdf" || content_type == "application/pdf" {
                ReceiptSource::PdfUpload
            } else {
                ReceiptSource::ImageUpload
            };
            file_bytes = Some(
                field
                    .bytes()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("Failed to read file: {e}")))?
                    .to_vec(),
            );
        }
    }

    let bytes =
        file_bytes.ok_or_else(|| AppError::BadRequest("No file provided (field 'image' or 'pdf')".into()))?;
    let file_hash = hex::encode(Sha256::digest(&bytes));

    let receipt_id = pipeline::insert_pending_receipt(
        &pool,
        auth.user_id,
        source,
        Some(&content_type),
        Some(&file_hash),
        GIT_COMMIT,
    )
    .await?;

    let ext = if source == ReceiptSource::PdfUpload {
        "pdf"
    } else {
        "jpg"
    };
    let key = format!("{}/{}.{}", auth.user_id, receipt_id, ext);
    storage.upload(&key, &bytes, &content_type).await?;
    pipeline::set_asset_key(&pool, receipt_id, &key).await?;

    // Extract off the request path.
    let pool2 = pool.clone();
    let user_id = auth.user_id;
    let mime2 = content_type.clone();
    tokio::spawn(async move {
        pipeline::run_extraction(&pool2, &extractor, receipt_id, user_id, &bytes, &mime2).await;
    });

    let receipt = fetch_receipt(&pool, receipt_id, auth.user_id).await?;
    Ok((StatusCode::CREATED, Json(receipt)))
}

pub async fn list(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Query(params): Query<ReceiptListQuery>,
) -> Result<Json<ReceiptListResponse>, AppError> {
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * per_page;

    let receipts = sqlx::query_as::<_, ReceiptSummary>(
        "SELECT id, store_name_raw, purchase_at, total, currency, extraction_status,
                extraction_conf, needs_review, created_at
         FROM receipts
         WHERE user_id = $1
           AND ($2::text IS NULL OR store_name_raw ILIKE '%' || $2 || '%')
           AND ($3::date IS NULL OR purchase_at::date >= $3)
           AND ($4::date IS NULL OR purchase_at::date <= $4)
         ORDER BY COALESCE(purchase_at, created_at) DESC
         LIMIT $5 OFFSET $6",
    )
    .bind(auth.user_id)
    .bind(&params.store)
    .bind(params.from)
    .bind(params.to)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&pool)
    .await?;

    let total_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM receipts
         WHERE user_id = $1
           AND ($2::text IS NULL OR store_name_raw ILIKE '%' || $2 || '%')
           AND ($3::date IS NULL OR purchase_at::date >= $3)
           AND ($4::date IS NULL OR purchase_at::date <= $4)",
    )
    .bind(auth.user_id)
    .bind(&params.store)
    .bind(params.from)
    .bind(params.to)
    .fetch_one(&pool)
    .await?;

    Ok(Json(ReceiptListResponse {
        receipts,
        total_count: total_count.0,
    }))
}

pub async fn get_one(
    auth: AuthUser,
    State(pool): State<PgPool>,
    State(storage): State<Storage>,
    Path(id): Path<Uuid>,
) -> Result<Json<ReceiptWithTransactions>, AppError> {
    let receipt = fetch_receipt(&pool, id, auth.user_id).await?;

    let transactions = sqlx::query_as::<_, Transaction>(
        "SELECT id, receipt_id, description_raw, description_clean, item_type,
                quantity, unit, unit_price, line_total, mva_rate
         FROM transactions WHERE receipt_id = $1 ORDER BY line_no NULLS LAST, id",
    )
    .bind(id)
    .fetch_all(&pool)
    .await?;

    let image_url = match &receipt.original_asset_key {
        Some(key) => storage.get_presigned_url(key, 3600).await.ok(),
        None => None,
    };

    Ok(Json(ReceiptWithTransactions {
        receipt,
        transactions,
        image_url,
    }))
}

pub async fn update(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateReceiptRequest>,
) -> Result<Json<Receipt>, AppError> {
    let receipt = sqlx::query_as::<_, Receipt>(&format!(
        "UPDATE receipts SET
            store_name_raw = COALESCE($3, store_name_raw),
            purchase_at = COALESCE($4, purchase_at),
            total = COALESCE($5, total),
            updated_at = now()
         WHERE id = $1 AND user_id = $2
         RETURNING {RECEIPT_COLS}"
    ))
    .bind(id)
    .bind(auth.user_id)
    .bind(&req.store_name)
    .bind(req.purchase_at)
    .bind(req.total)
    .fetch_optional(&pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(receipt))
}

pub async fn delete(
    auth: AuthUser,
    State(pool): State<PgPool>,
    State(storage): State<Storage>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let row: Option<(Option<String>,)> = sqlx::query_as(
        "DELETE FROM receipts WHERE id = $1 AND user_id = $2 RETURNING original_asset_key",
    )
    .bind(id)
    .bind(auth.user_id)
    .fetch_optional(&pool)
    .await?;

    match row {
        Some((Some(key),)) => {
            let _ = storage.delete(&key).await;
            Ok(StatusCode::NO_CONTENT)
        }
        Some((None,)) => Ok(StatusCode::NO_CONTENT),
        None => Err(AppError::NotFound),
    }
}

/// Choose the extractor for a (re)scan: an ad-hoc hosted extractor for the requested model
/// (debug model picker), or the process-wide default when no model is given.
fn pick_extractor(
    default: &Arc<dyn ReceiptExtractor>,
    config: &Config,
    model: Option<&str>,
) -> Arc<dyn ReceiptExtractor> {
    match model.map(str::trim).filter(|m| !m.is_empty()) {
        Some(model) => build_hosted_model(config, model),
        None => default.clone(),
    }
}

/// Re-run extraction on an already-uploaded receipt (e.g. after switching VLM models).
/// Atomically claims the receipt (skips if a scan is already in flight, unless it's been
/// stuck > 2 min) so concurrent/looping rescans can't duplicate line items or fan out paid
/// VLM calls. An optional `model` overrides the model for this scan only.
pub async fn reprocess(
    auth: AuthUser,
    State(pool): State<PgPool>,
    State(storage): State<Storage>,
    State(extractor): State<Arc<dyn ReceiptExtractor>>,
    State(config): State<Config>,
    Path(id): Path<Uuid>,
    Json(req): Json<ReprocessRequest>,
) -> Result<StatusCode, AppError> {
    let claimed: Option<(String, Option<String>)> = sqlx::query_as(
        "UPDATE receipts SET extraction_status = 'processing', updated_at = now()
         WHERE id = $1 AND user_id = $2 AND original_asset_key IS NOT NULL
           AND (extraction_status NOT IN ('pending','queued','processing')
                OR updated_at < now() - interval '2 minutes')
         RETURNING original_asset_key, original_mime",
    )
    .bind(id)
    .bind(auth.user_id)
    .fetch_optional(&pool)
    .await?;

    let (key, mime) = match claimed {
        Some((key, mime)) => (key, mime.unwrap_or_else(|| "image/jpeg".to_string())),
        None => {
            // Not owned, no stored image, or a scan is already in flight — accept as a
            // no-op if the receipt exists, otherwise 404.
            let exists: Option<(Uuid,)> =
                sqlx::query_as("SELECT id FROM receipts WHERE id = $1 AND user_id = $2")
                    .bind(id)
                    .bind(auth.user_id)
                    .fetch_optional(&pool)
                    .await?;
            return match exists {
                Some(_) => Ok(StatusCode::ACCEPTED),
                None => Err(AppError::NotFound),
            };
        }
    };

    let bytes = storage.get(&key).await?;
    let ex = pick_extractor(&extractor, &config, req.model.as_deref());

    let pool2 = pool.clone();
    let user_id = auth.user_id;
    tokio::spawn(async move {
        pipeline::run_extraction(&pool2, &ex, id, user_id, &bytes, &mime).await;
    });

    Ok(StatusCode::ACCEPTED)
}

/// Debug: re-extract every one of the caller's receipts that has a stored image and isn't
/// already in flight, using an optional model override. Runs sequentially in the background
/// to avoid hammering the VLM endpoint.
pub async fn reprocess_all(
    auth: AuthUser,
    State(pool): State<PgPool>,
    State(storage): State<Storage>,
    State(extractor): State<Arc<dyn ReceiptExtractor>>,
    State(config): State<Config>,
    Json(req): Json<ReprocessRequest>,
) -> Result<Json<ReprocessAllResponse>, AppError> {
    let claimed: Vec<(Uuid, String, Option<String>)> = sqlx::query_as(
        "UPDATE receipts SET extraction_status = 'processing', updated_at = now()
         WHERE user_id = $1 AND original_asset_key IS NOT NULL
           AND (extraction_status NOT IN ('pending','queued','processing')
                OR updated_at < now() - interval '2 minutes')
         RETURNING id, original_asset_key, original_mime",
    )
    .bind(auth.user_id)
    .fetch_all(&pool)
    .await?;

    let queued = claimed.len() as i64;
    let ex = pick_extractor(&extractor, &config, req.model.as_deref());
    let pool2 = pool.clone();
    let storage2 = storage.clone();
    let user_id = auth.user_id;

    tokio::spawn(async move {
        for (rid, key, mime) in claimed {
            match storage2.get(&key).await {
                Ok(bytes) => {
                    let mime = mime.unwrap_or_else(|| "image/jpeg".to_string());
                    pipeline::run_extraction(&pool2, &ex, rid, user_id, &bytes, &mime).await;
                }
                Err(e) => tracing::warn!("reprocess_all: storage get failed for {rid}: {e}"),
            }
        }
    });

    Ok(Json(ReprocessAllResponse { queued }))
}

/// Debug: the selectable extraction models for the in-app picker (recommended first).
pub async fn debug_models(
    _auth: AuthUser,
    State(config): State<Config>,
) -> Json<DebugModelsResponse> {
    let mut options = config.vlm_models.clone();
    if !options.iter().any(|m| m == &config.vlm_model) {
        options.push(config.vlm_model.clone());
    }
    Json(DebugModelsResponse {
        current: config.vlm_model.clone(),
        options,
    })
}

pub async fn status(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<ExtractionStatusResponse>, AppError> {
    let row = sqlx::query_as::<_, ExtractionStatusResponse>(
        "SELECT extraction_status, extraction_conf FROM receipts WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(auth.user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(row))
}
