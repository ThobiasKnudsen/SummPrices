use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use sqlx::PgPool;

use crate::auth::middleware::AuthUser;
use crate::errors::AppError;

use super::models::*;

const FILTERS: &str = "AND ($2::text IS NULL OR t.description_clean ILIKE '%' || $2 || '%' OR t.description_raw ILIKE '%' || $2 || '%')
           AND ($3::text IS NULL OR r.store_name_raw ILIKE '%' || $3 || '%')
           AND ($4::date IS NULL OR r.purchase_at::date >= $4)
           AND ($5::date IS NULL OR r.purchase_at::date <= $5)";

pub async fn list(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Query(params): Query<TransactionListQuery>,
) -> Result<Json<TransactionListResponse>, AppError> {
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(50).clamp(1, 200);
    let offset = (page - 1) * per_page;

    let transactions = sqlx::query_as::<_, TransactionWithContext>(&format!(
        "SELECT t.id, t.receipt_id, t.description_raw, t.description_clean, t.item_type,
                t.quantity, t.unit, t.unit_price, t.line_total, t.mva_rate,
                r.store_name_raw, r.currency, r.purchase_at
         FROM transactions t
         JOIN receipts r ON r.id = t.receipt_id
         WHERE t.user_id = $1 {FILTERS}
         ORDER BY r.purchase_at DESC NULLS LAST, t.id DESC
         LIMIT $6 OFFSET $7"
    ))
    .bind(auth.user_id)
    .bind(&params.q)
    .bind(&params.store)
    .bind(params.from)
    .bind(params.to)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&pool)
    .await?;

    let total_count: (i64,) = sqlx::query_as(&format!(
        "SELECT COUNT(*) FROM transactions t JOIN receipts r ON r.id = t.receipt_id
         WHERE t.user_id = $1 {FILTERS}"
    ))
    .bind(auth.user_id)
    .bind(&params.q)
    .bind(&params.store)
    .bind(params.from)
    .bind(params.to)
    .fetch_one(&pool)
    .await?;

    Ok(Json(TransactionListResponse {
        transactions,
        total_count: total_count.0,
    }))
}

pub async fn update(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateTransactionRequest>,
) -> Result<StatusCode, AppError> {
    let res = sqlx::query(
        "UPDATE transactions SET
            description_clean = COALESCE($3, description_clean),
            quantity = COALESCE($4, quantity),
            unit_price = COALESCE($5, unit_price),
            line_total = COALESCE($6, line_total)
         WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(auth.user_id)
    .bind(&req.description_clean)
    .bind(req.quantity)
    .bind(req.unit_price)
    .bind(req.line_total)
    .execute(&pool)
    .await?;

    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let res = sqlx::query("DELETE FROM transactions WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(auth.user_id)
        .execute(&pool)
        .await?;

    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}
