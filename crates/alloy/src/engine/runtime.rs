use std::sync::Arc;

use rhai::{Dynamic, Engine, RhaiNativeFunc, Scope};
use rustok_sandbox::rhai::{CompiledRhai, RhaiConfig, RhaiEngine};

use crate::context::ExecutionContext;
use crate::error::{ScriptError, ScriptResult};

pub struct CompiledScript(Arc<CompiledRhai>);

/// Alloy-specific adapter over the neutral Rhai execution kernel.
///
/// This type owns Alloy source compilation for CRUD validation. Production
/// execution is performed only by `AlloyDraftRuntime` through `SandboxRuntime`.
pub struct ScriptEngine {
    inner: RhaiEngine,
}

impl ScriptEngine {
    pub fn new(config: RhaiConfig) -> Self {
        Self {
            inner: RhaiEngine::new(config),
        }
    }

    pub fn register_fn<A, const N: usize, const X: bool, R, const F: bool>(
        &mut self,
        name: &str,
        function: impl RhaiNativeFunc<A, N, X, R, F> + Send + Sync + 'static,
    ) where
        A: 'static,
        R: 'static + Clone + Send + Sync,
    {
        self.inner.register_fn(name, function);
    }

    pub fn register_type<T: Clone + Send + Sync + 'static>(&mut self, name: &str) {
        self.inner.register_type::<T>(name);
    }

    pub fn engine_mut(&mut self) -> &mut Engine {
        self.inner.engine_mut()
    }

    pub fn compile(
        &self,
        name: &str,
        source: &str,
        scope: &mut Scope,
    ) -> ScriptResult<Arc<CompiledScript>> {
        self.inner
            .compile(name, source, scope)
            .map(|compiled| Arc::new(CompiledScript(compiled)))
            .map_err(ScriptError::from)
    }

    #[cfg(test)]
    pub fn execute(
        &self,
        name: &str,
        source: &str,
        context: &ExecutionContext,
    ) -> ScriptResult<Dynamic> {
        self.inner
            .execute(name, source, context)
            .map_err(ScriptError::from)
    }

    #[cfg(test)]
    pub fn execute_compiled(
        &self,
        compiled: &CompiledScript,
        context: &ExecutionContext,
    ) -> ScriptResult<Dynamic> {
        self.inner
            .execute_compiled(&compiled.0, context)
            .map_err(ScriptError::from)
    }

    pub fn invalidate(&self, name: &str) {
        self.inner.invalidate(name);
    }

    pub fn invalidate_all(&self) {
        self.inner.invalidate_all();
    }

    pub fn config(&self) -> &RhaiConfig {
        self.inner.config()
    }
}
