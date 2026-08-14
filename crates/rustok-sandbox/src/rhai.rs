//! Rhai executor adapter for the neutral sandbox runtime.

mod config;
mod engine;
mod error;

pub use config::{RhaiConfig, RhaiLimits};
pub use engine::{CompiledRhai, RhaiEngine, RhaiScopeProvider};
pub use error::{RhaiError, RhaiResult};

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use ::rhai;
use async_trait::async_trait;
use parking_lot::RwLock;
use rhai::{CustomType, Dynamic, Engine, EvalAltResult, Map, Scope, TypeBuilder};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tracing::{error, info, warn};

use crate::{
    CapabilityCall, CapabilityCallContext, CapabilityName, ExecutionMetrics,
    RHAI_SANDBOX_RUNTIME_ABI, RHAI_SOURCE_MEDIA_TYPE, RHAI_WORKSPACE_MEDIA_TYPE, RhaiBindingInput,
    RhaiBindingOutput, RhaiRecordInput, RhaiScopeOutput, RhaiWorkspace, SandboxError,
    SandboxExecutor, SandboxExecutorKind, SandboxHost, SandboxOutcome, SandboxRequest,
    SandboxResult,
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

/// Local preparation result for an immutable Rhai artifact. It proves syntax
/// and workspace resolution only; it never executes guest code or grants a
/// capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RhaiArtifactPreparation {
    pub runtime_fingerprint: String,
}

#[derive(Debug, Clone)]
struct RhaiRecord {
    id: String,
    record_type: String,
    state: Arc<RwLock<RhaiRecordState>>,
}

#[derive(Debug)]
struct RhaiRecordState {
    fields: HashMap<String, Dynamic>,
    changes: HashMap<String, Dynamic>,
}

impl RhaiRecord {
    fn from_input(input: &RhaiRecordInput) -> SandboxResult<Self> {
        let fields = input
            .fields
            .as_object()
            .ok_or_else(|| {
                SandboxError::InvalidRequest(
                    "Rhai scope record fields must be a JSON object".to_string(),
                )
            })?
            .iter()
            .map(|(key, value)| (key.clone(), json_to_dynamic(value)))
            .collect();
        Ok(Self {
            id: input.id.clone(),
            record_type: input.record_type.clone(),
            state: Arc::new(RwLock::new(RhaiRecordState {
                fields,
                changes: HashMap::new(),
            })),
        })
    }

    fn get(&self, field: &str) -> Dynamic {
        let state = self.state.read();
        state
            .changes
            .get(field)
            .or_else(|| state.fields.get(field))
            .cloned()
            .unwrap_or(Dynamic::UNIT)
    }

    fn set(&mut self, field: &str, value: Dynamic) {
        self.state.write().changes.insert(field.to_string(), value);
    }

    fn is_changed(&self, field: &str) -> bool {
        self.state.read().changes.contains_key(field)
    }

    fn has_changes(&self) -> bool {
        !self.state.read().changes.is_empty()
    }

    fn snapshot(&self) -> HashMap<String, Dynamic> {
        let state = self.state.read();
        let mut snapshot = state.fields.clone();
        snapshot.extend(state.changes.clone());
        snapshot
    }

    fn changes_json(&self) -> Value {
        Value::Object(
            self.state
                .read()
                .changes
                .iter()
                .map(|(key, value)| (key.clone(), dynamic_to_json(value.clone())))
                .collect(),
        )
    }
}

impl CustomType for RhaiRecord {
    fn build(mut builder: TypeBuilder<Self>) {
        builder
            .with_name("Record")
            .with_get("id", |record: &mut RhaiRecord| record.id.clone())
            .with_get("type", |record: &mut RhaiRecord| record.record_type.clone())
            .with_indexer_get(|record: &mut RhaiRecord, key: &str| record.get(key))
            .with_indexer_set(|record: &mut RhaiRecord, key: &str, value: Dynamic| {
                record.set(key, value);
            })
            .with_fn("is_changed", |record: &mut RhaiRecord, field: &str| {
                record.is_changed(field)
            })
            .with_fn("has_changes", |record: &mut RhaiRecord| {
                record.has_changes()
            })
            .with_fn("snapshot", |record: &mut RhaiRecord| record.snapshot());
    }
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
        let capability_host = host.clone();
        let capability_context = context.clone();
        engine.register_fn(
            "capability_call",
            move |name: &str, operation: &str, input: Dynamic| {
                invoke_capability(
                    &capability_host,
                    &capability_context,
                    name,
                    operation,
                    dynamic_to_json(input),
                )
            },
        );
        register_http_functions(engine, host, context);
        Ok(())
    }
}

