//! Wasmtime Component Model executor for untrusted module artifacts.
//!
//! The v1 component ABI is intentionally narrow: an artifact exports the
//! descriptor entrypoint with `(string) -> result<string, string>`. Input and
//! output strings carry canonical JSON. Components receive no WASI or other
//! ambient imports. Its one typed WIT import bridges through `SandboxHost`,
//! just as the Rhai adapters do.

use std::collections::{HashMap, VecDeque};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

use async_trait::async_trait;
use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Config, Engine, ResourceLimiter, Store, StoreLimits, StoreLimitsBuilder};

use crate::{
    CapabilityCall, CapabilityCallContext, CapabilityName, ExecutionMetrics, SandboxError,
    SandboxExecutor, SandboxExecutorKind, SandboxHost, SandboxOutcome, SandboxRequest,
    SandboxResult, SandboxSubject,
};

/// Immutable v1 Component Model ABI identity and wire encoding.
pub const WASM_COMPONENT_ABI_VERSION: &str = "v1";
pub const WASM_COMPONENT_WIT_PACKAGE: &str = "rustok:module@1.0.0";
pub const WASM_COMPONENT_WIT_WORLD: &str = "module-runtime";
pub const WASM_COMPONENT_ENTRYPOINT: &str = "run";
pub const WASM_COMPONENT_INPUT_ENCODING: &str = "application/json";
pub const WASM_COMPONENT_OUTPUT_ENCODING: &str = "application/json";
pub const WASM_COMPONENT_ERROR_ENCODING: &str = "wit-result-string";

/// Wasmtime version encoded into every serialized-component cache key.
///
/// Update this value in the same change as the pinned `wasmtime` dependency.
pub const WASMTIME_ENGINE_VERSION: &str = "46.0.1";

const MAX_COMPONENT_CACHE_ENTRIES: usize = 64;
const MAX_COMPONENT_CACHE_BYTES: usize = 128 * 1024 * 1024;

wasmtime::component::bindgen!({
    inline: r#"
        package rustok:module@1.0.0;

        interface host {
            invoke: func(capability: string, operation: string, input: string) -> result<string, string>;
        }

        world module-runtime {
            import host;
            export run: func(input: string) -> result<string, string>;
        }
    "#,
});

/// Bounded node-local policy for serialized compiled Components.
///
/// The cache never retains stores, host handles, tenant context, credentials,
/// or guest input/output. A cache hit still creates a request-private engine
/// and store before execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WasmComponentCachePolicy {
    pub max_entries: usize,
    pub max_serialized_bytes: usize,
}

impl Default for WasmComponentCachePolicy {
    fn default() -> Self {
        Self {
            max_entries: MAX_COMPONENT_CACHE_ENTRIES,
            max_serialized_bytes: MAX_COMPONENT_CACHE_BYTES,
        }
    }
}

