//! Local authoring harness over the production-neutral sandbox contracts.
//!
//! The harness deliberately owns no credentials, configuration, network, or
//! infrastructure clients. It executes the same `SandboxRequest` through the
//! same `SandboxRuntime` and replaces host capabilities only with explicit,
//! deterministic fixtures.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    CapabilityBroker, CapabilityCall, CapabilityGrant, CapabilityName, CapabilityResponse,
    ExecutorRegistry, SandboxError, SandboxOutcome, SandboxPolicy, SandboxRequest, SandboxResult,
    SandboxRuntime,
};

type FixtureKey = (CapabilityName, String);
const LOCAL_SCENARIO_SCHEMA_VERSION: u32 = 1;
const MAX_LOCAL_SCENARIO_BYTES: usize = 512 * 1024;
const MAX_LOCAL_SCENARIO_GRANTS: usize = 64;
const MAX_LOCAL_SCENARIO_FIXTURES: usize = 128;
const LOCAL_SCENARIO_DIGEST_DOMAIN: &[u8] = b"rustok.sandbox.local-scenario\0";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalSandboxScenario {
    pub schema_version: u32,
    pub input: Value,
    pub policy: SandboxPolicy,
    pub fixtures: Vec<LocalCapabilityFixture>,
    pub expectation: LocalSandboxExpectation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalCapabilityFixture {
    pub capability: CapabilityName,
    pub operation: String,
    pub output: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum LocalSandboxExpectation {
    Success { output: Value },
    Error { code: String },
}

#[derive(Clone, Debug, PartialEq)]
pub enum LocalSandboxScenarioOutcome {
    Success(SandboxOutcome),
    ExpectedError { code: String },
}

/// Redacted, deterministic local execution result for comparing independent
/// candidate implementations of one validated scenario. It deliberately omits
/// input, fixture responses, expected output, and executor metrics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalSandboxScenarioResult {
    Success,
    ExpectedError,
}

/// Comparison-safe local execution evidence. This is authoring feedback only;
/// it is not build, admission, or publication evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalSandboxScenarioComparison {
    pub scenario_digest: String,
    pub result: LocalSandboxScenarioResult,
}

impl LocalSandboxScenario {
    pub fn parse(bytes: &[u8]) -> SandboxResult<Self> {
        if bytes.is_empty() || bytes.len() > MAX_LOCAL_SCENARIO_BYTES {
            return Err(SandboxError::InvalidRequest(
                "local sandbox scenario must be bounded non-empty JSON".to_string(),
            ));
        }
        let scenario: Self = serde_json::from_slice(bytes).map_err(|error| {
            SandboxError::InvalidRequest(format!("local sandbox scenario is invalid: {error}"))
        })?;
        scenario.validate()?;
        Ok(scenario)
    }

