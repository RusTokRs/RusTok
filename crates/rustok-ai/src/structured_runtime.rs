use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use jsonschema::{Draft, PatternOptions, Validator};
use rustok_api::{PortContext, PortError, PortErrorKind, manifest_hash::hash_manifest};
use tokio::time::{Instant, MissedTickBehavior};
use uuid::Uuid;

use crate::{
    AiError, AiHostRuntime, AiManagementService, AiStructuredTaskAvailability,
    AiStructuredTaskCatalog, AiStructuredTaskDescriptor, AiStructuredTaskEstimate,
    AiStructuredTaskExecution, AiStructuredTaskExecutionKey, AiStructuredTaskExecutionRef,
    AiStructuredTaskHealth, AiStructuredTaskPort, AiStructuredTaskRequest, AiStructuredTaskStatus,
    ChatMessage, ChatMessageRole, InferenceEngine, ProviderCapability, ProviderChatRequest,
    ProviderStructuredRequest, ProviderStructuredResponse, RouterProviderProfile,
    accounting::{AttemptOutcome, StructuredAccounting, TerminalOutcome},
    router::ordered_provider_candidates,
    service::{
        list_router_provider_profiles, provider_config, require_provider_profile,
        runtime_inference_engine, task_profile_runtime,
    },
    structured::StructuredExecutionLedger,
    structured_result::{StructuredResultKeyring, StructuredResultStore},
};

const CANCELLATION_POLL_INTERVAL: Duration = Duration::from_millis(100);
const LEASE_RECOVERY_GRACE: Duration = Duration::from_secs(5);
const MAX_OUTPUT_SCHEMA_REGEX_BYTES: usize = 64 * 1024;

pub fn structured_task_port_from_context(
    context: &rustok_api::HostRuntimeContext,
    catalog: AiStructuredTaskCatalog,
) -> Result<Option<Arc<dyn AiStructuredTaskPort>>, String> {
    let Some(config) = context.shared_get::<crate::SharedAiStructuredResultKeyringConfig>() else {
        return Ok(None);
    };
    let runtime = crate::ai_host_runtime_from_context(context)?;
    let keyring = StructuredResultKeyring::new(config.0, runtime.secret_registry().clone())
        .map_err(|error| format!("invalid AI structured result keyring: {}", error.code))?;
    Ok(Some(Arc::new(DurableAiStructuredTaskPort::new(
        runtime, catalog, keyring,
    ))))
}

#[derive(Clone)]
pub(crate) struct DurableAiStructuredTaskPort {
    runtime: AiHostRuntime,
    catalog: AiStructuredTaskCatalog,
    ledger: StructuredExecutionLedger,
    accounting: StructuredAccounting,
    results: StructuredResultStore,
}

#[derive(Clone)]
struct ProviderCandidate {
    profile: RouterProviderProfile,
    config: crate::AiProviderConfig,
}

struct StructuredExecutionContext<'a> {
    execution_id: Uuid,
    request_digest: &'a str,
    lease_token: Uuid,
    deadline: Instant,
}

#[derive(Debug)]
struct ValidatedStructuredTask {
    descriptor: AiStructuredTaskDescriptor,
    output_validator: Validator,
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
    pub(crate) fn new(
        runtime: AiHostRuntime,
        catalog: AiStructuredTaskCatalog,
        keyring: StructuredResultKeyring,
    ) -> Self {
        let database = runtime.db_clone();
        Self {
            runtime,
            catalog,
            ledger: StructuredExecutionLedger::new(database.clone()),
            accounting: StructuredAccounting::new(database.clone()),
            results: StructuredResultStore::new(database, keyring),
        }
    }

