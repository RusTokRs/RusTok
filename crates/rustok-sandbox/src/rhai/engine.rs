use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use ::rhai;
use parking_lot::RwLock;
use rhai::{
    AST, Dynamic, Engine, EvalAltResult, LexError, ParseError, ParseErrorType, RhaiNativeFunc,
    Scope,
};

use super::{RhaiConfig, RhaiError, RhaiResult};

const TIMEOUT_MARKER: &str = "__RUSTOK_SANDBOX_TIMEOUT__";

thread_local! {
    static EXECUTION_STARTED: RefCell<Option<Instant>> = const { RefCell::new(None) };
}

struct ExecutionTimerGuard {
    previous: Option<Instant>,
}

impl ExecutionTimerGuard {
    fn start() -> Self {
        let previous = EXECUTION_STARTED.with(|started| started.replace(Some(Instant::now())));
        Self { previous }
    }
}

impl Drop for ExecutionTimerGuard {
    fn drop(&mut self) {
        EXECUTION_STARTED.with(|started| {
            started.replace(self.previous.take());
        });
    }
}

pub trait RhaiScopeProvider {
    fn rhai_scope(&self) -> Scope<'static>;
}

pub struct CompiledRhai {
    ast: AST,
    source_hash: u64,
}

pub struct RhaiEngine {
    engine: Engine,
    config: RhaiConfig,
    cache: RwLock<HashMap<String, Arc<CompiledRhai>>>,
}

impl RhaiEngine {
    pub fn new(config: RhaiConfig) -> Self {
        let mut engine = Engine::new();
        let timeout = config.timeout;
        engine.set_allow_looping(true);
        engine.set_allow_shadowing(true);
        engine.set_strict_variables(true);
        engine.set_max_operations(config.max_operations);
        engine.set_max_call_levels(config.max_call_depth);
        engine.set_max_string_size(config.max_string_size);
        engine.set_max_array_size(config.max_array_size);
        engine.set_max_map_size(config.max_map_size);
        engine.on_progress(move |_| {
            EXECUTION_STARTED.with(|started| {
                started
                    .borrow()
                    .as_ref()
                    .is_some_and(|started| started.elapsed() > timeout)
                    .then(|| Dynamic::from(TIMEOUT_MARKER))
            })
        });

        Self {
            engine,
            config,
            cache: RwLock::new(HashMap::new()),
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
        self.engine.register_fn(name, function);
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
    ) -> RhaiResult<Arc<CompiledRhai>> {
        let source_hash = source_hash(source);
        if let Some(compiled) = self.cache.read().get(name) {
            if compiled.source_hash == source_hash {
                return Ok(Arc::clone(compiled));
            }
        }

        let ast = self
            .engine
            .compile_with_scope(scope, source)
            .map_err(convert_compile_error)?;
        let compiled = Arc::new(CompiledRhai { ast, source_hash });
        self.cache
            .write()
            .insert(name.to_string(), Arc::clone(&compiled));
        Ok(compiled)
    }

    pub fn execute<P: RhaiScopeProvider>(
        &self,
        name: &str,
        source: &str,
        context: &P,
    ) -> RhaiResult<Dynamic> {
        let mut scope = context.rhai_scope();
        let compiled = self.compile(name, source, &mut scope)?;
        self.execute_compiled_scope(&compiled, scope)
    }

    pub fn execute_compiled<P: RhaiScopeProvider>(
        &self,
        compiled: &CompiledRhai,
        context: &P,
    ) -> RhaiResult<Dynamic> {
        self.execute_compiled_scope(compiled, context.rhai_scope())
    }

    fn execute_compiled_scope(
        &self,
        compiled: &CompiledRhai,
        mut scope: Scope,
    ) -> RhaiResult<Dynamic> {
        let timer = ExecutionTimerGuard::start();
        let result = self
            .engine
            .eval_ast_with_scope::<Dynamic>(&mut scope, &compiled.ast);
        drop(timer);
        result.map_err(|error| {
            convert_error(
                *error,
                self.config.max_operations,
                self.config
                    .timeout
                    .as_millis()
                    .try_into()
                    .unwrap_or(u64::MAX),
            )
        })
    }

    pub fn invalidate(&self, name: &str) {
        self.cache.write().remove(name);
    }

    pub fn invalidate_all(&self) {
        self.cache.write().clear();
    }

    pub fn config(&self) -> &RhaiConfig {
        &self.config
    }
}

fn source_hash(source: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}

fn convert_error(error: EvalAltResult, operation_limit: u64, timeout_ms: u64) -> RhaiError {
    match error {
        EvalAltResult::ErrorTerminated(reason, _) if reason.to_string() == TIMEOUT_MARKER => {
            RhaiError::Timeout {
                limit_ms: timeout_ms,
            }
        }
        EvalAltResult::ErrorTerminated(reason, _) => RhaiError::Aborted(reason.to_string()),
        EvalAltResult::ErrorTooManyOperations(_) => RhaiError::OperationLimit {
            limit: operation_limit,
        },
        EvalAltResult::ErrorDataTooLarge(resource, _) => RhaiError::ResourceLimit { resource },
        EvalAltResult::ErrorRuntime(message, _) => {
            let message = message.to_string();
            if message.starts_with("ABORT:") {
                RhaiError::Aborted(message.trim_start_matches("ABORT:").trim().to_string())
            } else {
                RhaiError::Runtime(message)
            }
        }
        other => RhaiError::Runtime(other.to_string()),
    }
}

fn convert_compile_error(error: ParseError) -> RhaiError {
    match error.err_type() {
        ParseErrorType::BadInput(LexError::StringTooLong(_)) => RhaiError::ResourceLimit {
            resource: "string_length".to_string(),
        },
        _ => RhaiError::Compilation(error.to_string()),
    }
}
