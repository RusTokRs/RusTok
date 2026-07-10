//! Runtime secret references for capability and module integrations.
//!
//! Callers persist only [`SecretRef`] values. Resolver endpoints and workload
//! identities belong to the process-owned [`SecretResolverRegistry`], so a
//! tenant cannot redirect secret resolution to an arbitrary endpoint.

use std::{collections::HashMap, fmt, path::PathBuf, sync::Arc, time::Duration};

use async_trait::async_trait;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SecretRef {
    pub resolver: String,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretRefStatus {
    pub resolver: String,
    pub configured: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("secret resolver `{0}` is not registered")]
    ResolverNotFound(String),
    #[error("secret reference key must not be blank")]
    BlankKey,
    #[error("secret `{key}` was not found in resolver `{resolver}`")]
    NotFound { resolver: String, key: String },
    #[error("secret resolver `{resolver}` failed: {message}")]
    Resolver { resolver: String, message: String },
}

#[async_trait]
pub trait SecretResolver: Send + Sync {
    async fn resolve(&self, key: &str) -> Result<SecretString, SecretError>;
}

#[derive(Clone)]
pub struct SecretResolverRegistry {
    resolvers: Arc<HashMap<String, Arc<dyn SecretResolver>>>,
    cache: Arc<RwLock<HashMap<SecretRef, CachedSecret>>>,
    ttl: Duration,
}

#[derive(Clone)]
struct CachedSecret {
    value: SecretString,
    expires_at: tokio::time::Instant,
}

impl fmt::Debug for SecretResolverRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretResolverRegistry")
            .field(
                "resolver_aliases",
                &self.resolvers.keys().collect::<Vec<_>>(),
            )
            .field("ttl", &self.ttl)
            .finish_non_exhaustive()
    }
}

impl SecretResolverRegistry {
    pub fn builder() -> SecretResolverRegistryBuilder {
        SecretResolverRegistryBuilder::default()
    }

    pub async fn resolve(&self, reference: &SecretRef) -> Result<SecretString, SecretError> {
        if reference.key.trim().is_empty() {
            return Err(SecretError::BlankKey);
        }
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(reference) {
                if cached.expires_at > tokio::time::Instant::now() {
                    return Ok(cached.value.clone());
                }
            }
        }
        let resolver = self
            .resolvers
            .get(&reference.resolver)
            .ok_or_else(|| SecretError::ResolverNotFound(reference.resolver.clone()))?;
        let value = resolver.resolve(&reference.key).await?;
        self.cache.write().await.insert(
            reference.clone(),
            CachedSecret {
                value: value.clone(),
                expires_at: tokio::time::Instant::now() + self.ttl,
            },
        );
        Ok(value)
    }

    pub async fn invalidate(&self, reference: Option<&SecretRef>) {
        let mut cache = self.cache.write().await;
        if let Some(reference) = reference {
            cache.remove(reference);
        } else {
            cache.clear();
        }
    }

    pub fn contains(&self, alias: &str) -> bool {
        self.resolvers.contains_key(alias)
    }
}

#[derive(Default)]
pub struct SecretResolverRegistryBuilder {
    resolvers: HashMap<String, Arc<dyn SecretResolver>>,
    ttl: Option<Duration>,
}

impl SecretResolverRegistryBuilder {
    pub fn resolver(
        mut self,
        alias: impl Into<String>,
        resolver: impl SecretResolver + 'static,
    ) -> Self {
        self.resolvers.insert(alias.into(), Arc::new(resolver));
        self
    }

    pub fn cache_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl.min(Duration::from_secs(60)));
        self
    }

    pub fn build(self) -> SecretResolverRegistry {
        SecretResolverRegistry {
            resolvers: Arc::new(self.resolvers),
            cache: Arc::new(RwLock::new(HashMap::new())),
            ttl: self.ttl.unwrap_or(Duration::from_secs(60)),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EnvResolver;

#[async_trait]
impl SecretResolver for EnvResolver {
    async fn resolve(&self, key: &str) -> Result<SecretString, SecretError> {
        std::env::var(key)
            .map(SecretString::from)
            .map_err(|_| SecretError::NotFound {
                resolver: "env".to_string(),
                key: key.to_string(),
            })
    }
}

#[derive(Debug, Clone)]
pub struct MountedFileResolver {
    root: PathBuf,
}

impl MountedFileResolver {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

#[async_trait]
impl SecretResolver for MountedFileResolver {
    async fn resolve(&self, key: &str) -> Result<SecretString, SecretError> {
        let path = self.root.join(key);
        let root = self
            .root
            .canonicalize()
            .map_err(|error| SecretError::Resolver {
                resolver: "mounted_file".to_string(),
                message: error.to_string(),
            })?;
        let path = path.canonicalize().map_err(|_| SecretError::NotFound {
            resolver: "mounted_file".to_string(),
            key: key.to_string(),
        })?;
        if !path.starts_with(&root) {
            return Err(SecretError::Resolver {
                resolver: "mounted_file".to_string(),
                message: "secret path escapes the configured mount root".to_string(),
            });
        }
        let value =
            tokio::fs::read_to_string(path)
                .await
                .map_err(|error| SecretError::Resolver {
                    resolver: "mounted_file".to_string(),
                    message: error.to_string(),
                })?;
        let value = value.trim_end_matches(['\r', '\n']);
        if value.is_empty() {
            return Err(SecretError::NotFound {
                resolver: "mounted_file".to_string(),
                key: key.to_string(),
            });
        }
        Ok(SecretString::from(value.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;

    #[tokio::test]
    async fn registry_resolves_and_redacts_env_secrets() {
        unsafe { std::env::set_var("RUSTOK_SECRET_TEST", "top-secret") };
        let registry = SecretResolverRegistry::builder()
            .resolver("env", EnvResolver)
            .build();
        let value = registry
            .resolve(&SecretRef {
                resolver: "env".to_string(),
                key: "RUSTOK_SECRET_TEST".to_string(),
            })
            .await
            .expect("env secret should resolve");
        assert_eq!(value.expose_secret(), "top-secret");
        assert!(!format!("{value:?}").contains("top-secret"));
        unsafe { std::env::remove_var("RUSTOK_SECRET_TEST") };
    }

    #[tokio::test]
    async fn unknown_resolver_fails_explicitly() {
        let registry = SecretResolverRegistry::builder().build();
        let error = registry
            .resolve(&SecretRef {
                resolver: "vault-prod".to_string(),
                key: "ai/openai".to_string(),
            })
            .await
            .expect_err("unknown resolver must fail");
        assert!(error.to_string().contains("vault-prod"));
    }
}
