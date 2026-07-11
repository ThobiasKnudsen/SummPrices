//! Compare receipt-extraction accuracy across candidate VLM models.
//!
//! Runs every image in BENCH_DIR (default `reciepts/`) through each model against your
//! OpenAI-compatible endpoint (VLM_URL / VLM_API_KEY) and prints, per receipt, what each
//! model read: currency, item count, whether the items reconcile to the printed total,
//! self-reported confidence, and latency — then a per-model scoreboard. "clean" reuses
//! the exact reconciliation the real pipeline uses (`pipeline::compute_review`).
//!
//! Usage (from backend/):
//!     VLM_URL=https://openrouter.ai/api/v1 VLM_API_KEY=sk-... \
//!     cargo run --bin bench_extractors -- anthropic/claude-opus-4-8 mistralai/pixtral-large-2411 qwen/qwen3-vl-8b
//!
//! Models can also come from VLM_MODELS (comma-separated); the folder from BENCH_DIR.
//! PDFs are skipped (image models only).
use std::time::Instant;

use rust_decimal::Decimal;

use kvitteringsapp_backend::extraction::hosted_vlm::HostedVlmExtractor;
use kvitteringsapp_backend::extraction::ReceiptExtractor;
use kvitteringsapp_backend::receipts::pipeline::compute_review;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let dir = std::env::var("BENCH_DIR").unwrap_or_else(|_| "reciepts".to_string());
    let url = std::env::var("VLM_URL").unwrap_or_else(|_| "http://localhost:11434/v1".to_string());
    let key = std::env::var("VLM_API_KEY").ok().filter(|s| !s.is_empty());

    let mut models: Vec<String> = std::env::args().skip(1).collect();
    if models.is_empty() {
        if let Ok(list) = std::env::var("VLM_MODELS") {
            models = list
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }
    if models.is_empty() {
        models = vec![std::env::var("VLM_MODEL").unwrap_or_else(|_| "qwen3-vl:8b".to_string())];
    }

    let mut files: Vec<std::path::PathBuf> = match std::fs::read_dir(&dir) {
        Ok(rd) => rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                matches!(
                    p.extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase()
                        .as_str(),
                    "jpg" | "jpeg" | "png" | "webp"
                )
            })
            .collect(),
        Err(e) => {
            eprintln!("Cannot read {dir}/: {e}");
            std::process::exit(1);
        }
    };
    files.sort();
    if files.is_empty() {
        eprintln!("No image receipts found in {dir}/");
        std::process::exit(1);
    }

    println!(
        "Benchmarking {} model(s) on {} receipt(s) in {dir}/\nendpoint: {url}\n",
        models.len(),
        files.len()
    );

    let mut clean = vec![0u32; models.len()];
    let mut total_ms = vec![0u128; models.len()];
    let mut runs = vec![0u32; models.len()];

    for file in &files {
        let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        let mime = match file
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str()
        {
            "png" => "image/png",
            "webp" => "image/webp",
            _ => "image/jpeg",
        };
        let bytes = match std::fs::read(file) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("skip {name}: {e}");
                continue;
            }
        };

        println!("=== {name} ===");
        println!(
            "  {:<30} {:>4} {:>4} {:>5} {:>9} {:>9} {:>3} {:>6}  review",
            "model", "conf", "cur", "items", "sum", "total", "ok", "ms"
        );

        for (i, model) in models.iter().enumerate() {
            let extractor = HostedVlmExtractor::new(&url, model, key.clone());
            let start = Instant::now();
            let result = extractor.extract(&bytes, mime).await;
            let ms = start.elapsed().as_millis();
            runs[i] += 1;
            total_ms[i] += ms;

            match result {
                Ok(ex) => {
                    let sum: Decimal = ex.line_items.iter().filter_map(|li| li.line_total).sum();
                    let (needs_review, reason) = compute_review(&ex);
                    if !needs_review {
                        clean[i] += 1;
                    }
                    let conf = ex
                        .confidence
                        .map(|c| format!("{:.0}%", c * 100.0))
                        .unwrap_or_else(|| "—".into());
                    let total = ex.total.map(|t| t.to_string()).unwrap_or_else(|| "—".into());
                    println!(
                        "  {:<30} {:>4} {:>4} {:>5} {:>9} {:>9} {:>3} {:>6}  {}",
                        truncate(model, 30),
                        conf,
                        ex.currency,
                        ex.line_items.len(),
                        sum,
                        total,
                        if needs_review { "✗" } else { "✓" },
                        ms,
                        reason.map(|r| truncate(&r, 70)).unwrap_or_default(),
                    );
                }
                Err(e) => println!(
                    "  {:<30} {:>4} {:>4} {:>5} {:>9} {:>9} {:>3} {:>6}  ERROR: {}",
                    truncate(model, 30),
                    "—",
                    "—",
                    "—",
                    "—",
                    "—",
                    "✗",
                    ms,
                    truncate(&e.to_string(), 60),
                ),
            }
        }
        println!();
    }

    println!("=== scoreboard (clean = reconciled + no review flag) ===");
    for (i, model) in models.iter().enumerate() {
        let avg = if runs[i] > 0 {
            total_ms[i] / runs[i] as u128
        } else {
            0
        };
        println!(
            "  {:<30} {}/{} clean   avg {} ms",
            truncate(model, 30),
            clean[i],
            runs[i],
            avg
        );
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!(
            "{}…",
            s.chars().take(n.saturating_sub(1)).collect::<String>()
        )
    }
}
