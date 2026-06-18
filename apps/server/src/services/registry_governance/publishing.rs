use super::*;

impl RegistryGovernanceService {
    pub async fn create_publish_request(
        &self,
        request: &RegistryPublishRequest,
        authority: &RegistryAuthority,
        warnings: &[String],
    ) -> anyhow::Result<registry_publish_request::Model> {
        self.ensure_authority_can_create_publish_request(authority, &request.module.slug)
            .await?;

        let existing_active_release = RegistryModuleReleaseEntity::find()
            .filter(registry_module_release::Column::Slug.eq(&request.module.slug))
            .filter(registry_module_release::Column::Version.eq(&request.module.version))
            .filter(registry_module_release::Column::Status.eq(RegistryModuleReleaseStatus::Active))
            .one(&self.db)
            .await?;
        if existing_active_release.is_some() {
            return Err(conflict_error(format!(
                "Published release '{}@{}' already exists",
                request.module.slug, request.module.version
            )));
        }

        let now = Utc::now();
        let request_id = format!("rpr_{}", uuid::Uuid::new_v4().simple());
        let model = registry_publish_request::Model {
            id: request_id.clone(),
            slug: request.module.slug.clone(),
            version: request.module.version.clone(),
            crate_name: request.module.crate_name.clone(),
            default_locale: normalize_registry_locale(&request.module.default_locale),
            ownership: request.module.ownership.clone(),
            trust_level: request.module.trust_level.clone(),
            license: request.module.license.clone(),
            entry_type: request.module.entry_type.clone(),
            marketplace: serde_json::to_value(&request.module.marketplace)
                .context("failed to serialize registry publish marketplace metadata")?,
            ui_packages: serde_json::to_value(&request.module.ui_packages)
                .context("failed to serialize registry publish ui_packages metadata")?,
            status: RegistryPublishRequestStatus::Draft,
            requested_by: authority.principal.to_json_value(),
            publisher_principal: Some(authority.principal.to_json_value()),
            approved_by: None,
            rejected_by: None,
            rejection_reason: None,
            changes_requested_by: None,
            changes_requested_reason: None,
            changes_requested_reason_code: None,
            changes_requested_at: None,
            held_by: None,
            held_reason: None,
            held_reason_code: None,
            held_at: None,
            held_from_status: None,
            validation_warnings: serde_json::to_value(warnings)
                .context("failed to serialize registry publish warnings")?,
            validation_errors: serde_json::json!([]),
            artifact_storage_key: None,
            artifact_checksum_sha256: None,
            artifact_size: None,
            artifact_content_type: None,
            submitted_at: None,
            validated_at: None,
            approved_at: None,
            published_at: None,
            created_at: now,
            updated_at: now,
        };

        let active_model = RegistryPublishRequestActiveModel {
            id: Set(model.id.clone()),
            slug: Set(model.slug.clone()),
            version: Set(model.version.clone()),
            crate_name: Set(model.crate_name.clone()),
            default_locale: Set(model.default_locale.clone()),
            ownership: Set(model.ownership.clone()),
            trust_level: Set(model.trust_level.clone()),
            license: Set(model.license.clone()),
            entry_type: Set(model.entry_type.clone()),
            marketplace: Set(model.marketplace.clone()),
            ui_packages: Set(model.ui_packages.clone()),
            status: Set(model.status.clone()),
            requested_by: Set(model.requested_by.clone()),
            publisher_principal: Set(model.publisher_principal.clone()),
            approved_by: Set(None),
            rejected_by: Set(None),
            rejection_reason: Set(None),
            changes_requested_by: Set(None),
            changes_requested_reason: Set(None),
            changes_requested_reason_code: Set(None),
            changes_requested_at: Set(None),
            held_by: Set(None),
            held_reason: Set(None),
            held_reason_code: Set(None),
            held_at: Set(None),
            held_from_status: Set(None),
            validation_warnings: Set(model.validation_warnings.clone()),
            validation_errors: Set(model.validation_errors.clone()),
            artifact_storage_key: Set(None),
            artifact_checksum_sha256: Set(None),
            artifact_size: Set(None),
            artifact_content_type: Set(None),
            submitted_at: Set(None),
            validated_at: Set(None),
            approved_at: Set(None),
            published_at: Set(None),
            created_at: Set(model.created_at),
            updated_at: Set(model.updated_at),
        };

        active_model.insert(&self.db).await?;
        self.upsert_publish_request_translation(
            &model.id,
            &model.default_locale,
            &request.module.name,
            &request.module.description,
        )
        .await?;
        self.record_governance_event(
            &model.slug,
            Some(&model.id),
            None,
            "request_created",
            authority_actor(authority),
            Some(authority.principal.label()),
            serde_json::json!({
                "version": model.version.clone(),
                "status": request_status_label(RegistryPublishRequestStatus::Draft),
                "warnings": warnings,
            }),
        )
        .await?;
        self.get_publish_request(&request_id).await?.ok_or_else(|| {
            anyhow::Error::new(RegistryGovernanceError::Internal(anyhow!(
                "registry publish request disappeared after insert"
            )))
        })
    }

