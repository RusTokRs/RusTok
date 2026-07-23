//! Runtime secret references for capability and module integrations.
//!
//! Callers persist only [`SecretRef`] values. Resolver endpoints and workload
//! identities belong to the process-owned [`SecretResolverRegistry`], so a
//! tenant cannot redirect secret resolution to an arbitrary endpoint.

use std::{collections::HashMap, fmt, path::PathBuf, sync::Arc, time::Duration};
use uuid::Uuid;

use async_trait::async_trait;
pub use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

mod cloud;
mod remote;

pub use cloud::{AwsSecretsManagerResolver, AzureKeyVaultResolver, GcpSecretManagerResolver};
pub use remote::{KubernetesSecretResolver, VaultAuth, VaultResolver};

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
    #[error("secret key `{key}` is forbidden by resolver `{resolver}` policy")]
    ForbiddenKey { resolver: String, key: String },
    #[error("secret `{key}` was not found in resolver `{resolver}`")]
    NotFound { resolver: String, key: String },
    #[error("secret resolver `{resolver}` failed: {message}")]
    Resolver { resolver: String, message: String },
}

#[async_trait]
pub trait SecretResolver: Send + Sync {
    async fn resolve(&self, key: &str) -> Result<SecretString, SecretError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretAccessPolicy {
    DenyAll,
    Exact(Vec<String>),
    Prefix(Vec<String>),
    TenantPrefix { prefix: String },
}

impl SecretAccessPolicy {
    fn allows(&self, tenant_id: Uuid, key: &str) -> bool {
        match self {
            Self::DenyAll => false,
            Self::Exact(values) => values.iter().any(|value| value == key),
            Self::Prefix(values) => values.iter().any(|value| key.starts_with(value)),
            Self::TenantPrefix { prefix } => key.starts_with(&format!("{prefix}{tenant_id}/")),
        }
    }
}

#[derive(Clone)]
struct ResolverRegistration {
    resolver: Arc<dyn SecretResolver>,
    policy: SecretAccessPolicy,
}

#[derive(Clone)]
pub struct SecretResolverRegistry {
    resolvers: Arc<HashMap<String, ResolverRegistration>>,
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

    pub async fn resolve_for_tenant(
        &self,
        tenant_id: Uuid,
        reference: &SecretRef,
    ) -> Result<SecretString, SecretError> {
        self.validate_reference_for_tenant(tenant_id, reference)?;
        let registration = self
            .resolvers
            .get(&reference.resolver)
            .expect("validated secret resolver registration must exist");
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(reference) {
                if cached.expires_at > tokio::time::Instant::now() {
                    return Ok(cached.value.clone());
                }
            }
        }
        let value = registration.resolver.resolve(&reference.key).await?;
        self.cache.write().await.insert(
            reference.clone(),
            CachedSecret {
                value: value.clone(),
                expires_at: tokio::time::Instant::now() + self.ttl,
            },
        );
        Ok(value)
    }