impl WasmComponentCachePolicy {
    fn validate(self) -> SandboxResult<Self> {
        if self.max_entries == 0
            || self.max_entries > MAX_COMPONENT_CACHE_ENTRIES
            || self.max_serialized_bytes == 0
            || self.max_serialized_bytes > MAX_COMPONENT_CACHE_BYTES
        {
            return Err(SandboxError::InvalidRequest(
                "Wasm component cache policy is outside the supported bounds".to_string(),
            ));
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct WasmComponentCacheKey {
    engine_version: &'static str,
    target: String,
    runtime_abi: String,
    digest: String,
}

#[derive(Debug)]
struct SerializedComponentCache {
    policy: WasmComponentCachePolicy,
    state: Mutex<SerializedComponentCacheState>,
}

#[derive(Debug, Default)]
struct SerializedComponentCacheState {
    bytes: usize,
    entries: HashMap<WasmComponentCacheKey, Vec<u8>>,
    lru: VecDeque<WasmComponentCacheKey>,
}

impl SerializedComponentCache {
    fn new(policy: WasmComponentCachePolicy) -> Self {
        Self {
            policy,
            state: Mutex::new(SerializedComponentCacheState::default()),
        }
    }

    fn get(&self, key: &WasmComponentCacheKey) -> SandboxResult<Option<Vec<u8>>> {
        let mut state = self.state.lock().map_err(|_| {
            SandboxError::Internal("Wasm component cache lock is poisoned".to_string())
        })?;
        let value = state.entries.get(key).cloned();
        if value.is_some() {
            state.lru.retain(|candidate| candidate != key);
            state.lru.push_back(key.clone());
        }
        Ok(value)
    }

    fn remove(&self, key: &WasmComponentCacheKey) -> SandboxResult<()> {
        let mut state = self.state.lock().map_err(|_| {
            SandboxError::Internal("Wasm component cache lock is poisoned".to_string())
        })?;
        if let Some(value) = state.entries.remove(key) {
            state.bytes = state.bytes.saturating_sub(value.len());
        }
        state.lru.retain(|candidate| candidate != key);
        Ok(())
    }

    fn insert(&self, key: WasmComponentCacheKey, bytes: Vec<u8>) -> SandboxResult<()> {
        if bytes.len() > self.policy.max_serialized_bytes {
            return Ok(());
        }
        let mut state = self.state.lock().map_err(|_| {
            SandboxError::Internal("Wasm component cache lock is poisoned".to_string())
        })?;
        if let Some(previous) = state.entries.remove(&key) {
            state.bytes = state.bytes.saturating_sub(previous.len());
        }
        state.lru.retain(|candidate| candidate != &key);
        while state.entries.len() >= self.policy.max_entries
            || state.bytes.saturating_add(bytes.len()) > self.policy.max_serialized_bytes
        {
            let Some(oldest) = state.lru.pop_front() else {
                break;
            };
            if let Some(evicted) = state.entries.remove(&oldest) {
                state.bytes = state.bytes.saturating_sub(evicted.len());
            }
        }
        state.bytes += bytes.len();
        state.entries.insert(key.clone(), bytes);
        state.lru.push_back(key);
        Ok(())
    }
}

/// Executes Component Model payloads without WASI, filesystem, network or
/// inherited environment access.
#[derive(Debug)]
pub struct WasmComponentExecutor {
    cache: Arc<SerializedComponentCache>,
}

struct WasmStoreState {
    limits: WasmStoreLimits,
    host: SandboxHost,
    execution_id: uuid::Uuid,
    subject: SandboxSubject,
    context: CapabilityCallContext,
}

/// Tracks guest linear-memory allocations that Wasmtime actually permits.
///
/// Wasmtime reports a failed permitted growth through `memory_grow_failed`; the
/// tracker removes that pending delta, so the metric never becomes a configured
/// limit or a failed allocation request. Shared memories are outside Wasmtime's
/// `ResourceLimiter` contract and therefore are not included.
struct WasmStoreLimits {
    limits: StoreLimits,
    allocated_linear_memory_bytes: u64,
    peak_linear_memory_bytes: u64,
    pending_memory_growth_bytes: Option<u64>,
}

impl WasmStoreLimits {
    fn new(limits: StoreLimits) -> Self {
        Self {
            limits,
            allocated_linear_memory_bytes: 0,
            peak_linear_memory_bytes: 0,
            pending_memory_growth_bytes: None,
        }
    }

    fn peak_linear_memory_bytes(&self) -> u64 {
        self.peak_linear_memory_bytes
    }
}

impl ResourceLimiter for WasmStoreLimits {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        let allowed = self.limits.memory_growing(current, desired, maximum)?;
        if allowed {
            let growth = u64::try_from(desired.saturating_sub(current)).unwrap_or(u64::MAX);
            self.allocated_linear_memory_bytes =
                self.allocated_linear_memory_bytes.saturating_add(growth);
            self.peak_linear_memory_bytes = self
                .peak_linear_memory_bytes
                .max(self.allocated_linear_memory_bytes);
            self.pending_memory_growth_bytes = Some(growth);
        }
        Ok(allowed)
    }

    fn memory_grow_failed(&mut self, error: wasmtime::Error) -> wasmtime::Result<()> {
        if let Some(growth) = self.pending_memory_growth_bytes.take() {
            self.allocated_linear_memory_bytes =
                self.allocated_linear_memory_bytes.saturating_sub(growth);
            if self.allocated_linear_memory_bytes < self.peak_linear_memory_bytes {
                self.peak_linear_memory_bytes = self.allocated_linear_memory_bytes;
            }
        }
        self.limits.memory_grow_failed(error)
    }

    fn table_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        self.limits.table_growing(current, desired, maximum)
    }

    fn table_grow_failed(&mut self, error: wasmtime::Error) -> wasmtime::Result<()> {
        self.limits.table_grow_failed(error)
    }

    fn instances(&self) -> usize {
        self.limits.instances()
    }

    fn tables(&self) -> usize {
        self.limits.tables()
    }

    fn memories(&self) -> usize {
        self.limits.memories()
    }
}

impl rustok::module::host::Host for WasmStoreState {
    fn invoke(
        &mut self,
        capability: String,
        operation: String,
        input: String,
    ) -> Result<String, String> {
        let capability = CapabilityName::new(capability).map_err(|error| error.to_string())?;
        let input = serde_json::from_str(&input).map_err(|error| error.to_string())?;
        let call = CapabilityCall {
            execution_id: self.execution_id,
            subject: self.subject.clone(),
            context: self.context.clone(),
            capability,
            operation,
            input,
        };
        self.host
            .invoke_blocking(&call)
            .map(|response| response.output.to_string())
            .map_err(|error| format!("{}: {error}", error.code()))
    }
}

impl WasmComponentExecutor {
    pub fn new() -> Self {
        Self::with_component_cache_policy(WasmComponentCachePolicy::default())
            .expect("default Wasm component cache policy must be valid")
    }

