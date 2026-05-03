use parking_lot::RwLock;
use rhai::{Dynamic, Engine, EvalAltResult, RhaiNativeFunc, Scope, AST};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use crate::context::ExecutionContext;
use crate::error::{ScriptError, ScriptResult};

use super::config::EngineConfig;

pub struct CompiledScript {
    ast: AST,
    source_hash: u64,
}

pub struct ScriptEngine {
    engine: Engine,
    config: EngineConfig,
    cache: RwLock<HashMap<String, Arc<CompiledScript>>>,
}

impl ScriptEngine {
    pub fn new(config: EngineConfig) -> Self {
        let mut engine = Engine::new();

        engine.set_allow_looping(true);
        engine.set_allow_shadowing(true);
        engine.set_strict_variables(true);

        Self {
            engine,
            config,
            cache: RwLock::new(HashMap::new()),
        }
    }

    fn compute_hash(source: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        source.hash(&mut hasher);
        hasher.finish()
    }

    /// Register a native function with Rhai using the required trait bounds.
    pub fn register_fn<A, const N: usize, const X: bool, R, const F: bool>(
        &mut self,
        name: &str,
        func: impl RhaiNativeFunc<A, N, X, R, F> + Send + Sync + 'static,
    ) where
        A: 'static,
        R: 'static + Clone + Send + Sync,
    {
        self.engine.register_fn(name, func);
    }

    pub fn register_type<T: Clone + Send + Sync + 'static>(&mut self, name: &str) {
        self.engine.register_type_with_name::<T>(name);
    }

    pub fn engine_mut(&mut self) -> &mut Engine {
        &mut self.engine
    }

    pub fn compile(
        &self,
        name: &str,
        source: &str,
        scope: &mut Scope,
    ) -> ScriptResult<Arc<CompiledScript>> {
        let source_hash = Self::compute_hash(source);

        {
            let cache = self.cache.read();
            if let Some(compiled) = cache.get(name) {
                if compiled.source_hash == source_hash {
                    return Ok(Arc::clone(compiled));
                }
            }
        }

        let ast = self
            .engine
            .compile_with_scope(scope, source)
            .map_err(|e| ScriptError::Compilation(e.to_string()))?;

        let compiled = Arc::new(CompiledScript { ast, source_hash });

        let mut cache = self.cache.write();
        cache.insert(name.to_string(), Arc::clone(&compiled));

        Ok(compiled)
    }

    pub fn invalidate(&self, name: &str) {
        let mut cache = self.cache.write();
        cache.remove(name);
    }

    pub fn invalidate_all(&self) {
        let mut cache = self.cache.write();
        cache.clear();
    }

    pub fn execute(
        &self,
        name: &str,
        source: &str,
        ctx: &ExecutionContext,
    ) -> ScriptResult<Dynamic> {
        let mut scope = ctx.to_scope();
        let compiled = self.compile(name, source, &mut scope)?;
        self.execute_compiled_with_timeout(&compiled, scope)
    }

    pub fn execute_compiled(
        &self,
        compiled: &CompiledScript,
        ctx: &ExecutionContext,
    ) -> ScriptResult<Dynamic> {
        let scope = ctx.to_scope();
        self.execute_compiled_with_timeout(compiled, scope)
    }

    fn execute_compiled_with_timeout(
        &self,
        compiled: &CompiledScript,
        mut scope: Scope,
    ) -> ScriptResult<Dynamic> {
        let timeout = self.config.timeout;
        let max_ops = self.config.max_operations;
        let start = Instant::now();

        let result = self
            .engine
            .eval_ast_with_scope::<Dynamic>(&mut scope, &compiled.ast);

        let elapsed = start.elapsed();
        if elapsed > timeout {
            tracing::warn!(
                "Script execution took {}ms, exceeding timeout of {}ms",
                elapsed.as_millis(),
                timeout.as_millis()
            );
        }

        result.map_err(|e| Self::convert_error(*e, max_ops))
    }

    fn convert_error(err: EvalAltResult, op_limit: u64) -> ScriptError {
        match err {
            EvalAltResult::ErrorTerminated(reason, _) => ScriptError::Aborted(reason.to_string()),
            EvalAltResult::ErrorTooManyOperations(_) => {
                ScriptError::OperationLimit { limit: op_limit }
            }
            EvalAltResult::ErrorRuntime(msg, _) => {
                let msg_str = msg.to_string();
                if msg_str.starts_with("ABORT:") {
                    ScriptError::Aborted(msg_str.trim_start_matches("ABORT:").trim().to_string())
                } else {
                    ScriptError::Runtime(msg_str)
                }
            }
            other => ScriptError::Runtime(other.to_string()),
        }
    }

    pub fn config(&self) -> &EngineConfig {
        &self.config
    }
}
