//! Rhai executor adapter for the neutral sandbox runtime.

mod config;
mod engine;
mod error;

pub use config::{RhaiConfig, RhaiLimits};
pub use engine::{CompiledRhai, RhaiEngine, RhaiScopeProvider};
pub use error::{RhaiError, RhaiResult};

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use ::rhai;
use async_trait::async_trait;
use rhai::{Dynamic, Engine, EvalAltResult, Map, Scope};
use serde_json::Value;

use crate::{
    CapabilityCall, CapabilityCallContext, CapabilityName, ExecutionMetrics, RhaiBindingInput,
    RhaiBindingOutput, SandboxError, SandboxExecutor, SandboxExecutorKind, SandboxHost,
    SandboxOutcome, SandboxRequest, SandboxResult,
};

const TIMEOUT_MARKER: &str = "__RUSTOK_SANDBOX_TIMEOUT__";
const CANCELLATION_MARKER: &str = "__RUSTOK_SANDBOX_CANCELLED__";

/// Executes pure Rhai payloads under the common sandbox limits.
///
/// Host functions are intentionally absent from this baseline executor. Consumers
/// must add broker-backed capabilities through an approved adapter rather than
/// registering direct network, storage or secret access.
pub struct RhaiExecutor {
    extensions: Vec<Arc<dyn RhaiHostExtension>>,
}

/// Language-specific adapter boundary for broker-backed host capabilities.
///
/// The sandbox remains independent from application capabilities. An adapter
/// can register Rhai functions for one request only, capturing the request's
/// `SandboxHost` and typed subject rather than opening direct infrastructure
/// access from script code.
pub trait RhaiHostExtension: Send + Sync {
    fn register(
        &self,
        engine: &mut Engine,
        request: &SandboxRequest,
        host: SandboxHost,
    ) -> SandboxResult<()>;

    /// Optionally resolves executable Rhai source from an owner-defined,
    /// immutable payload representation. The sandbox never materializes this
    /// representation on a guest filesystem. At most one extension may supply
    /// replacement source for a request.
    fn source_bytes(&self, _request: &SandboxRequest) -> SandboxResult<Option<Vec<u8>>> {
        Ok(None)
    }

    /// Adds request-scoped data to the Rhai scope after the neutral baseline
    /// context has been populated. Extensions must not keep data in a shared
    /// engine because a sandbox request may execute concurrently with another.
    fn populate_scope(
        &self,
        _scope: &mut Scope<'static>,
        _request: &SandboxRequest,
    ) -> SandboxResult<()> {
        Ok(())
    }

    /// Converts a successful Rhai value into the extension's public output
    /// binding. The scope is still available so adapters can extract bounded
    /// request-scoped state such as brokered entity changes.
    fn map_output(
        &self,
        _scope: &mut Scope<'static>,
        _request: &SandboxRequest,
        output: Value,
    ) -> SandboxResult<Value> {
        Ok(output)
    }
}

/// Neutral Rhai bridge for every brokered host capability. It exposes only
/// `capability_call(name, operation, input)` and forwards the request through
/// the current [`SandboxHost`]; extensions cannot give Rhai direct access to
/// network, filesystem, database, or credential clients.
#[derive(Debug, Default)]
pub struct RhaiCapabilityBridge;

impl RhaiHostExtension for RhaiCapabilityBridge {
    fn register(
        &self,
        engine: &mut Engine,
        request: &SandboxRequest,
        host: SandboxHost,
    ) -> SandboxResult<()> {
        let context = RhaiCapabilityContext::from_request(request);
        engine.register_fn(
            "capability_call",
            move |name: &str, operation: &str, input: Dynamic| {
                invoke_capability(&host, &context, name, operation, dynamic_to_json(input))
            },
        );
        Ok(())
    }
}

#[derive(Clone)]
struct RhaiCapabilityContext {
    execution_id: uuid::Uuid,
    subject: crate::SandboxSubject,
    context: CapabilityCallContext,
}

impl RhaiCapabilityContext {
    fn from_request(request: &SandboxRequest) -> Self {
        Self {
            execution_id: request.context.execution_id,
            subject: request.subject.clone(),
            context: CapabilityCallContext::from(&request.context),
        }
    }
}

