use super::*;

impl RegistryGovernanceService {
    pub async fn validate_publish_request(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
    ) -> anyhow::Result<RegistryValidationQueueResult> {
        let request = self.get_publish_request(request_id).await?.ok_or_else(|| {
            not_found_error(format!(
                "Registry publish request '{request_id}' was not found"
            ))
        })?;
        self.ensure_authority_can_manage_publish_request(authority, &request, "validate")
            .await?;

        let was_requeued = match request.status {
            RegistryPublishRequestStatus::Rejected => {
                let latest_event_type = self.latest_request_event_type(&request.id).await?;
                if rejected_publish_request_can_retry(
                    latest_event_type.as_deref(),
                    request.rejection_reason.as_deref(),
                ) {
                    true
                } else {
                    return Err(conflict_error(format!(
                        "Registry publish request '{}' was manually rejected by governance review and cannot be revalidated; create a new publish request instead",
                        request_id
                    )));
                }
            }
            _ => false,
        };

        match request.status {
            RegistryPublishRequestStatus::Draft => {
                return Err(conflict_error(format!(
                    "Registry publish request '{}' is still in draft status and must receive an artifact upload before validation",
                    request_id
                )));
            }
            RegistryPublishRequestStatus::Approved | RegistryPublishRequestStatus::Published => {
                return Ok(RegistryValidationQueueResult {
                    request,
                    queued: false,
                    validation_job_id: None,
                });
            }
            RegistryPublishRequestStatus::Validating => {
                if let Some(existing_job) = self.latest_active_validation_job(&request.id).await? {
                    return Ok(RegistryValidationQueueResult {
                        request,
                        queued: false,
                        validation_job_id: Some(existing_job.id),
                    });
                }

                let job = self
                    .create_validation_job(
                        &request,
                        authority_actor(authority),
                        "validation_resumed",
                    )
                    .await?;
                self.record_governance_event(
                    &request.slug,
                    Some(&request.id),
                    None,
                    "validation_job_queued",
                    authority_actor(authority),
                    None,
                    serde_json::json!({
                        "job_id": job.id.clone(),
                        "attempt_number": job.attempt_number,
                        "queue_reason": job.queue_reason.clone(),
                        "request_status": request_status_label(request.status.clone()),
                        "version": request.version.clone(),
                    }),
                )
                .await?;
                return Ok(RegistryValidationQueueResult {
                    request,
                    queued: true,
                    validation_job_id: Some(job.id),
                });
            }
            RegistryPublishRequestStatus::Rejected
            | RegistryPublishRequestStatus::ArtifactUploaded
            | RegistryPublishRequestStatus::Submitted => {}
            RegistryPublishRequestStatus::ChangesRequested => {
                return Err(conflict_error(format!(
                    "Registry publish request '{}' must receive a fresh artifact upload before validation can run again",
                    request_id
                )));
            }
            RegistryPublishRequestStatus::OnHold => {
                return Err(conflict_error(format!(
                    "Registry publish request '{}' is currently on hold and must be resumed before validation can run",
                    request_id
                )));
            }
        }

        let validating_at = Utc::now();
        let mut request_active: RegistryPublishRequestActiveModel = request.into();
        request_active.status = Set(RegistryPublishRequestStatus::Validating);
        request_active.validation_errors = Set(serde_json::json!([]));
        request_active.rejected_by = Set(None);
        request_active.rejection_reason = Set(None);
        request_active.validated_at = Set(None);
        request_active.updated_at = Set(validating_at);
        let request = request_active.update(&self.db).await?;
        let job = self
            .create_validation_job(
                &request,
                authority_actor(authority),
                if was_requeued {
                    "requeued_after_validation_failed"
                } else {
                    "initial_validation"
                },
            )
            .await?;
        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            if was_requeued {
                "validation_requeued"
            } else {
                "validation_queued"
            },
            authority_actor(authority),
            None,
            serde_json::json!({
                "job_id": job.id.clone(),
                "attempt_number": job.attempt_number,
                "queue_reason": job.queue_reason.clone(),
                "version": request.version.clone(),
                "status": request_status_label(request.status.clone()),
                "requeued": was_requeued,
                "follow_up_gates": follow_up_validation_gate_details(),
            }),
        )
        .await?;
        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            "validation_job_queued",
            authority_actor(authority),
            None,
            serde_json::json!({
                "job_id": job.id.clone(),
                "attempt_number": job.attempt_number,
                "queue_reason": job.queue_reason.clone(),
                "request_status": request_status_label(request.status.clone()),
                "version": request.version.clone(),
            }),
        )
        .await?;

        Ok(RegistryValidationQueueResult {
            request,
            queued: true,
            validation_job_id: Some(job.id),
        })
    }

    pub async fn run_publish_validation_job(
        &self,
        validation_job_id: &str,
        actor: &str,
    ) -> anyhow::Result<registry_publish_request::Model> {
        let claim = self
            .claim_validation_job(validation_job_id, actor)
            .await?
            .ok_or_else(|| {
                anyhow!("Registry validation job '{validation_job_id}' was not found")
            })?;
        if !claim.should_run {
            return Ok(claim.request);
        }
        let job = claim.job;
        let request = claim.request;

        let mut artifact_load_attempt = 1usize;
        let artifact = loop {
            match self.load_registry_artifact(&request).await {
                Ok(artifact) => break artifact,
                Err(error) => {
                    let existing_warnings = deserialize_message_list(&request.validation_warnings);
                    if let Some(retry_after_seconds) =
                        validation_retry_delay_seconds(artifact_load_attempt)
                    {
                        self.record_governance_event(
                            &request.slug,
                            Some(&request.id),
                            None,
                            "validation_retry_scheduled",
                            actor,
                            None,
                            serde_json::json!({
                                "job_id": job.id.clone(),
                                "job_attempt": job.attempt_number,
                                "version": request.version.clone(),
                                "status": request_status_label(request.status.clone()),
                                "attempt": artifact_load_attempt,
                                "next_attempt": artifact_load_attempt + 1,
                                "retry_after_seconds": retry_after_seconds,
                                "error": error.to_string(),
                            }),
                        )
                        .await?;
                        tokio::time::sleep(std::time::Duration::from_secs(retry_after_seconds))
                            .await;
                        artifact_load_attempt += 1;
                        continue;
                    }

                    self.record_governance_event(
                        &request.slug,
                        Some(&request.id),
                        None,
                        "validation_retry_exhausted",
                        actor,
                        None,
                        serde_json::json!({
                            "job_id": job.id.clone(),
                            "job_attempt": job.attempt_number,
                            "version": request.version.clone(),
                            "status": request_status_label(request.status.clone()),
                            "attempt": artifact_load_attempt,
                            "max_attempts": artifact_load_attempt,
                            "error": error.to_string(),
                        }),
                    )
                    .await?;
                    let errors = vec![format!(
                        "Validation job exhausted artifact-load retries before bundle checks: {error}"
                    )];
                    let request = self
                        .mark_validation_job_failed(
                            job.clone(),
                            actor,
                            Some(errors[0].as_str()),
                            &request,
                        )
                        .await?;
                    return self
                        .store_validation_rejection(
                            request,
                            actor,
                            &existing_warnings,
                            &errors,
                            validation_failed_check_details(&errors),
                        )
                        .await;
                }
            }
        };

        let validation = validate_registry_artifact_bundle(&self.db, &request, &artifact).await?;
        let mut warnings = deserialize_message_list(&request.validation_warnings);
        if artifact_load_attempt > 1 {
            warnings.push(format!(
                "Validation artifact load succeeded after retry attempt {}.",
                artifact_load_attempt
            ));
        }
        warnings.extend(validation.warnings);
        let warnings = dedupe_message_list(warnings);

        if !validation.errors.is_empty() {
            let errors = dedupe_message_list(validation.errors);
            let request = self
                .mark_validation_job_failed(
                    job.clone(),
                    actor,
                    errors.first().map(String::as_str),
                    &request,
                )
                .await?;
            return self
                .store_validation_rejection(
                    request,
                    actor,
                    &warnings,
                    &errors,
                    validation_failed_check_details(&errors),
                )
                .await;
        }

        let mut warnings = warnings;
        warnings.push(follow_up_validation_warning().to_string());
        let warnings = dedupe_message_list(warnings);
        let automated_checks = validation_passed_check_details();
        let follow_up_gates = follow_up_validation_gate_details();
        let approved_at = Utc::now();
        let mut request_active: RegistryPublishRequestActiveModel = request.into();
        request_active.status = Set(RegistryPublishRequestStatus::Approved);
        request_active.validation_warnings = Set(serde_json::to_value(&warnings)?);
        request_active.validation_errors = Set(serde_json::json!([]));
        request_active.rejected_by = Set(None);
        request_active.rejection_reason = Set(None);
        request_active.validated_at = Set(Some(approved_at));
        request_active.approved_by = Set(Some(actor_principal(actor).to_json_value()));
        request_active.approved_at = Set(Some(approved_at));
        request_active.updated_at = Set(approved_at);
        let request = request_active.update(&self.db).await?;
        let queued_stages = self
            .queue_follow_up_validation_stages(&request, actor, "validation_passed")
            .await?;
        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            "validation_passed",
            actor,
            None,
            serde_json::json!({
                "version": request.version.clone(),
                "status": request_status_label(request.status.clone()),
                "warnings": warnings.clone(),
                "automated_checks": automated_checks,
                "follow_up_gates": follow_up_gates,
                "validation_stages": queued_stages
                    .iter()
                    .map(validation_stage_details_value)
                    .collect::<Vec<_>>(),
            }),
        )
        .await?;
        self.mark_validation_job_succeeded(job, actor, &request)
            .await?;
        Ok(request)
    }

    pub async fn report_validation_stage(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
        stage_key: &str,
        status: &str,
        detail: Option<&str>,
        reason_code: Option<&str>,
        requeue: bool,
    ) -> anyhow::Result<RegistryValidationStageMutationResult> {
        let stage_key = normalize_validation_stage_key(stage_key)?;
        let requested_status = parse_validation_stage_status(status)?;
        let normalized_reason_code = reason_code
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_ascii_lowercase());
        if let Some(reason_code) = normalized_reason_code.as_deref() {
            if !REGISTRY_VALIDATION_STAGE_REASON_CODES
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(reason_code))
            {
                return Err(malformed_error(format!(
                    "Validation stage reason_code '{}' is not supported; expected one of {}",
                    reason_code,
                    REGISTRY_VALIDATION_STAGE_REASON_CODES.join(", ")
                )));
            }
        }
        if requeue && requested_status != RegistryValidationStageStatus::Queued {
            return Err(malformed_error(format!(
                "Validation stage requeue for '{}' requires status 'queued'",
                stage_key
            )));
        }
        if !requeue && requested_status == RegistryValidationStageStatus::Queued {
            return Err(malformed_error(format!(
                "Validation stage '{}' can only move back to 'queued' via requeue=true",
                stage_key
            )));
        }

        let request = self.get_publish_request(request_id).await?.ok_or_else(|| {
            not_found_error(format!(
                "Registry publish request '{request_id}' was not found"
            ))
        })?;
        self.ensure_authority_can_review_publish_request(
            authority,
            &request,
            "update validation stage",
        )
        .await?;
        if !matches!(
            request.status,
            RegistryPublishRequestStatus::Approved | RegistryPublishRequestStatus::Published
        ) {
            return Err(conflict_error(format!(
                "Registry publish request '{}' is in status '{}' and cannot accept follow-up stage updates",
                request_id,
                request_status_label(request.status.clone())
            )));
        }

        let detail = detail
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| default_validation_stage_detail(stage_key, &requested_status));

        let stage = if requeue {
            self.queue_validation_stage_attempt(
                &request,
                stage_key,
                authority_actor(authority),
                "manual_requeue",
                &detail,
            )
            .await?
        } else {
            let latest_stage = self
                .latest_validation_stage(&request.id, stage_key)
                .await?
                .ok_or_else(|| {
                    not_found_error(format!(
                        "Validation stage '{}' has not been queued yet for request '{}'",
                        stage_key, request_id
                    ))
                })?;
            self.update_validation_stage_status(
                latest_stage,
                &request,
                authority_actor(authority),
                requested_status,
                &detail,
                normalized_reason_code.as_deref(),
            )
            .await?
        };

        Ok(RegistryValidationStageMutationResult { request, stage })
    }

    pub async fn claim_remote_validation_stage(
        &self,
        runner_id: &str,
        supported_stages: &[String],
        lease_ttl_ms: u64,
    ) -> anyhow::Result<Option<RegistryRemoteValidationClaim>> {
        let runner_id = runner_id.trim();
        if runner_id.is_empty() {
            return Err(malformed_error(
                "Remote validation runner must provide a non-empty runner_id",
            ));
        }
        let normalized_supported_stages = supported_stages
            .iter()
            .map(|stage| normalize_validation_stage_key(stage))
            .collect::<anyhow::Result<Vec<_>>>()?;
        if normalized_supported_stages.is_empty() {
            return Ok(None);
        }

        let now = Utc::now();
        let candidates = RegistryValidationStageEntity::find()
            .filter(
                Condition::all()
                    .add(
                        registry_validation_stage::Column::Status
                            .eq(RegistryValidationStageStatus::Queued),
                    )
                    .add(
                        registry_validation_stage::Column::StageKey
                            .is_in(normalized_supported_stages.clone()),
                    )
                    .add(
                        Condition::any()
                            .add(registry_validation_stage::Column::ClaimExpiresAt.is_null())
                            .add(registry_validation_stage::Column::ClaimExpiresAt.lte(now)),
                    ),
            )
            .order_by_asc(registry_validation_stage::Column::CreatedAt)
            .all(&self.db)
            .await?;

        for candidate in candidates {
            let Some(request) = self.get_publish_request(&candidate.request_id).await? else {
                continue;
            };
            if !matches!(
                request.status,
                RegistryPublishRequestStatus::Approved | RegistryPublishRequestStatus::Published
            ) {
                continue;
            }
            let Some(_artifact_storage_key) = request.artifact_storage_key.clone() else {
                continue;
            };
            let Some(artifact_checksum_sha256) = request.artifact_checksum_sha256.clone() else {
                continue;
            };

            let claim_id = format!("rvc_{}", uuid::Uuid::new_v4().simple());
            let actor = remote_validation_runner_actor(runner_id);
            let now = Utc::now();
            let mut active: RegistryValidationStageActiveModel = candidate.clone().into();
            active.status = Set(RegistryValidationStageStatus::Running);
            active.detail = Set(remote_validation_stage_claim_detail(
                &candidate.stage_key,
                runner_id,
            ));
            active.started_at = Set(candidate.started_at.or(Some(now)));
            active.finished_at = Set(None);
            active.claim_id = Set(Some(claim_id.clone()));
            active.claimed_by = Set(Some(runner_id.to_string()));
            active.claim_expires_at = Set(Some(now + remote_validation_lease_ttl(lease_ttl_ms)));
            active.last_heartbeat_at = Set(Some(now));
            active.runner_kind = Set(Some("remote".to_string()));
            active.updated_at = Set(now);
            let stage = active.update(&self.db).await?;
            self.record_validation_stage_event(
                &request,
                &actor,
                &stage,
                "validation_stage_running",
                &stage.detail,
                None,
                Some(serde_json::json!({
                    "claim_id": claim_id.clone(),
                    "runner_id": runner_id,
                    "runner_kind": "remote",
                    "execution_mode": remote_validation_execution_mode(&stage.stage_key),
                })),
            )
            .await?;

            return Ok(Some(RegistryRemoteValidationClaim {
                claim_id,
                request_id: request.id.clone(),
                slug: request.slug,
                version: request.version,
                stage_key: stage.stage_key.clone(),
                execution_mode: remote_validation_execution_mode(&stage.stage_key).to_string(),
                runnable: true,
                requires_manual_confirmation: remote_validation_stage_requires_manual_confirmation(
                    &stage.stage_key,
                ),
                allowed_terminal_reason_codes: REGISTRY_VALIDATION_STAGE_REASON_CODES
                    .iter()
                    .map(|value| (*value).to_string())
                    .collect(),
                suggested_pass_reason_code: Some(
                    remote_validation_pass_reason_code(&stage.stage_key).to_string(),
                ),
                suggested_failure_reason_code: Some(
                    remote_validation_failure_reason_code(&stage.stage_key).to_string(),
                ),
                suggested_blocked_reason_code: Some(
                    remote_validation_blocked_reason_code(&stage.stage_key).to_string(),
                ),
                artifact_download_url: registry_artifact_download_path(&request.id),
                artifact_checksum_sha256,
                crate_name: request.crate_name,
            }));
        }

        Ok(None)
    }

    pub async fn heartbeat_remote_validation_stage(
        &self,
        claim_id: &str,
        runner_id: &str,
        lease_ttl_ms: u64,
    ) -> anyhow::Result<registry_validation_stage::Model> {
        let stage = self
            .remote_validation_stage_by_claim_id(claim_id)
            .await?
            .ok_or_else(|| {
                not_found_error(format!(
                    "Remote validation claim '{claim_id}' was not found"
                ))
            })?;
        ensure_remote_validation_claim_runner(&stage, runner_id)?;
        if stage.status != RegistryValidationStageStatus::Running {
            return Err(conflict_error(format!(
                "Remote validation claim '{}' is in status '{}' and cannot accept heartbeats",
                claim_id,
                validation_stage_status_label(stage.status.clone())
            )));
        }

        let now = Utc::now();
        if stage
            .claim_expires_at
            .as_ref()
            .is_some_and(|expires_at| *expires_at < now)
        {
            return Err(conflict_error(format!(
                "Remote validation claim '{claim_id}' has expired"
            )));
        }

        let mut active: RegistryValidationStageActiveModel = stage.into();
        active.last_heartbeat_at = Set(Some(now));
        active.claim_expires_at = Set(Some(now + remote_validation_lease_ttl(lease_ttl_ms)));
        active.updated_at = Set(now);
        Ok(active.update(&self.db).await?)
    }

    pub async fn complete_remote_validation_stage(
        &self,
        claim_id: &str,
        runner_id: &str,
        detail: Option<&str>,
        reason_code: Option<&str>,
    ) -> anyhow::Result<RegistryValidationStageMutationResult> {
        let (request, stage) = self
            .remote_validation_claim_context(claim_id, runner_id)
            .await?;
        let detail = detail
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| remote_validation_success_detail(&stage.stage_key, &request.slug));
        let reason_code = reason_code
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| remote_validation_pass_reason_code(&stage.stage_key));
        let normalized_reason_code = normalize_reason_code(
            reason_code,
            REGISTRY_VALIDATION_STAGE_REASON_CODES,
            "Remote validation completion",
        )?;
        let actor = remote_validation_runner_actor(runner_id);
        let stage = self
            .update_validation_stage_status(
                stage,
                &request,
                &actor,
                RegistryValidationStageStatus::Passed,
                &detail,
                Some(normalized_reason_code.as_str()),
            )
            .await?;
        Ok(RegistryValidationStageMutationResult { request, stage })
    }

    pub async fn fail_remote_validation_stage(
        &self,
        claim_id: &str,
        runner_id: &str,
        detail: Option<&str>,
        reason_code: Option<&str>,
    ) -> anyhow::Result<RegistryValidationStageMutationResult> {
        let (request, stage) = self
            .remote_validation_claim_context(claim_id, runner_id)
            .await?;
        let detail = detail
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| remote_validation_failure_detail(&stage.stage_key, &request.slug));
        let reason_code = reason_code
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| remote_validation_failure_reason_code(&stage.stage_key));
        let normalized_reason_code = normalize_reason_code(
            reason_code,
            REGISTRY_VALIDATION_STAGE_REASON_CODES,
            "Remote validation failure",
        )?;
        let actor = remote_validation_runner_actor(runner_id);
        let stage = self
            .update_validation_stage_status(
                stage,
                &request,
                &actor,
                RegistryValidationStageStatus::Failed,
                &detail,
                Some(normalized_reason_code.as_str()),
            )
            .await?;
        Ok(RegistryValidationStageMutationResult { request, stage })
    }

    pub async fn requeue_expired_remote_validation_claims(&self) -> anyhow::Result<usize> {
        let now = Utc::now();
        let expired_stages = RegistryValidationStageEntity::find()
            .filter(
                Condition::all()
                    .add(registry_validation_stage::Column::RunnerKind.eq("remote"))
                    .add(
                        registry_validation_stage::Column::Status
                            .eq(RegistryValidationStageStatus::Running),
                    )
                    .add(registry_validation_stage::Column::ClaimExpiresAt.lt(now)),
            )
            .order_by_asc(registry_validation_stage::Column::ClaimExpiresAt)
            .all(&self.db)
            .await?;

        let mut requeued = 0usize;
        for stage in expired_stages {
            let Some(request) = self.get_publish_request(&stage.request_id).await? else {
                continue;
            };
            let actor = "system:registry-runner-reaper";
            let detail = format!(
                "Remote validation lease expired for runner '{}' (claim '{}'); stage attempt will be requeued.",
                stage.claimed_by.as_deref().unwrap_or("unknown"),
                stage.claim_id.as_deref().unwrap_or("unknown"),
            );
            let _blocked = self
                .update_validation_stage_status(
                    stage.clone(),
                    &request,
                    actor,
                    RegistryValidationStageStatus::Blocked,
                    &detail,
                    None,
                )
                .await?;
            self.queue_validation_stage_attempt(
                &request,
                &stage.stage_key,
                actor,
                "remote_lease_expired",
                follow_up_gate_detail(&stage.stage_key),
            )
            .await?;
            requeued += 1;
        }

        Ok(requeued)
    }

    async fn latest_validation_job(
        &self,
        request_id: &str,
    ) -> anyhow::Result<Option<registry_validation_job::Model>> {
        Ok(RegistryValidationJobEntity::find()
            .filter(registry_validation_job::Column::RequestId.eq(request_id))
            .order_by_desc(registry_validation_job::Column::CreatedAt)
            .one(&self.db)
            .await?)
    }

    async fn latest_active_validation_job(
        &self,
        request_id: &str,
    ) -> anyhow::Result<Option<registry_validation_job::Model>> {
        Ok(RegistryValidationJobEntity::find()
            .filter(registry_validation_job::Column::RequestId.eq(request_id))
            .filter(registry_validation_job::Column::Status.is_in([
                RegistryValidationJobStatus::Queued,
                RegistryValidationJobStatus::Running,
            ]))
            .order_by_desc(registry_validation_job::Column::CreatedAt)
            .one(&self.db)
            .await?)
    }

    async fn create_validation_job(
        &self,
        request: &registry_publish_request::Model,
        actor: &str,
        queue_reason: &str,
    ) -> anyhow::Result<registry_validation_job::Model> {
        let now = Utc::now();
        let next_attempt_number = self
            .latest_validation_job(&request.id)
            .await?
            .map(|job| job.attempt_number + 1)
            .unwrap_or(1);
        let active = RegistryValidationJobActiveModel {
            id: Set(format!("rvj_{}", uuid::Uuid::new_v4().simple())),
            request_id: Set(request.id.clone()),
            slug: Set(request.slug.clone()),
            version: Set(request.version.clone()),
            status: Set(RegistryValidationJobStatus::Queued),
            triggered_by: Set(normalize_actor(actor)),
            queue_reason: Set(queue_reason.to_string()),
            attempt_number: Set(next_attempt_number),
            started_at: Set(None),
            finished_at: Set(None),
            last_error: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        };
        Ok(active.insert(&self.db).await?)
    }

    async fn claim_validation_job(
        &self,
        validation_job_id: &str,
        actor: &str,
    ) -> anyhow::Result<Option<RegistryValidationJobClaim>> {
        let Some(job) = RegistryValidationJobEntity::find_by_id(validation_job_id)
            .one(&self.db)
            .await?
        else {
            return Ok(None);
        };
        let request = self
            .get_publish_request(&job.request_id)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "Registry validation job '{}' points to missing publish request '{}'",
                    validation_job_id,
                    job.request_id
                )
            })?;

        match job.status {
            RegistryValidationJobStatus::Queued => {
                let started_at = Utc::now();
                let mut active: RegistryValidationJobActiveModel = job.clone().into();
                active.status = Set(RegistryValidationJobStatus::Running);
                active.started_at = Set(Some(started_at));
                active.finished_at = Set(None);
                active.last_error = Set(None);
                active.updated_at = Set(started_at);
                let job = active.update(&self.db).await?;
                self.record_governance_event(
                    &request.slug,
                    Some(&request.id),
                    None,
                    "validation_job_started",
                    actor,
                    None,
                    serde_json::json!({
                        "job_id": job.id.clone(),
                        "attempt_number": job.attempt_number,
                        "queue_reason": job.queue_reason.clone(),
                        "request_status": request_status_label(request.status.clone()),
                        "version": request.version.clone(),
                    }),
                )
                .await?;
                Ok(Some(RegistryValidationJobClaim {
                    job,
                    request,
                    should_run: true,
                }))
            }
            RegistryValidationJobStatus::Running
            | RegistryValidationJobStatus::Succeeded
            | RegistryValidationJobStatus::Failed => Ok(Some(RegistryValidationJobClaim {
                job,
                request,
                should_run: false,
            })),
        }
    }

    async fn mark_validation_job_succeeded(
        &self,
        job: registry_validation_job::Model,
        actor: &str,
        request: &registry_publish_request::Model,
    ) -> anyhow::Result<registry_validation_job::Model> {
        let finished_at = Utc::now();
        let mut active: RegistryValidationJobActiveModel = job.clone().into();
        active.status = Set(RegistryValidationJobStatus::Succeeded);
        active.finished_at = Set(Some(finished_at));
        active.last_error = Set(None);
        active.updated_at = Set(finished_at);
        let job = active.update(&self.db).await?;
        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            "validation_job_succeeded",
            actor,
            None,
            serde_json::json!({
                "job_id": job.id.clone(),
                "attempt_number": job.attempt_number,
                "queue_reason": job.queue_reason.clone(),
                "request_status": request_status_label(request.status.clone()),
                "version": request.version.clone(),
            }),
        )
        .await?;
        Ok(job)
    }

    async fn mark_validation_job_failed(
        &self,
        job: registry_validation_job::Model,
        actor: &str,
        last_error: Option<&str>,
        request: &registry_publish_request::Model,
    ) -> anyhow::Result<registry_publish_request::Model> {
        let finished_at = Utc::now();
        let mut active: RegistryValidationJobActiveModel = job.clone().into();
        active.status = Set(RegistryValidationJobStatus::Failed);
        active.finished_at = Set(Some(finished_at));
        active.last_error = Set(last_error.map(ToString::to_string));
        active.updated_at = Set(finished_at);
        let job = active.update(&self.db).await?;
        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            "validation_job_failed",
            actor,
            None,
            serde_json::json!({
                "job_id": job.id.clone(),
                "attempt_number": job.attempt_number,
                "queue_reason": job.queue_reason.clone(),
                "request_status": request_status_label(request.status.clone()),
                "version": request.version.clone(),
                "error": last_error,
            }),
        )
        .await?;
        Ok(request.clone())
    }

    pub(crate) async fn validation_stage_rows(
        &self,
        request_id: &str,
    ) -> anyhow::Result<Vec<registry_validation_stage::Model>> {
        Ok(RegistryValidationStageEntity::find()
            .filter(registry_validation_stage::Column::RequestId.eq(request_id))
            .order_by_desc(registry_validation_stage::Column::AttemptNumber)
            .order_by_desc(registry_validation_stage::Column::CreatedAt)
            .all(&self.db)
            .await?)
    }

    pub(crate) async fn latest_validation_stages_for_request(
        &self,
        request_id: &str,
    ) -> anyhow::Result<Vec<registry_validation_stage::Model>> {
        let mut latest = HashMap::<String, registry_validation_stage::Model>::new();
        for stage in self.validation_stage_rows(request_id).await? {
            latest.entry(stage.stage_key.clone()).or_insert(stage);
        }
        let mut stages = latest.into_values().collect::<Vec<_>>();
        stages.sort_by(|left, right| left.stage_key.cmp(&right.stage_key));
        Ok(stages)
    }

    async fn latest_validation_stage(
        &self,
        request_id: &str,
        stage_key: &str,
    ) -> anyhow::Result<Option<registry_validation_stage::Model>> {
        Ok(RegistryValidationStageEntity::find()
            .filter(registry_validation_stage::Column::RequestId.eq(request_id))
            .filter(registry_validation_stage::Column::StageKey.eq(stage_key))
            .order_by_desc(registry_validation_stage::Column::AttemptNumber)
            .order_by_desc(registry_validation_stage::Column::CreatedAt)
            .one(&self.db)
            .await?)
    }

    async fn remote_validation_stage_by_claim_id(
        &self,
        claim_id: &str,
    ) -> anyhow::Result<Option<registry_validation_stage::Model>> {
        Ok(RegistryValidationStageEntity::find()
            .filter(registry_validation_stage::Column::ClaimId.eq(claim_id))
            .one(&self.db)
            .await?)
    }

    async fn remote_validation_claim_context(
        &self,
        claim_id: &str,
        runner_id: &str,
    ) -> anyhow::Result<(
        registry_publish_request::Model,
        registry_validation_stage::Model,
    )> {
        let stage = self
            .remote_validation_stage_by_claim_id(claim_id)
            .await?
            .ok_or_else(|| {
                not_found_error(format!(
                    "Remote validation claim '{claim_id}' was not found"
                ))
            })?;
        ensure_remote_validation_claim_runner(&stage, runner_id)?;
        if stage.status != RegistryValidationStageStatus::Running {
            return Err(conflict_error(format!(
                "Remote validation claim '{}' is in status '{}' and cannot be completed",
                claim_id,
                validation_stage_status_label(stage.status.clone())
            )));
        }
        let now = Utc::now();
        if stage
            .claim_expires_at
            .as_ref()
            .is_some_and(|expires_at| *expires_at < now)
        {
            return Err(conflict_error(format!(
                "Remote validation claim '{claim_id}' has expired"
            )));
        }
        let request = self
            .get_publish_request(&stage.request_id)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "Remote validation claim '{}' points to missing request '{}'",
                    claim_id,
                    stage.request_id
                )
            })?;
        Ok((request, stage))
    }

    async fn latest_active_validation_stage(
        &self,
        request_id: &str,
        stage_key: &str,
    ) -> anyhow::Result<Option<registry_validation_stage::Model>> {
        Ok(RegistryValidationStageEntity::find()
            .filter(registry_validation_stage::Column::RequestId.eq(request_id))
            .filter(registry_validation_stage::Column::StageKey.eq(stage_key))
            .filter(registry_validation_stage::Column::Status.is_in([
                RegistryValidationStageStatus::Queued,
                RegistryValidationStageStatus::Running,
            ]))
            .order_by_desc(registry_validation_stage::Column::AttemptNumber)
            .order_by_desc(registry_validation_stage::Column::CreatedAt)
            .one(&self.db)
            .await?)
    }

    async fn queue_follow_up_validation_stages(
        &self,
        request: &registry_publish_request::Model,
        actor: &str,
        queue_reason: &str,
    ) -> anyhow::Result<Vec<registry_validation_stage::Model>> {
        let mut stages = Vec::new();

        for stage_key in REGISTRY_VALIDATION_FOLLOW_UP_GATES {
            if self
                .latest_active_validation_stage(&request.id, stage_key)
                .await?
                .is_some()
            {
                continue;
            }

            stages.push(
                self.queue_validation_stage_attempt(
                    request,
                    stage_key,
                    actor,
                    queue_reason,
                    follow_up_gate_detail(stage_key),
                )
                .await?,
            );
        }

        Ok(stages)
    }

    async fn queue_validation_stage_attempt(
        &self,
        request: &registry_publish_request::Model,
        stage_key: &str,
        actor: &str,
        queue_reason: &str,
        detail: &str,
    ) -> anyhow::Result<registry_validation_stage::Model> {
        let now = Utc::now();
        let next_attempt_number = self
            .latest_validation_stage(&request.id, stage_key)
            .await?
            .map(|stage| stage.attempt_number + 1)
            .unwrap_or(1);
        let active = RegistryValidationStageActiveModel {
            id: Set(format!("rvs_{}", uuid::Uuid::new_v4().simple())),
            request_id: Set(request.id.clone()),
            slug: Set(request.slug.clone()),
            version: Set(request.version.clone()),
            stage_key: Set(stage_key.to_string()),
            status: Set(RegistryValidationStageStatus::Queued),
            triggered_by: Set(normalize_actor(actor)),
            queue_reason: Set(queue_reason.to_string()),
            attempt_number: Set(next_attempt_number),
            detail: Set(detail.to_string()),
            started_at: Set(None),
            finished_at: Set(None),
            last_error: Set(None),
            claim_id: Set(None),
            claimed_by: Set(None),
            claim_expires_at: Set(None),
            last_heartbeat_at: Set(None),
            runner_kind: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        };
        let stage = active.insert(&self.db).await?;
        self.record_validation_stage_event(
            request,
            actor,
            &stage,
            "validation_stage_queued",
            detail,
            None,
            None,
        )
        .await?;
        self.record_follow_up_gate_event(
            request,
            actor,
            stage_key,
            "follow_up_gate_queued",
            "pending",
            detail,
            None,
        )
        .await?;
        Ok(stage)
    }

    async fn update_validation_stage_status(
        &self,
        stage: registry_validation_stage::Model,
        request: &registry_publish_request::Model,
        actor: &str,
        status: RegistryValidationStageStatus,
        detail: &str,
        reason_code: Option<&str>,
    ) -> anyhow::Result<registry_validation_stage::Model> {
        ensure_validation_stage_transition_allowed(&stage.status, &status, &stage.stage_key)?;

        let now = Utc::now();
        let existing_started_at = stage.started_at;
        let mut active: RegistryValidationStageActiveModel = stage.clone().into();
        active.status = Set(status.clone());
        active.detail = Set(detail.to_string());
        active.updated_at = Set(now);
        active.last_error = Set(match &status {
            RegistryValidationStageStatus::Failed => Some(detail.to_string()),
            _ => None,
        });
        match &status {
            RegistryValidationStageStatus::Queued => {
                active.started_at = Set(None);
                active.finished_at = Set(None);
                active.claim_id = Set(None);
                active.claimed_by = Set(None);
                active.claim_expires_at = Set(None);
                active.last_heartbeat_at = Set(None);
                active.runner_kind = Set(None);
            }
            RegistryValidationStageStatus::Running => {
                active.started_at = Set(existing_started_at.or(Some(now)));
                active.finished_at = Set(None);
            }
            RegistryValidationStageStatus::Passed
            | RegistryValidationStageStatus::Failed
            | RegistryValidationStageStatus::Blocked => {
                active.started_at = Set(existing_started_at.or(Some(now)));
                active.finished_at = Set(Some(now));
                active.claim_id = Set(None);
                active.claimed_by = Set(None);
                active.claim_expires_at = Set(None);
                active.last_heartbeat_at = Set(None);
                active.runner_kind = Set(None);
            }
        }
        let stage = active.update(&self.db).await?;
        let event_type = validation_stage_event_type(&status);
        self.record_validation_stage_event(
            request,
            actor,
            &stage,
            event_type,
            detail,
            reason_code,
            None,
        )
        .await?;
        match &status {
            RegistryValidationStageStatus::Passed => {
                self.record_follow_up_gate_event(
                    request,
                    actor,
                    &stage.stage_key,
                    "follow_up_gate_passed",
                    "passed",
                    detail,
                    reason_code,
                )
                .await?;
            }
            RegistryValidationStageStatus::Failed => {
                self.record_follow_up_gate_event(
                    request,
                    actor,
                    &stage.stage_key,
                    "follow_up_gate_failed",
                    "failed",
                    detail,
                    reason_code,
                )
                .await?;
            }
            _ => {}
        }
        Ok(stage)
    }

    async fn record_validation_stage_event(
        &self,
        request: &registry_publish_request::Model,
        actor: &str,
        stage: &registry_validation_stage::Model,
        event_type: &str,
        detail: &str,
        reason_code: Option<&str>,
        extra: Option<serde_json::Value>,
    ) -> anyhow::Result<registry_governance_event::Model> {
        let mut details = serde_json::json!({
            "stage_id": stage.id.clone(),
            "stage_key": stage.stage_key.clone(),
            "status": validation_stage_status_label(stage.status.clone()),
            "detail": detail,
            "attempt_number": stage.attempt_number,
            "queue_reason": stage.queue_reason.clone(),
            "request_status": request_status_label(request.status.clone()),
            "version": request.version.clone(),
            "started_at": stage.started_at.as_ref().map(|value| value.to_rfc3339()),
            "finished_at": stage.finished_at.as_ref().map(|value| value.to_rfc3339()),
        });
        if let Some(reason_code) = reason_code {
            details["reason_code"] = serde_json::Value::String(reason_code.to_string());
        }
        if let Some(extra) = extra {
            merge_json_object(&mut details, extra);
        }
        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            event_type,
            actor,
            None,
            details,
        )
        .await
    }

    async fn record_follow_up_gate_event(
        &self,
        request: &registry_publish_request::Model,
        actor: &str,
        stage_key: &str,
        event_type: &str,
        status: &str,
        detail: &str,
        reason_code: Option<&str>,
    ) -> anyhow::Result<registry_governance_event::Model> {
        let mut details = serde_json::json!({
            "stage_key": stage_key,
            "status": status,
            "detail": detail,
        });
        if let Some(reason_code) = reason_code {
            details["reason_code"] = serde_json::Value::String(reason_code.to_string());
        }
        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            event_type,
            actor,
            None,
            details,
        )
        .await
    }

    async fn latest_request_event_type(&self, request_id: &str) -> anyhow::Result<Option<String>> {
        Ok(RegistryGovernanceEventEntity::find()
            .filter(registry_governance_event::Column::RequestId.eq(request_id))
            .order_by_desc(registry_governance_event::Column::CreatedAt)
            .one(&self.db)
            .await?
            .map(|event| event.event_type))
    }

    async fn store_validation_rejection(
        &self,
        request: registry_publish_request::Model,
        actor: &str,
        warnings: &[String],
        errors: &[String],
        failed_checks: Vec<RegistryValidationCheckDetail>,
    ) -> anyhow::Result<registry_publish_request::Model> {
        let rejected_at = Utc::now();
        let errors = dedupe_message_list(errors.to_vec());
        let warnings = dedupe_message_list(warnings.to_vec());
        let mut request_active: RegistryPublishRequestActiveModel = request.into();
        request_active.status = Set(RegistryPublishRequestStatus::Rejected);
        request_active.validation_warnings = Set(serde_json::to_value(&warnings)?);
        request_active.validation_errors = Set(serde_json::to_value(&errors)?);
        request_active.rejected_by = Set(Some(actor_principal(actor).to_json_value()));
        request_active.rejection_reason = Set(errors.first().cloned());
        request_active.validated_at = Set(Some(rejected_at));
        request_active.approved_by = Set(None);
        request_active.approved_at = Set(None);
        request_active.published_at = Set(None);
        request_active.updated_at = Set(rejected_at);
        let request = request_active.update(&self.db).await?;
        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            "validation_failed",
            actor,
            None,
            serde_json::json!({
                "version": request.version.clone(),
                "status": request_status_label(request.status.clone()),
                "reason": request.rejection_reason.clone(),
                "warnings": warnings,
                "errors": errors,
                "automated_checks": failed_checks,
            }),
        )
        .await?;
        Ok(request)
    }
}

