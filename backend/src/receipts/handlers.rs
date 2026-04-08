use std::sync::Arc;

use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::middleware::AuthUser;
use crate::errors::AppError;
use crate::storage::s3::Storage;

use super::models::*;
use super::ocr::{self, TabscannerClient};

pub async fn upload(
    auth: AuthUser,
    State(pool): State<PgPool>,
    State(storage): State<Storage>,
    State(ocr_client): State<Arc<TabscannerClient>>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<Receipt>), AppError> {
    let mut image_data: Option<Vec<u8>> = None;
    let mut content_type = "image/jpeg".to_string();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid multipart: {e}")))?
    {
        if field.name() == Some("image") {
            if let Some(ct) = field.content_type() {
                content_type = ct.to_string();
            }
            image_data = Some(
                field
                    .bytes()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("Failed to read image: {e}")))?
                    .to_vec(),
            );
        }
    }

    let image_data = image_data.ok_or_else(|| AppError::BadRequest("No image provided".into()))?;
    let image_key = format!("{}/{}.jpg", auth.user_id, Uuid::new_v4());

    // Upload to S3
    storage
        .upload(&image_key, &image_data, &content_type)
        .await?;

    // Insert receipt row
    let receipt = sqlx::query_as::<_, Receipt>(
        "INSERT INTO receipts (user_id, image_key, ocr_status)
         VALUES ($1, $2, 'pending')
         RETURNING id, user_id, store_name, purchase_date, purchase_time,
                   total, subtotal, currency, image_key, ocr_confidence,
                   ocr_status, created_at, updated_at",
    )
    .bind(auth.user_id)
    .bind(&image_key)
    .fetch_one(&pool)
    .await?;

    // Submit to Tabscanner in background (don't block the response)
    let pool_clone = pool.clone();
    let receipt_id = receipt.id;
    let ocr = ocr_client.clone();
    let ct = content_type.clone();
    tokio::spawn(async move {
        if let Err(e) = ocr::submit_for_ocr(&pool_clone, receipt_id, &image_data, &ct, &ocr).await
        {
            tracing::error!("OCR submission failed for receipt {receipt_id}: {e}");
            let _ = sqlx::query("UPDATE receipts SET ocr_status = 'failed' WHERE id = $1")
                .bind(receipt_id)
                .execute(&pool_clone)
                .await;
        }
    });

    Ok((StatusCode::CREATED, Json(receipt)))
}

pub async fn list(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Query(params): Query<ReceiptListQuery>,
) -> Result<Json<ReceiptListResponse>, AppError> {
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let receipts = sqlx::query_as::<_, ReceiptSummary>(
        "SELECT id, store_name, purchase_date, total, currency, ocr_status, created_at
         FROM receipts
         WHERE user_id = $1
           AND ($2::text IS NULL OR store_name ILIKE '%' || $2 || '%')
           AND ($3::date IS NULL OR purchase_date >= $3)
           AND ($4::date IS NULL OR purchase_date <= $4)
         ORDER BY created_at DESC
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
           AND ($2::text IS NULL OR store_name ILIKE '%' || $2 || '%')
           AND ($3::date IS NULL OR purchase_date >= $3)
           AND ($4::date IS NULL OR purchase_date <= $4)",
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
) -> Result<Json<ReceiptWithItems>, AppError> {
    let receipt = sqlx::query_as::<_, Receipt>(
        "SELECT id, user_id, store_name, purchase_date, purchase_time,
                total, subtotal, currency, image_key, ocr_confidence,
                ocr_status, created_at, updated_at
         FROM receipts WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(auth.user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let items = sqlx::query_as::<_, Item>(
        "SELECT id, receipt_id, description, quantity, unit_price, line_total, product_code
         FROM items WHERE receipt_id = $1
         ORDER BY created_at",
    )
    .bind(id)
    .fetch_all(&pool)
    .await?;

    let image_url = storage
        .get_presigned_url(&receipt.image_key, 3600)
        .await
        .ok();

    Ok(Json(ReceiptWithItems {
        receipt,
        items,
        image_url,
    }))
}

pub async fn update(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateReceiptRequest>,
) -> Result<Json<Receipt>, AppError> {
    let receipt = sqlx::query_as::<_, Receipt>(
        "UPDATE receipts SET
            store_name = COALESCE($3, store_name),
            purchase_date = COALESCE($4, purchase_date),
            total = COALESCE($5, total),
            updated_at = now()
         WHERE id = $1 AND user_id = $2
         RETURNING id, user_id, store_name, purchase_date, purchase_time,
                   total, subtotal, currency, image_key, ocr_confidence,
                   ocr_status, created_at, updated_at",
    )
    .bind(id)
    .bind(auth.user_id)
    .bind(&req.store_name)
    .bind(req.purchase_date)
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
    let row: Option<(String,)> = sqlx::query_as(
        "DELETE FROM receipts WHERE id = $1 AND user_id = $2 RETURNING image_key",
    )
    .bind(id)
    .bind(auth.user_id)
    .fetch_optional(&pool)
    .await?;

    match row {
        Some((key,)) => {
            let _ = storage.delete(&key).await;
            Ok(StatusCode::NO_CONTENT)
        }
        None => Err(AppError::NotFound),
    }
}

pub async fn ocr_status(
    auth: AuthUser,
    State(pool): State<PgPool>,
    State(ocr_client): State<Arc<TabscannerClient>>,
    Path(id): Path<Uuid>,
) -> Result<Json<OcrStatusResponse>, AppError> {
    let row = sqlx::query_as::<_, (String, Option<f32>, Option<String>)>(
        "SELECT ocr_status, ocr_confidence, ocr_token FROM receipts WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(auth.user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let (mut status, confidence, ocr_token) = row;

    // If still processing and we have a token, lazy-poll Tabscanner
    if status == "processing" {
        if let Some(token) = &ocr_token {
            match ocr::poll_and_store(&pool, id, auth.user_id, token, &ocr_client).await {
                Ok(new_status) => status = new_status,
                Err(e) => tracing::warn!("OCR poll failed for receipt {id}: {e}"),
            }
        }
    }

    // Re-read confidence if status changed to done
    let confidence = if status == "done" && confidence.is_none() {
        sqlx::query_scalar::<_, Option<f32>>(
            "SELECT ocr_confidence FROM receipts WHERE id = $1",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap_or(None)
    } else {
        confidence
    };

    Ok(Json(OcrStatusResponse {
        ocr_status: status,
        ocr_confidence: confidence,
    }))
}