fn invoke_capability(
    host: &SandboxHost,
    context: &RhaiCapabilityContext,
    name: &str,
    operation: &str,
    input: Value,
) -> Map {
    let capability = match CapabilityName::new(name) {
        Ok(capability) => capability,
        Err(error) => return capability_error_map(error),
    };
    let call = CapabilityCall {
        execution_id: context.execution_id,
        subject: context.subject.clone(),
        context: context.context.clone(),
        capability,
        operation: operation.to_string(),
        input,
    };
    match host.invoke_blocking(&call) {
        Ok(response) => capability_response_map(response.output),
        Err(error) => capability_error_map(error),
    }
}

fn capability_response_map(output: Value) -> Map {
    let mut response = Map::new();
    response.insert("ok".into(), Dynamic::from(true));
    response.insert("output".into(), json_to_dynamic(&output));
    response
}

fn capability_error_map(error: SandboxError) -> Map {
    let mut response = Map::new();
    response.insert("ok".into(), Dynamic::from(false));
    response.insert("error_code".into(), Dynamic::from(error.code().to_string()));
    response
}

impl RhaiExecutor {
    pub fn new() -> Self {
        Self {
            extensions: Vec::new(),
        }
    }

    pub fn with_extension(mut self, extension: Arc<dyn RhaiHostExtension>) -> Self {
        self.extensions.push(extension);
        self
    }

    fn build_engine(
        request: &SandboxRequest,
        operations: Arc<AtomicU64>,
        host: SandboxHost,
    ) -> Engine {
        let mut engine = Engine::new();
        let limits = request.policy.limits;
        let started = Instant::now();

        engine.set_allow_looping(true);
        engine.set_allow_shadowing(true);
        engine.set_strict_variables(true);
        engine.set_max_operations(limits.instruction_budget);
        engine.set_max_call_levels(16);
        engine.set_max_string_size(limits.max_output_bytes.try_into().unwrap_or(usize::MAX));
        engine.set_max_array_size(10_000);
        engine.set_max_map_size(10_000);
        engine.on_progress(move |count| {
            operations.store(count, Ordering::Relaxed);
            if host.cancellation().is_cancelled() {
                Some(Dynamic::from(CANCELLATION_MARKER))
            } else {
                (started.elapsed().as_millis() > u128::from(limits.wall_clock_ms))
                    .then(|| Dynamic::from(TIMEOUT_MARKER))
            }
        });
        engine
    }

    fn build_scope(request: &SandboxRequest, input: &Value) -> Scope<'static> {
        let mut scope = Scope::new();
        scope.push_constant("EXECUTION_ID", request.context.execution_id.to_string());
        scope.push_constant("PHASE", format!("{:?}", request.context.phase));
        scope.push_constant("TIMESTAMP", request.context.timestamp.to_rfc3339());
        if let Some(tenant_id) = request.context.tenant_id {
            scope.push_constant("TENANT_ID", tenant_id.to_string());
        }
        if let Some(actor_id) = &request.context.actor_id {
            scope.push_constant("ACTOR_ID", actor_id.clone());
        }
        scope.push_constant("input", json_to_dynamic(input));
        scope
    }

    fn map_error(error: EvalAltResult, request: &SandboxRequest) -> SandboxError {
        match error {
            EvalAltResult::ErrorTerminated(reason, _)
                if reason.to_string() == CANCELLATION_MARKER =>
            {
                SandboxError::Cancelled
            }
            EvalAltResult::ErrorTerminated(reason, _) if reason.to_string() == TIMEOUT_MARKER => {
                SandboxError::Timeout {
                    limit_ms: request.policy.limits.wall_clock_ms,
                }
            }
            EvalAltResult::ErrorTooManyOperations(_) => SandboxError::LimitExceeded {
                resource: "instructions".to_string(),
                limit: request.policy.limits.instruction_budget,
            },
            EvalAltResult::ErrorDataTooLarge(resource, _) => SandboxError::LimitExceeded {
                resource,
                limit: request.policy.limits.max_memory_bytes,
            },
            EvalAltResult::ErrorTerminated(reason, _) => SandboxError::Aborted(reason.to_string()),
            EvalAltResult::ErrorRuntime(message, _)
                if message.to_string().starts_with("ABORT:") =>
            {
                SandboxError::Aborted(
                    message
                        .to_string()
                        .trim_start_matches("ABORT:")
                        .trim()
                        .to_string(),
                )
            }
            other => SandboxError::Trap(other.to_string()),
        }
    }
}