pub fn validation_stage_status_label(status: RegistryValidationStageStatus) -> &'static str {
    match status {
        RegistryValidationStageStatus::Queued => "queued",
        RegistryValidationStageStatus::Running => "running",
        RegistryValidationStageStatus::Passed => "passed",
        RegistryValidationStageStatus::Failed => "failed",
        RegistryValidationStageStatus::Blocked => "blocked",
    }
}

fn validation_stage_event_type(status: &RegistryValidationStageStatus) -> &'static str {
    match status {
        RegistryValidationStageStatus::Queued => "validation_stage_queued",
        RegistryValidationStageStatus::Running => "validation_stage_running",
        RegistryValidationStageStatus::Passed => "validation_stage_passed",
        RegistryValidationStageStatus::Failed => "validation_stage_failed",
        RegistryValidationStageStatus::Blocked => "validation_stage_blocked",
    }
}

fn parse_validation_stage_status(value: &str) -> anyhow::Result<RegistryValidationStageStatus> {
    match value.trim().to_ascii_lowercase().as_str() {
        "queued" => Ok(RegistryValidationStageStatus::Queued),
        "running" => Ok(RegistryValidationStageStatus::Running),
        "passed" => Ok(RegistryValidationStageStatus::Passed),
        "failed" => Ok(RegistryValidationStageStatus::Failed),
        "blocked" => Ok(RegistryValidationStageStatus::Blocked),
        other => Err(malformed_error(format!(
            "Unsupported validation stage status '{}'; expected queued, running, passed, failed, or blocked",
            other
        ))),
    }
}

