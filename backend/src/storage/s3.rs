use std::sync::Arc;

use s3::bucket::Bucket;
use s3::creds::Credentials;
use s3::region::Region;
use s3::BucketConfiguration;

use crate::config::Config;
use crate::errors::AppError;

fn region_of(config: &Config) -> Region {
    Region::Custom {
        region: config.s3_region.clone(),
        endpoint: config.s3_endpoint.clone(),
    }
}

fn credentials_of(config: &Config) -> Result<Credentials, AppError> {
    Credentials::new(
        Some(&config.s3_access_key),
        Some(&config.s3_secret_key),
        None,
        None,
        None,
    )
    .map_err(|e| AppError::Internal(format!("Failed to create S3 credentials: {e}")))
}

#[derive(Clone)]
pub struct Storage {
    bucket: Arc<Bucket>,
}

impl Storage {
    /// Create the bucket if it does not already exist. Best-effort: an
    /// "already owned / exists" response from MinIO is treated as success.
    pub async fn ensure_bucket(config: &Config) -> Result<(), AppError> {
        let creds = credentials_of(config)?;
        match Bucket::create_with_path_style(
            &config.s3_bucket,
            region_of(config),
            creds,
            BucketConfiguration::default(),
        )
        .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                // Bucket likely already exists; log and continue.
                tracing::warn!("ensure_bucket({}): {e}", config.s3_bucket);
                Ok(())
            }
        }
    }

    pub fn new(config: &Config) -> Self {
        let region = Region::Custom {
            region: config.s3_region.clone(),
            endpoint: config.s3_endpoint.clone(),
        };

        let credentials = Credentials::new(
            Some(&config.s3_access_key),
            Some(&config.s3_secret_key),
            None,
            None,
            None,
        )
        .expect("Failed to create S3 credentials");

        let bucket = Bucket::new(&config.s3_bucket, region, credentials)
            .expect("Failed to create S3 bucket handle")
            .with_path_style();

        Self { bucket: Arc::new(*bucket) }
    }

    pub async fn upload(&self, key: &str, data: &[u8], content_type: &str) -> Result<(), AppError> {
        self.bucket
            .put_object_with_content_type(key, data, content_type)
            .await
            .map_err(|e| AppError::Internal(format!("S3 upload failed: {e}")))?;
        Ok(())
    }

    /// Download the raw bytes of a stored object (used to re-extract on rescan).
    pub async fn get(&self, key: &str) -> Result<Vec<u8>, AppError> {
        let resp = self
            .bucket
            .get_object(key)
            .await
            .map_err(|e| AppError::Internal(format!("S3 get failed: {e}")))?;
        if resp.status_code() != 200 {
            return Err(AppError::Internal(format!(
                "S3 get {key} returned {}",
                resp.status_code()
            )));
        }
        Ok(resp.bytes().to_vec())
    }

    pub async fn get_presigned_url(&self, key: &str, expiry_secs: u32) -> Result<String, AppError> {
        self.bucket
            .presign_get(key, expiry_secs, None)
            .await
            .map_err(|e| AppError::Internal(format!("S3 presign failed: {e}")))
    }

    pub async fn delete(&self, key: &str) -> Result<(), AppError> {
        self.bucket
            .delete_object(key)
            .await
            .map_err(|e| AppError::Internal(format!("S3 delete failed: {e}")))?;
        Ok(())
    }
}
