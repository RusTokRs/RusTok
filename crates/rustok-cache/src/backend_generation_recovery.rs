const MAX_BACKEND_GENERATION_RECOVERIES_PER_HEALTH: usize = 64;

static BACKEND_GENERATION_RECOVERY_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

fn backend_generation_recovery_lock() -> &'static tokio::sync::Mutex<()> {
    BACKEND_GENERATION_RECOVERY_LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

impl CacheService {
    /// Recover process-isolated backend generations after Redis becomes reachable.
    ///
    /// Only states owned by exactly one prefix are recovered here. Multiple prefixes sharing one
    /// state are aliases whose canonical durable generation belongs to the owning domain runtime;
    /// choosing one alias from hash-map iteration would risk trusting the wrong namespace.
    pub(crate) async fn recover_untrusted_backend_generations(
        &self,
    ) -> Result<usize, CacheBackendGenerationError> {
        if !self.redis_configuration_present() {
            return Ok(0);
        }
        if !self.redis_client_initialized() {
            return Err(CacheBackendGenerationError::RedisClientUnavailable);
        }

        let _recovery_guard = backend_generation_recovery_lock().lock().await;
        let mut grouped: HashMap<usize, Vec<(String, Arc<BackendGenerationState>)>> = HashMap::new();
        {
            let registry = backend_generations()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for (prefix, state) in registry.iter() {
                if state.trusted.load(Ordering::Acquire) {
                    continue;
                }
                let identity = Arc::as_ptr(state) as usize;
                grouped
                    .entry(identity)
                    .or_default()
                    .push((prefix.clone(), Arc::clone(state)));
            }
        }

        let mut candidates = Vec::new();
        let mut aliased_groups = 0usize;
        for mut group in grouped.into_values() {
            if group.len() == 1 {
                candidates.push(group.pop().expect("single generation recovery candidate"));
            } else {
                aliased_groups = aliased_groups.saturating_add(1);
            }
        }
        candidates.sort_by(|left, right| left.0.cmp(&right.0));

        if aliased_groups > 0 {
            tracing::debug!(
                aliased_groups,
                "Skipped generic cache generation recovery for aliased states"
            );
        }

        let total_candidates = candidates.len();
        let generations = self.namespace_generations();
        let mut recovered = 0usize;
        for (prefix, state) in candidates
            .into_iter()
            .take(MAX_BACKEND_GENERATION_RECOVERIES_PER_HEALTH)
        {
            if state.trusted.load(Ordering::Acquire) {
                continue;
            }
            let generation = generations.read(&prefix).await?;
            state.observe(generation.value())?;
            recovered = recovered.saturating_add(1);
            tracing::info!(
                prefix,
                generation = generation.value(),
                source = ?generation.source(),
                "Recovered trusted cache backend generation"
            );
        }

        if total_candidates > MAX_BACKEND_GENERATION_RECOVERIES_PER_HEALTH {
            return Err(CacheBackendGenerationError::SharedGeneration(format!(
                "cache backend generation recovery remains pending for {} unique prefixes",
                total_candidates - MAX_BACKEND_GENERATION_RECOVERIES_PER_HEALTH
            )));
        }

        Ok(recovered)
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;

    #[tokio::test]
    async fn memory_only_service_has_no_shared_generation_recovery_work() {
        let service = CacheService::from_url(None);
        assert_eq!(
            service
                .recover_untrusted_backend_generations()
                .await
                .unwrap(),
            0
        );
    }

    #[cfg(feature = "redis-cache")]
    #[tokio::test]
    async fn invalid_redis_configuration_fails_generation_recovery_closed() {
        let service = CacheService::from_url(Some("://invalid-redis-url"));
        assert!(matches!(
            service.recover_untrusted_backend_generations().await,
            Err(CacheBackendGenerationError::RedisClientUnavailable)
        ));
    }
}