fn normalize_validation_stage_key(value: &str) -> anyhow::Result<&str> {
    let value = value.trim();
    if REGISTRY_VALIDATION_FOLLOW_UP_GATES
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(value))
    {
        let canonical = REGISTRY_VALIDATION_FOLLOW_UP_GATES
            .iter()
            .find(|candidate| candidate.eq_ignore_ascii_case(value))
            .copied()
            .expect("validated gate must exist");
        return Ok(canonical);
    }

    Err(malformed_error(format!(
        "Unsupported validation stage '{}'; expected one of {}",
        value,
        REGISTRY_VALIDATION_FOLLOW_UP_GATES.join(", ")
    )))
}

fn default_validation_stage_detail(
    stage_key: &str,
    status: &RegistryValidationStageStatus,
) -> String {
    match status {
        RegistryValidationStageStatus::Queued => follow_up_gate_detail(stage_key).to_string(),
        RegistryValidationStageStatus::Running => {
            format!("Validation stage '{stage_key}' is now running.")
        }
        RegistryValidationStageStatus::Passed => {
            format!("Validation stage '{stage_key}' passed.")
        }
        RegistryValidationStageStatus::Failed => {
            format!("Validation stage '{stage_key}' failed.")
        }
        RegistryValidationStageStatus::Blocked => {
            format!("Validation stage '{stage_key}' is blocked on external follow-up.")
        }
    }
}

