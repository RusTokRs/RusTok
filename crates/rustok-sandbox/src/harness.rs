//! Local authoring harness over the production-neutral sandbox contracts.
//!
//! The harness deliberately owns no credentials, configuration, network, or
//! infrastructure clients. It executes the same `SandboxRequest` through the
//! same `SandboxRuntime` and replaces host capabilities only with explicit,
//! deterministic fixtures.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::{
    CapabilityBroker, CapabilityCall, CapabilityGrant, CapabilityName, CapabilityResponse,
    ExecutorRegistry, SandboxError, SandboxOutcome, SandboxRequest, SandboxResult, SandboxRuntime,
};

type FixtureKey = (CapabilityName, String);

/// Deterministic capability responses for local authoring and test harnesses.
///
/// Responses are addressed by the exact capability and operation. An
/// unregistered fixture remains denied, as it would under the production
/// default-deny policy. The broker has no environment, file, network, database,
/// secret, or MCP access.
#[derive(Clone, Default)]
pub struct FixtureCapabilityBroker {
    responses: Arc<Mutex<HashMap<FixtureKey, CapabilityResponse>>>,
}

impl FixtureCapabilityBroker {
    /// Adds or replaces one local response for an exact capability operation.
    pub fn respond(
        &self,
        capability: CapabilityName,
        operation: impl Into<String>,
        response: CapabilityResponse,
    ) -> SandboxResult<()> {
        let operation = operation.into();
        validate_operation(&operation)?;
        self.responses
            .lock()
            .map_err(|_| SandboxError::Internal("fixture capability lock is poisoned".to_string()))?
            .insert((capability, operation), response);
        Ok(())
    }

    /// Removes all configured local responses without changing execution policy.
    pub fn clear(&self) -> SandboxResult<()> {
        self.responses
            .lock()
            .map_err(|_| SandboxError::Internal("fixture capability lock is poisoned".to_string()))?
            .clear();
        Ok(())
    }
}

#[async_trait]
impl CapabilityBroker for FixtureCapabilityBroker {
    async fn invoke(
        &self,
        call: &CapabilityCall,
        grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse> {
        if grant.name != call.capability {
            return Err(SandboxError::CapabilityDenied(call.capability.clone()));
        }
        self.responses
            .lock()
            .map_err(|_| SandboxError::Internal("fixture capability lock is poisoned".to_string()))?
            .get(&(call.capability.clone(), call.operation.clone()))
            .cloned()
            .ok_or_else(|| SandboxError::CapabilityDenied(call.capability.clone()))
    }
}

/// Local entry point that preserves the production sandbox request, policy,
/// execution, cancellation, and error contracts while exposing fixture-only
/// capabilities.
#[derive(Clone)]
pub struct LocalSandboxHarness {
    runtime: SandboxRuntime,
    fixtures: FixtureCapabilityBroker,
}

impl LocalSandboxHarness {
    pub fn new(executors: ExecutorRegistry) -> Self {
        let fixtures = FixtureCapabilityBroker::default();
        let runtime = SandboxRuntime::new(executors, Arc::new(fixtures.clone()));
        Self { runtime, fixtures }
    }

    #[cfg(feature = "rhai")]
    pub fn rhai() -> SandboxResult<Self> {
        let mut executors = ExecutorRegistry::new();
        executors.register(crate::rhai::RhaiExecutor::new())?;
        Ok(Self::new(executors))
    }

    pub fn fixtures(&self) -> FixtureCapabilityBroker {
        self.fixtures.clone()
    }

    pub async fn execute(&self, request: SandboxRequest) -> SandboxResult<SandboxOutcome> {
        self.runtime.execute(request).await
    }
}

fn validate_operation(operation: &str) -> SandboxResult<()> {
    if operation.is_empty() || operation.len() > 64 || operation.contains(char::is_control) {
        return Err(SandboxError::InvalidRequest(
            "fixture capability operation must be a bounded visible string".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use chrono::Utc;
    use serde_json::json;
    use uuid::Uuid;

    use super::*;
    use crate::{
        CapabilityCallContext, ExecutionMetrics, ExecutionPhase, SandboxContext, SandboxExecutor,
        SandboxExecutorKind, SandboxHost, SandboxPayload, SandboxPolicy, SandboxSubject,
    };

    struct FixtureExecutor;

    #[async_trait]
    impl SandboxExecutor for FixtureExecutor {
        fn kind(&self) -> SandboxExecutorKind {
            SandboxExecutorKind::Rhai
        }

        async fn execute(
            &self,
            request: &SandboxRequest,
            host: SandboxHost,
        ) -> SandboxResult<SandboxOutcome> {
            let response = host
                .invoke(&CapabilityCall {
                    execution_id: request.context.execution_id,
                    subject: request.subject.clone(),
                    context: CapabilityCallContext::from(&request.context),
                    capability: CapabilityName::new("fixture.echo")?,
                    operation: "call".to_string(),
                    input: json!({ "ignored": true }),
                })
                .await?;
            Ok(SandboxOutcome {
                execution_id: request.context.execution_id,
                output: response.output,
                metrics: ExecutionMetrics::default(),
            })
        }
    }

    fn request() -> SandboxRequest {
        SandboxRequest {
            subject: SandboxSubject::ModuleArtifact {
                installation_id: uuid::Uuid::new_v4(),
                slug: "sample_module".to_string(),
                version: "1.0.0".to_string(),
                digest: "sha256:sample".to_string(),
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
                executor: SandboxExecutorKind::Rhai,
                media_type: "application/test".to_string(),
                digest: "sha256:payload".to_string(),
                runtime_abi: "rustok:module/runtime@1".to_string(),
                entrypoint: "run".to_string(),
                bytes: Vec::new(),
            },
            input: serde_json::Value::Null,
            policy: SandboxPolicy {
                grants: vec![CapabilityGrant {
                    name: CapabilityName::new("fixture.echo").expect("fixture capability"),
                    constraints: serde_json::Value::Null,
                }],
                ..Default::default()
            },
        }
    }

    #[tokio::test]
    async fn harness_requires_an_explicit_fixture_response() {
        let mut executors = ExecutorRegistry::new();
        executors
            .register(FixtureExecutor)
            .expect("fixture executor");
        let harness = LocalSandboxHarness::new(executors);

        assert!(matches!(
            harness.execute(request()).await,
            Err(SandboxError::CapabilityDenied(_))
        ));

        harness
            .fixtures()
            .respond(
                CapabilityName::new("fixture.echo").expect("fixture capability"),
                "call",
                CapabilityResponse {
                    output: json!({ "value": "fixture" }),
                },
            )
            .expect("fixture response");
        let outcome = harness.execute(request()).await.expect("fixture execution");
        assert_eq!(outcome.output, json!({ "value": "fixture" }));
    }
}
