use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use rustok_api::{PortContext, PortError, PortErrorKind, manifest_hash::hash_manifest};
use tokio::time::{Instant, MissedTickBehavior};
use uuid::Uuid;

use crate::{
    AiError, AiHostRuntime, AiManagementService, AiStructuredTaskAvailability,
    AiStructuredTaskCatalog, AiStructuredTaskDescriptor, AiStructuredTaskExecution,
    AiStructuredTaskExecutionRef, AiStructuredTaskHealth, AiStructuredTaskPort,
    AiStructuredTaskRequest, ChatMessage, ChatMessageRole, InferenceEngine, ProviderCapability,
    ProviderChatRequest, ProviderStructuredRequest, ProviderStructuredResponse,
    RouterProviderProfile,
    accounting::{AttemptOutcome, StructuredAccounting, TerminalOutcome},
    router::ordered_provider_candidates,
    service::{
        list_router_provider_profiles, provider_config, require_provider_profile,
        runtime_inference_engine, task_profile_runtime,
    },
    structured::StructuredExecutionLedger,
};

const CANCELLATION_POLL_INTERVAL: Duration = Duration::from_millis(100);
const LEASE_RECOVERY_GRACE: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub(crate) struct DurableAiStructuredTaskPort {
    runtime: AiHostRuntime,
    catalog: AiStructuredTaskCatalog,
    ledger: StructuredExecutionLedger,
    accounting: StructuredAccounting,
}

#[derive(Clone)]
struct ProviderCandidate {
    profile: RouterProviderProfile,
    config: crate::AiProviderConfig,
}

#[derive(Debug, Clone)]
struct AttemptFailure {
    kind: PortErrorKind,
    code: String,
    retryable: bool,
    retry_after_ms: Option<u64>,
}

enum ProviderCall {
    Response(ProviderStructuredResponse),
    Failed(AttemptFailure),
    Cancelled,
    DeadlineExceeded,
}

impl DurableAiStructuredTaskPort {
    pub(crate) fn new(runtime: AiHostRuntime, catalog: AiStructuredTaskCatalog) -> Self {
        let database = runtime.db_clone();
        Self {
            runtime,
            catalog,
            ledger: StructuredExecutionLedger::new(database.clone()),
            accounting: StructuredAccounting::new(database),
        }
    }