fn ensure_validation_stage_transition_allowed(
    current: &RegistryValidationStageStatus,
    next: &RegistryValidationStageStatus,
    stage_key: &str,
) -> anyhow::Result<()> {
    let allowed = match current {
        RegistryValidationStageStatus::Queued => matches!(
            next,
            RegistryValidationStageStatus::Running
                | RegistryValidationStageStatus::Passed
                | RegistryValidationStageStatus::Failed
                | RegistryValidationStageStatus::Blocked
        ),
        RegistryValidationStageStatus::Running => matches!(
            next,
            RegistryValidationStageStatus::Running
                | RegistryValidationStageStatus::Passed
                | RegistryValidationStageStatus::Failed
                | RegistryValidationStageStatus::Blocked
        ),
        RegistryValidationStageStatus::Blocked => matches!(
            next,
            RegistryValidationStageStatus::Running
                | RegistryValidationStageStatus::Passed
                | RegistryValidationStageStatus::Failed
                | RegistryValidationStageStatus::Blocked
        ),
        RegistryValidationStageStatus::Passed | RegistryValidationStageStatus::Failed => false,
    };

    if allowed {
        return Ok(());
    }

    Err(conflict_error(format!(
        "Validation stage '{}' cannot move from '{}' to '{}' without requeue",
        stage_key,
        validation_stage_status_label(current.clone()),
        validation_stage_status_label(next.clone())
    )))
}

