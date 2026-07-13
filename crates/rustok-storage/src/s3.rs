#![cfg(feature = "s3")]

use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_s3::config::{Builder as S3ConfigBuilder, Credentials, Region};
use aws_sdk_s3::error::ProvideErrorMetadata;
use aws_sdk_s3::operation::get_object::GetObjectOutput;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;

use crate::{
    backend::{StorageBackend, StoredObject, UploadedObject},
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
        let bucket = config.bucket.trim();
        if bucket.is_empty() || bucket.chars().any(char::is_control) {
            return Err(StorageError::Backend(
                "S3 storage bucket must be a non-empty value without control characters"
                    .to_string(),
            ));
        }

        let key_prefix = config
            .key_prefix
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| validate_s3_path(value, false))
            .transpose()?;
        let public_base_url = config
            .public_base_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(validate_public_base_url)
            .transpose()?;

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
            if endpoint_url.chars().any(char::is_control) {
                return Err(StorageError::Backend(
                    "S3 endpoint_url contains control characters".to_string(),
                ));
            }
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
            bucket: bucket.to_string(),
            key_prefix,
            public_base_url,
        })
    }

    fn object_key(&self, path: &str) -> Result<String> {
        self.object_key_with_empty(path, false)
    }

    fn object_key_with_empty(&self, path: &str, allow_empty: bool) -> Result<String> {
        let relative = validate_s3_path(path, allow_empty)?;
        Ok(match (&self.key_prefix, relative.is_empty()) {
            (Some(prefix), false) => format!("{prefix}/{relative}"),
            (Some(prefix), true) => prefix.clone(),
            (None, _) => relative,
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

    fn relative_path(&self, key: &str) -> Option<String> {
        let relative = match &self.key_prefix {
            Some(prefix) => key
                .strip_prefix(prefix)
                .and_then(|suffix| suffix.strip_prefix('/'))?,
            None => key,
        };
        validate_s3_path(relative, false).ok()
    }

    fn invalid_public_url(&self) -> String {
        if let Some(base) = &self.public_base_url {
            format!("{base}/invalid-object")
        } else {
            format!("s3://{}/invalid-object", self.bucket)
        }
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
        validate_content_type(content_type)?;
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

    async fn store_if_absent(
        &self,
        path: &str,
        data: bytes::Bytes,
        content_type: &str,
    ) -> Result<bool> {
        validate_content_type(content_type)?;
        let key = self.object_key(path)?;
        match self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(ByteStream::from(data))
            .content_type(content_type)
            .if_none_match("*")
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(error)
                if error
                    .as_service_error()
                    .and_then(|service| service.code())
                    .is_some_and(|code| code == "PreconditionFailed") =>
            {
                Ok(false)
            }
            Err(error) => Err(StorageError::Backend(error.to_string())),
        }
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

    async fn list(&self, prefix: &str) -> Result<Vec<StoredObject>> {
        let prefix = self.object_key_with_empty(prefix, true)?;
        let mut continuation_token = None;
        let mut objects = Vec::new();
        loop {
            let response = self
                .client
                .list_objects_v2()
                .bucket(&self.bucket)
                .prefix(&prefix)
                .set_continuation_token(continuation_token)
                .send()
                .await
                .map_err(|error| StorageError::Backend(error.to_string()))?;
            for object in response.contents() {
                let Some(key) = object.key() else {
                    continue;
                };
                let Some(path) = self.relative_path(key) else {
                    tracing::warn!(key, "Ignoring S3 object outside validated storage key policy");
                    continue;
                };
                let size = u64::try_from(object.size().unwrap_or_default()).map_err(|_| {
                    StorageError::Backend(format!("S3 object `{key}` has a negative size"))
                })?;
                objects.push(StoredObject { path, size });
            }
            if !response.is_truncated().unwrap_or(false) {
                break;
            }
            continuation_token = response.next_continuation_token().map(ToString::to_string);
            if continuation_token.is_none() {
                return Err(StorageError::Backend(
                    "S3 listed a truncated result without continuation token".to_string(),
                ));
            }
        }
        Ok(objects)
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
            Err(_) => return self.invalid_public_url(),
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

fn validate_s3_path(path: &str, allow_empty: bool) -> Result<String> {
    if path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || path.contains('%')
        || path.contains('?')
        || path.contains('#')
        || path.chars().any(char::is_control)
    {
        return Err(StorageError::InvalidPath(path.to_string()));
    }

    let path = path.trim();
    if path.is_empty() {
        return if allow_empty {
            Ok(String::new())
        } else {
            Err(StorageError::InvalidPath(path.to_string()))
        };
    }

    if path.split('/').any(|segment| {
        segment.is_empty()
            || matches!(segment, "." | "..")
            || !segment.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
            })
    }) {
        return Err(StorageError::InvalidPath(path.to_string()));
    }

    Ok(path.to_string())
}

fn validate_content_type(content_type: &str) -> Result<()> {
    let valid = !content_type.trim().is_empty()
        && content_type.len() <= 255
        && content_type
            .chars()
            .all(|character| character.is_ascii_graphic() && !matches!(character, '\r' | '\n'));
    if valid {
        Ok(())
    } else {
        Err(StorageError::Backend(
            "S3 content type contains unsupported characters".to_string(),
        ))
    }
}

fn validate_public_base_url(value: &str) -> Result<String> {
    let value = value.trim_end_matches('/');
    if !(value.starts_with("https://") || value.starts_with("http://"))
        || value.contains(['\r', '\n', '?', '#'])
        || value.chars().any(char::is_control)
    {
        return Err(StorageError::Backend(
            "S3 public_base_url must be a plain HTTP(S) URL".to_string(),
        ));
    }
    Ok(value.to_string())
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

#[cfg(test)]
mod tests {
    use super::{validate_public_base_url, validate_s3_path};

    #[test]
    fn s3_keys_are_relative_and_url_safe() {
        assert_eq!(
            validate_s3_path("tenant/2026/asset.png", false).unwrap(),
            "tenant/2026/asset.png"
        );
        assert!(validate_s3_path("../secret", false).is_err());
        assert!(validate_s3_path("tenant/%2e%2e/secret", false).is_err());
        assert!(validate_s3_path("tenant\\secret", false).is_err());
        assert!(validate_s3_path("/absolute", false).is_err());
    }

    #[test]
    fn public_base_url_rejects_query_or_fragment_confusion() {
        assert!(validate_public_base_url("https://cdn.example/media").is_ok());
        assert!(validate_public_base_url("https://cdn.example/media?token=x").is_err());
        assert!(validate_public_base_url("javascript:alert(1)").is_err());
    }
}