    pub fn validate(&self) -> SandboxResult<()> {
        if self.schema_version != LOCAL_SCENARIO_SCHEMA_VERSION {
            return Err(SandboxError::InvalidRequest(
                "local sandbox scenario schema version is not current".to_string(),
            ));
        }
        validate_local_limits(&self.policy)?;
        if self.policy.grants.len() > MAX_LOCAL_SCENARIO_GRANTS
            || self.fixtures.len() > MAX_LOCAL_SCENARIO_FIXTURES
        {
            return Err(SandboxError::InvalidRequest(
                "local sandbox scenario grants or fixtures exceed their limits".to_string(),
            ));
        }
        let mut grants = BTreeSet::new();
        for grant in &self.policy.grants {
            if !grants.insert(grant.name.as_str()) {
                return Err(SandboxError::InvalidRequest(
                    "local sandbox scenario grants contain a duplicate capability".to_string(),
                ));
            }
        }
        let mut fixtures = BTreeSet::new();
        for fixture in &self.fixtures {
            validate_operation(&fixture.operation)?;
            if !grants.contains(fixture.capability.as_str()) {
                return Err(SandboxError::InvalidRequest(
                    "local sandbox fixture capability is not granted by the scenario".to_string(),
                ));
            }
            if !fixtures.insert((fixture.capability.as_str(), fixture.operation.as_str())) {
                return Err(SandboxError::InvalidRequest(
                    "local sandbox scenario contains a duplicate fixture".to_string(),
                ));
            }
        }
        if let LocalSandboxExpectation::Error { code } = &self.expectation
            && (code.is_empty()
                || code.len() > 64
                || !code
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte == b'_'))
        {
            return Err(SandboxError::InvalidRequest(
                "local sandbox expected error code is invalid".to_string(),
            ));
        }
        let encoded =
            serde_json::to_vec(self).map_err(|error| SandboxError::Internal(error.to_string()))?;
        if encoded.len() > MAX_LOCAL_SCENARIO_BYTES {
            return Err(SandboxError::InvalidRequest(
                "local sandbox scenario exceeds its encoded size limit".to_string(),
            ));
        }
        Ok(())
    }

    /// Returns the domain-separated digest of the validated semantic scenario
    /// contract. Local runners can report this identifier without exposing
    /// input, fixtures, policy details, or execution output as durable evidence.
    pub fn canonical_digest(&self) -> SandboxResult<String> {
        self.validate()?;
        let bytes =
            serde_json::to_vec(self).map_err(|error| SandboxError::Internal(error.to_string()))?;
        let mut hasher = Sha256::new();
        hasher.update(LOCAL_SCENARIO_DIGEST_DOMAIN);
        hasher.update(bytes);
        Ok(format!("sha256:{}", hex::encode(hasher.finalize())))
    }

    pub fn configure(&self, fixtures: &FixtureCapabilityBroker) -> SandboxResult<()> {
        self.validate()?;
        fixtures.clear()?;
        for fixture in &self.fixtures {
            fixtures.respond(
                fixture.capability.clone(),
                fixture.operation.clone(),
                CapabilityResponse {
                    output: fixture.output.clone(),
                },
            )?;
        }
        Ok(())
    }

    pub fn evaluate(
        &self,
        result: SandboxResult<SandboxOutcome>,
    ) -> SandboxResult<LocalSandboxScenarioOutcome> {
        match (&self.expectation, result) {
            (LocalSandboxExpectation::Success { output }, Ok(outcome))
                if &outcome.output == output =>
            {
                Ok(LocalSandboxScenarioOutcome::Success(outcome))
            }
            (LocalSandboxExpectation::Success { .. }, Ok(_)) => Err(SandboxError::InvalidRequest(
                "local sandbox scenario output does not match its expectation".to_string(),
            )),
            (LocalSandboxExpectation::Success { .. }, Err(error)) => Err(error),
            (LocalSandboxExpectation::Error { code }, Err(error)) if error.code() == code => {
                Ok(LocalSandboxScenarioOutcome::ExpectedError { code: code.clone() })
            }
            (LocalSandboxExpectation::Error { .. }, Err(error)) => Err(error),
            (LocalSandboxExpectation::Error { .. }, Ok(_)) => Err(SandboxError::InvalidRequest(
                "local sandbox scenario expected an error but execution succeeded".to_string(),
            )),
        }
    }

    /// Produces the redacted comparison tuple only for an outcome that still
    /// satisfies this scenario's exact expectation. The scenario digest binds
    /// the omitted input, fixtures, policy, and expected result.
    pub fn comparison(
        &self,
        outcome: &LocalSandboxScenarioOutcome,
    ) -> SandboxResult<LocalSandboxScenarioComparison> {
        self.validate()?;
        let result = match (&self.expectation, outcome) {
            (
                LocalSandboxExpectation::Success { output },
                LocalSandboxScenarioOutcome::Success(actual),
            ) if &actual.output == output => LocalSandboxScenarioResult::Success,
            (
                LocalSandboxExpectation::Error { code: expected },
                LocalSandboxScenarioOutcome::ExpectedError { code: actual },
            ) if actual == expected => LocalSandboxScenarioResult::ExpectedError,
            _ => {
                return Err(SandboxError::InvalidRequest(
                    "local sandbox comparison outcome does not match its scenario".to_string(),
                ));
            }
        };
        Ok(LocalSandboxScenarioComparison {
            scenario_digest: self.canonical_digest()?,
            result,
        })
    }
}