fn remote_validation_runner_actor(runner_id: &str) -> String {
    normalize_actor(&format!("remote-runner:{runner_id}"))
}

fn remote_validation_execution_mode(_stage_key: &str) -> &'static str {
    "local_workspace"
}

fn remote_validation_stage_requires_manual_confirmation(stage_key: &str) -> bool {
    stage_key == "security_policy_review"
}

fn remote_validation_pass_reason_code(stage_key: &str) -> &'static str {
    match stage_key {
        "security_policy_review" => "manual_review_complete",
        _ => "local_runner_passed",
    }
}

fn remote_validation_failure_reason_code(stage_key: &str) -> &'static str {
    match stage_key {
        "compile_smoke" => "build_failure",
        "targeted_tests" => "test_failure",
        "security_policy_review" => "policy_preflight_failed",
        _ => "manual_override",
    }
}

fn remote_validation_blocked_reason_code(stage_key: &str) -> &'static str {
    match stage_key {
        "security_policy_review" => "security_findings",
        _ => "manual_override",
    }
}

fn remote_validation_stage_claim_detail(stage_key: &str, runner_id: &str) -> String {
    format!(
        "Remote runner '{}' claimed validation stage '{}'.",
        runner_id, stage_key
    )
}

fn remote_validation_success_detail(stage_key: &str, slug: &str) -> String {
    match stage_key {
        "compile_smoke" => {
            format!("Remote compile smoke completed successfully for module '{slug}'.")
        }
        "targeted_tests" => {
            format!("Remote targeted tests completed successfully for module '{slug}'.")
        }
        "security_policy_review" => format!(
            "Remote security/policy preflight completed and manual review was confirmed for module '{slug}'."
        ),
        _ => format!("Remote validation stage '{stage_key}' completed successfully for '{slug}'."),
    }
}

