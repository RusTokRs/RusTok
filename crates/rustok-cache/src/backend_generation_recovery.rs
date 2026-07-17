const MAX_GENERATION_RECOVERIES_PER_PROBE: usize = 16;

struct GenerationRecoveryOwner {
    identity: std::sync::Weak<dyn std::any::Any + Send + Sync>,
    state: std::sync::Weak<BackendGenerationState>,
}

static GENERATION_RECOVERY_OWNERS: OnceLock<Mutex<HashMap<usize, GenerationRecoveryOwner>>> =
    OnceLock::new();

fn generation_recovery_owners() -> &'static Mutex<HashMap<usize, GenerationRecoveryOwner>> {
    GENERATION_RECOVERY_OWNERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn bind_generation_recovery_owner(
    state: &Arc<BackendGenerationState>,
    identity: &Arc<dyn std::any::Any + Send + Sync>,
) -> Result<(), CacheBackendGenerationError> {
    let state_identity = Arc::as_ptr(state) as usize;
    let mut owners = generation_recovery_owners()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    if let Some(owner) = owners.get(&state_identity) {
        let Some(existing) = owner.identity.upgrade() else {
            return Err(CacheBackendGenerationError::SharedGeneration(
                "cache backend generation recovery owner expired; refusing unsafe owner rebinding"
                    .to_string(),
            ));
        };
        if Arc::ptr_eq(&existing, identity) {
            return Ok(());
        }
        return Err(CacheBackendGenerationError::SharedGeneration(
            "cache backend generation prefix is already owned by another CacheService"
                .to_string(),
        ));
    }

    owners.insert(
        state_identity,
        GenerationRecoveryOwner {
            identity: Arc::downgrade(identity),
            state: Arc::downgrade(state),
        },
    );
    Ok(())
}

fn generation_recovery_states_for(
    identity: &Arc<dyn std::any::Any + Send + Sync>,
) -> Vec<Arc<BackendGenerationState>> {
    let owners = generation_recovery_owners()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    owners
        .values()
        .filter_map(|owner| {
            let existing = owner.identity.upgrade()?;
            Arc::ptr_eq(&existing, identity)
                .then(|| owner.state.upgrade())
                .flatten()
        })
        .collect()
}

fn unique_generation_recovery_namespace(
    state: &Arc<BackendGenerationState>,
) -> Option<String> {
    let registry = backend_generations()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut prefixes = registry.iter().filter_map(|(prefix, candidate)| {
        Arc::ptr_eq(candidate, state).then_some(prefix.clone())
    });
    let prefix = prefixes.next()?;
    if prefixes.next().is_some() {
        None
    } else {
        Some(prefix)
    }
}

struct GenerationRecoveryHealthBackend {
    inner: Arc<dyn CacheBackend>,
    state: Arc<BackendGenerationState>,
    generations: crate::CacheNamespaceGenerationStore,
    namespace: Option<String>,
    redis_client_initialized: bool,
    owner_error: Option<CacheBackendGenerationError>,
    _owner_identity: Arc<dyn std::any::Any + Send + Sync>,
}

impl CacheService {
    pub(crate) fn wrap_generation_recovery_health(
        &self,
        prefix: &str,
        inner: Arc<dyn CacheBackend>,
    ) -> Arc<dyn CacheBackend> {
        if !self.redis_configuration_present() {
            return inner;
        }

        let identity = self.generation_store_identity();
        let state = match generation_state(prefix) {
            Ok(state) => state,
            Err(error) => {
                return Arc::new(GenerationRecoveryHealthBackend {
                    inner,
                    state: Arc::new(BackendGenerationState::untrusted()),
                    generations: self.namespace_generations(),
                    namespace: None,
                    redis_client_initialized: self.redis_client_initialized(),
                    owner_error: Some(error),
                    _owner_identity: identity,
                });
            }
        };
        let owner_error = bind_generation_recovery_owner(&state, &identity).err();
        let namespace = unique_generation_recovery_namespace(&state);

        Arc::new(GenerationRecoveryHealthBackend {
            inner,
            state,
            generations: self.namespace_generations(),
            namespace,
            redis_client_initialized: self.redis_client_initialized(),
            owner_error,
            _owner_identity: identity,
        })
    }

