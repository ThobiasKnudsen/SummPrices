//! Developer folder-ingest harness.
//!
//! Drop receipt files (jpg/jpeg/png/webp/pdf) into DEV_RECEIPTS_DIR (default
//! `backend/dev_receipts/`) and run:
//!
//!     cargo test --test ingest -- --ignored
//!
//! It parses only files that are new, or whose stored parser commit differs from the
//! current build (code changed -> re-parse), and skips the rest. Requires local Postgres
//! + MinIO. Uses the mock extractor unless EXTRACTOR is set.
use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};
use uuid::Uuid;

use kvitteringsapp_backend::config::Config;
use kvitteringsapp_backend::enums::ReceiptSource;
use kvitteringsapp_backend::storage::s3::Storage;
use kvitteringsapp_backend::{db, extraction, receipts::pipeline, GIT_COMMIT};

#[tokio::test]
#[ignore = "requires local Postgres + MinIO; run with: cargo test --test ingest -- --ignored"]
async fn ingest_dev_receipts() {
    dotenvy::dotenv().ok();
    // Default to the mock extractor so this runs with no GPU/model.
    if std::env::var("EXTRACTOR").is_err() {
        // SAFETY: set before any other threads read the environment.
        unsafe {
            std::env::set_var("EXTRACTOR", "mock");
        }
    }
    let config = Config::from_env();

    let pool = db::create_pool(&config.database_url).await;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("run migrations");
    Storage::ensure_bucket(&config)
        .await
        .expect("ensure bucket");
    let storage = Storage::new(&config);
    let extractor = extraction::build_from_env(&config);

    pipeline::ensure_dev_user(&pool).await.expect("dev user");

    let dir = &config.dev_receipts_dir;
    let (mut new, mut reparsed, mut skipped) = (0u32, 0u32, 0u32);

    if Path::new(dir).exists() {
        for entry in fs::read_dir(dir).expect("read dev_receipts dir") {
            let path = entry.expect("dir entry").path();
            if !path.is_file() {
                continue;
            }
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            let (mime, source) = match ext.as_str() {
                "jpg" | "jpeg" => ("image/jpeg", ReceiptSource::ImageUpload),
                "png" => ("image/png", ReceiptSource::ImageUpload),
                "webp" => ("image/webp", ReceiptSource::ImageUpload),
                "pdf" => ("application/pdf", ReceiptSource::PdfUpload),
                _ => continue,
            };

            let bytes = fs::read(&path).expect("read file");
            let hash = hex::encode(Sha256::digest(&bytes));

            let existing: Option<(Uuid, Option<String>)> = sqlx::query_as(
                "SELECT id, parser_commit FROM receipts WHERE user_id = $1 AND source_file_hash = $2",
            )
            .bind(pipeline::DEV_USER_ID)
            .bind(&hash)
            .fetch_optional(&pool)
            .await
            .expect("lookup by file hash");

            match existing {
                None => {
                    let receipt_id = pipeline::insert_pending_receipt(
                        &pool,
                        pipeline::DEV_USER_ID,
                        source,
                        Some(mime),
                        Some(&hash),
                        GIT_COMMIT,
                    )
                    .await
                    .expect("insert receipt");
                    let key = format!("{}/{}.{}", pipeline::DEV_USER_ID, receipt_id, ext);
                    storage.upload(&key, &bytes, mime).await.expect("upload");
                    pipeline::set_asset_key(&pool, receipt_id, &key)
                        .await
                        .expect("set asset key");
                    pipeline::run_extraction(
                        &pool,
                        &extractor,
                        receipt_id,
                        pipeline::DEV_USER_ID,
                        &bytes,
                        mime,
                    )
                    .await;
                    new += 1;
                }
                Some((_, Some(ref commit))) if commit == GIT_COMMIT => {
                    skipped += 1;
                }
                Some((id, _)) => {
                    // Parser code changed since last parse -> re-parse (reuses stored asset).
                    pipeline::run_extraction(
                        &pool,
                        &extractor,
                        id,
                        pipeline::DEV_USER_ID,
                        &bytes,
                        mime,
                    )
                    .await;
                    sqlx::query("UPDATE receipts SET parser_commit = $2 WHERE id = $1")
                        .bind(id)
                        .bind(GIT_COMMIT)
                        .execute(&pool)
                        .await
                        .expect("update parser_commit");
                    reparsed += 1;
                }
            }
        }
    }

    println!("ingest: {new} new, {reparsed} reparsed, {skipped} skipped (commit {GIT_COMMIT})");

    // Show what the current parser produced for every dev-user receipt so the run is
    // legible without a DB client.
    let rows: Vec<(
        Option<String>,
        String,
        Option<String>,
        String,
        bool,
        Option<String>,
        i64,
    )> = sqlx::query_as(
        "SELECT r.store_name_raw, r.currency, r.total::text, r.extraction_status::text,
                r.needs_review, r.review_reason,
                (SELECT count(*) FROM transactions t WHERE t.receipt_id = r.id)
         FROM receipts r WHERE r.user_id = $1
         ORDER BY r.created_at",
    )
    .bind(pipeline::DEV_USER_ID)
    .fetch_all(&pool)
    .await
    .expect("summary query");

    println!("\n--- dev-user receipts after ingest ---");
    for (store, currency, total, status, needs_review, reason, items) in rows {
        let store: String = store.unwrap_or_else(|| "—".into()).chars().take(26).collect();
        let total = total.unwrap_or_else(|| "—".into());
        let flag = if needs_review { " ⚠ needs_review" } else { "" };
        println!("  {store:<26} {items:>3} items  {total:>9} {currency}  [{status}]{flag}");
        if let Some(reason) = reason {
            println!("       ↳ {reason}");
        }
    }
}