fn remote_validation_failure_detail(stage_key: &str, slug: &str) -> String {
    match stage_key {
        "compile_smoke" => format!("Remote compile smoke failed for module '{slug}'."),
        "targeted_tests" => format!("Remote targeted tests failed for module '{slug}'."),
        "security_policy_review" => {
            format!("Remote security/policy preflight failed for module '{slug}'.")
        }
        _ => format!("Remote validation stage '{stage_key}' failed for '{slug}'."),
    }
}

fn remote_validation_lease_ttl(lease_ttl_ms: u64) -> Duration {
    Duration::milliseconds(lease_ttl_ms.max(1).min(i64::MAX as u64) as i64)
}

fn ensure_remote_validation_claim_runner(
    stage: &registry_validation_stage::Model,
    runner_id: &str,
) -> anyhow::Result<()> {
    let claimed_by = stage.claimed_by.as_deref().ok_or_else(|| {
        conflict_error(format!(
            "Remote validation stage '{}' is not currently claimed",
            stage.id
        ))
    })?;
    if claimed_by != runner_id {
        return Err(forbidden_error(format!(
            "Remote validation claim '{}' belongs to runner '{}', not '{}'",
            stage.claim_id.as_deref().unwrap_or("unknown"),
            claimed_by,
            runner_id
        )));
    }
    if stage.runner_kind.as_deref() != Some("remote") {
        return Err(forbidden_error(format!(
            "Remote validation claim '{}' is not owned by a remote runner",
            stage.claim_id.as_deref().unwrap_or("unknown")
        )));
    }
    Ok(())
}

pub(crate) fn validation_stage_details_value(
    stage: &registry_validation_stage::Model,
) -> serde_json::Value {
    serde_json::json!({
        "stage_id": stage.id.clone(),
        "stage_key": stage.stage_key.clone(),
        "status": validation_stage_status_label(stage.status.clone()),
        "detail": stage.detail.clone(),
        "attempt_number": stage.attempt_number,
        "queue_reason": stage.queue_reason.clone(),
        "started_at": stage.started_at.as_ref().map(|value| value.to_rfc3339()),
        "finished_at": stage.finished_at.as_ref().map(|value| value.to_rfc3339()),
        "updated_at": stage.updated_at.to_rfc3339(),
    })
}

fn merge_json_object(target: &mut serde_json::Value, extra: serde_json::Value) {
    let Some(target_map) = target.as_object_mut() else {
        return;
    };
    let Some(extra_map) = extra.as_object() else {
        return;
    };
    for (key, value) in extra_map {
        target_map.insert(key.clone(), value.clone());
    }
}

pub(crate) fn derive_validation_stage_snapshots(
    latest_request: Option<&registry_publish_request::Model>,
    recent_events: &[registry_governance_event::Model],
    stage_rows: &[registry_validation_stage::Model],
) -> Vec<RegistryValidationStageSnapshot> {
    let mut snapshots = Vec::new();
    let mut seen = HashSet::new();
    let mut latest_by_stage = HashMap::new();

    for stage in stage_rows {
        if seen.insert(stage.stage_key.as_str()) {
            latest_by_stage.insert(stage.stage_key.as_str(), stage);
        }
    }

    for stage_key in REGISTRY_VALIDATION_FOLLOW_UP_GATES {
        if let Some(stage) = latest_by_stage.get(stage_key) {
            snapshots.push(RegistryValidationStageSnapshot {
                key: (*stage_key).to_string(),
                status: validation_stage_status_label(stage.status.clone()).to_string(),
                detail: stage.detail.clone(),
                attempt_number: stage.attempt_number,
                updated_at: stage.updated_at.to_rfc3339(),
                started_at: stage.started_at.as_ref().map(|value| value.to_rfc3339()),
                finished_at: stage.finished_at.as_ref().map(|value| value.to_rfc3339()),
            });
            continue;
        }

        let latest_event = recent_events.iter().find(|event| {
            matches!(
                event.event_type.as_str(),
                "follow_up_gate_queued" | "follow_up_gate_passed" | "follow_up_gate_failed"
            ) && event
                .details
                .get("stage_key")
                .and_then(serde_json::Value::as_str)
                == Some(*stage_key)
        });

        if let Some(event) = latest_event {
            let status = event
                .details
                .get("status")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| match event.event_type.as_str() {
                    "follow_up_gate_passed" => "passed",
                    "follow_up_gate_failed" => "failed",
                    _ => "queued",
                });
            let normalized_status = if status.eq_ignore_ascii_case("pending") {
                "queued"
            } else {
                status
            };
            let detail = event
                .details
                .get("detail")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| follow_up_gate_detail(stage_key));
            snapshots.push(RegistryValidationStageSnapshot {
                key: (*stage_key).to_string(),
                status: normalized_status.to_string(),
                detail: detail.to_string(),
                attempt_number: 0,
                updated_at: event.created_at.to_rfc3339(),
                started_at: None,
                finished_at: None,
            });
            continue;
        }

        if latest_request.is_some_and(|request| {
            matches!(
                request.status,
                RegistryPublishRequestStatus::Approved | RegistryPublishRequestStatus::Published
            )
        }) {
            snapshots.push(RegistryValidationStageSnapshot {
                key: (*stage_key).to_string(),
                status: "queued".to_string(),
                detail: follow_up_gate_detail(stage_key).to_string(),
                attempt_number: 0,
                updated_at: latest_request
                    .and_then(|request| {
                        request
                            .validated_at
                            .as_ref()
                            .or(request.approved_at.as_ref())
                    })
                    .map(|ts| ts.to_rfc3339())
                    .unwrap_or_default(),
                started_at: None,
                finished_at: None,
            });
        }
    }

    snapshots
}

pub(crate) fn derive_follow_up_gate_snapshots(
    latest_request: Option<&registry_publish_request::Model>,
    recent_events: &[registry_governance_event::Model],
    validation_stages: &[RegistryValidationStageSnapshot],
) -> Vec<RegistryFollowUpGateSnapshot> {
    if !validation_stages.is_empty() {
        return validation_stages
            .iter()
            .map(|stage| RegistryFollowUpGateSnapshot {
                key: stage.key.clone(),
                status: match stage.status.as_str() {
                    "queued" => "pending".to_string(),
                    other => other.to_string(),
                },
                detail: stage.detail.clone(),
                updated_at: stage.updated_at.clone(),
            })
            .collect();
    }

    let mut snapshots = Vec::new();

    for gate in REGISTRY_VALIDATION_FOLLOW_UP_GATES {
        let latest_event = recent_events.iter().find(|event| {
            matches!(
                event.event_type.as_str(),
                "follow_up_gate_queued" | "follow_up_gate_passed" | "follow_up_gate_failed"
            ) && event
                .details
                .get("stage_key")
                .and_then(serde_json::Value::as_str)
                == Some(*gate)
        });

        if let Some(event) = latest_event {
            let status = event
                .details
                .get("status")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| match event.event_type.as_str() {
                    "follow_up_gate_passed" => "passed",
                    "follow_up_gate_failed" => "failed",
                    _ => "pending",
                });
            let detail = event
                .details
                .get("detail")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| follow_up_gate_detail(gate));

            snapshots.push(RegistryFollowUpGateSnapshot {
                key: (*gate).to_string(),
                status: status.to_string(),
                detail: detail.to_string(),
                updated_at: event.created_at.to_rfc3339(),
            });
            continue;
        }

        if latest_request.is_some_and(|request| {
            matches!(
                request.status,
                RegistryPublishRequestStatus::Approved | RegistryPublishRequestStatus::Published
            )
        }) {
            snapshots.push(RegistryFollowUpGateSnapshot {
                key: (*gate).to_string(),
                status: "pending".to_string(),
                detail: follow_up_gate_detail(gate).to_string(),
                updated_at: latest_request
                    .and_then(|request| {
                        request
                            .validated_at
                            .as_ref()
                            .or(request.approved_at.as_ref())
                    })
                    .map(|ts| ts.to_rfc3339())
                    .unwrap_or_default(),
            });
        }
    }

    snapshots
}

