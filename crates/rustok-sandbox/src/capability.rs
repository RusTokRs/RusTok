use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use url::Url;
use uuid::Uuid;

use crate::{
    ExecutionPhase, SandboxCancellation, SandboxContext, SandboxError, SandboxPolicy,
    SandboxResult, SandboxSubject,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct CapabilityName(String);

impl CapabilityName {
    pub fn new(value: impl Into<String>) -> SandboxResult<Self> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 96
            && value.chars().all(|character| {
                character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || matches!(character, '_' | '.' | ':')
            });
        if !valid {
            return Err(SandboxError::InvalidRequest(format!(
                "invalid capability name `{value}`"
            )));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CapabilityName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for CapabilityName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityGrant {
    pub name: CapabilityName,
    #[serde(default)]
    pub constraints: Value,
}

/// Typed policy for the `platform.http` capability.
///
/// A grant must name every allowed host, HTTP method and path prefix. Matching
/// is exact for hosts and methods and prefix-based for paths; there are no
/// implicit wildcards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpCapabilityConstraints {
    pub hosts: Vec<String>,
    pub methods: Vec<String>,
    pub path_prefixes: Vec<String>,
}

impl HttpCapabilityConstraints {
    fn from_grant(grant: &CapabilityGrant) -> SandboxResult<Self> {
        let constraints =
            serde_json::from_value::<Self>(grant.constraints.clone()).map_err(|error| {
                SandboxError::CapabilityConstraintDenied {
                    capability: grant.name.clone(),
                    reason: format!("invalid HTTP constraints: {error}"),
                }
            })?;
        if constraints.hosts.is_empty()
            || constraints.methods.is_empty()
            || constraints.path_prefixes.is_empty()
        {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason: "HTTP constraints require non-empty hosts, methods, and path_prefixes"
                    .to_string(),
            });
        }
        if constraints.hosts.iter().any(|host| host.trim().is_empty())
            || constraints
                .methods
                .iter()
                .any(|method| method.trim().is_empty())
            || constraints
                .path_prefixes
                .iter()
                .any(|prefix| !prefix.starts_with('/'))
        {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason:
                    "HTTP hosts and methods must be non-empty and path_prefixes must start with `/`"
                        .to_string(),
            });
        }
        Ok(constraints)
    }

    fn validate(&self, call: &CapabilityCall) -> SandboxResult<()> {
        let input =
            call.input
                .as_object()
                .ok_or_else(|| SandboxError::CapabilityConstraintDenied {
                    capability: call.capability.clone(),
                    reason: "HTTP input must be an object".to_string(),
                })?;
        let method = input.get("method").and_then(Value::as_str).ok_or_else(|| {
            SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: "HTTP input must contain a string method".to_string(),
            }
        })?;
        let raw_url = input.get("url").and_then(Value::as_str).ok_or_else(|| {
            SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: "HTTP input must contain a string url".to_string(),
            }
        })?;
        let url = Url::parse(raw_url).map_err(|_| SandboxError::CapabilityConstraintDenied {
            capability: call.capability.clone(),
            reason: "HTTP url must be absolute".to_string(),
        })?;
        let host = url
            .host_str()
            .ok_or_else(|| SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: "HTTP url must include a host".to_string(),
            })?;

        if !self
            .hosts
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(host))
        {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: format!("HTTP host `{host}` is not allowed"),
            });
        }
        if !self.methods.iter().any(|allowed| allowed == method) {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: format!("HTTP method `{method}` is not allowed"),
            });
        }
        if !self
            .path_prefixes
            .iter()
            .any(|prefix| url.path().starts_with(prefix))
        {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: format!("HTTP path `{}` is not allowed", url.path()),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityCall {
    pub execution_id: Uuid,
    pub subject: SandboxSubject,
    pub context: CapabilityCallContext,
    pub capability: CapabilityName,
    pub operation: String,
    #[serde(default)]
    pub input: Value,
}

/// Request identity propagated to every broker call.
///
/// The host compares this value with the active sandbox request before it
/// evaluates a grant or invokes a broker, preventing an adapter from invoking
/// a granted capability on behalf of another tenant or actor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityCallContext {
    pub phase: ExecutionPhase,
    pub tenant_id: Option<Uuid>,
    pub actor_id: Option<String>,
    pub trace_id: Option<String>,
}