    pub async fn get_publish_request(
        &self,
        request_id: &str,
    ) -> anyhow::Result<Option<registry_publish_request::Model>> {
        Ok(RegistryPublishRequestEntity::find_by_id(request_id)
            .one(&self.db)
            .await?)
    }

    async fn publish_request_default_metadata(
        &self,
        request: &registry_publish_request::Model,
    ) -> anyhow::Result<RegistryLocalizedMetadata> {
        load_publish_request_metadata(
            &self.db,
            &request.id,
            Some(request.default_locale.as_str()),
            Some(request.default_locale.as_str()),
        )
        .await
    }

    async fn upsert_publish_request_translation(
        &self,
        request_id: &str,
        locale: &str,
        name: &str,
        description: &str,
    ) -> anyhow::Result<()> {
        upsert_publish_request_translation_record(&self.db, request_id, locale, name, description)
            .await
    }

    async fn replace_release_translations_from_request(
        &self,
        release_id: &str,
        request: &registry_publish_request::Model,
    ) -> anyhow::Result<()> {
        let translations = load_publish_request_translation_rows(&self.db, &request.id).await?;
        if translations.is_empty() {
            let metadata = self.publish_request_default_metadata(request).await?;
            upsert_release_translation_record(
                &self.db,
                release_id,
                &metadata.locale,
                &metadata.name,
                &metadata.description,
            )
            .await?;
            return Ok(());
        }

        RegistryModuleReleaseTranslationEntity::delete_many()
            .filter(registry_module_release_translation::Column::ReleaseId.eq(release_id))
            .exec(&self.db)
            .await?;

        for translation in translations {
            upsert_release_translation_record(
                &self.db,
                release_id,
                &translation.locale,
                &translation.name,
                &translation.description,
            )
            .await?;
        }

        Ok(())
    }

