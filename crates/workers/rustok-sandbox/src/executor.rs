use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    SandboxError, SandboxExecutorKind, SandboxExecutorPlacement, SandboxHost, SandboxOutcome,
    SandboxRequest, SandboxResult,
};

#[async_trait]
pub trait SandboxExecutor: Send + Sync {
    fn kind(&self) -> SandboxExecutorKind;

    async fn execute(
        &self,
        request: &SandboxRequest,
        host: SandboxHost,
    ) -> SandboxResult<SandboxOutcome>;
}

#[derive(Clone)]
struct ExecutorRegistration {
    placement: SandboxExecutorPlacement,
    executor: Arc<dyn SandboxExecutor>,
}

#[derive(Clone, Default)]
pub struct ExecutorRegistry {
    executors: HashMap<SandboxExecutorKind, ExecutorRegistration>,
}

impl ExecutorRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an executor that shares the caller process. Production hosts
    /// must use this only where the executor threat model explicitly permits
    /// in-process placement.
    pub fn register_in_process<E>(&mut self, executor: E) -> SandboxResult<()>
    where
        E: SandboxExecutor + 'static,
    {
        self.register(SandboxExecutorPlacement::InProcess, executor)
    }

    /// Registers an executor whose implementation crosses the supervised
    /// isolated-worker transport. This placement is metadata, not a fallback:
    /// duplicate kinds remain rejected across both placements.
    pub fn register_isolated_worker<E>(&mut self, executor: E) -> SandboxResult<()>
    where
        E: SandboxExecutor + 'static,
    {
        self.register(SandboxExecutorPlacement::IsolatedWorker, executor)
    }

    fn register<E>(&mut self, placement: SandboxExecutorPlacement, executor: E) -> SandboxResult<()>
    where
        E: SandboxExecutor + 'static,
    {
        let kind = executor.kind();
        if self.executors.contains_key(&kind) {
            return Err(SandboxError::ExecutorAlreadyRegistered(kind));
        }
        self.executors.insert(
            kind,
            ExecutorRegistration {
                placement,
                executor: Arc::new(executor),
            },
        );
        Ok(())
    }

    pub fn get(&self, kind: SandboxExecutorKind) -> SandboxResult<Arc<dyn SandboxExecutor>> {
        self.executors
            .get(&kind)
            .map(|registration| Arc::clone(&registration.executor))
            .ok_or(SandboxError::ExecutorNotRegistered(kind))
    }

    pub fn placement(&self, kind: SandboxExecutorKind) -> SandboxResult<SandboxExecutorPlacement> {
        self.executors
            .get(&kind)
            .map(|registration| registration.placement)
            .ok_or(SandboxError::ExecutorNotRegistered(kind))
    }

    pub fn contains(&self, kind: SandboxExecutorKind) -> bool {
        self.executors.contains_key(&kind)
    }
}