    pub fn with_component_cache_policy(policy: WasmComponentCachePolicy) -> SandboxResult<Self> {
        Ok(Self {
            cache: Arc::new(SerializedComponentCache::new(policy.validate()?)),
        })
    }

    fn engine() -> SandboxResult<Engine> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.consume_fuel(true);
        config.epoch_interruption(true);
        Engine::new(&config).map_err(|error| SandboxError::Internal(error.to_string()))
    }

    fn cache_key(request: &SandboxRequest) -> WasmComponentCacheKey {
        WasmComponentCacheKey {
            engine_version: WASMTIME_ENGINE_VERSION,
            target: format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
            runtime_abi: request.payload.runtime_abi.clone(),
            digest: request.payload.digest.clone(),
        }
    }

    fn load_component(&self, request: &SandboxRequest) -> SandboxResult<(Engine, Component)> {
        let key = Self::cache_key(request);
        let engine = Self::engine()?;
        if let Some(serialized) = self.cache.get(&key)? {
            // Safety: only `Component::serialize` output produced by this
            // process is inserted into the private cache. The cache key binds
            // Wasmtime version, host target, runtime ABI, and admitted digest.
            match unsafe { Component::deserialize(&engine, &serialized) } {
                Ok(component) => return Ok((engine, component)),
                Err(_) => self.cache.remove(&key)?,
            }
        }
        let component = Component::new(&engine, &request.payload.bytes)
            .map_err(|error| SandboxError::Compilation(error.to_string()))?;
        let serialized = component
            .serialize()
            .map_err(|error| SandboxError::Internal(error.to_string()))?;
        self.cache.insert(key, serialized)?;
        Ok((engine, component))
    }

    fn execute_component(
        &self,
        request: &SandboxRequest,
        host: SandboxHost,
    ) -> SandboxResult<SandboxOutcome> {
        let (engine, component) = self.load_component(request)?;
        let mut linker = Linker::<WasmStoreState>::new(&engine);
        ModuleRuntime::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)
            .map_err(|error| SandboxError::Internal(error.to_string()))?;
        let limits = StoreLimitsBuilder::new()
            .memory_size(
                request
                    .policy
                    .limits
                    .max_memory_bytes
                    .try_into()
                    .unwrap_or(usize::MAX),
            )
            .build();
        let cancellation = host.cancellation();
        let mut store = Store::new(
            &engine,
            WasmStoreState {
                limits: WasmStoreLimits::new(limits),
                host,
                execution_id: request.context.execution_id,
                subject: request.subject.clone(),
                context: CapabilityCallContext::from(&request.context),
            },
        );
        store.limiter(|state| &mut state.limits);
        store
            .set_fuel(request.policy.limits.instruction_budget)
            .map_err(|error| SandboxError::Internal(error.to_string()))?;
        store.set_epoch_deadline(1);