    pub async fn upload_publish_artifact(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
        artifact: RegistryArtifactUpload,
    ) -> anyhow::Result<registry_publish_request::Model> {
        let request = self.get_publish_request(request_id).await?.ok_or_else(|| {
            not_found_error(format!(
                "Registry publish request '{request_id}' was not found"
            ))
        })?;
        self.ensure_authority_can_manage_publish_request(
            authority,
            &request,
            "upload an artifact for",
        )
        .await?;
        let reupload_after_changes_requested =
            request.status == RegistryPublishRequestStatus::ChangesRequested;
        if request.status != RegistryPublishRequestStatus::Draft
            && !reupload_after_changes_requested
        {
            return Err(conflict_error(format!(
                "Registry publish request '{}' is in status '{}' and can no longer accept an artifact upload",
                request_id,
                request_status_label(request.status.clone())
            )));
        }

        let checksum = hex::encode(Sha256::digest(&artifact.bytes));
        let previous_storage_key = request.artifact_storage_key.clone();
        let stored = self
            .store_registry_artifact(&request, &artifact)
            .await
            .context("failed to persist registry artifact")?;
        if let Some(previous_storage_key) =
            previous_storage_key.filter(|value| value != &stored.artifact_storage_key)
        {
            self.require_storage()?
                .delete(&previous_storage_key)
                .await
                .with_context(|| {
                    format!(
                        "failed to delete previous registry artifact '{}'",
                        previous_storage_key
                    )
                })?;
        }
        let artifact_uploaded_at = Utc::now();

        let mut request_active: RegistryPublishRequestActiveModel = request.clone().into();
        request_active.status = Set(RegistryPublishRequestStatus::ArtifactUploaded);
        request_active.artifact_storage_key = Set(Some(stored.artifact_storage_key.clone()));
        request_active.artifact_checksum_sha256 = Set(Some(checksum.clone()));
        request_active.artifact_size = Set(Some(stored.artifact_size));
        request_active.artifact_content_type = Set(Some(artifact.content_type.clone()));
        request_active.updated_at = Set(artifact_uploaded_at);
        let request = request_active.update(&self.db).await?;
        if reupload_after_changes_requested {
            RegistryValidationStageEntity::delete_many()
                .filter(registry_validation_stage::Column::RequestId.eq(request.id.clone()))
                .exec(&self.db)
                .await?;
            RegistryValidationJobEntity::delete_many()
                .filter(registry_validation_job::Column::RequestId.eq(request.id.clone()))
                .exec(&self.db)
                .await?;
        }

        let mut warnings = if reupload_after_changes_requested {
            Vec::new()
        } else {
            deserialize_message_list(&request.validation_warnings)
        };
        let upload_actor = authority_actor(authority).to_string();
        let requested_by = principal_display_label(&request.requested_by);
        if upload_actor != requested_by {
            warnings.push(format!(
                "Artifact was uploaded by '{}' for publish request originally created by '{}'.",
                upload_actor, requested_by
            ));
        }
        let warnings = dedupe_message_list(warnings);

        let submitted_at = Utc::now();
        let mut request_active: RegistryPublishRequestActiveModel = request.into();
        request_active.status = Set(RegistryPublishRequestStatus::Submitted);
        request_active.submitted_at = Set(Some(submitted_at));
        request_active.validation_warnings = Set(serde_json::to_value(&warnings)?);
        request_active.validation_errors = Set(serde_json::json!([]));
        request_active.approved_by = Set(None);
        request_active.rejected_by = Set(None);
        request_active.rejection_reason = Set(None);
        request_active.validated_at = Set(None);
        request_active.approved_at = Set(None);
        request_active.published_at = Set(None);
        request_active.updated_at = Set(submitted_at);
        let request = request_active
            .update(&self.db)
            .await
            .map_err(anyhow::Error::from)?;
        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            "artifact_uploaded",
            authority_actor(authority),
            None,
            serde_json::json!({
                "version": request.version.clone(),
                "status": request_status_label(request.status.clone()),
                "artifact_size": request.artifact_size,
                "content_type": request.artifact_content_type.clone(),
                "checksum_sha256": request.artifact_checksum_sha256.clone(),
            }),
        )
        .await?;
        if reupload_after_changes_requested {
            let publisher = optional_principal_display_label(&request.publisher_principal);
            self.record_governance_event(
                &request.slug,
                Some(&request.id),
                None,
                "artifact_reuploaded_after_changes_requested",
                authority_actor(authority),
                publisher.as_deref(),
                serde_json::json!({
                    "version": request.version.clone(),
                    "status": request_status_label(request.status.clone()),
                    "artifact_size": request.artifact_size,
                    "content_type": request.artifact_content_type.clone(),
                    "checksum_sha256": request.artifact_checksum_sha256.clone(),
                }),
            )
            .await?;
        }
        Ok(request)
    }

    pub async fn approve_publish_request(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
        reason: Option<&str>,
        reason_code: Option<&str>,
    ) -> anyhow::Result<registry_publish_request::Model> {
        let request = self.get_publish_request(request_id).await?.ok_or_else(|| {
            not_found_error(format!(
                "Registry publish request '{request_id}' was not found"
            ))
        })?;
        self.ensure_authority_can_review_publish_request(authority, &request, "approve")
            .await?;
        if request.status != RegistryPublishRequestStatus::Approved {
            return Err(conflict_error(format!(
                "Registry publish request '{}' is in status '{}' and cannot be approved",
                request_id,
                request_status_label(request.status.clone())
            )));
        }

        let stored = StoredRegistryArtifact {
            artifact_storage_key: request.artifact_storage_key.clone().ok_or_else(|| {
                anyhow!("Registry publish request '{request_id}' is missing artifact_storage_key")
            })?,
            artifact_size: request.artifact_size.ok_or_else(|| {
                anyhow!("Registry publish request '{request_id}' is missing artifact_size")
            })?,
        };
        let checksum = request.artifact_checksum_sha256.clone().ok_or_else(|| {
            anyhow!("Registry publish request '{request_id}' is missing artifact_checksum_sha256")
        })?;
        let latest_validation_stages = self
            .latest_validation_stages_for_request(&request.id)
            .await?;
        let override_stages = latest_validation_stages
            .iter()
            .filter(|stage| stage.status != RegistryValidationStageStatus::Passed)
            .cloned()
            .collect::<Vec<_>>();
        let effective_publisher = self
            .resolve_effective_publisher(&request, authority)
            .await?;
        if !override_stages.is_empty() {
            let reason = reason
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    malformed_error(format!(
                        "Registry publish request '{}' still has non-passed follow-up validation stages; approval override requires a non-empty reason",
                        request_id
                    ))
                })?;
            let reason_code = reason_code
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    malformed_error(format!(
                        "Registry publish request '{}' still has non-passed follow-up validation stages; approval override requires a non-empty reason_code",
                        request_id
                    ))
                })?;
            if !REGISTRY_APPROVE_OVERRIDE_REASON_CODES
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(reason_code))
            {
                return Err(malformed_error(format!(
                    "Registry publish approval override reason_code '{}' is not supported; expected one of {}",
                    reason_code,
                    REGISTRY_APPROVE_OVERRIDE_REASON_CODES.join(", ")
                )));
            }
            self.record_governance_event(
                &request.slug,
                Some(&request.id),
                None,
                "publish_approval_override",
                authority_actor(authority),
                Some(&effective_publisher),
                serde_json::json!({
                    "version": request.version.clone(),
                    "reason": reason,
                    "reason_code": reason_code.to_ascii_lowercase(),
                    "validation_stages": override_stages
                        .iter()
                        .map(validation_stage_details_value)
                        .collect::<Vec<_>>(),
                }),
            )
            .await?;
        }
        let published_at = Utc::now();
        let release = self
            .upsert_release_from_request(
                request_id,
                authority_actor(authority),
                &effective_publisher,
                checksum,
                stored,
                published_at,
                &request,
            )
            .await?;
        self.bind_registry_slug_owner(
            &request.slug,
            &RegistryPrincipalRef::from_legacy_value(&effective_publisher),
            authority,
        )
        .await?;

        let mut request_active: RegistryPublishRequestActiveModel = request.into();
        request_active.status = Set(RegistryPublishRequestStatus::Published);
        request_active.approved_by = Set(Some(authority.principal.to_json_value()));
        request_active.approved_at = Set(Some(published_at));
        request_active.published_at = Set(Some(published_at));
        request_active.updated_at = Set(published_at);
        let request = request_active
            .update(&self.db)
            .await
            .map_err(anyhow::Error::from)?;
        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            Some(&release.id),
            "release_published",
            authority_actor(authority),
            Some(&effective_publisher),
            serde_json::json!({
                "version": request.version.clone(),
                "status": request_status_label(request.status.clone()),
                "publisher": effective_publisher.clone(),
                "checksum_sha256": release.checksum_sha256.clone(),
                "release_status": release_status_label(release.status.clone()),
            }),
        )
        .await?;
        Ok(request)
    }

    pub async fn reject_publish_request(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
        reason: &str,
        reason_code: &str,
    ) -> anyhow::Result<registry_publish_request::Model> {
        let request = self.get_publish_request(request_id).await?.ok_or_else(|| {
            not_found_error(format!(
                "Registry publish request '{request_id}' was not found"
            ))
        })?;
        self.ensure_authority_can_review_publish_request(authority, &request, "reject")
            .await?;
        if matches!(
            request.status,
            RegistryPublishRequestStatus::Published
                | RegistryPublishRequestStatus::Rejected
                | RegistryPublishRequestStatus::OnHold
        ) {
            return Err(conflict_error(format!(
                "Registry publish request '{}' is in status '{}' and cannot be rejected",
                request_id,
                request_status_label(request.status.clone())
            )));
        }
        let normalized_reason = normalize_required_reason(reason, "Registry publish reject")?;
        let normalized_reason_code = normalize_reason_code(
            reason_code,
            REGISTRY_REJECT_REASON_CODES,
            "Registry publish reject",
        )?;

        let rejected_at = Utc::now();
        let mut errors = deserialize_message_list(&request.validation_errors);
        errors.push(format!(
            "Governance rejection reason: {}",
            normalized_reason
        ));
        let mut request_active: RegistryPublishRequestActiveModel = request.into();
        request_active.status = Set(RegistryPublishRequestStatus::Rejected);
        request_active.rejected_by = Set(Some(authority.principal.to_json_value()));
        request_active.rejection_reason = Set(Some(normalized_reason.clone()));
        request_active.validation_errors = Set(serde_json::to_value(dedupe_message_list(errors))?);
        request_active.updated_at = Set(rejected_at);
        let request = request_active
            .update(&self.db)
            .await
            .map_err(anyhow::Error::from)?;
        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            "request_rejected",
            authority_actor(authority),
            None,
            serde_json::json!({
                "version": request.version.clone(),
                "status": request_status_label(request.status.clone()),
                "reason": request.rejection_reason.clone(),
                "reason_code": normalized_reason_code,
                "errors": deserialize_message_list(&request.validation_errors),
            }),
        )
        .await?;
        Ok(request)
    }

    pub async fn request_changes_publish_request(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
        reason: &str,
        reason_code: &str,
    ) -> anyhow::Result<registry_publish_request::Model> {
        let request = self.get_publish_request(request_id).await?.ok_or_else(|| {
            not_found_error(format!(
                "Registry publish request '{request_id}' was not found"
            ))
        })?;
        self.ensure_authority_can_review_publish_request(
            authority,
            &request,
            "request changes for",
        )
        .await?;
        if request.status != RegistryPublishRequestStatus::Approved {
            return Err(conflict_error(format!(
                "Registry publish request '{}' is in status '{}' and cannot move to changes_requested",
                request_id,
                request_status_label(request.status.clone())
            )));
        }
        let normalized_reason =
            normalize_required_reason(reason, "Registry publish request-changes")?;
        let normalized_reason_code = normalize_reason_code(
            reason_code,
            REGISTRY_REQUEST_CHANGES_REASON_CODES,
            "Registry publish request-changes",
        )?;
        let requested_at = Utc::now();
        let mut request_active: RegistryPublishRequestActiveModel = request.into();
        request_active.status = Set(RegistryPublishRequestStatus::ChangesRequested);
        request_active.changes_requested_by = Set(Some(authority.principal.to_json_value()));
        request_active.changes_requested_reason = Set(Some(normalized_reason.clone()));
        request_active.changes_requested_reason_code = Set(Some(normalized_reason_code.clone()));
        request_active.changes_requested_at = Set(Some(requested_at));
        request_active.updated_at = Set(requested_at);
        let request = request_active.update(&self.db).await?;
        let publisher = optional_principal_display_label(&request.publisher_principal);
        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            "changes_requested",
            authority_actor(authority),
            publisher.as_deref(),
            serde_json::json!({
                "version": request.version.clone(),
                "status": request_status_label(request.status.clone()),
                "reason": normalized_reason,
                "reason_code": normalized_reason_code,
            }),
        )
        .await?;
        Ok(request)
    }

    pub async fn hold_publish_request(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
        reason: &str,
        reason_code: &str,
    ) -> anyhow::Result<registry_publish_request::Model> {
        let request = self.get_publish_request(request_id).await?.ok_or_else(|| {
            not_found_error(format!(
                "Registry publish request '{request_id}' was not found"
            ))
        })?;
        self.ensure_authority_can_review_publish_request(authority, &request, "hold")
            .await?;
        if !matches!(
            request.status,
            RegistryPublishRequestStatus::Submitted
                | RegistryPublishRequestStatus::Approved
                | RegistryPublishRequestStatus::ChangesRequested
        ) {
            return Err(conflict_error(format!(
                "Registry publish request '{}' is in status '{}' and cannot be placed on hold",
                request_id,
                request_status_label(request.status.clone())
            )));
        }
        let normalized_reason = normalize_required_reason(reason, "Registry publish hold")?;
        let normalized_reason_code = normalize_reason_code(
            reason_code,
            REGISTRY_HOLD_REASON_CODES,
            "Registry publish hold",
        )?;
        let held_at = Utc::now();
        let previous_status = request.status.clone();
        let mut request_active: RegistryPublishRequestActiveModel = request.into();
        request_active.status = Set(RegistryPublishRequestStatus::OnHold);
        request_active.held_by = Set(Some(authority.principal.to_json_value()));
        request_active.held_reason = Set(Some(normalized_reason.clone()));
        request_active.held_reason_code = Set(Some(normalized_reason_code.clone()));
        request_active.held_at = Set(Some(held_at));
        request_active.held_from_status = Set(Some(
            request_status_label(previous_status.clone()).to_string(),
        ));
        request_active.updated_at = Set(held_at);
        let request = request_active.update(&self.db).await?;
        let publisher = optional_principal_display_label(&request.publisher_principal);
        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            "request_held",
            authority_actor(authority),
            publisher.as_deref(),
            serde_json::json!({
                "version": request.version.clone(),
                "status": request_status_label(request.status.clone()),
                "held_from_status": request.held_from_status.clone(),
                "reason": normalized_reason,
                "reason_code": normalized_reason_code,
            }),
        )
        .await?;
        Ok(request)
    }

    pub async fn resume_publish_request(
        &self,
        request_id: &str,
        authority: &RegistryAuthority,
        reason: &str,
        reason_code: &str,
    ) -> anyhow::Result<registry_publish_request::Model> {
        let request = self.get_publish_request(request_id).await?.ok_or_else(|| {
            not_found_error(format!(
                "Registry publish request '{request_id}' was not found"
            ))
        })?;
        self.ensure_authority_can_review_publish_request(authority, &request, "resume")
            .await?;
        if request.status != RegistryPublishRequestStatus::OnHold {
            return Err(conflict_error(format!(
                "Registry publish request '{}' is in status '{}' and cannot be resumed",
                request_id,
                request_status_label(request.status.clone())
            )));
        }
        let resumed_status = request
            .held_from_status
            .as_deref()
            .and_then(parse_request_status_label)
            .ok_or_else(|| {
                conflict_error(format!(
                    "Registry publish request '{}' is on hold without a valid held_from_status",
                    request_id
                ))
            })?;
        let normalized_reason = normalize_required_reason(reason, "Registry publish resume")?;
        let normalized_reason_code = normalize_reason_code(
            reason_code,
            REGISTRY_RESUME_REASON_CODES,
            "Registry publish resume",
        )?;
        let resumed_at = Utc::now();
        let mut request_active: RegistryPublishRequestActiveModel = request.into();
        request_active.status = Set(resumed_status.clone());
        request_active.updated_at = Set(resumed_at);
        let request = request_active.update(&self.db).await?;
        let publisher = optional_principal_display_label(&request.publisher_principal);
        self.record_governance_event(
            &request.slug,
            Some(&request.id),
            None,
            "request_resumed",
            authority_actor(authority),
            publisher.as_deref(),
            serde_json::json!({
                "version": request.version.clone(),
                "status": request_status_label(request.status.clone()),
                "resumed_from_hold": true,
                "resumed_to_status": request_status_label(resumed_status),
                "reason": normalized_reason,
                "reason_code": normalized_reason_code,
            }),
        )
        .await?;
        Ok(request)
    }

