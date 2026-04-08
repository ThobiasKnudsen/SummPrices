use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::middleware::AuthUser;
use crate::errors::AppError;
use crate::receipts::models::Item;

use super::models::*;

pub async fn list(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Query(params): Query<ItemListQuery>,
) -> Result<Json<ItemListResponse>, AppError> {
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(50).min(100);
    let offset = (page - 1) * per_page;

    let items = sqlx::query_as::<_, ItemWithContext>(
        "SELECT i.id, i.receipt_id, i.description, i.quantity, i.unit_price,
                i.line_total, i.product_code, r.store_name, r.purchase_date
         FROM items i
         JOIN receipts r ON r.id = i.receipt_id
         WHERE i.user_id = $1
           AND ($2::text IS NULL OR i.description ILIKE '%' || $2 || '%')
           AND ($3::date IS NULL OR r.purchase_date >= $3)
           AND ($4::date IS NULL OR r.purchase_date <= $4)
           AND ($5::text IS NULL OR r.store_name ILIKE '%' || $5 || '%')
         ORDER BY r.purchase_date DESC NULLS LAST, i.created_at
         LIMIT $6 OFFSET $7",
    )
    .bind(auth.user_id)
    .bind(&params.q)
    .bind(params.from)
    .bind(params.to)
    .bind(&params.store)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&pool)
    .await?;

    let total_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)
         FROM items i
         JOIN receipts r ON r.id = i.receipt_id
         WHERE i.user_id = $1
           AND ($2::text IS NULL OR i.description ILIKE '%' || $2 || '%')
           AND ($3::date IS NULL OR r.purchase_date >= $3)
           AND ($4::date IS NULL OR r.purchase_date <= $4)
           AND ($5::text IS NULL OR r.store_name ILIKE '%' || $5 || '%')",
    )
    .bind(auth.user_id)
    .bind(&params.q)
    .bind(params.from)
    .bind(params.to)
    .bind(&params.store)
    .fetch_one(&pool)
    .await?;

    Ok(Json(ItemListResponse {
        items,
        total_count: total_count.0,
    }))
}

pub async fn update(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateItemRequest>,
) -> Result<Json<Item>, AppError> {
    let item = sqlx::query_as::<_, Item>(
        "UPDATE items SET
            description = COALESCE($3, description),
            quantity = COALESCE($4, quantity),
            unit_price = COALESCE($5, unit_price),
            line_total = COALESCE($6, line_total)
         WHERE id = $1 AND user_id = $2
         RETURNING id, receipt_id, description, quantity, unit_price, line_total, product_code",
    )
    .bind(id)
    .bind(auth.user_id)
    .bind(&req.description)
    .bind(req.quantity)
    .bind(req.unit_price)
    .bind(req.line_total)
    .fetch_optional(&pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(item))
}

pub async fn delete(
    auth: AuthUser,
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let result = sqlx::query("DELETE FROM items WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(auth.user_id)
        .execute(&pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}