    async fn view_with_result(
        &self,
        context: &PortContext,
        execution_id: Uuid,
    ) -> Result<AiStructuredTaskExecution, PortError> {
        let durable = self.ledger.load(context, execution_id).await?;
        let mut execution = self.ledger.view(context, execution_id).await?;
        if execution.status == AiStructuredTaskStatus::Completed {
            execution.output = Some(
                self.results
                    .replay(
                        durable.tenant_id,
                        execution_id,
                        &execution.request_digest,
                        durable.max_output_bytes,
                    )
                    .await?,
            );
        }
        Ok(execution)
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
    ) -> Result<ValidatedStructuredTask, PortError> {
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
        let output_validator = jsonschema::options()
            .with_draft(Draft::Draft202012)
            .should_validate_formats(true)
            .should_ignore_unknown_formats(false)
            .with_pattern_options(
                PatternOptions::regex()
                    .size_limit(MAX_OUTPUT_SCHEMA_REGEX_BYTES)
                    .dfa_size_limit(MAX_OUTPUT_SCHEMA_REGEX_BYTES),
            )
            .build(&request.output_schema)
            .map_err(|_| {
                PortError::validation(
                    "ai.structured.output_schema_invalid",
                    "structured task output_schema must be a valid bounded JSON Schema",
                )
            })?;
        Ok(ValidatedStructuredTask {
            descriptor,
            output_validator,
        })
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
        task: &ValidatedStructuredTask,
        execution: StructuredExecutionContext<'_>,
        candidates: Vec<ProviderCandidate>,
    ) -> Result<AiStructuredTaskExecution, PortError> {
        let StructuredExecutionContext {
            execution_id,
            request_digest,
            lease_token,
            deadline,
        } = execution;
        let provider_request = ProviderChatRequest {
            model: String::new(),
            messages: vec![
                ChatMessage {
                    role: ChatMessageRole::System,
                    content: Some(task.descriptor.system_prompt.clone()),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                    metadata: serde_json::json!({"policy_digest": task.descriptor.prompt_policy_digest}),
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
        self.results.prepare(parse_tenant_id(context)?).await?;
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
                    } else if !task.output_validator.is_valid(&response.output) {
                        Some(AttemptFailure::provider_contract(
                            "ai.structured.provider_output_schema_invalid",
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
                    let sealed = self
                        .results
                        .seal(
                            parse_tenant_id(context)?,
                            execution_id,
                            request_digest,
                            &response.output,
                        )
                        .await?;
                    self.accounting
                        .complete_attempt(
                            execution_id,
                            lease_token,
                            attempt.model.id,
                            usage,
                            sealed,
                        )
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

    async fn estimate(
        &self,
        context: PortContext,
        request: AiStructuredTaskRequest,
    ) -> Result<AiStructuredTaskEstimate, PortError> {
        request.validate(&context)?;
        Self::validate_descriptor(&self.catalog, &request)?;
        let tenant_id = parse_tenant_id(&context)?;
        let mut candidates = self
            .candidates(tenant_id, &request.task_slug, &context.roles)
            .await?;
        candidates.truncate(usize::from(request.limits.max_attempts));
        let input_tokens_upper_bound = u64::try_from(
            serde_json::to_vec(&request.input)
                .map_err(|_| runtime_invariant())?
                .len(),
        )
        .map_err(|_| runtime_invariant())?;
        self.accounting
            .estimate(
                tenant_id,
                input_tokens_upper_bound,
                u64::from(request.limits.max_output_bytes),
                request.limits.max_attempts,
                &candidates
                    .iter()
                    .map(|candidate| candidate.profile.id)
                    .collect::<Vec<_>>(),
            )
            .await
    }

    async fn execute(
        &self,
        context: PortContext,
        request: AiStructuredTaskRequest,
    ) -> Result<AiStructuredTaskExecution, PortError> {
        request.validate(&context)?;
        let deadline =
            Instant::now() + Duration::from_millis(context.deadline_ms.unwrap_or_default());
        let task = Self::validate_descriptor(&self.catalog, &request)?;
        let tenant_id = parse_tenant_id(&context)?;
        let mut candidates = self
            .candidates(tenant_id, &request.task_slug, &context.roles)
            .await?;
        candidates.truncate(usize::from(request.limits.max_attempts));
        let registered = self.ledger.register(&context, &request).await?;
        let execution_key = AiStructuredTaskExecutionKey {
            owner: request.owner.clone(),
            idempotency_key: context.idempotency_key.clone().unwrap_or_default(),
        };
        if let Some(cancelled) = self
            .ledger
            .apply_cancellation_intent(&context, &execution_key)
            .await?
        {
            if cancelled.status == "queued" {
                self.accounting.cancel_queued(cancelled.id).await?;
            }
            return self.view_with_result(&context, cancelled.id).await;
        }
        if registered.replayed && registered.execution.status != "queued" {
            return self
                .view_with_result(&context, registered.execution.id)
                .await;
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
            return self
                .view_with_result(&context, registered.execution.id)
                .await;
        };
        self.run(
            &context,
            &request,
            &task,
            StructuredExecutionContext {
                execution_id: registered.execution.id,
                request_digest: &registered.execution.request_digest,
                lease_token: lease.token,
                deadline,
            },
            candidates,
        )
        .await
    }

    async fn status(
        &self,
        context: PortContext,
        execution: AiStructuredTaskExecutionRef,
    ) -> Result<AiStructuredTaskExecution, PortError> {
        context.require_read_semantics()?;
        self.view_with_result(&context, parse_execution_id(&execution)?)
            .await
    }

    async fn resolve(
        &self,
        context: PortContext,
        execution: AiStructuredTaskExecutionKey,
    ) -> Result<Option<AiStructuredTaskExecution>, PortError> {
        execution.validate()?;
        let Some(resolved) = self.ledger.resolve_by_key(&context, &execution).await? else {
            return Ok(None);
        };
        self.view_with_result(&context, parse_execution_uuid(&resolved.execution_id)?)
            .await
            .map(Some)
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
        self.view_with_result(&context, execution_id).await
    }

    async fn cancel_by_key(
        &self,
        context: PortContext,
        execution: AiStructuredTaskExecutionKey,
    ) -> Result<Option<AiStructuredTaskExecution>, PortError> {
        execution.validate()?;
        self.ledger
            .put_cancellation_intent(&context, &execution)
            .await?;
        let Some(cancelled) = self
            .ledger
            .apply_cancellation_intent(&context, &execution)
            .await?
        else {
            return Ok(None);
        };
        if cancelled.status == "queued" {
            self.accounting.cancel_queued(cancelled.id).await?;
        }
        self.view_with_result(&context, cancelled.id)
            .await
            .map(Some)
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
    parse_execution_uuid(&execution.execution_id)
}

fn parse_execution_uuid(execution_id: &str) -> Result<Uuid, PortError> {
    Uuid::parse_str(execution_id).map_err(|_| {
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
    use std::{
        collections::{BTreeMap, VecDeque},
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use async_trait::async_trait;
    use rustok_api::{PortActor, manifest_hash::hash_manifest};
    use rustok_core::ModuleRegistry;
    use rustok_outbox::{OutboxTransport, TransactionalEventBus};
    use rustok_secrets::SecretResolverRegistry;
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set};
    use serde_json::json;
    use tokio::sync::Notify;

    use super::*;
    use crate::{
        AiProviderConfig, ProviderImageRequest, ProviderImageResponse, ProviderTestResult,
        ProviderUsage,
        accounting::{BudgetPolicy, ProviderPolicy},
        engine::{AiProviderTarget, AiProviderTargetCatalog, ProviderEgressPolicy},
        entities::{
            ai_provider_profiles, ai_structured_budgets, ai_structured_executions,
            ai_structured_reservations,
        },
        structured_result::StructuredResultKeyring,
        structured_test_support,
    };

    enum StructuredStep {
        ProviderFailure,
        Pending(Arc<Notify>),
        Success(serde_json::Value),
    }

    struct ScriptedStructuredEngine {
        steps: Mutex<VecDeque<StructuredStep>>,
        calls: AtomicUsize,
    }

    impl ScriptedStructuredEngine {
        fn new(steps: impl IntoIterator<Item = StructuredStep>) -> Self {
            Self {
                steps: Mutex::new(steps.into_iter().collect()),
                calls: AtomicUsize::new(0),
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl InferenceEngine for ScriptedStructuredEngine {
        async fn test_connection(
            &self,
            _config: &AiProviderConfig,
        ) -> crate::AiResult<ProviderTestResult> {
            Err(AiError::Runtime(
                "unexpected structured test connection probe".to_string(),
            ))
        }

        async fn complete(
            &self,
            _config: &AiProviderConfig,
            _request: ProviderChatRequest,
        ) -> crate::AiResult<crate::ProviderChatResponse> {
            Err(AiError::Runtime(
                "unexpected unstructured provider call".to_string(),
            ))
        }

        async fn complete_stream(
            &self,
            _config: &AiProviderConfig,
            _request: ProviderChatRequest,
            _emitter: Option<crate::ProviderStreamEmitter>,
        ) -> crate::AiResult<crate::ProviderChatResponse> {
            Err(AiError::Runtime(
                "unexpected streaming provider call".to_string(),
            ))
        }

        async fn complete_structured(
            &self,
            _request: ProviderStructuredRequest,
        ) -> crate::AiResult<ProviderStructuredResponse> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let step = self
                .steps
                .lock()
                .expect("structured provider script lock")
                .pop_front()
                .expect("structured provider script step");
            match step {
                StructuredStep::ProviderFailure => Err(AiError::Provider(
                    "private upstream failure body".to_string(),
                )),
                StructuredStep::Pending(started) => {
                    started.notify_one();
                    std::future::pending().await
                }
                StructuredStep::Success(output) => Ok(ProviderStructuredResponse {
                    output,
                    usage: Some(ProviderUsage::normalized(20, 5, None)),
                }),
            }
        }

        async fn generate_image(
            &self,
            _config: &AiProviderConfig,
            _request: ProviderImageRequest,
        ) -> crate::AiResult<ProviderImageResponse> {
            Err(AiError::Runtime(
                "unexpected image provider call".to_string(),
            ))
        }
    }

    fn descriptor_and_request() -> (AiStructuredTaskDescriptor, AiStructuredTaskRequest) {
        let output_schema = json!({
            "type": "object",
            "required": ["translated_text"],
            "properties": {"translated_text": {"type": "string"}}
        });
        let descriptor = AiStructuredTaskDescriptor {
            owner: "translation".to_string(),
            task_slug: "machine_translation".to_string(),
            prompt_policy_digest: "a".repeat(64),
            input_schema_digest: "b".repeat(64),
            output_schema_digest: hash_manifest(&output_schema).expect("output schema digest"),
            system_prompt: "Return only the translated structured payload.".to_string(),
            allowed_classifications: vec![crate::AiTaskDataClassification::TenantPrivate],
            max_input_bytes: 4096,
            max_output_bytes: 4096,
            max_attempts: 3,
        };
        let request = AiStructuredTaskRequest {
            owner: descriptor.owner.clone(),
            task_slug: descriptor.task_slug.clone(),
            prompt_policy_digest: descriptor.prompt_policy_digest.clone(),
            input_schema_digest: descriptor.input_schema_digest.clone(),
            input: json!({"source_locale": "de", "target_locale": "en", "text": "Hallo"}),
            output_schema,
            classification: crate::AiTaskDataClassification::TenantPrivate,
            evidence: BTreeMap::from([("translation_job_id".to_string(), "job-a".to_string())]),
            limits: crate::AiStructuredTaskLimits {
                max_output_bytes: 4096,
                max_attempts: 3,
            },
        };
        (descriptor, request)
    }

    fn provider_runtime(
        database: sea_orm::DatabaseConnection,
        engine: Arc<ScriptedStructuredEngine>,
        provider_targets: AiProviderTargetCatalog,
        egress_policy: ProviderEgressPolicy,
    ) -> AiHostRuntime {
        AiHostRuntime::new(
            database.clone(),
            TransactionalEventBus::new(Arc::new(OutboxTransport::new(database))),
            ModuleRegistry::new(),
            SecretResolverRegistry::builder().build(),
            egress_policy,
            provider_targets,
        )
        .with_test_inference_engine(engine)
    }

    fn request_context(tenant_id: Uuid) -> PortContext {
        PortContext::new(
            tenant_id.to_string(),
            PortActor::service("translation-worker"),
            "en",
            "translation-runtime-evidence",
        )
        .with_idempotency_key("translation-job-a")
        .with_deadline(Duration::from_secs(5))
    }

    #[tokio::test]
    async fn structured_runtime_preserves_contract_and_accounting_across_failure_paths() {
        let database = structured_test_support::database().await;
        let tenant_id = Uuid::new_v4();
        let primary_provider_id = Uuid::new_v4();
        let invalid_output_provider_id = Uuid::new_v4();
        let successful_fallback_provider_id = Uuid::new_v4();
        structured_test_support::insert_tenant(&database, tenant_id).await;
        structured_test_support::insert_provider_profile(
            &database,
            tenant_id,
            primary_provider_id,
            "primary",
        )
        .await;
        structured_test_support::insert_provider_profile(
            &database,
            tenant_id,
            invalid_output_provider_id,
            "invalid-output",
        )
        .await;
        structured_test_support::insert_provider_profile(
            &database,
            tenant_id,
            successful_fallback_provider_id,
            "successful-fallback",
        )
        .await;
        structured_test_support::insert_task_profile(
            &database,
            tenant_id,
            &[
                primary_provider_id,
                invalid_output_provider_id,
                successful_fallback_provider_id,
            ],
        )
        .await;

        let accounting = StructuredAccounting::new(database.clone());
        accounting
            .put_budget(BudgetPolicy {
                tenant_id,
                currency_code: "USD".to_string(),
                limit_minor_units: 100_000,
                max_concurrent: 1,
            })
            .await
            .expect("structured budget policy");
        for provider_profile_id in [
            primary_provider_id,
            invalid_output_provider_id,
            successful_fallback_provider_id,
        ] {
            accounting
                .put_provider_policy(ProviderPolicy {
                    tenant_id,
                    provider_profile_id,
                    currency_code: "USD".to_string(),
                    input_cost_per_million_minor: 1_000_000,
                    output_cost_per_million_minor: 2_000_000,
                    max_concurrent: 1,
                    is_active: true,
                })
                .await
                .expect("structured provider policy");
        }

        let egress_policy = ProviderEgressPolicy {
            allowed_origins: vec!["provider.example.test".to_string()],
            allow_local_origins: false,
        };
        let provider_targets = AiProviderTargetCatalog::new_with_egress_policy(
            vec![AiProviderTarget {
                id: crate::ProviderTargetId::new("openai_compatible").expect("provider target id"),
                provider_slug: crate::ProviderSlug::openai_compatible(),
                display_name: "Structured runtime provider".to_string(),
                auth: crate::ProviderTargetAuth::None,
                settings: BTreeMap::from([(
                    "base_url".to_string(),
                    json!("https://provider.example.test/v1"),
                )]),
            }],
            &egress_policy,
        )
        .expect("structured provider targets");
        let (descriptor, request) = descriptor_and_request();
        let catalog = AiStructuredTaskCatalog::default();
        catalog
            .register(descriptor)
            .expect("structured task descriptor");
        let keyring = StructuredResultKeyring::for_test(
            "test-v1",
            Duration::from_secs(300),
            BTreeMap::from([("test-v1".to_string(), [7_u8; 32])]),
        );
        let first_engine = Arc::new(ScriptedStructuredEngine::new([
            StructuredStep::ProviderFailure,
            StructuredStep::Success(json!({"translated_text": 42})),
            StructuredStep::Success(json!({"translated_text": "Hello"})),
        ]));
        let first_port = DurableAiStructuredTaskPort::new(
            provider_runtime(
                database.clone(),
                Arc::clone(&first_engine),
                provider_targets.clone(),
                egress_policy.clone(),
            ),
            catalog.clone(),
            keyring.clone(),
        );
        let context = request_context(tenant_id);
        let estimate = first_port
            .estimate(context.clone(), request.clone())
            .await
            .expect("structured estimate");
        assert_eq!(estimate.output_tokens_upper_bound, 4096);
        assert_eq!(estimate.attempts_upper_bound, 3);
        assert_eq!(estimate.currency_code, "USD");
        assert_eq!(estimate.price_snapshot_digest.len(), 64);
        assert_eq!(first_engine.calls(), 0);
        assert_eq!(
            ai_structured_executions::Entity::find()
                .count(&database)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            ai_structured_reservations::Entity::find()
                .count(&database)
                .await
                .unwrap(),
            0
        );
        let budget_before_execution = ai_structured_budgets::Entity::find()
            .one(&database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(budget_before_execution.reserved_minor_units, 0);
        assert_eq!(budget_before_execution.committed_minor_units, 0);
        assert_eq!(budget_before_execution.in_flight, 0);

        let completed = first_port
            .execute(context.clone(), request.clone())
            .await
            .expect("fallback execution");

        assert_eq!(completed.status, AiStructuredTaskStatus::Completed);
        assert_eq!(completed.output, Some(json!({"translated_text": "Hello"})));
        assert_eq!(first_engine.calls(), 3);
        assert_eq!(completed.attempts.len(), 3);
        assert_eq!(
            completed.attempts[0].provider_profile_id,
            primary_provider_id.to_string()
        );
        assert!(!completed.attempts[0].fallback);
        assert_eq!(
            completed.attempts[0].error_code.as_deref(),
            Some("ai.structured.provider_error")
        );
        assert_eq!(
            completed.attempts[1].provider_profile_id,
            invalid_output_provider_id.to_string()
        );
        assert!(completed.attempts[1].fallback);
        assert_eq!(
            completed.attempts[1].error_code.as_deref(),
            Some("ai.structured.provider_output_schema_invalid")
        );
        assert_eq!(
            completed.attempts[2].provider_profile_id,
            successful_fallback_provider_id.to_string()
        );
        assert!(completed.attempts[2].fallback);
        assert_eq!(
            completed.usage.as_ref().map(|usage| usage.cost_minor_units),
            Some(30)
        );
        let committed_before_restart = ai_structured_budgets::Entity::find()
            .filter(ai_structured_budgets::Column::TenantId.eq(tenant_id))
            .one(&database)
            .await
            .expect("structured budget query")
            .expect("structured budget")
            .committed_minor_units;

        let restarted_engine = Arc::new(ScriptedStructuredEngine::new([]));
        let restarted_port = DurableAiStructuredTaskPort::new(
            provider_runtime(
                database.clone(),
                Arc::clone(&restarted_engine),
                provider_targets.clone(),
                egress_policy.clone(),
            ),
            catalog.clone(),
            keyring.clone(),
        );
        let mut conflicting_request = request.clone();
        conflicting_request.input["text"] = json!("Guten Tag");
        let conflict = restarted_port
            .execute(context.clone(), conflicting_request)
            .await
            .expect_err("request digest drift must fail closed");
        assert_eq!(conflict.code, "ai.structured.idempotency_conflict");

        let replayed = restarted_port
            .execute(context, request.clone())
            .await
            .expect("restart replay");
        assert_eq!(replayed.execution_id, completed.execution_id);
        assert_eq!(replayed.output, completed.output);
        assert_eq!(restarted_engine.calls(), 0);
        let committed_after_restart = ai_structured_budgets::Entity::find()
            .filter(ai_structured_budgets::Column::TenantId.eq(tenant_id))
            .one(&database)
            .await
            .expect("structured budget query")
            .expect("structured budget")
            .committed_minor_units;
        assert_eq!(committed_after_restart, committed_before_restart);

        let cancellation_started = Arc::new(Notify::new());
        let cancellation_engine =
            Arc::new(ScriptedStructuredEngine::new([StructuredStep::Pending(
                Arc::clone(&cancellation_started),
            )]));
        let cancellation_port = DurableAiStructuredTaskPort::new(
            provider_runtime(
                database.clone(),
                Arc::clone(&cancellation_engine),
                provider_targets.clone(),
                egress_policy.clone(),
            ),
            catalog.clone(),
            keyring.clone(),
        );
        let cancellation_context =
            request_context(tenant_id).with_idempotency_key("translation-job-cancel");
        let executing_port = cancellation_port.clone();
        let executing_context = cancellation_context.clone();
        let executing_request = request.clone();
        let execution = tokio::spawn(async move {
            executing_port
                .execute(executing_context, executing_request)
                .await
        });
        cancellation_started.notified().await;
        cancellation_port
            .cancel_by_key(
                cancellation_context,
                AiStructuredTaskExecutionKey {
                    owner: request.owner.clone(),
                    idempotency_key: "translation-job-cancel".to_string(),
                },
            )
            .await
            .expect("durable running cancellation")
            .expect("registered cancellation target");
        let cancelled = tokio::time::timeout(Duration::from_secs(2), execution)
            .await
            .expect("running cancellation timeout")
            .expect("running cancellation join")
            .expect("running cancellation result");
        assert_eq!(cancelled.status, AiStructuredTaskStatus::Cancelled);
        assert_eq!(cancellation_engine.calls(), 1);
        assert_eq!(cancelled.attempts.len(), 1);
        assert_eq!(
            cancelled.attempts[0].status,
            AiStructuredTaskStatus::Cancelled
        );
        let budget_after_cancellation = ai_structured_budgets::Entity::find()
            .filter(ai_structured_budgets::Column::TenantId.eq(tenant_id))
            .one(&database)
            .await
            .expect("structured budget query")
            .expect("structured budget");
        assert_eq!(budget_after_cancellation.committed_minor_units, 30);
        assert_eq!(budget_after_cancellation.reserved_minor_units, 0);
        assert_eq!(budget_after_cancellation.in_flight, 0);

        accounting
            .put_budget(BudgetPolicy {
                tenant_id,
                currency_code: "USD".to_string(),
                limit_minor_units: 30,
                max_concurrent: 1,
            })
            .await
            .expect("exhausted structured budget policy");
        let quota_engine = Arc::new(ScriptedStructuredEngine::new([StructuredStep::Success(
            json!({"translated_text": "Must not run"}),
        )]));
        let quota_port = DurableAiStructuredTaskPort::new(
            provider_runtime(
                database.clone(),
                Arc::clone(&quota_engine),
                provider_targets,
                egress_policy,
            ),
            catalog,
            keyring,
        );
        let quota_error = quota_port
            .execute(
                request_context(tenant_id).with_idempotency_key("translation-job-quota"),
                request,
            )
            .await
            .expect_err("exhausted budget must fail before provider execution");
        assert_eq!(quota_error.code, "ai.structured.quota_exhausted");
        assert_eq!(quota_engine.calls(), 0);
        let exhausted_budget = ai_structured_budgets::Entity::find()
            .filter(ai_structured_budgets::Column::TenantId.eq(tenant_id))
            .one(&database)
            .await
            .expect("structured budget query")
            .expect("structured budget");
        assert_eq!(exhausted_budget.committed_minor_units, 30);
        assert_eq!(exhausted_budget.reserved_minor_units, 0);
        assert_eq!(exhausted_budget.in_flight, 0);

        let provider_profiles = ai_provider_profiles::Entity::find()
            .filter(ai_provider_profiles::Column::TenantId.eq(tenant_id))
            .all(&database)
            .await
            .expect("structured provider profiles");
        for provider_profile in provider_profiles {
            let mut provider_profile: ai_provider_profiles::ActiveModel = provider_profile.into();
            provider_profile.is_active = Set(false);
            provider_profile
                .update(&database)
                .await
                .expect("disable structured provider profile");
        }
        let health = quota_port
            .health(
                request_context(tenant_id),
                "machine_translation".to_string(),
            )
            .await
            .expect("degraded structured provider health");
        assert_eq!(health.availability, AiStructuredTaskAvailability::Degraded);
        assert_eq!(
            health.reason_code.as_deref(),
            Some("ai.structured.provider_unavailable")
        );
    }

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

    #[test]
    fn registered_descriptor_rejects_an_invalid_output_schema() {
        let output_schema = serde_json::json!({"type": "string", "pattern": "["});
        let catalog = AiStructuredTaskCatalog::default();
        catalog
            .register(AiStructuredTaskDescriptor {
                owner: "translation".to_string(),
                task_slug: "machine_translation".to_string(),
                prompt_policy_digest: "a".repeat(64),
                input_schema_digest: "b".repeat(64),
                output_schema_digest: hash_manifest(&output_schema).unwrap(),
                system_prompt: "Return structured output.".to_string(),
                allowed_classifications: vec![crate::AiTaskDataClassification::TenantPrivate],
                max_input_bytes: 1024,
                max_output_bytes: 1024,
                max_attempts: 2,
            })
            .unwrap();
        let request = AiStructuredTaskRequest {
            owner: "translation".to_string(),
            task_slug: "machine_translation".to_string(),
            prompt_policy_digest: "a".repeat(64),
            input_schema_digest: "b".repeat(64),
            input: serde_json::json!({"value": "test"}),
            output_schema,
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
            "ai.structured.output_schema_invalid"
        );
    }
}