pub(crate) fn rejected_publish_request_can_retry(
    latest_event_type: Option<&str>,
    rejection_reason: Option<&str>,
) -> bool {
    if matches!(latest_event_type, Some("validation_failed")) {
        return true;
    }

    rejection_reason
        .is_some_and(|reason| !reason.trim().starts_with("Governance rejection reason:"))
}

pub(crate) fn normalize_actor(value: &str) -> String {
    let actor = value.trim();
    if actor.is_empty() {
        "system:auto".to_string()
    } else {
        actor.to_string()
    }
}

pub(crate) fn dedupe_message_list(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for value in values {
        let value = value.trim().to_string();
        if value.is_empty() {
            continue;
        }
        if seen.insert(value.clone()) {
            deduped.push(value);
        }
    }
    deduped
}

pub(crate) fn deserialize_message_list(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(ToString::to_string))
        .collect()
}

pub(crate) fn compare_semver_desc(left: &str, right: &str) -> std::cmp::Ordering {
    match (semver::Version::parse(left), semver::Version::parse(right)) {
        (Ok(left), Ok(right)) => right.cmp(&left),
        (Ok(_), Err(_)) => std::cmp::Ordering::Less,
        (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
        (Err(_), Err(_)) => std::cmp::Ordering::Equal,
    }
}

pub(crate) async fn validate_registry_artifact_bundle(
    db: &DatabaseConnection,
    request: &registry_publish_request::Model,
    artifact: &RegistryArtifactUpload,
) -> anyhow::Result<RegistryArtifactValidation> {
    let mut validation = RegistryArtifactValidation::default();

    if !artifact
        .content_type
        .eq_ignore_ascii_case("application/json")
    {
        validation.warnings.push(format!(
            "Artifact upload content-type '{}' is accepted, but application/json is the canonical bundle content-type.",
            artifact.content_type
        ));
    }

    let bundle = match serde_json::from_slice::<RegistryPublishArtifactBundle>(&artifact.bytes) {
        Ok(bundle) => bundle,
        Err(error) => {
            validation.errors.push(format!(
                "Artifact bundle is not valid JSON for the registry publish contract: {error}"
            ));
            return Ok(validation);
        }
    };
    let request_metadata = load_publish_request_metadata(
        db,
        &request.id,
        Some(request.default_locale.as_str()),
        Some(request.default_locale.as_str()),
    )
    .await?;

    if bundle.schema_version != REGISTRY_MUTATION_SCHEMA_VERSION {
        validation.errors.push(format!(
            "Artifact bundle schema_version '{}' does not match registry mutation schema '{}'.",
            bundle.schema_version, REGISTRY_MUTATION_SCHEMA_VERSION
        ));
    }
    if bundle.artifact_type != REGISTRY_ARTIFACT_BUNDLE_TYPE {
        validation.errors.push(format!(
            "Artifact bundle type '{}' does not match expected '{}'.",
            bundle.artifact_type, REGISTRY_ARTIFACT_BUNDLE_TYPE
        ));
    }

    validate_artifact_module_contract(request, &request_metadata, &bundle, &mut validation);
    validate_artifact_file_contract(request, &request_metadata, &bundle, &mut validation);

    validation.warnings = dedupe_message_list(validation.warnings);
    validation.errors = dedupe_message_list(validation.errors);
    Ok(validation)
}

fn validate_artifact_module_contract(
    request: &registry_publish_request::Model,
    request_metadata: &RegistryLocalizedMetadata,
    bundle: &RegistryPublishArtifactBundle,
    validation: &mut RegistryArtifactValidation,
) {
    let request_marketplace: RegistryPublishMarketplaceRequest =
        serde_json::from_value(request.marketplace.clone()).unwrap_or_default();
    let request_ui = request_ui_packages(request);

    validate_exact_field(
        "module.slug",
        &bundle.module.slug,
        &request.slug,
        &mut validation.errors,
    );
    validate_exact_field(
        "module.version",
        &bundle.module.version,
        &request.version,
        &mut validation.errors,
    );
    validate_exact_field(
        "module.crate_name",
        &bundle.module.crate_name,
        &request.crate_name,
        &mut validation.errors,
    );
    validate_exact_field(
        "module.name",
        &bundle.module.module_name,
        &request_metadata.name,
        &mut validation.errors,
    );
    validate_exact_field(
        "module.description",
        &bundle.module.module_description,
        &request_metadata.description,
        &mut validation.errors,
    );
    validate_exact_field(
        "module.ownership",
        &bundle.module.ownership,
        &request.ownership,
        &mut validation.errors,
    );
    validate_exact_field(
        "module.trust_level",
        &bundle.module.trust_level,
        &request.trust_level,
        &mut validation.errors,
    );
    validate_exact_field(
        "module.license",
        &bundle.module.license,
        &request.license,
        &mut validation.errors,
    );
    validate_optional_field(
        "module.entry_type",
        bundle.module.module_entry_type.as_deref(),
        request.entry_type.as_deref(),
        &mut validation.errors,
    );
    validate_optional_field(
        "module.marketplace.category",
        bundle.module.marketplace.category.as_deref(),
        request_marketplace.category.as_deref(),
        &mut validation.errors,
    );

    if normalize_string_list(&bundle.module.marketplace.tags)
        != normalize_string_list(&request_marketplace.tags)
    {
        validation.errors.push(format!(
            "Artifact bundle module.marketplace.tags {:?} does not match publish request {:?}.",
            bundle.module.marketplace.tags, request_marketplace.tags
        ));
    }

    validate_optional_field(
        "module.ui_packages.admin.crate_name",
        bundle
            .module
            .ui_packages
            .admin
            .as_ref()
            .map(|ui| ui.crate_name.as_str()),
        request_ui.admin.as_ref().map(|ui| ui.crate_name.as_str()),
        &mut validation.errors,
    );
    validate_optional_field(
        "module.ui_packages.storefront.crate_name",
        bundle
            .module
            .ui_packages
            .storefront
            .as_ref()
            .map(|ui| ui.crate_name.as_str()),
        request_ui
            .storefront
            .as_ref()
            .map(|ui| ui.crate_name.as_str()),
        &mut validation.errors,
    );
}

fn validate_artifact_file_contract(
    request: &registry_publish_request::Model,
    request_metadata: &RegistryLocalizedMetadata,
    bundle: &RegistryPublishArtifactBundle,
    validation: &mut RegistryArtifactValidation,
) {
    let request_marketplace: RegistryPublishMarketplaceRequest =
        serde_json::from_value(request.marketplace.clone()).unwrap_or_default();
    let request_ui = request_ui_packages(request);

    let package_manifest = require_bundle_file(
        "rustok-module.toml",
        bundle.files.package_manifest.as_deref(),
        &mut validation.errors,
    );
    let crate_manifest = require_bundle_file(
        "Cargo.toml",
        bundle.files.crate_manifest.as_deref(),
        &mut validation.errors,
    );

    match (&request_ui.admin, bundle.files.admin_manifest.as_deref()) {
        (Some(_), None) => validation.errors.push(
            "Artifact bundle must include admin/Cargo.toml because the publish request declares an admin UI package."
                .to_string(),
        ),
        (None, Some(_)) => validation.errors.push(
            "Artifact bundle includes admin/Cargo.toml, but the publish request does not declare an admin UI package."
                .to_string(),
        ),
        _ => {}
    }
    match (&request_ui.storefront, bundle.files.storefront_manifest.as_deref()) {
        (Some(_), None) => validation.errors.push(
            "Artifact bundle must include storefront/Cargo.toml because the publish request declares a storefront UI package."
                .to_string(),
        ),
        (None, Some(_)) => validation.errors.push(
            "Artifact bundle includes storefront/Cargo.toml, but the publish request does not declare a storefront UI package."
                .to_string(),
        ),
        _ => {}
    }

    if let Some(source) = package_manifest {
        validate_package_manifest_contract(
            source,
            request,
            request_metadata,
            &request_marketplace,
            &request_ui,
            validation,
        );
    }
    if let Some(source) = crate_manifest {
        validate_cargo_manifest_contract(
            "Cargo.toml",
            source,
            &request.crate_name,
            &request.version,
            Some(&request.license),
            validation,
        );
    }
    if let (Some(ui), Some(source)) = (&request_ui.admin, bundle.files.admin_manifest.as_deref()) {
        validate_cargo_manifest_contract(
            "admin/Cargo.toml",
            source,
            &ui.crate_name,
            &request.version,
            None,
            validation,
        );
    }
    if let (Some(ui), Some(source)) = (
        &request_ui.storefront,
        bundle.files.storefront_manifest.as_deref(),
    ) {
        validate_cargo_manifest_contract(
            "storefront/Cargo.toml",
            source,
            &ui.crate_name,
            &request.version,
            None,
            validation,
        );
    }
}

fn validate_package_manifest_contract(
    source: &str,
    request: &registry_publish_request::Model,
    request_metadata: &RegistryLocalizedMetadata,
    request_marketplace: &RegistryPublishMarketplaceRequest,
    request_ui: &RegistryPublishUiPackagesRequest,
    validation: &mut RegistryArtifactValidation,
) {
    let manifest = match source.parse::<toml::Table>() {
        Ok(manifest) => toml::Value::Table(manifest),
        Err(error) => {
            validation.errors.push(format!(
                "Artifact file rustok-module.toml is not valid TOML: {error}"
            ));
            return;
        }
    };

    validate_toml_string_field(
        &manifest,
        &["module", "slug"],
        "rustok-module.toml [module].slug",
        &request.slug,
        &mut validation.errors,
    );
    validate_toml_string_field(
        &manifest,
        &["module", "name"],
        "rustok-module.toml [module].name",
        &request_metadata.name,
        &mut validation.errors,
    );
    validate_toml_string_field(
        &manifest,
        &["module", "version"],
        "rustok-module.toml [module].version",
        &request.version,
        &mut validation.errors,
    );
    validate_toml_string_field(
        &manifest,
        &["module", "description"],
        "rustok-module.toml [module].description",
        &request_metadata.description,
        &mut validation.errors,
    );
    validate_toml_string_field(
        &manifest,
        &["module", "ownership"],
        "rustok-module.toml [module].ownership",
        &request.ownership,
        &mut validation.errors,
    );
    validate_toml_string_field(
        &manifest,
        &["module", "trust_level"],
        "rustok-module.toml [module].trust_level",
        &request.trust_level,
        &mut validation.errors,
    );
    validate_toml_optional_string_field(
        &manifest,
        &["marketplace", "category"],
        "rustok-module.toml [marketplace].category",
        request_marketplace.category.as_deref(),
        &mut validation.errors,
    );
    validate_toml_optional_string_field(
        &manifest,
        &["crate", "entry_type"],
        "rustok-module.toml [crate].entry_type",
        request.entry_type.as_deref(),
        &mut validation.errors,
    );

    if toml_string_list_field(&manifest, &["marketplace", "tags"])
        != normalize_string_list(&request_marketplace.tags)
    {
        validation.errors.push(format!(
            "Artifact file rustok-module.toml [marketplace].tags {:?} does not match publish request {:?}.",
            toml_string_list_field(&manifest, &["marketplace", "tags"]),
            request_marketplace.tags
        ));
    }

    validate_toml_optional_string_field(
        &manifest,
        &["provides", "admin_ui", "leptos_crate"],
        "rustok-module.toml [provides.admin_ui].leptos_crate",
        request_ui.admin.as_ref().map(|ui| ui.crate_name.as_str()),
        &mut validation.errors,
    );
    validate_toml_optional_string_field(
        &manifest,
        &["provides", "storefront_ui", "leptos_crate"],
        "rustok-module.toml [provides.storefront_ui].leptos_crate",
        request_ui
            .storefront
            .as_ref()
            .map(|ui| ui.crate_name.as_str()),
        &mut validation.errors,
    );
}

fn validate_cargo_manifest_contract(
    label: &str,
    source: &str,
    expected_name: &str,
    expected_version: &str,
    expected_license: Option<&str>,
    validation: &mut RegistryArtifactValidation,
) {
    let manifest = match source.parse::<toml::Table>() {
        Ok(manifest) => toml::Value::Table(manifest),
        Err(error) => {
            validation
                .errors
                .push(format!("Artifact file {label} is not valid TOML: {error}"));
            return;
        }
    };

    validate_toml_string_field(
        &manifest,
        &["package", "name"],
        &format!("{label} [package].name"),
        expected_name,
        &mut validation.errors,
    );
    validate_toml_workspace_aware_string_field(
        &manifest,
        &["package", "version"],
        &format!("{label} [package].version"),
        expected_version,
        &mut validation.warnings,
        &mut validation.errors,
    );
    if let Some(expected_license) = expected_license {
        validate_toml_workspace_aware_string_field(
            &manifest,
            &["package", "license"],
            &format!("{label} [package].license"),
            expected_license,
            &mut validation.warnings,
            &mut validation.errors,
        );
    }
}

fn validate_exact_field(label: &str, actual: &str, expected: &str, errors: &mut Vec<String>) {
    if actual.trim() != expected.trim() {
        errors.push(format!(
            "Artifact bundle {label} '{}' does not match publish request '{}'.",
            actual, expected
        ));
    }
}

fn validate_optional_field(
    label: &str,
    actual: Option<&str>,
    expected: Option<&str>,
    errors: &mut Vec<String>,
) {
    let actual = actual.map(str::trim).filter(|value| !value.is_empty());
    let expected = expected.map(str::trim).filter(|value| !value.is_empty());
    if actual != expected {
        errors.push(format!(
            "Artifact bundle {label} {:?} does not match publish request {:?}.",
            actual, expected
        ));
    }
}

fn require_bundle_file<'a>(
    label: &str,
    source: Option<&'a str>,
    errors: &mut Vec<String>,
) -> Option<&'a str> {
    match source.map(str::trim) {
        Some(source) if !source.is_empty() => Some(source),
        _ => {
            errors.push(format!(
                "Artifact bundle must include non-empty file '{label}'."
            ));
            None
        }
    }
}

