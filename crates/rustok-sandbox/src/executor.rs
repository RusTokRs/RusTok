use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    SandboxError, SandboxExecutorKind, SandboxHost, SandboxOutcome, SandboxRequest, SandboxResult,
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

#[derive(Clone, Default)]
pub struct ExecutorRegistry {
    executors: HashMap<SandboxExecutorKind, Arc<dyn SandboxExecutor>>,
}

impl ExecutorRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<E>(&mut self, executor: E) -> SandboxResult<()>
    where
        E: SandboxExecutor + 'static,
    {
        let kind = executor.kind();
        if self.executors.contains_key(&kind) {
            return Err(SandboxError::ExecutorAlreadyRegistered(kind));
        }
        self.executors.insert(kind, Arc::new(executor));
        Ok(())
    }

    pub fn get(&self, kind: SandboxExecutorKind) -> SandboxResult<Arc<dyn SandboxExecutor>> {
        self.executors
            .get(&kind)
            .cloned()
            .ok_or(SandboxError::ExecutorNotRegistered(kind))
    }

    pub fn contains(&self, kind: SandboxExecutorKind) -> bool {
        self.executors.contains_key(&kind)
    }
}