    #[cfg(feature = "redis-cache")]
    pub(crate) async fn recover_registered_backend_generations(
        &self,
    ) -> rustok_core::Result<usize> {
        if !self.redis_configuration_present() {
            return Ok(0);
        }
        if !self.redis_client_initialized() {
            return Err(rustok_core::Error::Cache(
                "Redis is configured but its client is unavailable for cache generation recovery"
                    .to_string(),
            ));
        }

        let identity = self.generation_store_identity();
        let mut aliased = 0usize;
        let mut candidates = generation_recovery_states_for(&identity)
            .into_iter()
            .filter(|state| !state.snapshot().trusted)
            .filter_map(|state| match unique_generation_recovery_namespace(&state) {
                Some(namespace) => Some((namespace, state)),
                None => {
                    aliased = aliased.saturating_add(1);
                    None
                }
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| left.0.cmp(&right.0));

        if aliased > 0 {
            tracing::debug!(
                aliased,
                "Skipped generic recovery for aliased cache generation states"
            );
        }

        let total = candidates.len();
        let generations = self.namespace_generations();
        let reads = candidates
            .into_iter()
            .take(MAX_GENERATION_RECOVERIES_PER_PROBE)
            .map(|(namespace, state)| {
                let generations = generations.clone();
                async move {
                    let result = generations.read(&namespace).await;
                    (namespace, state, result)
                }
            });

        let mut recovered = 0usize;
        let mut first_error = None;
        for (namespace, state, result) in futures_util::future::join_all(reads).await {
            match result {
                Ok(generation)
                    if generation.source() == crate::CacheGenerationSource::SharedRedis =>
                {
                    match state.observe(generation.value()) {
                        Ok(()) => {
                            recovered = recovered.saturating_add(1);
                            tracing::info!(
                                namespace = %namespace,
                                generation = generation.value(),
                                "Recovered trusted shared cache backend generation"
                            );
                        }
                        Err(error) => first_error.get_or_insert_with(|| error.to_string()),
                    }
                }
                Ok(_) => {
                    first_error.get_or_insert_with(|| {
                        format!(
                            "cache backend generation recovery for {namespace:?} did not verify shared Redis state"
                        )
                    });
                }
                Err(error) => {
                    first_error.get_or_insert_with(|| error.to_string());
                }
            }
        }

        if let Some(error) = first_error {
            return Err(rustok_core::Error::Cache(error));
        }
        if total > MAX_GENERATION_RECOVERIES_PER_PROBE {
            return Err(rustok_core::Error::Cache(format!(
                "cache backend generation recovery remains pending for {} unique namespaces",
                total - MAX_GENERATION_RECOVERIES_PER_PROBE
            )));
        }
        Ok(recovered)
    }

    #[cfg(not(feature = "redis-cache"))]
    pub(crate) async fn recover_registered_backend_generations(
        &self,
    ) -> rustok_core::Result<usize> {
        Ok(0)
    }
}

impl GenerationRecoveryHealthBackend {
    fn error(error: impl std::fmt::Display) -> rustok_core::Error {
        rustok_core::Error::Cache(error.to_string())
    }

    async fn recover_if_needed(&self) -> rustok_core::Result<()> {
        if self.state.snapshot().trusted {
            return Ok(());
        }
        let namespace = self.namespace.as_deref().ok_or_else(|| {
            rustok_core::Error::Cache(
                "untrusted aliased cache generation requires domain-owned durable recovery"
                    .to_string(),
            )
        })?;
        let generation = self
            .generations
            .read(namespace)
            .await
            .map_err(Self::error)?;
        if generation.source() != crate::CacheGenerationSource::SharedRedis {
            return Err(rustok_core::Error::Cache(
                "cache backend generation recovery did not verify shared Redis state".to_string(),
            ));
        }
        self.state.observe(generation.value()).map_err(Self::error)?;
        tracing::info!(
            namespace = %namespace,
            generation = generation.value(),
            "Recovered trusted shared cache backend generation"
        );
        Ok(())
    }
}

#[async_trait]
impl CacheBackend for GenerationRecoveryHealthBackend {
    async fn health(&self) -> rustok_core::Result<()> {
        if let Some(error) = &self.owner_error {
            return Err(Self::error(error));
        }
        if !self.redis_client_initialized {
            return Err(rustok_core::Error::Cache(
                "Redis is configured but its client is unavailable for cache generation recovery"
                    .to_string(),
            ));
        }
        self.inner.health().await?;
        self.recover_if_needed().await
    }

