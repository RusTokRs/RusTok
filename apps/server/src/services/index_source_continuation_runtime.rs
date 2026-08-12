use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rustok_secrets::{
    EnvResolver, ExposeSecret, MountedFileResolver, SecretAccessPolicy, SecretRef,
    SecretResolverRegistry,
};
use serde::Deserialize;
use thiserror::Error;
use uuid::Uuid;

const KEYRING_ENV: &str = "RUSTOK_INDEX_SOURCE_CONTINUATION_KEYRING_JSON";
const SECRET_MOUNT_ROOT_ENV: &str = "RUSTOK_INDEX_SOURCE_CONTINUATION_SECRET_MOUNT_ROOT";
const ENV_RESOLVER_ALIAS: &str = "env";
const MOUNTED_FILE_RESOLVER_ALIAS: &str = "mounted_file";
const KEY_BYTES: usize = 32;
const ENCODED_KEY_BYTES: usize = 43;
const MAX_CONFIG_BYTES: usize = 16 * 1024;
const MAX_KEYS: usize = 16;
const MAX_KEY_ID_BYTES: usize = 64;
const MAX_SECRET_REFERENCE_BYTES: usize = 256;
const MIN_LIFETIME_SECONDS: u64 = 1;
const MAX_LIFETIME_SECONDS: u64 = 15 * 60;
const DEFAULT_LIFETIME_SECONDS: u64 = 5 * 60;
const DEPLOYMENT_SECRET_SCOPE: Uuid = Uuid::from_u128(0);

#[derive(Clone, Deserialize)]
struct IndexSourceContinuationKeyringConfig {
    active_key_id: String,
    #[serde(default = "default_lifetime_seconds")]
    lifetime_seconds: u64,
    keys: BTreeMap<String, SecretRef>,
}

fn default_lifetime_seconds() -> u64 {
    DEFAULT_LIFETIME_SECONDS
}

#[derive(Debug, Error)]
pub(super) enum IndexSourceContinuationKeyringError {
    #[error("Index source continuation deployment keyring configuration is invalid")]
    InvalidConfiguration,
    #[error("Index source continuation deployment secret is unavailable")]
    SecretUnavailable,
    #[error("Index source continuation deployment key material is invalid")]
    InvalidKeyMaterial,
    #[error(transparent)]
    Codec(#[from] rustok_index::IndexSourceContinuationError),
}

/// Deployment-owned resolver boundary for source continuation encryption keys.
///
/// The runtime retains only bounded key identifiers and `SecretRef` values. Raw key bytes are
/// resolved into one short-lived codec for a single sealed page call and are never inserted into
/// `ModuleRuntimeExtensions`, settings, logs, or debug output.
#[derive(Clone)]
pub(super) struct IndexSourceContinuationKeyringRuntime {
    active_key_id: String,
    lifetime: Duration,
    keys: Arc<BTreeMap<String, SecretRef>>,
    secrets: SecretResolverRegistry,
}

impl IndexSourceContinuationKeyringRuntime {
    pub(super) fn from_environment() -> Result<Option<Self>, IndexSourceContinuationKeyringError> {
        let Some(raw) = std::env::var_os(KEYRING_ENV) else {
            return Ok(None);
        };
        let raw = raw.to_string_lossy();
        if raw.len() > MAX_CONFIG_BYTES {
            return Err(IndexSourceContinuationKeyringError::InvalidConfiguration);
        }
        let config = serde_json::from_str::<IndexSourceContinuationKeyringConfig>(raw.as_ref())
            .map_err(|_| IndexSourceContinuationKeyringError::InvalidConfiguration)?;
        let secrets = local_secret_registry(&config)?;
        Self::from_config(config, secrets).map(Some)
    }

    fn from_config(
        config: IndexSourceContinuationKeyringConfig,
        secrets: SecretResolverRegistry,
    ) -> Result<Self, IndexSourceContinuationKeyringError> {
        validate_config(&config)?;
        for reference in config.keys.values() {
            secrets
                .validate_reference_for_tenant(DEPLOYMENT_SECRET_SCOPE, reference)
                .map_err(|_| IndexSourceContinuationKeyringError::InvalidConfiguration)?;
        }
        Ok(Self {
            active_key_id: config.active_key_id,
            lifetime: Duration::from_secs(config.lifetime_seconds),
            keys: Arc::new(config.keys),
            secrets,
        })
    }