    async fn descriptor_for_health(
        &self,
        task_slug: &str,
    ) -> Result<AiStructuredTaskDescriptor, PortError> {
        let matches = self
            .catalog
            .descriptors()
            .into_iter()
            .filter(|descriptor| descriptor.task_slug == task_slug)
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [descriptor] => Ok(descriptor.clone()),
            [] => Err(PortError::not_found(
                "ai.structured.task_not_registered",
                "structured task is not registered",
            )),
            _ => Err(PortError::conflict(
                "ai.structured.task_identity_ambiguous",
                "structured task slug is registered by more than one owner",
            )),
        }
    }

    fn validate_descriptor(
        catalog: &AiStructuredTaskCatalog,
        request: &AiStructuredTaskRequest,
    ) -> Result<AiStructuredTaskDescriptor, PortError> {
        let descriptor = catalog
            .get(&request.owner, &request.task_slug)
            .ok_or_else(|| {
                PortError::not_found(
                    "ai.structured.task_not_registered",
                    "structured task is not registered",
                )
            })?;
        let output_schema_digest =
            hash_manifest(&request.output_schema).map_err(|_| runtime_invariant())?;
        let input_bytes = serde_json::to_vec(&request.input)
            .map_err(|_| runtime_invariant())?
            .len();
        if request.prompt_policy_digest != descriptor.prompt_policy_digest
            || request.input_schema_digest != descriptor.input_schema_digest
            || output_schema_digest != descriptor.output_schema_digest
        {
            return Err(PortError::conflict(
                "ai.structured.task_contract_drift",
                "structured task request does not match its registered policy and schemas",
            ));
        }
        if !descriptor
            .allowed_classifications
            .contains(&request.classification)
        {
            return Err(PortError::forbidden(
                "ai.structured.classification_denied",
                "structured task data classification is not allowed by its descriptor",
            ));
        }
        if input_bytes > descriptor.max_input_bytes as usize
            || request.limits.max_output_bytes > descriptor.max_output_bytes
            || request.limits.max_attempts > descriptor.max_attempts
        {
            return Err(PortError::validation(
                "ai.structured.task_limits_exceeded",
                "structured task request exceeds its registered limits",
            ));
        }
        Ok(descriptor)
    }

    async fn candidates(
        &self,
        tenant_id: Uuid,
        task_slug: &str,
        roles: &[String],
    ) -> Result<Vec<ProviderCandidate>, PortError> {
        let task = AiManagementService::list_task_profiles(self.runtime.db(), tenant_id)
            .await
            .map_err(map_runtime_error)?
            .into_iter()
            .find(|profile| profile.slug == task_slug && profile.is_active)
            .ok_or_else(|| {
                PortError::unavailable(
                    "ai.structured.task_profile_unavailable",
                    "structured task has no active tenant routing profile",
                )
            })?;
        if task.target_capability != ProviderCapability::StructuredGeneration {
            return Err(PortError::conflict(
                "ai.structured.task_profile_capability_invalid",
                "structured task routing profile does not target structured generation",
            ));
        }
        if task.fallback_strategy != "ordered" {
            return Err(PortError::conflict(
                "ai.structured.fallback_strategy_invalid",
                "structured task routing profile must use ordered fallback",
            ));
        }
        let task = task_profile_runtime(&task);
        let providers = list_router_provider_profiles(self.runtime.db(), tenant_id)
            .await
            .map_err(map_runtime_error)?;
        let ordered = ordered_provider_candidates(&task, &providers, roles);
        let mut candidates = Vec::with_capacity(ordered.len());
        for profile in ordered {
            let model = require_provider_profile(self.runtime.db(), tenant_id, profile.id)
                .await
                .map_err(map_runtime_error)?;
            let config = provider_config(
                &model,
                self.runtime.provider_targets(),
                self.runtime.egress_policy(),
            )
            .map_err(map_runtime_error)?;
            candidates.push(ProviderCandidate { profile, config });
        }
        if candidates.is_empty() {
            return Err(PortError::unavailable(
                "ai.structured.provider_unavailable",
                "no structured generation provider is eligible",
            ));
        }
        Ok(candidates)
    }

    async fn run(
        &self,
        context: &PortContext,
        request: &AiStructuredTaskRequest,
        descriptor: &AiStructuredTaskDescriptor,
        execution_id: Uuid,
        lease_token: Uuid,
        candidates: Vec<ProviderCandidate>,
        deadline: Instant,
    ) -> Result<AiStructuredTaskExecution, PortError> {
        let provider_request = ProviderChatRequest {
            model: String::new(),
            messages: vec![
                ChatMessage {
                    role: ChatMessageRole::System,
                    content: Some(descriptor.system_prompt.clone()),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                    metadata: serde_json::json!({"policy_digest": descriptor.prompt_policy_digest}),
                },
                ChatMessage {
                    role: ChatMessageRole::User,
                    content: Some(
                        serde_json::to_string(&request.input).map_err(|_| runtime_invariant())?,
                    ),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                    metadata: serde_json::json!({"content_type": "application/json"}),
                },
            ],
            tools: Vec::new(),
            temperature: None,
            max_tokens: None,
            locale: Some(context.locale.clone()),
        };
        let mut last_failure = None;
        for candidate in candidates
            .into_iter()
            .take(usize::from(request.limits.max_attempts))
        {
            if self
                .ledger
                .cancellation_requested(execution_id, lease_token)
                .await?
            {
                self.accounting
                    .finalize(execution_id, lease_token, TerminalOutcome::Cancelled)
                    .await?;
                return self.ledger.view(context, execution_id).await;
            }
            if Instant::now() >= deadline {
                let failure = deadline_failure();
                self.accounting
                    .finalize(
                        execution_id,
                        lease_token,
                        TerminalOutcome::Failed {
                            error_code: failure.code.clone(),
                            retryable: failure.retryable,
                            retry_after_ms: failure.retry_after_ms,
                        },
                    )
                    .await?;
                return Err(failure.into_port_error());
            }
            let attempt = match self
                .accounting
                .begin_attempt(
                    execution_id,
                    lease_token,
                    candidate.profile.id,
                    candidate.profile.provider_slug.as_str(),
                    &candidate.profile.model,
                )
                .await
            {
                Ok(attempt) => attempt,
                Err(error) if error.retryable => {
                    last_failure = Some(AttemptFailure::from_port_error(error));
                    continue;
                }
                Err(error) => return Err(error),
            };
            let engine = match runtime_inference_engine(
                &self.runtime,
                &candidate.profile.provider_slug,
                &candidate.config,
            )
            .await
            {
                Ok(engine) => engine,
                Err(error) => {
                    let failure = classify_ai_error(&error);
                    self.accounting
                        .finish_attempt(
                            execution_id,
                            lease_token,
                            attempt.model.id,
                            AttemptOutcome::Failed {
                                usage: None,
                                error_code: failure.code.clone(),
                                retryable: failure.retryable,
                                retry_after_ms: failure.retry_after_ms,
                            },
                        )
                        .await?;
                    last_failure = Some(failure);
                    continue;
                }
            };
            let mut attempt_request = provider_request.clone();
            attempt_request.model = candidate.config.model.clone();
            attempt_request.temperature = candidate.config.temperature;
            attempt_request.max_tokens = candidate.config.max_tokens;
            match self
                .call_provider(
                    execution_id,
                    lease_token,
                    engine,
                    ProviderStructuredRequest {
                        request: attempt_request,
                        output_schema: request.output_schema.clone(),
                    },
                    deadline,
                )
                .await?
            {
                ProviderCall::Response(response) => {
                    let output_bytes = serde_json::to_vec(&response.output)
                        .map_err(|_| runtime_invariant())?
                        .len();
                    let failure = if output_bytes > request.limits.max_output_bytes as usize {
                        Some(AttemptFailure::provider_contract(
                            "ai.structured.provider_output_too_large",
                        ))
                    } else if response.usage.as_ref().is_none_or(|usage| {
                        usage.total_tokens != usage.input_tokens.saturating_add(usage.output_tokens)
                    }) {
                        Some(AttemptFailure::provider_contract(
                            "ai.structured.provider_usage_invalid",
                        ))
                    } else {
                        None
                    };
                    if let Some(failure) = failure {
                        self.accounting
                            .finish_attempt(
                                execution_id,
                                lease_token,
                                attempt.model.id,
                                AttemptOutcome::Failed {
                                    usage: None,
                                    error_code: failure.code.clone(),
                                    retryable: failure.retryable,
                                    retry_after_ms: failure.retry_after_ms,
                                },
                            )
                            .await?;
                        last_failure = Some(failure);
                        continue;
                    }
                    let usage = response.usage.ok_or_else(runtime_invariant)?;
                    self.accounting
                        .finish_attempt(
                            execution_id,
                            lease_token,
                            attempt.model.id,
                            AttemptOutcome::Completed { usage },
                        )
                        .await?;
                    self.accounting
                        .finalize(execution_id, lease_token, TerminalOutcome::Completed)
                        .await?;
                    let mut execution = self.ledger.view(context, execution_id).await?;
                    execution.output = Some(response.output);
                    return Ok(execution);
                }
                ProviderCall::Failed(failure) => {
                    self.accounting
                        .finish_attempt(
                            execution_id,
                            lease_token,
                            attempt.model.id,
                            AttemptOutcome::Failed {
                                usage: None,
                                error_code: failure.code.clone(),
                                retryable: failure.retryable,
                                retry_after_ms: failure.retry_after_ms,
                            },
                        )
                        .await?;
                    last_failure = Some(failure);
                }
                ProviderCall::Cancelled => {
                    self.accounting
                        .finish_attempt(
                            execution_id,
                            lease_token,
                            attempt.model.id,
                            AttemptOutcome::Cancelled { usage: None },
                        )
                        .await?;
                    self.accounting
                        .finalize(execution_id, lease_token, TerminalOutcome::Cancelled)
                        .await?;
                    return self.ledger.view(context, execution_id).await;
                }
                ProviderCall::DeadlineExceeded => {
                    let failure = deadline_failure();
                    self.accounting
                        .finish_attempt(
                            execution_id,
                            lease_token,
                            attempt.model.id,
                            AttemptOutcome::Failed {
                                usage: None,
                                error_code: failure.code.clone(),
                                retryable: failure.retryable,
                                retry_after_ms: failure.retry_after_ms,
                            },
                        )
                        .await?;
                    self.accounting
                        .finalize(
                            execution_id,
                            lease_token,
                            TerminalOutcome::Failed {
                                error_code: failure.code.clone(),
                                retryable: failure.retryable,
                                retry_after_ms: failure.retry_after_ms,
                            },
                        )
                        .await?;
                    return Err(failure.into_port_error());
                }
            }
        }
        let failure = last_failure.unwrap_or_else(|| AttemptFailure {
            kind: PortErrorKind::Unavailable,
            code: "ai.structured.provider_unavailable".to_string(),
            retryable: true,
            retry_after_ms: None,
        });
        self.accounting
            .finalize(
                execution_id,
                lease_token,
                TerminalOutcome::Failed {
                    error_code: failure.code.clone(),
                    retryable: failure.retryable,
                    retry_after_ms: failure.retry_after_ms,
                },
            )
            .await?;
        Err(failure.into_port_error())
    }

    async fn call_provider(
        &self,
        execution_id: Uuid,
        lease_token: Uuid,
        engine: Arc<dyn InferenceEngine>,
        request: ProviderStructuredRequest,
        deadline: Instant,
    ) -> Result<ProviderCall, PortError> {
        let call = engine.complete_structured(request);
        tokio::pin!(call);
        let deadline_sleep = tokio::time::sleep_until(deadline);
        tokio::pin!(deadline_sleep);
        let mut cancellation = tokio::time::interval(CANCELLATION_POLL_INTERVAL);
        cancellation.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                response = &mut call => {
                    return Ok(match response {
                        Ok(response) => ProviderCall::Response(response),
                        Err(error) => ProviderCall::Failed(classify_ai_error(&error)),
                    });
                }
                _ = &mut deadline_sleep => return Ok(ProviderCall::DeadlineExceeded),
                _ = cancellation.tick() => {
                    if self
                        .ledger
                        .cancellation_requested(execution_id, lease_token)
                        .await?
                    {
                        return Ok(ProviderCall::Cancelled);
                    }
                }
            }
        }
    }
}