pub fn request_status_label(status: RegistryPublishRequestStatus) -> &'static str {
    match status {
        RegistryPublishRequestStatus::Draft => "draft",
        RegistryPublishRequestStatus::ArtifactUploaded => "artifact_uploaded",
        RegistryPublishRequestStatus::Submitted => "submitted",
        RegistryPublishRequestStatus::Validating => "validating",
        RegistryPublishRequestStatus::Approved => "approved",
        RegistryPublishRequestStatus::ChangesRequested => "changes_requested",
        RegistryPublishRequestStatus::OnHold => "on_hold",
        RegistryPublishRequestStatus::Rejected => "rejected",
        RegistryPublishRequestStatus::Published => "published",
    }
}

fn parse_request_status_label(value: &str) -> Option<RegistryPublishRequestStatus> {
    match value.trim().to_ascii_lowercase().as_str() {
        "draft" => Some(RegistryPublishRequestStatus::Draft),
        "artifact_uploaded" => Some(RegistryPublishRequestStatus::ArtifactUploaded),
        "submitted" => Some(RegistryPublishRequestStatus::Submitted),
        "validating" => Some(RegistryPublishRequestStatus::Validating),
        "approved" => Some(RegistryPublishRequestStatus::Approved),
        "changes_requested" => Some(RegistryPublishRequestStatus::ChangesRequested),
        "on_hold" => Some(RegistryPublishRequestStatus::OnHold),
        "rejected" => Some(RegistryPublishRequestStatus::Rejected),
        "published" => Some(RegistryPublishRequestStatus::Published),
        _ => None,
    }
}