    pub(super) fn lifetime(&self) -> Duration {
        self.lifetime
    }

    pub(super) async fn resolve_codec(
        &self,
    ) -> Result<rustok_index::IndexSourceContinuationCodec, IndexSourceContinuationKeyringError>
    {
        let mut material = BTreeMap::new();
        for (key_id, reference) in self.keys.iter() {
            let secret = self
                .secrets
                .resolve_for_tenant(DEPLOYMENT_SECRET_SCOPE, reference)
                .await
                .map_err(|_| IndexSourceContinuationKeyringError::SecretUnavailable)?;
            let encoded = secret.expose_secret().trim();
            if encoded.len() != ENCODED_KEY_BYTES {
                return Err(IndexSourceContinuationKeyringError::InvalidKeyMaterial);
            }
            let decoded = URL_SAFE_NO_PAD
                .decode(encoded)
                .map_err(|_| IndexSourceContinuationKeyringError::InvalidKeyMaterial)?;
            let key = <[u8; KEY_BYTES]>::try_from(decoded.as_slice())
                .map_err(|_| IndexSourceContinuationKeyringError::InvalidKeyMaterial)?;
            material.insert(key_id.clone(), key);
        }
        rustok_index::IndexSourceContinuationCodec::new(self.active_key_id.clone(), material)
            .map_err(Into::into)
    }
}

impl fmt::Debug for IndexSourceContinuationKeyringRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexSourceContinuationKeyringRuntime")
            .field("active_key_id", &self.active_key_id)
            .field("key_count", &self.keys.len())
            .field("lifetime", &self.lifetime)
            .finish_non_exhaustive()
    }
}

pub(super) fn materialize_index_source_continuation_keyring()
-> Result<Option<IndexSourceContinuationKeyringRuntime>, IndexSourceContinuationKeyringError> {
    IndexSourceContinuationKeyringRuntime::from_environment()
}

fn validate_config(
    config: &IndexSourceContinuationKeyringConfig,
) -> Result<(), IndexSourceContinuationKeyringError> {
    if config.keys.is_empty()
        || config.keys.len() > MAX_KEYS
        || !config.keys.contains_key(&config.active_key_id)
        || !(MIN_LIFETIME_SECONDS..=MAX_LIFETIME_SECONDS).contains(&config.lifetime_seconds)
    {
        return Err(IndexSourceContinuationKeyringError::InvalidConfiguration);
    }

    let mut references = BTreeSet::new();
    for (key_id, reference) in &config.keys {
        if !valid_key_id(key_id)
            || !valid_secret_reference(reference)
            || !references.insert((reference.resolver.clone(), reference.key.clone()))
        {
            return Err(IndexSourceContinuationKeyringError::InvalidConfiguration);
        }
    }
    Ok(())
}

fn valid_key_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_KEY_ID_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        })
}

fn valid_secret_reference(reference: &SecretRef) -> bool {
    let key = reference.key.as_str();
    !reference.resolver.trim().is_empty()
        && !key.is_empty()
        && key.len() <= MAX_SECRET_REFERENCE_BYTES
        && key == key.trim()
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/'))
}

