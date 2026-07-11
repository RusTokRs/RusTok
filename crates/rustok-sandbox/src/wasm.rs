//! Wasmtime Component Model executor for untrusted module artifacts.
//!
//! The v1 component ABI is intentionally narrow: an artifact exports the
//! descriptor entrypoint with `(string) -> result<string, string>`. Input and
//! output strings carry canonical JSON. Components receive no WASI or other
//! ambient imports. Its one typed WIT import bridges through `SandboxHost`,
//! just as the Rhai adapters do.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

use async_trait::async_trait;
use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder};

use crate::{
    CapabilityCall, CapabilityName, ExecutionMetrics, SandboxError, SandboxExecutor,
    SandboxExecutorKind, SandboxHost, SandboxOutcome, SandboxRequest, SandboxResult,
    SandboxSubject,
};

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

/// Executes Component Model payloads without WASI, filesystem, network or
/// inherited environment access.
#[derive(Debug, Default)]
pub struct WasmComponentExecutor;

struct WasmStoreState {
    limits: StoreLimits,
    host: SandboxHost,
    execution_id: uuid::Uuid,
    subject: SandboxSubject,
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
        Self
    }

    fn engine() -> SandboxResult<Engine> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.consume_fuel(true);
        config.epoch_interruption(true);
        Engine::new(&config).map_err(|error| SandboxError::Internal(error.to_string()))
    }

    fn execute_component(
        request: &SandboxRequest,
        host: SandboxHost,
    ) -> SandboxResult<SandboxOutcome> {
        let engine = Self::engine()?;
        let component = Component::new(&engine, &request.payload.bytes)
            .map_err(|error| SandboxError::Compilation(error.to_string()))?;
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
        let mut store = Store::new(
            &engine,
            WasmStoreState {
                limits,
                host,
                execution_id: request.context.execution_id,
                subject: request.subject.clone(),
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
        let completed = Arc::new(AtomicBool::new(false));
        let timeout_engine = engine.clone();
        let timeout_completed = Arc::clone(&completed);
        let timeout_fired = Arc::clone(&timed_out);
        let timeout = request.policy.limits.wall_clock_ms;
        let watchdog = thread::spawn(move || {
            thread::sleep(Duration::from_millis(timeout));
            if !timeout_completed.load(Ordering::Acquire) {
                timeout_fired.store(true, Ordering::Release);
                timeout_engine.increment_epoch();
            }
        });

        let result = (|| {
            let instance = ModuleRuntime::instantiate(&mut store, &component, &linker)
                .map_err(|error| SandboxError::Trap(error.to_string()))?;
            if request.payload.entrypoint != "run" {
                return Err(SandboxError::InvalidRequest(
                    "Wasm Component v1 entrypoint must be `run`".to_string(),
                ));
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
                    output_bytes: Some(output_bytes),
                    ..Default::default()
                },
            })
        })();

        completed.store(true, Ordering::Release);
        let _ = watchdog.join();
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
        Self::execute_component(request, host)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use chrono::Utc;
    use serde_json::{Value, json};
    use uuid::Uuid;

    use super::WasmComponentExecutor;
    use crate::{
        ExecutionPhase, SandboxContext, SandboxError, SandboxExecutorKind, SandboxPayload,
        SandboxPolicy, SandboxRequest, SandboxSubject,
    };

    #[tokio::test]
    async fn invalid_component_bytes_are_rejected_before_instantiation() {
        let request = SandboxRequest {
            subject: SandboxSubject::ModuleArtifact {
                slug: "sample_module".to_string(),
                version: "1.0.0".to_string(),
                digest: "sha256:fixture".to_string(),
            },
            context: SandboxContext {
                execution_id: Uuid::new_v4(),
                phase: ExecutionPhase::Test,
                timestamp: Utc::now(),
                tenant_id: None,
                actor_id: None,
                trace_id: None,
            },
            payload: SandboxPayload {
                executor: SandboxExecutorKind::WasmComponent,
                media_type: "application/wasm".to_string(),
                digest: "sha256:fixture".to_string(),
                entrypoint: "run".to_string(),
                bytes: b"not a component".to_vec(),
            },
            input: Value::Null,
            policy: SandboxPolicy::default(),
        };

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
        let mut state = super::WasmStoreState {
            limits: wasmtime::StoreLimitsBuilder::new().build(),
            host: crate::SandboxHost::new(
                Arc::new(SandboxPolicy {
                    grants: vec![crate::CapabilityGrant {
                        name: capability.clone(),
                        constraints: json!({}),
                    }],
                    ..Default::default()
                }),
                broker.clone(),
            ),
            execution_id,
            subject: subject.clone(),
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