impl From<&SandboxContext> for CapabilityCallContext {
    fn from(context: &SandboxContext) -> Self {
        Self {
            phase: context.phase,
            tenant_id: context.tenant_id,
            actor_id: context.actor_id.clone(),
            trace_id: context.trace_id.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityResponse {
    #[serde(default)]
    pub output: Value,
}

#[async_trait]
pub trait CapabilityBroker: Send + Sync {
    async fn invoke(
        &self,
        call: &CapabilityCall,
        grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse>;
}

/// Redacted evidence for one capability attempt.
///
/// This record intentionally excludes capability input, output, credentials and
/// broker error text. Durable observers can correlate a denial without turning
/// protected payload into audit data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityAuditRecord {
    pub execution_id: Uuid,
    pub subject: SandboxSubject,
    pub context: CapabilityCallContext,
    pub capability: CapabilityName,
    pub operation: String,
    pub timestamp: DateTime<Utc>,
    pub outcome: CapabilityAuditOutcome,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityAuditOutcome {
    Succeeded,
    Denied,
    Failed,
}

#[async_trait]
pub trait CapabilityObserver: Send + Sync {
    async fn observe(&self, record: &CapabilityAuditRecord);
}

#[derive(Clone)]
pub struct SandboxHost {
    policy: Arc<SandboxPolicy>,
    broker: Arc<dyn CapabilityBroker>,
    execution_id: Uuid,
    subject: SandboxSubject,
    context: CapabilityCallContext,
    budget: Arc<CapabilityBudget>,
    observers: Arc<Vec<Arc<dyn CapabilityObserver>>>,
    cancellation: SandboxCancellation,
}

#[derive(Debug, Default)]
struct CapabilityBudget {
    calls: AtomicU32,
    blocking_bridges: AtomicU32,
    rate_window: Mutex<VecDeque<Instant>>,
}

struct BlockingBridgePermit<'a>(&'a AtomicU32);

impl Drop for BlockingBridgePermit<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Release);
    }
}

impl SandboxHost {
    pub(crate) fn new(
        policy: Arc<SandboxPolicy>,
        broker: Arc<dyn CapabilityBroker>,
        subject: SandboxSubject,
        context: &SandboxContext,
        observers: Arc<Vec<Arc<dyn CapabilityObserver>>>,
        cancellation: SandboxCancellation,
    ) -> Self {
        Self {
            policy,
            broker,
            execution_id: context.execution_id,
            subject,
            context: CapabilityCallContext::from(context),
            budget: Arc::new(CapabilityBudget::default()),
            observers,
            cancellation,
        }
    }

    pub fn policy(&self) -> &SandboxPolicy {
        &self.policy
    }

    pub fn cancellation(&self) -> SandboxCancellation {
        self.cancellation.clone()
    }

    pub async fn invoke(&self, call: &CapabilityCall) -> SandboxResult<CapabilityResponse> {
        let result = self.invoke_inner(call).await;
        self.observe_capability(call, &result).await;
        result
    }

    async fn invoke_inner(&self, call: &CapabilityCall) -> SandboxResult<CapabilityResponse> {
        if self.cancellation.is_cancelled() {
            return Err(SandboxError::Cancelled);
        }
        self.validate_call_context(call)?;
        self.admit_capability_call(call)?;
        let grant = self
            .policy
            .grant(&call.capability)
            .ok_or_else(|| SandboxError::CapabilityDenied(call.capability.clone()))?;
        self.validate_constraints(call, grant)?;
        self.broker.invoke(call, grant).await
    }

    async fn observe_capability(
        &self,
        call: &CapabilityCall,
        result: &SandboxResult<CapabilityResponse>,
    ) {
        let (outcome, error_code) = match result {
            Ok(_) => (CapabilityAuditOutcome::Succeeded, None),
            Err(error) if is_denied(error) => (
                CapabilityAuditOutcome::Denied,
                Some(error.code().to_string()),
            ),
            Err(error) => (
                CapabilityAuditOutcome::Failed,
                Some(error.code().to_string()),
            ),
        };
        let record = CapabilityAuditRecord {
            execution_id: self.execution_id,
            subject: self.subject.clone(),
            context: self.context.clone(),
            capability: call.capability.clone(),
            operation: call.operation.clone(),
            timestamp: Utc::now(),
            outcome,
            error_code,
        };
        for observer in self.observers.iter() {
            observer.observe(&record).await;
        }
    }