impl Default for RhaiExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SandboxExecutor for RhaiExecutor {
    fn kind(&self) -> SandboxExecutorKind {
        SandboxExecutorKind::Rhai
    }

    async fn execute(
        &self,
        request: &SandboxRequest,
        host: SandboxHost,
    ) -> SandboxResult<SandboxOutcome> {
        let binding = RhaiBindingInput::decode(request.input.clone())
            .map_err(|error| SandboxError::InvalidRequest(error.to_string()))?;
        let operations = Arc::new(AtomicU64::new(0));
        let mut engine = Self::build_engine(request, Arc::clone(&operations), host.clone());
        for extension in &self.extensions {
            extension.register(&mut engine, request, host.clone())?;
        }
        let mut resolved_source = None;
        for extension in &self.extensions {
            if let Some(source) = extension.source_bytes(request)? {
                if resolved_source.replace(source).is_some() {
                    return Err(SandboxError::InvalidRequest(
                        "multiple Rhai extensions supplied request source".to_string(),
                    ));
                }
            }
        }
        let source = resolved_source.unwrap_or_else(|| request.payload.bytes.clone());
        let source = std::str::from_utf8(&source)
            .map_err(|error| SandboxError::Compilation(error.to_string()))?;
        let mut scope = Self::build_scope(request, &binding.input);
        for extension in &self.extensions {
            extension.populate_scope(&mut scope, request)?;
        }
        let mut ast = engine
            .compile_with_scope(&scope, source)
            .map_err(|error| SandboxError::Compilation(error.to_string()))?;
        ast.set_source(&request.payload.entrypoint);
        let output = engine
            .eval_ast_with_scope::<Dynamic>(&mut scope, &ast)
            .map_err(|error| Self::map_error(*error, request))?;
        let mut output = dynamic_to_json(output);
        for extension in &self.extensions {
            output = extension.map_output(&mut scope, request, output)?;
        }
        let output = serde_json::to_value(RhaiBindingOutput::new(output))
            .map_err(|error| SandboxError::Internal(error.to_string()))?;
        let output_bytes = serde_json::to_vec(&output)
            .map_err(|error| SandboxError::Internal(error.to_string()))?
            .len() as u64;
        if output_bytes > request.policy.limits.max_output_bytes {
            return Err(SandboxError::LimitExceeded {
                resource: "output_bytes".to_string(),
                limit: request.policy.limits.max_output_bytes,
            });
        }

        Ok(SandboxOutcome {
            execution_id: request.context.execution_id,
            output,
            metrics: ExecutionMetrics {
                instructions_consumed: Some(operations.load(Ordering::Relaxed)),
                output_bytes: Some(output_bytes),
                ..Default::default()
            },
        })
    }
}

fn json_to_dynamic(value: &Value) -> Dynamic {
    match value {
        Value::Null => Dynamic::UNIT,
        Value::Bool(value) => Dynamic::from(*value),
        Value::Number(value) => value
            .as_i64()
            .map(Dynamic::from)
            .or_else(|| value.as_f64().map(Dynamic::from))
            .unwrap_or(Dynamic::UNIT),
        Value::String(value) => Dynamic::from(value.clone()),
        Value::Array(values) => Dynamic::from_array(values.iter().map(json_to_dynamic).collect()),
        Value::Object(values) => {
            let map: Map = values
                .iter()
                .map(|(key, value)| (key.clone().into(), json_to_dynamic(value)))
                .collect();
            Dynamic::from_map(map)
        }
    }
}

fn dynamic_to_json(value: Dynamic) -> Value {
    if value.is_unit() {
        Value::Null
    } else if value.is::<bool>() {
        Value::Bool(value.cast::<bool>())
    } else if value.is::<i64>() {
        Value::from(value.cast::<i64>())
    } else if value.is::<f64>() {
        serde_json::Number::from_f64(value.cast::<f64>())
            .map(Value::Number)
            .unwrap_or(Value::Null)
    } else if value.is::<String>() {
        Value::String(value.cast::<String>())
    } else if value.is_array() {
        Value::Array(
            value
                .cast::<rhai::Array>()
                .into_iter()
                .map(dynamic_to_json)
                .collect(),
        )
    } else if value.is_map() {
        Value::Object(
            value
                .cast::<Map>()
                .into_iter()
                .map(|(key, value)| (key.to_string(), dynamic_to_json(value)))
                .collect(),
        )
    } else {
        Value::String(value.to_string())
    }
}
