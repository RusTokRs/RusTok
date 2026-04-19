#![cfg(feature = "s3")]

use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_s3::config::{Builder as S3ConfigBuilder, Credentials, Region};
use aws_sdk_s3::operation::get_object::GetObjectOutput;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;

use crate::{
    backend::{StorageBackend, UploadedObject},
    error::{Result, StorageError},
};

#[derive(Clone, Debug)]
pub struct S3Storage {
    client: Client,
    bucket: String,
    key_prefix: Option<String>,
    public_base_url: Option<String>,
}

impl S3Storage {
    pub async fn from_config(config: &S3StorageConfig) -> Result<Self> {
        if config.bucket.trim().is_empty() {
            return Err(StorageError::Backend(
                "S3 storage bucket must not be empty".to_string(),
            ));
        }

        let mut loader = aws_config::defaults(BehaviorVersion::latest());
        if let Some(region) = config
            .region
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            loader = loader.region(Region::new(region.to_string()));
        }

        let shared = loader.load().await;
        let mut builder = S3ConfigBuilder::from(&shared);

        if let Some(endpoint_url) = config
            .endpoint_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            builder = builder.endpoint_url(endpoint_url.to_string());
        }

        if let Some(access_key_id) = config
            .access_key_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let secret_access_key = config
                .secret_access_key
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    StorageError::Backend(
                        "S3 secret_access_key must be configured when access_key_id is set"
                            .to_string(),
                    )
                })?;
            builder = builder.credentials_provider(Credentials::new(
                access_key_id.to_string(),
                secret_access_key.to_string(),
                None,
                None,
                "rustok-storage-config",
            ));
        }

        let client = Client::from_conf(builder.build());
        Ok(Self {
            client,
            bucket: config.bucket.trim().to_string(),
            key_prefix: config
                .key_prefix
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
            public_base_url: config
                .public_base_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.trim_end_matches('/').to_string()),
        })
    }

    fn object_key(&self, path: &str) -> Result<String> {
        let trimmed = path.trim().trim_start_matches('/');
        if trimmed.is_empty() || trimmed.contains("..") {
            return Err(StorageError::InvalidPath(path.to_string()));
        }
        Ok(match &self.key_prefix {
            Some(prefix) => format!("{}/{}", prefix.trim_end_matches('/'), trimmed),
            None => trimmed.to_string(),
        })
    }

    async fn collect_bytes(output: GetObjectOutput) -> Result<bytes::Bytes> {
        output
            .body
            .collect()
            .await
            .map(|aggregated| aggregated.into_bytes())
            .map_err(|error| StorageError::Backend(error.to_string()))
    }
}

#[async_trait]
impl StorageBackend for S3Storage {
    async fn store(
        &self,
        path: &str,
        data: bytes::Bytes,
        content_type: &str,
    ) -> Result<UploadedObject> {
        let key = self.object_key(path)?;
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(ByteStream::from(data.clone()))
            .content_type(content_type)
            .send()
            .await
            .map_err(|error| StorageError::Backend(error.to_string()))?;

        Ok(UploadedObject {
            path: path.to_string(),
            public_url: self.public_url(path),
            size: data.len() as u64,
        })
    }

    async fn delete(&self, path: &str) -> Result<()> {
        let key = self.object_key(path)?;
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|error| StorageError::Backend(error.to_string()))?;
        Ok(())
    }

    async fn read(&self, path: &str) -> Result<bytes::Bytes> {
        let key = self.object_key(path)?;
        let output = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|error| StorageError::Backend(error.to_string()))?;
        Self::collect_bytes(output).await
    }

    async fn private_download_url(
        &self,
        path: &str,
        expires_in: std::time::Duration,
    ) -> Result<Option<String>> {
        let key = self.object_key(path)?;
        let request = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .presigned(
                PresigningConfig::expires_in(expires_in)
                    .map_err(|error| StorageError::Backend(error.to_string()))?,
            )
            .await
            .map_err(|error| StorageError::Backend(error.to_string()))?;
        Ok(Some(request.uri().to_string()))
    }

    fn public_url(&self, path: &str) -> String {
        let key = match self.object_key(path) {
            Ok(key) => key,
            Err(_) => path.trim_start_matches('/').to_string(),
        };
        if let Some(base) = &self.public_base_url {
            return format!("{base}/{key}");
        }
        format!("s3://{}/{}", self.bucket, key)
    }

    fn backend_name(&self) -> &'static str {
        "s3"
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Default)]
pub struct S3StorageConfig {
    #[serde(default)]
    pub bucket: String,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub endpoint_url: Option<String>,
    #[serde(default)]
    pub access_key_id: Option<String>,
    #[serde(default)]
    pub secret_access_key: Option<String>,
    #[serde(default)]
    pub public_base_url: Option<String>,
    #[serde(default)]
    pub key_prefix: Option<String>,
}
