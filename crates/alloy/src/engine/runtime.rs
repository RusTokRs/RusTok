use crate::error::{ScriptError, ScriptResult};
use rhai::{Engine, RhaiNativeFunc, Scope};
use rustok_sandbox::rhai::{RhaiConfig, RhaiEngine};

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

    pub fn compile(&self, name: &str, source: &str, scope: &mut Scope) -> ScriptResult<()> {
        self.inner
            .compile(name, source, scope)
            .map(|_| ())
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