    async fn get(&self, key: &str) -> rustok_core::Result<Option<Vec<u8>>> {
        self.inner.get(key).await
    }

    async fn set(&self, key: String, value: Vec<u8>) -> rustok_core::Result<()> {
        self.inner.set(key, value).await
    }

    async fn set_with_ttl(
        &self,
        key: String,
        value: Vec<u8>,
        ttl: Duration,
    ) -> rustok_core::Result<()> {
        self.inner.set_with_ttl(key, value, ttl).await
    }

    async fn compare_and_set(
        &self,
        key: &str,
        expected: &[u8],
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> rustok_core::Result<CacheCompareAndSetOutcome> {
        self.inner.compare_and_set(key, expected, value, ttl).await
    }

    async fn invalidate(&self, key: &str) -> rustok_core::Result<()> {
        self.inner.invalidate(key).await
    }

    fn stats(&self) -> CacheStats {
        self.inner.stats()
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;

    fn unique_prefix(name: &str) -> String {
        format!("test:generation-recovery:{name}:{}", Uuid::new_v4().simple())
    }

    #[test]
    fn memory_only_service_does_not_add_a_recovery_wrapper() {
        let service = CacheService::from_url(None);
        let inner = service.memory_backend(Duration::from_secs(60), 16);
        let wrapped = service.wrap_generation_recovery_health("memory-only", Arc::clone(&inner));

        assert!(Arc::ptr_eq(&inner, &wrapped));
    }

    #[cfg(feature = "redis-cache")]
    #[tokio::test]
    async fn invalid_redis_client_keeps_backend_health_degraded() {
        let service = CacheService::from_url(Some("://invalid-redis-url"));
        let prefix = unique_prefix("invalid-client");
        let inner = service.memory_backend(Duration::from_secs(60), 16);
        let wrapped = service.wrap_generation_recovery_health(&prefix, inner);

        let error = wrapped.health().await.unwrap_err().to_string();
        assert!(error.contains("client is unavailable"));
        assert!(!cache_backend_generation_snapshot(&prefix).unwrap().trusted);
    }

    #[cfg(feature = "redis-cache")]
    #[tokio::test]
    async fn different_services_cannot_claim_the_same_generation_state() {
        let prefix = unique_prefix("owner");
        let first = CacheService::from_url(Some("redis://127.0.0.1:1/"));
        let second = CacheService::from_url(Some("redis://127.0.0.1:2/"));
        let first_inner = first.memory_backend(Duration::from_secs(60), 16);
        let second_inner = second.memory_backend(Duration::from_secs(60), 16);

        let _first = first.wrap_generation_recovery_health(&prefix, first_inner);
        let second = second.wrap_generation_recovery_health(&prefix, second_inner);
        let error = second.health().await.unwrap_err().to_string();

        assert!(error.contains("another CacheService"));
    }

    #[cfg(feature = "redis-cache")]
    #[tokio::test]
    async fn aliased_untrusted_generation_requires_domain_recovery() {
        let canonical = unique_prefix("canonical");
        let alias = unique_prefix("alias");
        bind_cache_backend_generation_aliases(&canonical, &[&alias]).unwrap();
        let service = CacheService::from_url(Some("redis://127.0.0.1:1/"));
        let inner = service.memory_backend(Duration::from_secs(60), 16);
        let wrapped = service.wrap_generation_recovery_health(&alias, inner);

        let error = wrapped.health().await.unwrap_err().to_string();
        assert!(error.contains("aliased cache generation"));
    }
}