    /// Validates an externally persisted secret reference without resolving or
    /// exposing its value. Profile create/update paths use this before writing
    /// a reference so invalid resolver aliases and tenant-forbidden keys fail
    /// at the management boundary rather than during model inference.
    pub fn validate_reference_for_tenant(
        &self,
        tenant_id: Uuid,
        reference: &SecretRef,
    ) -> Result<(), SecretError> {
        if reference.key.trim().is_empty() {
            return Err(SecretError::BlankKey);
        }
        let registration = self
            .resolvers
            .get(&reference.resolver)
            .ok_or_else(|| SecretError::ResolverNotFound(reference.resolver.clone()))?;
        if !registration.policy.allows(tenant_id, &reference.key) {
            return Err(SecretError::ForbiddenKey {
                resolver: reference.resolver.clone(),
                key: reference.key.clone(),
            });
        }
        Ok(())
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
    resolvers: HashMap<String, ResolverRegistration>,
    ttl: Option<Duration>,
}

impl SecretResolverRegistryBuilder {
    pub fn resolver(
        mut self,
        alias: impl Into<String>,
        resolver: impl SecretResolver + 'static,
        policy: SecretAccessPolicy,
    ) -> Self {
        self.resolvers.insert(
            alias.into(),
            ResolverRegistration {
                resolver: Arc::new(resolver),
                policy,
            },
        );
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
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::*;
    use secrecy::ExposeSecret;
    use tokio::sync::Mutex;

    #[derive(Clone)]
    struct RotatingResolver {
        value: Arc<Mutex<String>>,
        calls: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl SecretResolver for RotatingResolver {
        async fn resolve(&self, _key: &str) -> Result<SecretString, SecretError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(SecretString::from(self.value.lock().await.clone()))
        }
    }

    #[tokio::test]
    async fn registry_resolves_and_redacts_env_secrets() {
        unsafe { std::env::set_var("RUSTOK_SECRET_TEST", "top-secret") };
        let registry = SecretResolverRegistry::builder()
            .resolver(
                "env",
                EnvResolver,
                SecretAccessPolicy::Exact(vec!["RUSTOK_SECRET_TEST".to_string()]),
            )
            .build();
        let value = registry
            .resolve_for_tenant(
                Uuid::nil(),
                &SecretRef {
                    resolver: "env".to_string(),
                    key: "RUSTOK_SECRET_TEST".to_string(),
                },
            )
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
            .resolve_for_tenant(
                Uuid::nil(),
                &SecretRef {
                    resolver: "vault-prod".to_string(),
                    key: "ai/openai".to_string(),
                },
            )
            .await
            .expect_err("unknown resolver must fail");
        assert!(error.to_string().contains("vault-prod"));
    }

    #[test]
    fn reference_validation_rejects_unknown_alias_and_cross_tenant_key_without_resolving() {
        let tenant = Uuid::new_v4();
        let other = Uuid::new_v4();
        let registry = SecretResolverRegistry::builder()
            .resolver(
                "env",
                EnvResolver,
                SecretAccessPolicy::TenantPrefix {
                    prefix: "tenants/".to_string(),
                },
            )
            .build();
        assert!(
            registry
                .validate_reference_for_tenant(
                    tenant,
                    &SecretRef {
                        resolver: "missing".to_string(),
                        key: "tenants/ignored/key".to_string(),
                    },
                )
                .is_err()
        );
        let error = registry
            .validate_reference_for_tenant(
                other,
                &SecretRef {
                    resolver: "env".to_string(),
                    key: format!("tenants/{tenant}/provider-key"),
                },
            )
            .expect_err("other tenant must not validate this key");
        assert!(matches!(error, SecretError::ForbiddenKey { .. }));
    }

    #[tokio::test]
    async fn cache_is_invalidated_after_secret_rotation() {
        let resolver = RotatingResolver {
            value: Arc::new(Mutex::new("first".to_string())),
            calls: Arc::new(AtomicUsize::new(0)),
        };
        let calls = Arc::clone(&resolver.calls);
        let value = Arc::clone(&resolver.value);
        let reference = SecretRef {
            resolver: "rotation".to_string(),
            key: "tenant/secret".to_string(),
        };
        let registry = SecretResolverRegistry::builder()
            .resolver(
                "rotation",
                resolver,
                SecretAccessPolicy::Prefix(vec!["tenant/".to_string()]),
            )
            .build();

        assert_eq!(
            registry
                .resolve_for_tenant(Uuid::nil(), &reference)
                .await
                .unwrap()
                .expose_secret(),
            "first"
        );
        *value.lock().await = "rotated".to_string();
        assert_eq!(
            registry
                .resolve_for_tenant(Uuid::nil(), &reference)
                .await
                .unwrap()
                .expose_secret(),
            "first"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        registry.invalidate(Some(&reference)).await;
        assert_eq!(
            registry
                .resolve_for_tenant(Uuid::nil(), &reference)
                .await
                .unwrap()
                .expose_secret(),
            "rotated"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn tenant_prefix_policy_prevents_cross_tenant_resolution() {
        let tenant = Uuid::new_v4();
        let other = Uuid::new_v4();
        let key = format!("tenants/{tenant}/provider-key");
        unsafe { std::env::set_var(&key, "scoped-secret") };
        let registry = SecretResolverRegistry::builder()
            .resolver(
                "env",
                EnvResolver,
                SecretAccessPolicy::TenantPrefix {
                    prefix: "tenants/".to_string(),
                },
            )
            .build();
        let reference = SecretRef {
            resolver: "env".to_string(),
            key: key.clone(),
        };
        registry
            .resolve_for_tenant(tenant, &reference)
            .await
            .unwrap();
        let error = registry
            .resolve_for_tenant(other, &reference)
            .await
            .unwrap_err();
        assert!(matches!(error, SecretError::ForbiddenKey { .. }));
        unsafe { std::env::remove_var(key) };
    }
}