fn normalize_string_list(values: &[String]) -> Vec<String> {
    let mut values = values
        .iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn toml_value_at_path<'a>(value: &'a toml::Value, path: &[&str]) -> Option<&'a toml::Value> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}

fn toml_string_field(value: &toml::Value, path: &[&str]) -> Option<String> {
    toml_value_at_path(value, path)
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn toml_string_list_field(value: &toml::Value, path: &[&str]) -> Vec<String> {
    toml_value_at_path(value, path)
        .and_then(toml::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::trim).map(ToString::to_string))
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .map(|mut values| {
            values.sort();
            values.dedup();
            values
        })
        .unwrap_or_default()
}

fn toml_is_workspace_inherited(value: &toml::Value, path: &[&str]) -> bool {
    toml_value_at_path(value, path)
        .and_then(toml::Value::as_table)
        .and_then(|table| table.get("workspace"))
        .and_then(toml::Value::as_bool)
        == Some(true)
}

fn validate_toml_string_field(
    manifest: &toml::Value,
    path: &[&str],
    label: &str,
    expected: &str,
    errors: &mut Vec<String>,
) {
    let actual = toml_string_field(manifest, path);
    if actual.as_deref() != Some(expected.trim()) {
        errors.push(format!(
            "Artifact file {label} {:?} does not match publish request '{}'.",
            actual, expected
        ));
    }
}

fn validate_toml_optional_string_field(
    manifest: &toml::Value,
    path: &[&str],
    label: &str,
    expected: Option<&str>,
    errors: &mut Vec<String>,
) {
    let actual = toml_string_field(manifest, path);
    let expected = expected
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    if actual != expected {
        errors.push(format!(
            "Artifact file {label} {:?} does not match publish request {:?}.",
            actual, expected
        ));
    }
}

fn validate_toml_workspace_aware_string_field(
    manifest: &toml::Value,
    path: &[&str],
    label: &str,
    expected: &str,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    if let Some(actual) = toml_string_field(manifest, path) {
        if actual != expected.trim() {
            errors.push(format!(
                "Artifact file {label} '{}' does not match publish request '{}'.",
                actual, expected
            ));
        }
        return;
    }

    if toml_is_workspace_inherited(manifest, path) {
        warnings.push(format!(
            "Artifact file {label} uses workspace inheritance, so the registry validator cannot verify it from the uploaded bundle alone."
        ));
        return;
    }

    warnings.push(format!(
        "Artifact file {label} is missing, so the registry validator could not verify it from the uploaded bundle."
    ));
}