fn local_secret_registry(
    config: &IndexSourceContinuationKeyringConfig,
) -> Result<SecretResolverRegistry, IndexSourceContinuationKeyringError> {
    validate_config(config)?;
    let mut env_keys = Vec::new();
    let mut mounted_file_keys = Vec::new();
    for reference in config.keys.values() {
        match reference.resolver.as_str() {
            ENV_RESOLVER_ALIAS => env_keys.push(reference.key.clone()),
            MOUNTED_FILE_RESOLVER_ALIAS => mounted_file_keys.push(reference.key.clone()),
            _ => return Err(IndexSourceContinuationKeyringError::InvalidConfiguration),
        }
    }

    let mut builder = SecretResolverRegistry::builder();
    if !env_keys.is_empty() {
        env_keys.sort();
        env_keys.dedup();
        builder = builder.resolver(
            ENV_RESOLVER_ALIAS,
            EnvResolver,
            SecretAccessPolicy::Exact(env_keys),
        );
    }
    if !mounted_file_keys.is_empty() {
        let root = std::env::var_os(SECRET_MOUNT_ROOT_ENV)
            .map(PathBuf::from)
            .ok_or(IndexSourceContinuationKeyringError::InvalidConfiguration)?;
        mounted_file_keys.sort();
        mounted_file_keys.dedup();
        builder = builder.resolver(
            MOUNTED_FILE_RESOLVER_ALIAS,
            MountedFileResolver::new(root),
            SecretAccessPolicy::Exact(mounted_file_keys),
        );
    }
    Ok(builder.build())
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use async_trait::async_trait;
    use rustok_secrets::{SecretAccessPolicy, SecretError, SecretResolver, SecretString};

    use super::*;

    #[derive(Clone)]
    struct StaticResolver {
        values: Arc<HashMap<String, String>>,
    }

    #[async_trait]
    impl SecretResolver for StaticResolver {
        async fn resolve(&self, key: &str) -> Result<SecretString, SecretError> {
            self.values
                .get(key)
                .cloned()
                .map(SecretString::from)
                .ok_or_else(|| SecretError::NotFound {
                    resolver: "test".to_string(),
                    key: key.to_string(),
                })
        }
    }

    fn config(active: &str, values: &[(&str, &str)]) -> IndexSourceContinuationKeyringConfig {
        IndexSourceContinuationKeyringConfig {
            active_key_id: active.to_string(),
            lifetime_seconds: 300,
            keys: values
                .iter()
                .map(|(key_id, key)| {
                    (
                        (*key_id).to_string(),
                        SecretRef {
                            resolver: "test".to_string(),
                            key: (*key).to_string(),
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>(),
        }
    }

    fn registry(values: &[(&str, [u8; KEY_BYTES])]) -> SecretResolverRegistry {
        let encoded = values
            .iter()
            .map(|(name, value)| ((*name).to_string(), URL_SAFE_NO_PAD.encode(value)))
            .collect::<HashMap<_, _>>();
        let keys = encoded.keys().cloned().collect::<Vec<_>>();
        SecretResolverRegistry::builder()
            .resolver(
                "test",
                StaticResolver {
                    values: Arc::new(encoded),
                },
                SecretAccessPolicy::Exact(keys),
            )
            .build()
    }

    #[tokio::test]
    async fn resolves_exact_key_bytes_without_exposing_references_in_debug() {
        let runtime = IndexSourceContinuationKeyringRuntime::from_config(
            config("current", &[("current", "current-key"), ("old", "old-key")]),
            registry(&[("current-key", [7; KEY_BYTES]), ("old-key", [3; KEY_BYTES])]),
        )
        .unwrap();

        let codec = runtime.resolve_codec().await.unwrap();
        assert_eq!(codec.active_key_id(), "current");
        assert_eq!(codec.key_count(), 2);
        let debug = format!("{runtime:?}");
        assert!(!debug.contains("current-key"));
        assert!(!debug.contains("old-key"));
    }

    #[tokio::test]
    async fn rejects_encoded_key_material_that_is_not_canonical_32_bytes() {
        let values = HashMap::from([(
            "short-key".to_string(),
            URL_SAFE_NO_PAD.encode([5_u8; KEY_BYTES - 1]),
        )]);
        let secrets = SecretResolverRegistry::builder()
            .resolver(
                "test",
                StaticResolver {
                    values: Arc::new(values),
                },
                SecretAccessPolicy::Exact(vec!["short-key".to_string()]),
            )
            .build();
        let runtime = IndexSourceContinuationKeyringRuntime::from_config(
            config("current", &[("current", "short-key")]),
            secrets,
        )
        .unwrap();

        assert!(matches!(
            runtime.resolve_codec().await,
            Err(IndexSourceContinuationKeyringError::InvalidKeyMaterial)
        ));
    }

    #[test]
    fn rejects_duplicate_references_out_of_range_lifetime_and_unbounded_keys() {
        let duplicate = config("current", &[("current", "same"), ("old", "same")]);
        assert!(matches!(
            validate_config(&duplicate),
            Err(IndexSourceContinuationKeyringError::InvalidConfiguration)
        ));

        let mut lifetime = config("current", &[("current", "current-key")]);
        lifetime.lifetime_seconds = MAX_LIFETIME_SECONDS + 1;
        assert!(matches!(
            validate_config(&lifetime),
            Err(IndexSourceContinuationKeyringError::InvalidConfiguration)
        ));

        let oversized = "x".repeat(MAX_SECRET_REFERENCE_BYTES + 1);
        let unbounded = config("current", &[("current", oversized.as_str())]);
        assert!(matches!(
            validate_config(&unbounded),
            Err(IndexSourceContinuationKeyringError::InvalidConfiguration)
        ));
    }
}