#[async_trait]
impl AiStructuredTaskPort for DurableAiStructuredTaskPort {
    async fn health(
        &self,
        context: PortContext,
        task_slug: String,
    ) -> Result<AiStructuredTaskHealth, PortError> {
        context.require_read_semantics()?;
        self.descriptor_for_health(&task_slug).await?;
        let tenant_id = parse_tenant_id(&context)?;
        match self.candidates(tenant_id, &task_slug, &context.roles).await {
            Ok(_) => Ok(AiStructuredTaskHealth {
                availability: AiStructuredTaskAvailability::Available,
                reason_code: None,
                retry_after_ms: None,
            }),
            Err(error) => Ok(AiStructuredTaskHealth {
                availability: if error.retryable {
                    AiStructuredTaskAvailability::Degraded
                } else {
                    AiStructuredTaskAvailability::Unavailable
                },
                reason_code: Some(error.code),
                retry_after_ms: None,
            }),
        }
    }

    async fn execute(
        &self,
        context: PortContext,
        request: AiStructuredTaskRequest,
    ) -> Result<AiStructuredTaskExecution, PortError> {
        request.validate(&context)?;
        let deadline =
            Instant::now() + Duration::from_millis(context.deadline_ms.unwrap_or_default());
        let descriptor = Self::validate_descriptor(&self.catalog, &request)?;
        let tenant_id = parse_tenant_id(&context)?;
        let mut candidates = self
            .candidates(tenant_id, &request.task_slug, &context.roles)
            .await?;
        candidates.truncate(usize::from(request.limits.max_attempts));
        let registered = self.ledger.register(&context, &request).await?;
        if registered.replayed && registered.execution.status != "queued" {
            return self.ledger.view(&context, registered.execution.id).await;
        }
        self.accounting
            .reserve(
                registered.execution.id,
                &candidates
                    .iter()
                    .map(|candidate| candidate.profile.id)
                    .collect::<Vec<_>>(),
            )
            .await?;
        if Instant::now() >= deadline {
            self.ledger
                .request_cancel(&context, registered.execution.id)
                .await?;
            self.accounting
                .cancel_queued(registered.execution.id)
                .await?;
            return Err(deadline_failure().into_port_error());
        }
        let lease_duration = deadline
            .saturating_duration_since(Instant::now())
            .saturating_add(LEASE_RECOVERY_GRACE);
        let Some(lease) = self
            .ledger
            .claim(registered.execution.id, lease_duration)
            .await?
        else {
            return self.ledger.view(&context, registered.execution.id).await;
        };
        self.run(
            &context,
            &request,
            &descriptor,
            registered.execution.id,
            lease.token,
            candidates,
            deadline,
        )
        .await
    }

