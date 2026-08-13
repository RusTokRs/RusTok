use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, RwLock},
};

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_STRUCTURED_TASK_INPUT_BYTES: usize = 1_048_576;
pub const MAX_STRUCTURED_TASK_SCHEMA_BYTES: usize = 262_144;
pub const MAX_STRUCTURED_TASK_OUTPUT_BYTES: u32 = 1_048_576;
pub const MAX_STRUCTURED_TASK_EVIDENCE_ENTRIES: usize = 32;
pub const MAX_STRUCTURED_TASK_SYSTEM_PROMPT_BYTES: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiTaskDataClassification {
    Public,
    TenantPrivate,
    Personal,
    Sensitive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiStructuredTaskDescriptor {
    pub owner: String,
    pub task_slug: String,
    pub prompt_policy_digest: String,
    pub input_schema_digest: String,
    pub output_schema_digest: String,
    pub system_prompt: String,
    pub allowed_classifications: Vec<AiTaskDataClassification>,
    pub max_input_bytes: u32,
    pub max_output_bytes: u32,
    pub max_attempts: u16,
}

impl AiStructuredTaskDescriptor {
    pub fn validate(&self) -> Result<(), PortError> {
        require_identity("owner", &self.owner)?;
        require_identity("task_slug", &self.task_slug)?;
        require_digest("prompt_policy_digest", &self.prompt_policy_digest)?;
        require_digest("input_schema_digest", &self.input_schema_digest)?;
        require_digest("output_schema_digest", &self.output_schema_digest)?;
        if self.system_prompt.trim().is_empty()
            || self.system_prompt.len() > MAX_STRUCTURED_TASK_SYSTEM_PROMPT_BYTES
        {
            return Err(PortError::validation(
                "ai.structured.system_prompt_invalid",
                format!(
                    "structured task system prompt must contain 1..={MAX_STRUCTURED_TASK_SYSTEM_PROMPT_BYTES} bytes"
                ),
            ));
        }
        let unique_classifications = self
            .allowed_classifications
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if unique_classifications.is_empty()
            || unique_classifications.len() != self.allowed_classifications.len()
        {
            return Err(PortError::validation(
                "ai.structured.classification_policy_invalid",
                "structured task classification policy must be non-empty and unique",
            ));
        }
        if self.max_input_bytes == 0
            || usize::try_from(self.max_input_bytes)
                .map_or(true, |value| value > MAX_STRUCTURED_TASK_INPUT_BYTES)
            || self.max_output_bytes == 0
            || self.max_output_bytes > MAX_STRUCTURED_TASK_OUTPUT_BYTES
            || self.max_attempts == 0
            || self.max_attempts > 8
        {
            return Err(PortError::validation(
                "ai.structured.descriptor_limits_invalid",
                "structured task descriptor limits exceed the platform bounds",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct AiStructuredTaskCatalog {
    descriptors: Arc<RwLock<BTreeMap<(String, String), AiStructuredTaskDescriptor>>>,
}

impl AiStructuredTaskCatalog {
    pub fn register(&self, descriptor: AiStructuredTaskDescriptor) -> Result<(), PortError> {
        descriptor.validate()?;
        let key = (descriptor.owner.clone(), descriptor.task_slug.clone());
        let mut descriptors = self.descriptors.write().map_err(|_| {
            PortError::unavailable(
                "ai.structured.catalog_unavailable",
                "structured task catalog is unavailable",
            )
        })?;
        if let Some(existing) = descriptors.get(&key) {
            if existing == &descriptor {
                return Ok(());
            }
            return Err(PortError::conflict(
                "ai.structured.descriptor_conflict",
                "structured task identity is already registered with another contract",
            ));
        }
        descriptors.insert(key, descriptor);
        Ok(())
    }

    pub fn get(&self, owner: &str, task_slug: &str) -> Option<AiStructuredTaskDescriptor> {
        self.descriptors.read().ok().and_then(|descriptors| {
            descriptors
                .get(&(owner.to_string(), task_slug.to_string()))
                .cloned()
        })
    }

    pub fn descriptors(&self) -> Vec<AiStructuredTaskDescriptor> {
        self.descriptors
            .read()
            .map(|descriptors| descriptors.values().cloned().collect())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiStructuredTaskLimits {
    pub max_output_bytes: u32,
    pub max_attempts: u16,
}

impl Default for AiStructuredTaskLimits {
    fn default() -> Self {
        Self {
            max_output_bytes: MAX_STRUCTURED_TASK_OUTPUT_BYTES,
            max_attempts: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiStructuredTaskRequest {
    pub owner: String,
    pub task_slug: String,
    pub prompt_policy_digest: String,
    pub input_schema_digest: String,
    pub input: Value,
    pub output_schema: Value,
    pub classification: AiTaskDataClassification,
    pub evidence: BTreeMap<String, String>,
    pub limits: AiStructuredTaskLimits,
}

impl AiStructuredTaskRequest {
    pub fn validate(&self, context: &PortContext) -> Result<(), PortError> {
        context.require_policy(PortCallPolicy::write())?;
        require_identity("owner", &self.owner)?;
        require_identity("task_slug", &self.task_slug)?;
        require_digest("prompt_policy_digest", &self.prompt_policy_digest)?;
        require_digest("input_schema_digest", &self.input_schema_digest)?;

        if !self.output_schema.is_object() {
            return Err(PortError::validation(
                "ai.structured.output_schema_invalid",
                "structured task output_schema must be a JSON object",
            ));
        }

        let input_bytes = serialized_size(&self.input)?;
        if input_bytes > MAX_STRUCTURED_TASK_INPUT_BYTES {
            return Err(PortError::validation(
                "ai.structured.input_too_large",
                format!("structured task input exceeds {MAX_STRUCTURED_TASK_INPUT_BYTES} bytes"),
            ));
        }

        let schema_bytes = serialized_size(&self.output_schema)?;
        if schema_bytes > MAX_STRUCTURED_TASK_SCHEMA_BYTES {
            return Err(PortError::validation(
                "ai.structured.output_schema_too_large",
                format!(
                    "structured task output schema exceeds {MAX_STRUCTURED_TASK_SCHEMA_BYTES} bytes"
                ),
            ));
        }

        validate_limits(&self.limits)?;

        if self.evidence.len() > MAX_STRUCTURED_TASK_EVIDENCE_ENTRIES
            || self.evidence.iter().any(|(key, value)| {
                key.trim().is_empty()
                    || key.len() > 64
                    || value.trim().is_empty()
                    || value.len() > 256
            })
        {
            return Err(PortError::validation(
                "ai.structured.evidence_invalid",
                "structured task evidence must contain at most 32 bounded non-empty entries",
            ));
        }
        Ok(())
    }

    /// Produces the content-free request binding persisted with a durable
    /// execution. Callers can compare it on recovery without retaining the
    /// structured input or depending on actor-specific execution hashes.
    pub fn binding(&self) -> Result<AiStructuredTaskRequestBinding, PortError> {
        let binding = AiStructuredTaskRequestBinding {
            owner: self.owner.clone(),
            task_slug: self.task_slug.clone(),
            prompt_policy_digest: self.prompt_policy_digest.clone(),
            input_schema_digest: self.input_schema_digest.clone(),
            input_digest: manifest_digest(&self.input)?,
            output_schema_digest: manifest_digest(&self.output_schema)?,
            classification: self.classification,
            evidence_digest: manifest_digest(&self.evidence)?,
            limits: self.limits.clone(),
        };
        binding.validate()?;
        Ok(binding)
    }
}

/// Content-free, durable binding between a structured-task request and an
/// execution. It intentionally excludes raw input, output schema, and
/// evidence values while preserving every request field that affects execution
/// semantics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiStructuredTaskRequestBinding {
    pub owner: String,
    pub task_slug: String,
    pub prompt_policy_digest: String,
    pub input_schema_digest: String,
    pub input_digest: String,
    pub output_schema_digest: String,
    pub classification: AiTaskDataClassification,
    pub evidence_digest: String,
    pub limits: AiStructuredTaskLimits,
}

impl AiStructuredTaskRequestBinding {
    pub fn validate(&self) -> Result<(), PortError> {
        require_identity("owner", &self.owner)?;
        require_identity("task_slug", &self.task_slug)?;
        require_digest("prompt_policy_digest", &self.prompt_policy_digest)?;
        require_digest("input_schema_digest", &self.input_schema_digest)?;
        require_digest("input_digest", &self.input_digest)?;
        require_digest("output_schema_digest", &self.output_schema_digest)?;
        require_digest("evidence_digest", &self.evidence_digest)?;
        validate_limits(&self.limits)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiStructuredTaskStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiStructuredTaskAvailability {
    Available,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiStructuredTaskHealth {
    pub availability: AiStructuredTaskAvailability,
    pub reason_code: Option<String>,
    pub retry_after_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiStructuredTaskHealthRequest {
    pub task_slug: String,
    pub classification: AiTaskDataClassification,
}

impl AiStructuredTaskHealthRequest {
    pub fn validate(&self) -> Result<(), PortError> {
        require_identity("task_slug", &self.task_slug)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiStructuredTaskEstimate {
    pub input_tokens_upper_bound: u64,
    pub output_tokens_upper_bound: u64,
    pub attempts_upper_bound: u16,
    pub cost_minor_units_upper_bound: u64,
    pub currency_code: String,
    pub price_snapshot_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiStructuredTaskUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cost_minor_units: u64,
    pub currency_code: String,
    pub price_snapshot_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiStructuredTaskAttempt {
    pub attempt: u16,
    pub provider_profile_id: String,
    pub provider_slug: String,
    pub model: String,
    pub fallback: bool,
    pub status: AiStructuredTaskStatus,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiStructuredTaskExecution {
    pub execution_id: String,
    pub request_digest: String,
    pub binding: AiStructuredTaskRequestBinding,
    pub status: AiStructuredTaskStatus,
    pub output: Option<Value>,
    pub attempts: Vec<AiStructuredTaskAttempt>,
    pub usage: Option<AiStructuredTaskUsage>,
    pub retry_after_ms: Option<u64>,
}

impl AiStructuredTaskExecution {
    pub fn validate_completed_output(&self, max_output_bytes: u32) -> Result<&Value, PortError> {
        if self.status != AiStructuredTaskStatus::Completed {
            return Err(PortError::unavailable(
                "ai.structured.execution_incomplete",
                "structured task execution has not completed",
            ));
        }
        let output = self.output.as_ref().ok_or_else(|| {
            PortError::invariant_violation(
                "ai.structured.output_missing",
                "completed structured task execution has no output",
            )
        })?;
        if serialized_size(output)? > max_output_bytes as usize {
            return Err(PortError::invariant_violation(
                "ai.structured.output_too_large",
                "structured task output exceeds the accepted response limit",
            ));
        }
        Ok(output)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiStructuredTaskExecutionRef {
    pub execution_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiStructuredTaskExecutionKey {
    pub owner: String,
    pub idempotency_key: String,
}

impl AiStructuredTaskExecutionKey {
    pub fn validate(&self) -> Result<(), PortError> {
        require_identity("owner", &self.owner)?;
        if self.idempotency_key.trim().is_empty() || self.idempotency_key.len() > 191 {
            return Err(PortError::validation(
                "ai.structured.idempotency_key_invalid",
                "structured task idempotency key must contain 1..=191 non-whitespace bytes",
            ));
        }
        Ok(())
    }
}

#[async_trait]
pub trait AiStructuredTaskPort: Send + Sync {
    async fn health(
        &self,
        context: PortContext,
        request: AiStructuredTaskHealthRequest,
    ) -> Result<AiStructuredTaskHealth, PortError>;

    /// Non-billable conservative cost projection. Implementations must use the
    /// same tenant routing and immutable pricing policies as `execute`, without
    /// registering an execution, reserving budget, or calling a provider.
    async fn estimate(
        &self,
        context: PortContext,
        request: AiStructuredTaskRequest,
    ) -> Result<AiStructuredTaskEstimate, PortError>;

    /// Billable structured inference. Implementations must enforce write-like
    /// deadline and idempotency semantics and persist execution evidence.
    async fn execute(
        &self,
        context: PortContext,
        request: AiStructuredTaskRequest,
    ) -> Result<AiStructuredTaskExecution, PortError>;

    async fn status(
        &self,
        context: PortContext,
        execution: AiStructuredTaskExecutionRef,
    ) -> Result<AiStructuredTaskExecution, PortError>;

    /// Resolve a durable execution without requiring the caller to have
    /// observed its generated execution id before a crash or timeout.
    async fn resolve(
        &self,
        context: PortContext,
        execution: AiStructuredTaskExecutionKey,
    ) -> Result<Option<AiStructuredTaskExecution>, PortError>;

    async fn cancel(
        &self,
        context: PortContext,
        execution: AiStructuredTaskExecutionRef,
    ) -> Result<AiStructuredTaskExecution, PortError>;

    /// Persist cancellation against the stable owner/idempotency identity.
    /// The intent must also stop a matching execution registered after this
    /// call returns, closing the submit/cancel race.
    async fn cancel_by_key(
        &self,
        context: PortContext,
        execution: AiStructuredTaskExecutionKey,
    ) -> Result<Option<AiStructuredTaskExecution>, PortError>;
}

fn require_identity(field: &'static str, value: &str) -> Result<(), PortError> {
    if value.trim().is_empty() || value.len() > 128 {
        return Err(PortError::validation(
            format!("ai.structured.{field}_invalid"),
            format!("{field} must contain 1..=128 non-whitespace bytes"),
        ));
    }
    Ok(())
}

fn require_digest(field: &'static str, value: &str) -> Result<(), PortError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(PortError::validation(
            format!("ai.structured.{field}_invalid"),
            format!("{field} must be a SHA-256 hex digest"),
        ));
    }
    Ok(())
}

fn validate_limits(limits: &AiStructuredTaskLimits) -> Result<(), PortError> {
    if limits.max_output_bytes == 0 || limits.max_output_bytes > MAX_STRUCTURED_TASK_OUTPUT_BYTES {
        return Err(PortError::validation(
            "ai.structured.output_limit_invalid",
            format!("max_output_bytes must be between 1 and {MAX_STRUCTURED_TASK_OUTPUT_BYTES}"),
        ));
    }
    if limits.max_attempts == 0 || limits.max_attempts > 8 {
        return Err(PortError::validation(
            "ai.structured.attempt_limit_invalid",
            "max_attempts must be between 1 and 8",
        ));
    }
    Ok(())
}

fn manifest_digest<T: Serialize>(value: &T) -> Result<String, PortError> {
    rustok_api::manifest_hash::hash_manifest(value).map_err(|_| {
        PortError::invariant_violation(
            "ai.structured.digest_failed",
            "structured task request could not be hashed",
        )
    })
}

fn serialized_size(value: &Value) -> Result<usize, PortError> {
    serde_json::to_vec(value)
        .map(|encoded| encoded.len())
        .map_err(|_| {
            PortError::validation(
                "ai.structured.json_invalid",
                "structured task JSON could not be serialized",
            )
        })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rustok_api::{PortActor, PortErrorKind};
    use serde_json::json;

    use super::*;

    fn context() -> PortContext {
        PortContext::new("tenant-a", PortActor::service("service-a"), "en", "corr-a")
            .with_idempotency_key("idem-a")
            .with_deadline(Duration::from_secs(5))
    }

    fn sample_request() -> AiStructuredTaskRequest {
        AiStructuredTaskRequest {
            owner: "translation".to_string(),
            task_slug: "machine_translation".to_string(),
            prompt_policy_digest: "a".repeat(64),
            input_schema_digest: "b".repeat(64),
            input: json!({"units": []}),
            output_schema: json!({"type": "object"}),
            classification: AiTaskDataClassification::TenantPrivate,
            evidence: BTreeMap::from([("job_id".to_string(), "job-a".to_string())]),
            limits: AiStructuredTaskLimits::default(),
        }
    }

    fn descriptor() -> AiStructuredTaskDescriptor {
        AiStructuredTaskDescriptor {
            owner: "translation".to_string(),
            task_slug: "machine_translation".to_string(),
            prompt_policy_digest: "a".repeat(64),
            input_schema_digest: "b".repeat(64),
            output_schema_digest: "c".repeat(64),
            system_prompt: "Translate the bounded structured input.".to_string(),
            allowed_classifications: vec![
                AiTaskDataClassification::Public,
                AiTaskDataClassification::TenantPrivate,
            ],
            max_input_bytes: 4096,
            max_output_bytes: 4096,
            max_attempts: 3,
        }
    }

    #[test]
    fn billable_execution_requires_write_semantics() {
        let error = sample_request()
            .validate(&PortContext::new(
                "tenant-a",
                PortActor::service("service-a"),
                "en",
                "corr-a",
            ))
            .unwrap_err();
        assert_eq!(error.kind, PortErrorKind::Validation);
        assert_eq!(error.code, "port.idempotency_key_required");
    }

    #[test]
    fn request_rejects_unbounded_schema_and_attempts() {
        let mut request = sample_request();
        request.output_schema = json!(["not", "an", "object"]);
        assert_eq!(
            request.validate(&context()).unwrap_err().code,
            "ai.structured.output_schema_invalid"
        );

        let mut request = sample_request();
        request.limits.max_attempts = 9;
        assert_eq!(
            request.validate(&context()).unwrap_err().code,
            "ai.structured.attempt_limit_invalid"
        );
    }

    #[test]
    fn completed_execution_requires_bounded_output() {
        let binding = sample_request().binding().unwrap();
        let execution = AiStructuredTaskExecution {
            execution_id: "execution-a".to_string(),
            request_digest: "c".repeat(64),
            binding,
            status: AiStructuredTaskStatus::Completed,
            output: Some(json!({"ok": true})),
            attempts: Vec::new(),
            usage: None,
            retry_after_ms: None,
        };
        assert_eq!(
            execution
                .validate_completed_output(MAX_STRUCTURED_TASK_OUTPUT_BYTES)
                .unwrap(),
            &json!({"ok": true})
        );
    }

    #[test]
    fn catalog_is_idempotent_and_rejects_contract_drift() {
        let catalog = AiStructuredTaskCatalog::default();
        catalog.register(descriptor()).unwrap();
        catalog.register(descriptor()).unwrap();
        assert_eq!(catalog.descriptors(), vec![descriptor()]);

        let mut changed = descriptor();
        changed.system_prompt = "Changed policy".to_string();
        assert_eq!(
            catalog.register(changed).unwrap_err().code,
            "ai.structured.descriptor_conflict"
        );
    }

    #[test]
    fn request_binding_changes_when_execution_semantics_change() {
        let request = sample_request();
        let binding = request.binding().unwrap();
        binding.validate().unwrap();

        let mut changed = request;
        changed
            .evidence
            .insert("item_id".to_string(), "item-a".to_string());
        assert_ne!(binding, changed.binding().unwrap());

        let mut changed = sample_request();
        changed.limits.max_attempts = 2;
        assert_ne!(binding, changed.binding().unwrap());
    }
}