/// Registers the deterministic, infrastructure-free Rhai standard library used
/// by both draft and admitted artifact execution.
#[derive(Debug, Default)]
pub struct RhaiStandardLibrary;

impl RhaiHostExtension for RhaiStandardLibrary {
    fn register(
        &self,
        engine: &mut Engine,
        request: &SandboxRequest,
        _host: SandboxHost,
    ) -> SandboxResult<()> {
        register_standard_library(engine, request.context.phase);
        Ok(())
    }
}

pub fn register_standard_library(engine: &mut Engine, phase: crate::ExecutionPhase) {
    register_standard_functions(engine);
    if validation_helpers_enabled(phase) {
        register_validation_functions(engine);
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

fn register_http_functions(engine: &mut Engine, host: SandboxHost, context: RhaiCapabilityContext) {
    let get_host = host.clone();
    let get_context = context.clone();
    engine.register_fn("http_get", move |url: &str| {
        invoke_http(&get_host, &get_context, "GET", url, Value::Null, Map::new())
    });

    let get_headers_host = host.clone();
    let get_headers_context = context.clone();
    engine.register_fn("http_get", move |url: &str, headers: Map| {
        invoke_http(
            &get_headers_host,
            &get_headers_context,
            "GET",
            url,
            Value::Null,
            headers,
        )
    });

    let post_host = host.clone();
    let post_context = context.clone();
    engine.register_fn("http_post", move |url: &str, body: Dynamic| {
        invoke_http(
            &post_host,
            &post_context,
            "POST",
            url,
            dynamic_to_json(body),
            Map::new(),
        )
    });

    let post_headers_host = host.clone();
    let post_headers_context = context.clone();
    engine.register_fn(
        "http_post",
        move |url: &str, body: Dynamic, headers: Map| {
            invoke_http(
                &post_headers_host,
                &post_headers_context,
                "POST",
                url,
                dynamic_to_json(body),
                headers,
            )
        },
    );

    engine.register_fn(
        "http_request",
        move |method: &str, url: &str, body: Dynamic, headers: Map| {
            invoke_http(
                &host,
                &context,
                &method.to_ascii_uppercase(),
                url,
                dynamic_to_json(body),
                headers,
            )
        },
    );
}

fn invoke_http(
    host: &SandboxHost,
    context: &RhaiCapabilityContext,
    method: &str,
    url: &str,
    body: Value,
    headers: Map,
) -> Map {
    let headers = headers
        .into_iter()
        .filter_map(|(key, value)| {
            value
                .try_cast::<String>()
                .map(|value| (key.to_string(), value))
        })
        .collect::<BTreeMap<_, _>>();
    let response = invoke_capability(
        host,
        context,
        "platform.http",
        "request",
        json!({
            "method": method,
            "url": url,
            "headers": headers,
            "body": body,
        }),
    );
    if response
        .get("ok")
        .and_then(|value| value.clone().try_cast::<bool>())
        == Some(false)
    {
        return response;
    }
    let output = response.get("output").cloned().unwrap_or(Dynamic::UNIT);
    if output.is_map() {
        output.cast::<Map>()
    } else {
        let mut result = Map::new();
        result.insert("ok".into(), Dynamic::from(true));
        result.insert("body".into(), output);
        result
    }
}

fn register_standard_functions(engine: &mut Engine) {
    engine.register_fn("log", |message: &str| {
        info!(target: "rustok_sandbox::rhai", "{}", message);
    });
    engine.register_fn("log_warn", |message: &str| {
        warn!(target: "rustok_sandbox::rhai", "{}", message);
    });
    engine.register_fn("log_error", |message: &str| {
        error!(target: "rustok_sandbox::rhai", "{}", message);
    });
    engine.register_fn("log", |source: &str, message: &str| {
        info!(target: "rustok_sandbox::rhai", source, "{}", message);
    });
    engine.register_fn("log_warn", |source: &str, message: &str| {
        warn!(target: "rustok_sandbox::rhai", source, "{}", message);
    });
    engine.register_fn("log_error", |source: &str, message: &str| {
        error!(target: "rustok_sandbox::rhai", source, "{}", message);
    });
    engine.register_fn("now", || chrono::Utc::now().to_rfc3339());
    engine.register_fn("now_unix", || chrono::Utc::now().timestamp());
    engine.register_fn(
        "abort",
        |message: &str| -> Result<Dynamic, Box<EvalAltResult>> {
            Err(Box::new(EvalAltResult::ErrorRuntime(
                format!("ABORT:{message}").into(),
                rhai::Position::NONE,
            )))
        },
    );
    engine.register_fn("format_money", |amount: i64| {
        let digits = amount.abs().to_string();
        let mut formatted = String::new();
        for (index, character) in digits.chars().rev().enumerate() {
            if index > 0 && index % 3 == 0 {
                formatted.push(' ');
            }
            formatted.push(character);
        }
        if amount < 0 {
            formatted.push('-');
        }
        formatted.chars().rev().collect::<String>()
    });
    engine.register_fn("is_empty", |value: Dynamic| {
        value.is_unit()
            || value
                .clone()
                .try_cast::<String>()
                .is_some_and(|value| value.is_empty())
            || value
                .try_cast::<rhai::Array>()
                .is_some_and(|value| value.is_empty())
    });
    engine.register_fn(
        "coalesce",
        |value: Dynamic, default: Dynamic| {
            if value.is_unit() { default } else { value }
        },
    );
}

fn validation_helpers_enabled(phase: crate::ExecutionPhase) -> bool {
    !matches!(
        phase,
        crate::ExecutionPhase::AfterHook | crate::ExecutionPhase::Event
    )
}

fn register_validation_functions(engine: &mut Engine) {
    engine.register_fn("validate_email", |email: &str| {
        email_address::EmailAddress::is_valid(email)
    });
    engine.register_fn("validate_required", |value: &str| !value.trim().is_empty());
    engine.register_fn("validate_min_length", |value: &str, min: i64| {
        value.len() as i64 >= min
    });
    engine.register_fn("validate_max_length", |value: &str, max: i64| {
        value.len() as i64 <= max
    });
    engine.register_fn("validate_range", |value: i64, min: i64, max: i64| {
        value >= min && value <= max
    });
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
    response.insert("status".into(), Dynamic::from(0_i64));
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

    /// Validates one immutable Rhai payload before a node reports it prepared.
    /// The check uses no host capability bridge and never evaluates the source.
    pub fn prepare_artifact_payload(
        &self,
        runtime_abi: &str,
        media_type: &str,
        payload_digest: &str,
        payload: &[u8],
    ) -> SandboxResult<RhaiArtifactPreparation> {
        if runtime_abi != RHAI_SANDBOX_RUNTIME_ABI {
            return Err(SandboxError::InvalidRequest(
                "Rhai artifact runtime ABI is not supported by this sandbox".to_string(),
            ));
        }
        if !canonical_digest(payload_digest) {
            return Err(SandboxError::InvalidRequest(
                "Rhai artifact payload digest is invalid".to_string(),
            ));
        }
        let received = format!("sha256:{}", hex::encode(Sha256::digest(payload)));
        if received != payload_digest {
            return Err(SandboxError::InvalidRequest(
                "Rhai artifact payload digest does not match its bytes".to_string(),
            ));
        }

        let mut engine = Engine::new();
        let source = match media_type {
            RHAI_WORKSPACE_MEDIA_TYPE => {
                let workspace: RhaiWorkspace =
                    serde_json::from_slice(payload).map_err(|error| {
                        SandboxError::InvalidRequest(format!("invalid Rhai workspace: {error}"))
                    })?;
                let workspace_digest = workspace
                    .digest()
                    .map_err(|error| SandboxError::InvalidRequest(error.to_string()))?;
                if workspace_digest != payload_digest {
                    return Err(SandboxError::InvalidRequest(
                        "Rhai workspace digest does not match the artifact payload digest"
                            .to_string(),
                    ));
                }
                workspace
                    .configure_rhai_engine(&mut engine)
                    .map_err(|error| SandboxError::InvalidRequest(error.to_string()))?;
                workspace
                    .entrypoint_source()
                    .map_err(|error| SandboxError::InvalidRequest(error.to_string()))?
                    .to_string()
            }
            RHAI_SOURCE_MEDIA_TYPE => std::str::from_utf8(payload)
                .map_err(|error| SandboxError::Compilation(error.to_string()))?
                .to_string(),
            _ => {
                return Err(SandboxError::InvalidRequest(
                    "Rhai artifact media type is not supported by this sandbox".to_string(),
                ));
            }
        };

        engine
            .compile(&source)
            .map_err(|error| SandboxError::Compilation(error.to_string()))?;
        Ok(RhaiArtifactPreparation {
            runtime_fingerprint: rhai_runtime_fingerprint(media_type),
        })
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
        engine.build_type::<RhaiRecord>();
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

    fn resolve_source(request: &SandboxRequest, engine: &mut Engine) -> SandboxResult<Vec<u8>> {
        if request.payload.media_type != RHAI_WORKSPACE_MEDIA_TYPE {
            return Ok(request.payload.bytes.clone());
        }

        let workspace: RhaiWorkspace =
            serde_json::from_slice(&request.payload.bytes).map_err(|error| {
                SandboxError::InvalidRequest(format!("invalid Rhai workspace: {error}"))
            })?;
        let digest = workspace
            .digest()
            .map_err(|error| SandboxError::InvalidRequest(error.to_string()))?;
        if digest != request.payload.digest {
            return Err(SandboxError::InvalidRequest(
                "Rhai workspace digest does not match the request payload digest".to_string(),
            ));
        }
        workspace
            .configure_rhai_engine_for_entrypoint(engine, &request.payload.entrypoint)
            .map_err(|error| SandboxError::InvalidRequest(error.to_string()))?;
        workspace
            .executable_source(&request.payload.entrypoint)
            .map(|source| source.as_bytes().to_vec())
            .map_err(|error| SandboxError::InvalidRequest(error.to_string()))
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

    fn populate_serialized_scope(
        scope: &mut Scope<'static>,
        request: &SandboxRequest,
    ) -> SandboxResult<()> {
        let Some(bindings) = &request.rhai_scope else {
            return Ok(());
        };
        bindings
            .validate()
            .map_err(|error| SandboxError::InvalidRequest(error.to_string()))?;
        for (name, value) in &bindings.constants {
            scope.push_constant(name.clone(), json_to_dynamic(value));
        }
        for (name, input) in &bindings.records {
            let record = RhaiRecord::from_input(input)?;
            if input.mutable {
                scope.push(name.clone(), record);
            } else {
                scope.push_constant(name.clone(), record);
            }
        }
        Ok(())
    }

    fn collect_serialized_scope(
        scope: &mut Scope<'static>,
        request: &SandboxRequest,
    ) -> SandboxResult<Option<RhaiScopeOutput>> {
        let Some(bindings) = &request.rhai_scope else {
            return Ok(None);
        };
        let mut record_changes = BTreeMap::new();
        for (name, input) in &bindings.records {
            if !input.mutable {
                continue;
            }
            let record = scope.get_value::<RhaiRecord>(name).ok_or_else(|| {
                SandboxError::Internal(format!("Rhai scope record `{name}` is unavailable"))
            })?;
            record_changes.insert(name.clone(), record.changes_json());
        }
        Ok(Some(RhaiScopeOutput { record_changes }))
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

fn canonical_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

fn rhai_runtime_fingerprint(media_type: &str) -> String {
    format!(
        "sha256:{}",
        hex::encode(Sha256::digest(format!(
            "rustok.rhai.artifact-preparation\\0{RHAI_SANDBOX_RUNTIME_ABI}\\0{media_type}"
        )))
    )
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
        let source = Self::resolve_source(request, &mut engine)?;
        let source = std::str::from_utf8(&source)
            .map_err(|error| SandboxError::Compilation(error.to_string()))?;
        let mut scope = Self::build_scope(request, &binding.input);
        Self::populate_serialized_scope(&mut scope, request)?;
        let mut ast = engine
            .compile_with_scope(&scope, source)
            .map_err(|error| SandboxError::Compilation(error.to_string()))?;
        ast.set_source(&request.payload.entrypoint);
        let output = engine
            .eval_ast_with_scope::<Dynamic>(&mut scope, &ast)
            .map_err(|error| Self::map_error(*error, request))?;
        let output = dynamic_to_json(output);
        let rhai_scope = Self::collect_serialized_scope(&mut scope, request)?;
        let output = serde_json::to_value(RhaiBindingOutput::new(output))
            .map_err(|error| SandboxError::Internal(error.to_string()))?;
        let output_bytes = serde_json::to_vec(&(&output, &rhai_scope))
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
            rhai_scope,
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

#[cfg(test)]
mod tests {
    use super::RhaiExecutor;
    use crate::{RHAI_SANDBOX_RUNTIME_ABI, RHAI_WORKSPACE_MEDIA_TYPE, RhaiWorkspace};

    #[test]
    fn preparation_accepts_the_canonical_workspace_without_evaluating_it() {
        let workspace = RhaiWorkspace::single_source("40 + 2");
        let bytes = workspace.canonical_bytes().expect("canonical workspace");
        let digest = workspace.digest().expect("workspace digest");

        let prepared = RhaiExecutor::new()
            .prepare_artifact_payload(
                RHAI_SANDBOX_RUNTIME_ABI,
                RHAI_WORKSPACE_MEDIA_TYPE,
                &digest,
                &bytes,
            )
            .expect("prepared workspace");

        assert!(prepared.runtime_fingerprint.starts_with("sha256:"));
    }

    #[test]
    fn preparation_rejects_bytes_that_do_not_match_the_admitted_digest() {
        assert!(
            RhaiExecutor::new()
                .prepare_artifact_payload(
                    RHAI_SANDBOX_RUNTIME_ABI,
                    RHAI_WORKSPACE_MEDIA_TYPE,
                    &format!("sha256:{}", "a".repeat(64)),
                    b"wrong payload",
                )
                .is_err()
        );
    }
}