        // The engine is private to this request, so incrementing its epoch
        // cannot interrupt another tenant's execution.
        let timed_out = Arc::new(AtomicBool::new(false));
        let cancelled = Arc::new(AtomicBool::new(false));
        let completed = Arc::new(AtomicBool::new(false));
        let watchdog_engine = engine.clone();
        let watchdog_completed = Arc::clone(&completed);
        let watchdog_timed_out = Arc::clone(&timed_out);
        let watchdog_cancelled = Arc::clone(&cancelled);
        let timeout = request.policy.limits.wall_clock_ms;
        let watchdog = thread::spawn(move || {
            let started = std::time::Instant::now();
            while !watchdog_completed.load(Ordering::Acquire) {
                if cancellation.is_cancelled() {
                    watchdog_cancelled.store(true, Ordering::Release);
                    watchdog_engine.increment_epoch();
                    break;
                }
                if started.elapsed() >= Duration::from_millis(timeout) {
                    watchdog_timed_out.store(true, Ordering::Release);
                    watchdog_engine.increment_epoch();
                    break;
                }
                thread::sleep(Duration::from_millis(1));
            }
        });

        let result = (|| {
            let instance = ModuleRuntime::instantiate(&mut store, &component, &linker)
                .map_err(|error| SandboxError::Trap(error.to_string()))?;
            if request.payload.entrypoint != WASM_COMPONENT_ENTRYPOINT {
                return Err(SandboxError::InvalidRequest(format!(
                    "Wasm Component {WASM_COMPONENT_ABI_VERSION} entrypoint must be `{WASM_COMPONENT_ENTRYPOINT}`"
                )));
            }
            let input = serde_json::to_string(&request.input)
                .map_err(|error| SandboxError::Internal(error.to_string()))?;
            let output = instance
                .call_run(&mut store, &input)
                .map_err(|error| SandboxError::Trap(error.to_string()))?
                .map_err(SandboxError::Trap)?;
            let output = serde_json::from_str(&output).unwrap_or(serde_json::Value::String(output));
            let output_bytes = serde_json::to_vec(&output)
                .map_err(|error| SandboxError::Internal(error.to_string()))?
                .len() as u64;
            if output_bytes > request.policy.limits.max_output_bytes {
                return Err(SandboxError::LimitExceeded {
                    resource: "output_bytes".to_string(),
                    limit: request.policy.limits.max_output_bytes,
                });
            }
            let fuel_remaining = store.get_fuel().unwrap_or(0);
            let peak_memory_bytes = store.data().limits.peak_linear_memory_bytes();
            Ok(SandboxOutcome {
                execution_id: request.context.execution_id,
                output,
                metrics: ExecutionMetrics {
                    instructions_consumed: Some(
                        request
                            .policy
                            .limits
                            .instruction_budget
                            .saturating_sub(fuel_remaining),
                    ),
                    peak_memory_bytes: Some(peak_memory_bytes),
                    output_bytes: Some(output_bytes),
                    ..Default::default()
                },
            })
        })();

        completed.store(true, Ordering::Release);
        let _ = watchdog.join();
        if cancelled.load(Ordering::Acquire) {
            return Err(SandboxError::Cancelled);
        }
        if timed_out.load(Ordering::Acquire) {
            return Err(SandboxError::Timeout {
                limit_ms: request.policy.limits.wall_clock_ms,
            });
        }
        result
    }
}

#[async_trait]
impl SandboxExecutor for WasmComponentExecutor {
    fn kind(&self) -> SandboxExecutorKind {
        SandboxExecutorKind::WasmComponent
    }

    async fn execute(
        &self,
        request: &SandboxRequest,
        host: SandboxHost,
    ) -> SandboxResult<SandboxOutcome> {
        self.execute_component(request, host)
    }
}