    async fn status(
        &self,
        context: PortContext,
        execution: AiStructuredTaskExecutionRef,
    ) -> Result<AiStructuredTaskExecution, PortError> {
        context.require_read_semantics()?;
        self.ledger
            .view(&context, parse_execution_id(&execution)?)
            .await
    }

    async fn cancel(
        &self,
        context: PortContext,
        execution: AiStructuredTaskExecutionRef,
    ) -> Result<AiStructuredTaskExecution, PortError> {
        let execution_id = parse_execution_id(&execution)?;
        let cancelled = self.ledger.request_cancel(&context, execution_id).await?;
        if cancelled.status == "queued" {
            self.accounting.cancel_queued(execution_id).await?;
        }
        self.ledger.view(&context, execution_id).await
    }
}

impl AttemptFailure {
    fn provider_contract(code: &'static str) -> Self {
        Self {
            kind: PortErrorKind::Unavailable,
            code: code.to_string(),
            retryable: true,
            retry_after_ms: None,
        }
    }

    fn from_port_error(error: PortError) -> Self {
        Self {
            kind: error.kind,
            code: error.code,
            retryable: error.retryable,
            retry_after_ms: None,
        }
    }

    fn into_port_error(self) -> PortError {
        PortError::new(
            self.kind,
            self.code,
            "structured generation provider could not complete the task",
            self.retryable,
        )
    }
}

