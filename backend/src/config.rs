use std::env;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub s3_endpoint: String,
    pub s3_bucket: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,
    pub s3_region: String,
    // Extraction
    pub extractor: String, // "mock" | "hosted"
    pub vlm_url: String,   // OpenAI-compatible endpoint (OpenRouter / Mistral / vLLM / Ollama)
    pub vlm_model: String,
    pub vlm_api_key: Option<String>, // bearer key for hosted APIs (OpenRouter/Mistral); None for local
    pub vlm_models: Vec<String>,     // selectable models for the debug model picker (first = recommended default)
    pub dev_receipts_dir: String,
}

/// Curated OpenRouter vision models for the debug picker, strongest first (the first is the
/// recommended default). Order reflects a benchmark on the sample receipts: gemini-2.5-pro
/// reconciled 8/9 vs ~5/9 for the others. Override with the VLM_MODELS env var to match your
/// endpoint/account (e.g. to add an Anthropic model once enabled).
fn default_vlm_models() -> Vec<String> {
    [
        "google/gemini-2.5-pro",
        "qwen/qwen-2.5-vl-72b-instruct",
        "openai/gpt-4o",
        "openai/gpt-4o-mini",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://kvittering:localdev@localhost:5432/kvitteringsapp".to_string()),
            jwt_secret: env::var("JWT_SECRET").expect("JWT_SECRET must be set"),
            s3_endpoint: env::var("S3_ENDPOINT").unwrap_or_else(|_| "http://localhost:9000".to_string()),
            s3_bucket: env::var("S3_BUCKET").unwrap_or_else(|_| "receipts".to_string()),
            s3_access_key: env::var("S3_ACCESS_KEY").unwrap_or_else(|_| "minioadmin".to_string()),
            s3_secret_key: env::var("S3_SECRET_KEY").unwrap_or_else(|_| "minioadmin".to_string()),
            s3_region: env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_string()),
            extractor: env::var("EXTRACTOR").unwrap_or_else(|_| "mock".to_string()),
            vlm_url: env::var("VLM_URL").unwrap_or_else(|_| "http://localhost:11434/v1".to_string()),
            vlm_model: env::var("VLM_MODEL").unwrap_or_else(|_| "qwen3-vl:8b".to_string()),
            vlm_api_key: env::var("VLM_API_KEY").ok().filter(|s| !s.is_empty()),
            vlm_models: env::var("VLM_MODELS")
                .ok()
                .map(|s| {
                    s.split(',')
                        .map(|m| m.trim().to_string())
                        .filter(|m| !m.is_empty())
                        .collect::<Vec<_>>()
                })
                .filter(|v| !v.is_empty())
                .unwrap_or_else(default_vlm_models),
            dev_receipts_dir: env::var("DEV_RECEIPTS_DIR").unwrap_or_else(|_| "dev_receipts".to_string()),
        }
    }
}