    fn validate_call_context(&self, call: &CapabilityCall) -> SandboxResult<()> {
        if call.execution_id != self.execution_id {
            return Err(SandboxError::CapabilityContextMismatch {
                field: "execution_id",
            });
        }
        if call.subject != self.subject {
            return Err(SandboxError::CapabilityContextMismatch { field: "subject" });
        }
        if call.context != self.context {
            return Err(SandboxError::CapabilityContextMismatch { field: "context" });
        }
        Ok(())
    }

    fn admit_capability_call(&self, call: &CapabilityCall) -> SandboxResult<()> {
        let input_bytes = serde_json::to_vec(&call.input)
            .map_err(|error| SandboxError::Internal(error.to_string()))?
            .len() as u64;
        let limits = &self.policy.limits;
        if input_bytes > limits.max_capability_input_bytes {
            return Err(SandboxError::LimitExceeded {
                resource: "capability_input_bytes".to_string(),
                limit: limits.max_capability_input_bytes,
            });
        }

        self.admit_capability_rate(limits.max_capability_calls_per_second)?;

        let previous = self.budget.calls.fetch_add(1, Ordering::AcqRel);
        if previous >= limits.max_capability_calls {
            self.budget.calls.fetch_sub(1, Ordering::AcqRel);
            return Err(SandboxError::LimitExceeded {
                resource: "capability_calls".to_string(),
                limit: limits.max_capability_calls.into(),
            });
        }
        Ok(())
    }

    fn admit_capability_rate(&self, max_calls_per_second: u32) -> SandboxResult<()> {
        let now = Instant::now();
        let mut calls = self.budget.rate_window.lock().map_err(|_| {
            SandboxError::Internal("sandbox capability rate budget lock is poisoned".to_string())
        })?;
        while calls
            .front()
            .is_some_and(|started_at| now.duration_since(*started_at) >= Duration::from_secs(1))
        {
            calls.pop_front();
        }
        if calls.len() >= max_calls_per_second as usize {
            return Err(SandboxError::LimitExceeded {
                resource: "capability_calls_per_second".to_string(),
                limit: max_calls_per_second.into(),
            });
        }
        calls.push_back(now);
        Ok(())
    }

    fn validate_constraints(
        &self,
        call: &CapabilityCall,
        grant: &CapabilityGrant,
    ) -> SandboxResult<()> {
        if call.capability.as_str() == "platform.http" {
            HttpCapabilityConstraints::from_grant(grant)?.validate(call)?;
        }
        Ok(())
    }

    fn admit_blocking_bridge(&self) -> SandboxResult<BlockingBridgePermit<'_>> {
        self.budget
            .blocking_bridges
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| BlockingBridgePermit(&self.budget.blocking_bridges))
            .map_err(|_| SandboxError::LimitExceeded {
                resource: "blocking_capability_bridges".to_string(),
                limit: 1,
            })
    }

    /// Calls an async broker from a synchronous language binding.
    ///
    /// Rhai and synchronous Component Model imports use this bridge instead of
    /// opening their own network or storage clients. At most one native bridge
    /// thread may be active per execution. It requires an active Tokio runtime
    /// because the broker may perform async host I/O.
    pub fn invoke_blocking(&self, call: &CapabilityCall) -> SandboxResult<CapabilityResponse> {
        let handle = tokio::runtime::Handle::try_current().map_err(|error| {
            SandboxError::Internal(format!(
                "sandbox host capability requires an active Tokio runtime: {error}"
            ))
        })?;
        let _permit = self.admit_blocking_bridge()?;
        let host = self.clone();
        std::thread::scope(|scope| {
            scope
                .spawn(|| handle.block_on(host.invoke(call)))
                .join()
                .map_err(|_| {
                    SandboxError::Internal("sandbox host capability thread panicked".to_string())
                })?
        })
    }
}

fn is_denied(error: &SandboxError) -> bool {
    matches!(
        error,
        SandboxError::CapabilityDenied(_)
            | SandboxError::CapabilityConstraintDenied { .. }
            | SandboxError::CapabilityContextMismatch { .. }
    ) || matches!(error, SandboxError::LimitExceeded { resource, .. } if resource.starts_with("capability_"))
}