fn classify_ai_error(error: &AiError) -> AttemptFailure {
    match error {
        AiError::Transport(error) if error.is_timeout() => deadline_failure(),
        AiError::Transport(error) => AttemptFailure {
            kind: PortErrorKind::Unavailable,
            code: match error.status().map(|status| status.as_u16()) {
                Some(429) => "ai.structured.provider_rate_limited",
                Some(status) if status >= 500 => "ai.structured.provider_server_error",
                Some(_) => "ai.structured.provider_request_rejected",
                None => "ai.structured.provider_transport",
            }
            .to_string(),
            retryable: error
                .status()
                .is_none_or(|status| status.as_u16() == 429 || status.is_server_error()),
            retry_after_ms: None,
        },
        AiError::Provider(_) => AttemptFailure {
            kind: PortErrorKind::Unavailable,
            code: "ai.structured.provider_error".to_string(),
            retryable: true,
            retry_after_ms: None,
        },
        AiError::InvalidConfig(_) | AiError::NotFound(_) => AttemptFailure {
            kind: PortErrorKind::Unavailable,
            code: "ai.structured.provider_configuration_invalid".to_string(),
            retryable: false,
            retry_after_ms: None,
        },
        AiError::Runtime(_) => AttemptFailure {
            kind: PortErrorKind::Unavailable,
            code: "ai.structured.provider_runtime_unavailable".to_string(),
            retryable: true,
            retry_after_ms: None,
        },
        AiError::Validation(_) | AiError::Serialization(_) | AiError::Json(_) => {
            AttemptFailure::provider_contract("ai.structured.provider_response_invalid")
        }
        AiError::Mcp(_) | AiError::ApprovalRequired(_) => AttemptFailure {
            kind: PortErrorKind::InvariantViolation,
            code: "ai.structured.provider_path_invalid".to_string(),
            retryable: false,
            retry_after_ms: None,
        },
    }
}