fn lifecycle_governance_actions(
    latest_request: Option<&registry_publish_request::Model>,
    latest_release: Option<&registry_module_release::Model>,
    owner_binding: Option<&registry_module_owner::Model>,
    validation_stages: &[RegistryValidationStageSnapshot],
) -> Vec<RegistryGovernanceActionSnapshot> {
    let mut actions = latest_request
        .map(|request| {
            let approval_override_required = request.status
                == RegistryPublishRequestStatus::Approved
                && validation_stages
                    .iter()
                    .any(|stage| !stage.status.eq_ignore_ascii_case("passed"));
            publish_request_governance_actions(
                request,
                validation_stages,
                approval_override_required,
            )
        })
        .unwrap_or_default();

    if latest_request.is_some_and(|request| {
        request
            .publisher_principal
            .as_ref()
            .is_some_and(|publisher| {
                owner_binding.is_none_or(|owner| owner.owner_principal != *publisher)
            })
    }) || owner_binding.is_some()
    {
        actions.push(governance_action_snapshot(
            "owner_transfer",
            true,
            true,
            REGISTRY_OWNER_TRANSFER_REASON_CODES,
            true,
        ));
    }

    if latest_release.is_some_and(|release| release.status == RegistryModuleReleaseStatus::Active) {
        actions.push(governance_action_snapshot(
            "yank",
            true,
            true,
            REGISTRY_YANK_REASON_CODES,
            true,
        ));
    }

    dedupe_governance_actions(actions)
}