fn validate_local_limits(policy: &SandboxPolicy) -> SandboxResult<()> {
    let limits = policy.limits;
    if limits.wall_clock_ms == 0
        || limits.wall_clock_ms > 10_000
        || limits.instruction_budget == 0
        || limits.instruction_budget > 10_000_000
        || limits.max_memory_bytes == 0
        || limits.max_memory_bytes > 256 * 1024 * 1024
        || limits.max_output_bytes == 0
        || limits.max_output_bytes > 4 * 1024 * 1024
        || limits.max_concurrency != 1
        || limits.max_capability_calls == 0
        || limits.max_capability_calls > 256
        || limits.max_capability_input_bytes == 0
        || limits.max_capability_input_bytes > 1024 * 1024
        || limits.max_capability_calls_per_second == 0
        || limits.max_capability_calls_per_second > 256
    {
        return Err(SandboxError::InvalidRequest(
            "local sandbox scenario limits are outside the authoring profile".to_string(),
        ));
    }
    Ok(())
}

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
        executors.register_in_process(crate::rhai::RhaiExecutor::new())?;
        Ok(Self::new(executors))
    }

    #[cfg(feature = "wasm-component")]
    pub fn wasm_component() -> SandboxResult<Self> {
        let mut executors = ExecutorRegistry::new();
        let executor = crate::wasm::WasmComponentExecutor::with_component_cache_policy(
            crate::wasm::WasmComponentCachePolicy::default(),
        )?;
        executors.register_in_process(executor)?;
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
                rhai_scope: None,
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
                audit_label: None,
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
            rhai_scope: None,
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
            .register_in_process(FixtureExecutor)
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

    #[tokio::test]
    async fn scenario_configures_policy_fixtures_and_expected_output() {
        let scenario = LocalSandboxScenario::parse(
            serde_json::to_vec(&json!({
                "schema_version": 1,
                "input": {"value": "input"},
                "policy": {
                    "grants": [{"name": "fixture.echo", "constraints": null}],
                    "limits": SandboxPolicy::default().limits
                },
                "fixtures": [{
                    "capability": "fixture.echo",
                    "operation": "call",
                    "output": {"value": "fixture"}
                }],
                "expectation": {
                    "outcome": "success",
                    "output": {"value": "fixture"}
                }
            }))
            .expect("scenario JSON")
            .as_slice(),
        )
        .expect("scenario");
        let mut executors = ExecutorRegistry::new();
        executors
            .register_in_process(FixtureExecutor)
            .expect("fixture executor");
        let harness = LocalSandboxHarness::new(executors);
        scenario
            .configure(&harness.fixtures())
            .expect("fixture configuration");
        let mut request = request();
        request.input = scenario.input.clone();
        request.policy = scenario.policy.clone();
        let result = scenario
            .evaluate(harness.execute(request).await)
            .expect("expected scenario outcome");
        assert!(matches!(result, LocalSandboxScenarioOutcome::Success(_)));
        assert_eq!(
            scenario.comparison(&result).expect("comparison"),
            LocalSandboxScenarioComparison {
                scenario_digest: scenario.canonical_digest().expect("scenario digest"),
                result: LocalSandboxScenarioResult::Success,
            }
        );
    }

    #[test]
    fn scenario_rejects_a_fixture_without_a_matching_grant() {
        let bytes = serde_json::to_vec(&json!({
            "schema_version": 1,
            "input": null,
            "policy": {"grants": [], "limits": SandboxPolicy::default().limits},
            "fixtures": [{
                "capability": "fixture.echo",
                "operation": "call",
                "output": null
            }],
            "expectation": {"outcome": "error", "code": "CAPABILITY_DENIED"}
        }))
        .expect("scenario JSON");
        assert!(matches!(
            LocalSandboxScenario::parse(&bytes),
            Err(SandboxError::InvalidRequest(_))
        ));
    }

    #[test]
    fn scenario_digest_is_stable_and_changes_with_the_contract() {
        let scenario = LocalSandboxScenario::parse(
            serde_json::to_vec(&json!({
                "schema_version": 1,
                "input": {"value": "input"},
                "policy": {
                    "grants": [],
                    "limits": SandboxPolicy::default().limits
                },
                "fixtures": [],
                "expectation": {
                    "outcome": "success",
                    "output": {"value": "output"}
                }
            }))
            .expect("scenario JSON")
            .as_slice(),
        )
        .expect("scenario");
        let digest = scenario.canonical_digest().expect("scenario digest");
        assert_eq!(
            digest,
            "sha256:b1d8a43f89551031131c687630f6191019c47a459ba6265d240e3d4cbfd00245"
        );
        assert_eq!(digest, scenario.canonical_digest().expect("stable digest"));

        let mut changed = scenario.clone();
        changed.input = json!({"value": "different"});
        assert_ne!(digest, changed.canonical_digest().expect("changed digest"));

        let mismatched = LocalSandboxScenarioOutcome::ExpectedError {
            code: "UNEXPECTED".to_string(),
        };
        assert!(matches!(
            scenario.comparison(&mismatched),
            Err(SandboxError::InvalidRequest(_))
        ));
    }
}