fn deadline_failure() -> AttemptFailure {
    AttemptFailure {
        kind: PortErrorKind::Timeout,
        code: "ai.structured.deadline_exceeded".to_string(),
        retryable: true,
        retry_after_ms: Some(0),
    }
}

fn map_runtime_error(error: AiError) -> PortError {
    classify_ai_error(&error).into_port_error()
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "ai.structured.tenant_id_invalid",
            "structured task tenant_id must be a UUID",
        )
    })
}

fn parse_execution_id(execution: &AiStructuredTaskExecutionRef) -> Result<Uuid, PortError> {
    Uuid::parse_str(&execution.execution_id).map_err(|_| {
        PortError::validation(
            "ai.structured.execution_id_invalid",
            "structured task execution_id must be a UUID",
        )
    })
}

fn runtime_invariant() -> PortError {
    PortError::invariant_violation(
        "ai.structured.runtime_invalid",
        "structured task runtime contains invalid evidence",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_errors_are_mapped_without_persisting_provider_messages() {
        let failure = classify_ai_error(&AiError::Provider(
            "secret provider response body".to_string(),
        ));
        assert_eq!(failure.code, "ai.structured.provider_error");
        assert!(failure.retryable);
        assert!(!failure.code.contains("secret"));
    }

    #[test]
    fn configuration_errors_are_typed_and_not_retryable() {
        let failure = classify_ai_error(&AiError::InvalidConfig("credential missing".to_string()));
        assert_eq!(failure.code, "ai.structured.provider_configuration_invalid");
        assert!(!failure.retryable);
    }

    #[test]
    fn registered_descriptor_rejects_contract_drift() {
        let catalog = AiStructuredTaskCatalog::default();
        let descriptor = AiStructuredTaskDescriptor {
            owner: "translation".to_string(),
            task_slug: "machine_translation".to_string(),
            prompt_policy_digest: "a".repeat(64),
            input_schema_digest: "b".repeat(64),
            output_schema_digest: hash_manifest(&serde_json::json!({"type": "object"})).unwrap(),
            system_prompt: "Return structured output.".to_string(),
            allowed_classifications: vec![crate::AiTaskDataClassification::TenantPrivate],
            max_input_bytes: 1024,
            max_output_bytes: 1024,
            max_attempts: 2,
        };
        catalog.register(descriptor).unwrap();
        let request = AiStructuredTaskRequest {
            owner: "translation".to_string(),
            task_slug: "machine_translation".to_string(),
            prompt_policy_digest: "c".repeat(64),
            input_schema_digest: "b".repeat(64),
            input: serde_json::json!({"value": "test"}),
            output_schema: serde_json::json!({"type": "object"}),
            classification: crate::AiTaskDataClassification::TenantPrivate,
            evidence: Default::default(),
            limits: crate::AiStructuredTaskLimits {
                max_output_bytes: 1024,
                max_attempts: 2,
            },
        };
        assert_eq!(
            DurableAiStructuredTaskPort::validate_descriptor(&catalog, &request)
                .unwrap_err()
                .code,
            "ai.structured.task_contract_drift"
        );
    }
}