fn publish_request_governance_actions(
    request: &registry_publish_request::Model,
    validation_stages: &[RegistryValidationStageSnapshot],
    approval_override_required: bool,
) -> Vec<RegistryGovernanceActionSnapshot> {
    publish_request_governance_actions_for_authority(
        request,
        None,
        validation_stages,
        approval_override_required,
        &RegistryAuthority {
            principal: RegistryPrincipalRef::legacy(""),
            can_manage_modules: true,
        },
    )
}

fn publish_request_governance_actions_for_authority(
    request: &registry_publish_request::Model,
    owner_binding: Option<&registry_module_owner::Model>,
    _validation_stages: &[RegistryValidationStageSnapshot],
    approval_override_required: bool,
    authority: &RegistryAuthority,
) -> Vec<RegistryGovernanceActionSnapshot> {
    let mut actions = Vec::new();
    let can_manage = authority.can_manage_modules
        || authority_can_manage_publish_request(authority, request, owner_binding);
    let can_review = authority.can_manage_modules
        || authority_can_review_publish_request(authority, owner_binding);

    if can_manage
        && matches!(
            request.status,
            RegistryPublishRequestStatus::ArtifactUploaded
                | RegistryPublishRequestStatus::Submitted
        )
    {
        actions.push(governance_action_snapshot(
            "validate",
            false,
            false,
            &[],
            false,
        ));
    }

    if can_review && request.status == RegistryPublishRequestStatus::Approved {
        actions.push(governance_action_snapshot(
            "approve",
            approval_override_required,
            approval_override_required,
            if approval_override_required {
                REGISTRY_APPROVE_OVERRIDE_REASON_CODES
            } else {
                &[]
            },
            false,
        ));
        actions.push(governance_action_snapshot(
            "request_changes",
            true,
            true,
            REGISTRY_REQUEST_CHANGES_REASON_CODES,
            false,
        ));
    }

    if can_review
        && matches!(
            request.status,
            RegistryPublishRequestStatus::Submitted
                | RegistryPublishRequestStatus::Approved
                | RegistryPublishRequestStatus::ChangesRequested
        )
    {
        actions.push(governance_action_snapshot(
            "hold",
            true,
            true,
            REGISTRY_HOLD_REASON_CODES,
            false,
        ));
    }

    if can_review && request.status == RegistryPublishRequestStatus::OnHold {
        actions.push(governance_action_snapshot(
            "resume",
            true,
            true,
            REGISTRY_RESUME_REASON_CODES,
            false,
        ));
    }

    if can_review
        && !matches!(
            request.status,
            RegistryPublishRequestStatus::Rejected
                | RegistryPublishRequestStatus::Published
                | RegistryPublishRequestStatus::OnHold
        )
    {
        actions.push(governance_action_snapshot(
            "reject",
            true,
            true,
            REGISTRY_REJECT_REASON_CODES,
            true,
        ));
    }

    dedupe_governance_actions(actions)
}

fn governance_action_snapshot(
    key: &str,
    reason_required: bool,
    reason_code_required: bool,
    reason_codes: &[&str],
    destructive: bool,
) -> RegistryGovernanceActionSnapshot {
    RegistryGovernanceActionSnapshot {
        key: key.to_string(),
        reason_required,
        reason_code_required,
        reason_codes: reason_codes
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        destructive,
    }
}

fn dedupe_governance_actions(
    actions: Vec<RegistryGovernanceActionSnapshot>,
) -> Vec<RegistryGovernanceActionSnapshot> {
    let mut seen = std::collections::HashSet::new();

    actions
        .into_iter()
        .filter(|action| seen.insert(action.key.clone()))
        .collect()
}

fn normalize_registry_locale(locale: &str) -> String {
    normalize_locale_tag(locale).unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string())
}

}