impl Default for WasmComponentExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use chrono::Utc;
    use serde_json::{Value, json};
    use uuid::Uuid;

    use super::{
        SerializedComponentCache, WASMTIME_ENGINE_VERSION, WasmComponentCacheKey,
        WasmComponentCachePolicy, WasmComponentExecutor, WasmStoreLimits,
    };
    use crate::{
        ExecutionPhase, SandboxContext, SandboxError, SandboxExecutorKind, SandboxPayload,
        SandboxPolicy, SandboxRequest, SandboxSubject,
    };
    use wasmtime::ResourceLimiter;

    #[test]
    fn v1_abi_constants_match_the_component_contract() {
        assert_eq!(super::WASM_COMPONENT_ABI_VERSION, "v1");
        assert_eq!(super::WASM_COMPONENT_WIT_PACKAGE, "rustok:module@1.0.0");
        assert_eq!(super::WASM_COMPONENT_WIT_WORLD, "module-runtime");
        assert_eq!(super::WASM_COMPONENT_ENTRYPOINT, "run");
        assert_eq!(super::WASM_COMPONENT_INPUT_ENCODING, "application/json");
        assert_eq!(super::WASM_COMPONENT_OUTPUT_ENCODING, "application/json");
        assert_eq!(super::WASM_COMPONENT_ERROR_ENCODING, "wit-result-string");
    }

    #[test]
    fn component_cache_key_binds_engine_target_abi_and_admitted_digest() {
        let request = cache_request("rustok:module/runtime@1", "sha256:first");
        let mut changed_abi = request.clone();
        changed_abi.payload.runtime_abi = "rustok:module/runtime@2".to_string();
        let mut changed_digest = request.clone();
        changed_digest.payload.digest = "sha256:second".to_string();

        let key = WasmComponentExecutor::cache_key(&request);
        assert_eq!(key.engine_version, WASMTIME_ENGINE_VERSION);
        assert_eq!(
            key.target,
            format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)
        );
        assert_ne!(key, WasmComponentExecutor::cache_key(&changed_abi));
        assert_ne!(key, WasmComponentExecutor::cache_key(&changed_digest));
    }

    #[test]
    fn serialized_component_cache_evicts_the_least_recently_used_entry() {
        let cache = SerializedComponentCache::new(WasmComponentCachePolicy {
            max_entries: 2,
            max_serialized_bytes: 6,
        });
        let first = cache_key("first");
        let second = cache_key("second");
        let third = cache_key("third");
        cache
            .insert(first.clone(), vec![1, 2])
            .expect("insert first entry");
        cache
            .insert(second.clone(), vec![3, 4])
            .expect("insert second entry");
        assert_eq!(
            cache.get(&first).expect("touch first entry"),
            Some(vec![1, 2])
        );
        cache
            .insert(third.clone(), vec![5, 6])
            .expect("insert third entry");

        assert_eq!(
            cache.get(&first).expect("read first entry"),
            Some(vec![1, 2])
        );
        assert_eq!(cache.get(&second).expect("read second entry"), None);
        assert_eq!(
            cache.get(&third).expect("read third entry"),
            Some(vec![5, 6])
        );
    }

    #[test]
    fn component_cache_policy_rejects_unbounded_capacity() {
        assert!(
            WasmComponentExecutor::with_component_cache_policy(WasmComponentCachePolicy {
                max_entries: 0,
                max_serialized_bytes: 1,
            })
            .is_err()
        );
        assert!(
            WasmComponentExecutor::with_component_cache_policy(WasmComponentCachePolicy {
                max_entries: 1,
                max_serialized_bytes: super::MAX_COMPONENT_CACHE_BYTES + 1,
            })
            .is_err()
        );
    }

    #[test]
    fn peak_memory_tracks_permitted_growth_and_discards_failed_growth() {
        let mut limits = WasmStoreLimits::new(wasmtime::StoreLimitsBuilder::new().build());
        assert!(
            limits
                .memory_growing(0, 64 * 1024, None)
                .expect("permit initial memory")
        );
        assert!(
            limits
                .memory_growing(64 * 1024, 128 * 1024, None)
                .expect("permit memory growth")
        );
        assert_eq!(limits.peak_linear_memory_bytes(), 128 * 1024);

        assert!(
            limits
                .memory_growing(128 * 1024, 192 * 1024, None)
                .expect("permit pending memory growth")
        );
        limits
            .memory_grow_failed(wasmtime::Error::msg("synthetic allocation failure"))
            .expect("record failed memory growth");
        assert_eq!(limits.peak_linear_memory_bytes(), 128 * 1024);
    }

    fn cache_request(runtime_abi: &str, digest: &str) -> SandboxRequest {
        SandboxRequest {
            subject: SandboxSubject::ModuleArtifact {
                installation_id: uuid::Uuid::new_v4(),
                slug: "sample_module".to_string(),
                version: "1.0.0".to_string(),
                digest: digest.to_string(),
            },
            context: SandboxContext::new(ExecutionPhase::Test),
            payload: SandboxPayload {
                executor: SandboxExecutorKind::WasmComponent,
                media_type: "application/wasm".to_string(),
                digest: digest.to_string(),
                runtime_abi: runtime_abi.to_string(),
                entrypoint: "run".to_string(),
                bytes: Vec::new(),
            },
            input: Value::Null,
            policy: SandboxPolicy::default(),
        }
    }

    fn cache_key(digest: &str) -> WasmComponentCacheKey {
        WasmComponentCacheKey {
            engine_version: WASMTIME_ENGINE_VERSION,
            target: "test-target".to_string(),
            runtime_abi: "rustok:module/runtime@1".to_string(),
            digest: digest.to_string(),
        }
    }

    #[tokio::test]
    async fn invalid_component_bytes_are_rejected_before_instantiation() {
        let mut request = cache_request("rustok:module/runtime@1", "sha256:fixture");
        request.context.execution_id = Uuid::new_v4();
        request.context.timestamp = Utc::now();
        request.payload.bytes = b"not a component".to_vec();

        struct NoCapabilities;

        #[async_trait::async_trait]
        impl crate::CapabilityBroker for NoCapabilities {
            async fn invoke(
                &self,
                _call: &crate::CapabilityCall,
                _grant: &crate::CapabilityGrant,
            ) -> crate::SandboxResult<crate::CapabilityResponse> {
                unreachable!("invalid component must not invoke a capability")
            }
        }

        let mut executors = crate::ExecutorRegistry::new();
        executors
            .register(WasmComponentExecutor::new())
            .expect("executor registration");
        let runtime = crate::SandboxRuntime::new(executors, std::sync::Arc::new(NoCapabilities));
        assert!(matches!(
            runtime.execute(request).await,
            Err(SandboxError::Compilation(_))
        ));
    }

    #[derive(Default)]
    struct CapturingBroker(Mutex<Vec<crate::CapabilityCall>>);

    #[async_trait::async_trait]
    impl crate::CapabilityBroker for CapturingBroker {
        async fn invoke(
            &self,
            call: &crate::CapabilityCall,
            _grant: &crate::CapabilityGrant,
        ) -> crate::SandboxResult<crate::CapabilityResponse> {
            self.0.lock().expect("calls lock").push(call.clone());
            Ok(crate::CapabilityResponse {
                output: json!({ "accepted": true }),
            })
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn wit_host_import_uses_the_same_capability_broker_and_default_deny_policy() {
        let broker = Arc::new(CapturingBroker::default());
        let capability = crate::CapabilityName::new("platform.events").expect("capability");
        let execution_id = Uuid::new_v4();
        let subject = SandboxSubject::AlloyDraft {
            draft_id: Uuid::new_v4(),
            revision: 1,
        };
        let mut context = SandboxContext::new(ExecutionPhase::Test);
        context.execution_id = execution_id;
        let mut state = super::WasmStoreState {
            limits: WasmStoreLimits::new(wasmtime::StoreLimitsBuilder::new().build()),
            host: crate::SandboxHost::new(
                Arc::new(SandboxPolicy {
                    grants: vec![crate::CapabilityGrant {
                        name: capability.clone(),
                        constraints: json!({
                            "topics": ["module.installed"],
                            "operations": ["publish"]
                        }),
                    }],
                    ..Default::default()
                }),
                broker.clone(),
                subject.clone(),
                &context,
                Arc::new(Vec::new()),
                crate::SandboxCancellation::new(),
            ),
            execution_id,
            subject: subject.clone(),
            context: crate::CapabilityCallContext::from(&context),
        };

        let output = <super::WasmStoreState as super::rustok::module::host::Host>::invoke(
            &mut state,
            capability.as_str().to_string(),
            "publish".to_string(),
            r#"{"topic":"module.installed"}"#.to_string(),
        )
        .expect("granted host import");
        assert_eq!(output, r#"{"accepted":true}"#);
        assert_eq!(broker.0.lock().expect("calls lock").len(), 1);

        let denied = <super::WasmStoreState as super::rustok::module::host::Host>::invoke(
            &mut state,
            "platform.secrets".to_string(),
            "read".to_string(),
            "{}".to_string(),
        );
        assert!(
            denied
                .expect_err("denied capability")
                .contains("CAPABILITY_DENIED")
        );
        assert_eq!(broker.0.lock().expect("calls lock").len(), 1);
    }
}
