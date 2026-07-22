use std::{path::PathBuf, sync::Arc};

use object_store::{ObjectStore, local::LocalFileSystem, signer::Signer};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct StorageRuntime {
    pub objects: Arc<dyn ObjectStore>,
    pub signer: Option<Arc<dyn Signer>>,
    pub kind: StorageKind,
    public_base_url: Option<String>,
}

impl StorageRuntime {
    pub async fn from_config(config: &StorageConfig) -> object_store::Result<Self> {
        match config.driver {
            StorageDriver::Local => Self::local(&config.local),
            #[cfg(feature = "s3")]
            StorageDriver::S3 => Self::s3(&config.s3),
        }
    }

    pub fn local(config: &LocalStorageConfig) -> object_store::Result<Self> {
        let base_dir = PathBuf::from(&config.base_dir);
        std::fs::create_dir_all(&base_dir).map_err(|source| object_store::Error::Generic {
            store: "LocalFileSystem",
            source: Box::new(source),
        })?;
        let store = LocalFileSystem::new_with_prefix(&base_dir)?
            .with_automatic_cleanup(true)
            .with_fsync(config.fsync);
        Ok(Self {
            objects: Arc::new(store),
            signer: None,
            kind: StorageKind::Local,
            public_base_url: normalize_public_base_url(Some(&config.base_url)),
        })
    }

    pub fn with_signer(mut self, signer: Arc<dyn Signer>) -> Self {
        self.signer = Some(signer);
        self
    }

    #[cfg(feature = "s3")]
    fn s3(config: &S3StorageConfig) -> object_store::Result<Self> {
        use object_store::aws::AmazonS3Builder;

        let mut builder = AmazonS3Builder::from_env()
            .with_bucket_name(config.bucket.trim())
            .with_allow_http(config.allow_http)
            .with_virtual_hosted_style_request(config.virtual_hosted_style_request);
        if let Some(region) = non_empty(config.region.as_deref()) {
            builder = builder.with_region(region);
        }
        if let Some(endpoint) = non_empty(config.endpoint_url.as_deref()) {
            builder = builder.with_endpoint(endpoint);
        }
        if let Some(access_key_id) = non_empty(config.access_key_id.as_deref()) {
            builder = builder.with_access_key_id(access_key_id);
        }
        if let Some(secret_access_key) = non_empty(config.secret_access_key.as_deref()) {
            builder = builder.with_secret_access_key(secret_access_key);
        }
        if let Some(token) = non_empty(config.session_token.as_deref()) {
            builder = builder.with_token(token);
        }
        let store = builder.build()?;
        Ok(Self {
            objects: Arc::new(store.clone()),
            signer: Some(Arc::new(store)),
            kind: StorageKind::S3,
            public_base_url: normalize_public_base_url(config.public_base_url.as_deref()),
        })
    }

    pub fn public_url(&self, path: &object_store::path::Path) -> Option<String> {
        self.public_base_url
            .as_ref()
            .map(|base| format!("{base}/{path}"))
    }

    /// Builds backend-compatible write metadata without wrapping the object operation.
    ///
    /// `LocalFileSystem` rejects custom attributes, while S3 persists the content type.
    pub fn put_options(&self, content_type: impl Into<String>) -> object_store::PutOptions {
        if self.kind == StorageKind::Local {
            return object_store::PutOptions::default();
        }
        let mut attributes = object_store::Attributes::new();
        attributes.insert(
            object_store::Attribute::ContentType,
            object_store::AttributeValue::from(content_type.into()),
        );
        object_store::PutOptions {
            attributes,
            ..Default::default()
        }
    }

    pub async fn signed_download_url(
        &self,
        path: &object_store::path::Path,
        expires_in: std::time::Duration,
    ) -> object_store::Result<Option<String>> {
        let Some(signer) = &self.signer else {
            return Ok(None);
        };
        signer
            .signed_url(reqwest::Method::GET, path, expires_in)
            .await
            .map(|url| Some(url.to_string()))
    }

    pub async fn signed_upload_url(
        &self,
        path: &object_store::path::Path,
        expires_in: std::time::Duration,
    ) -> object_store::Result<Option<String>> {
        let Some(signer) = &self.signer else {
            return Ok(None);
        };
        signer
            .signed_url(reqwest::Method::PUT, path, expires_in)
            .await
            .map(|url| Some(url.to_string()))
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn normalize_public_base_url(value: Option<&str>) -> Option<String> {
    non_empty(value).map(|value| value.trim_end_matches('/').to_string())
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StorageDriver {
    #[default]
    Local,
    #[cfg(feature = "s3")]
    S3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageKind {
    Local,
    S3,
}

impl StorageKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::S3 => "s3",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    #[serde(default)]
    pub driver: StorageDriver,
    #[serde(default)]
    pub local: LocalStorageConfig,
    #[cfg(feature = "s3")]
    #[serde(default)]
    pub s3: S3StorageConfig,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            driver: StorageDriver::Local,
            local: LocalStorageConfig::default(),
            #[cfg(feature = "s3")]
            s3: S3StorageConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalStorageConfig {
    pub base_dir: String,
    #[serde(default = "default_local_base_url")]
    pub base_url: String,
    #[serde(default)]
    pub fsync: bool,
}

impl Default for LocalStorageConfig {
    fn default() -> Self {
        Self {
            base_dir: "storage".to_string(),
            base_url: default_local_base_url(),
            fsync: false,
        }
    }
}

fn default_local_base_url() -> String {
    "/media".to_string()
}

#[derive(Clone, Serialize, Deserialize, Default)]
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
    pub session_token: Option<String>,
    #[serde(default)]
    pub public_base_url: Option<String>,
    #[serde(default)]
    pub allow_http: bool,
    #[serde(default)]
    pub virtual_hosted_style_request: bool,
}

impl std::fmt::Debug for S3StorageConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("S3StorageConfig")
            .field("bucket", &self.bucket)
            .field("region", &self.region)
            .field("endpoint_url", &self.endpoint_url)
            .field("access_key_id_configured", &self.access_key_id.is_some())
            .field(
                "secret_access_key_configured",
                &self.secret_access_key.is_some(),
            )
            .field("session_token_configured", &self.session_token.is_some())
            .field("public_base_url", &self.public_base_url)
            .field("allow_http", &self.allow_http)
            .field(
                "virtual_hosted_style_request",
                &self.virtual_hosted_style_request,
            )
            .finish()
    }
}
