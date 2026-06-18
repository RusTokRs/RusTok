use super::*;

impl RegistryGovernanceService {
    pub async fn yank_release(
        &self,
        slug: &str,
        version: &str,
        reason: &str,
        reason_code: &str,
        authority: &RegistryAuthority,
    ) -> anyhow::Result<registry_module_release::Model> {
        let release = RegistryModuleReleaseEntity::find()
            .filter(registry_module_release::Column::Slug.eq(slug))
            .filter(registry_module_release::Column::Version.eq(version))
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                not_found_error(format!(
                    "Published release '{slug}@{version}' was not found"
                ))
            })?;
        self.ensure_authority_can_manage_release(authority, &release, "yank")
            .await?;
        let normalized_reason = normalize_required_reason(reason, "Registry yank")?;
        let normalized_reason_code =
            normalize_reason_code(reason_code, REGISTRY_YANK_REASON_CODES, "Registry yank")?;

        let mut active: RegistryModuleReleaseActiveModel = release.into();
        active.status = Set(RegistryModuleReleaseStatus::Yanked);
        active.yanked_reason = Set(Some(normalized_reason.clone()));
        active.yanked_by = Set(Some(authority.principal.to_json_value()));
        active.yanked_at = Set(Some(Utc::now()));
        active.updated_at = Set(Utc::now());
        let release = active.update(&self.db).await?;
        let publisher = principal_display_label(&release.publisher);
        self.record_governance_event(
            &release.slug,
            release.request_id.as_deref(),
            Some(&release.id),
            "release_yanked",
            authority_actor(authority),
            Some(&publisher),
            serde_json::json!({
                "version": release.version.clone(),
                "status": release_status_label(release.status.clone()),
                "reason_code": normalized_reason_code,
                "reason": release.yanked_reason.clone(),
            }),
        )
        .await?;
        Ok(release)
    }

    pub async fn transfer_registry_slug_owner(
        &self,
        slug: &str,
        new_owner: &RegistryPrincipalRef,
        reason: &str,
        reason_code: &str,
        authority: &RegistryAuthority,
    ) -> anyhow::Result<registry_module_owner::Model> {
        let existing = self.registry_slug_owner(slug).await?.ok_or_else(|| {
            not_found_error(format!(
                "Registry owner binding for slug '{slug}' was not found"
            ))
        })?;
        self.ensure_authority_can_transfer_registry_owner(
            authority,
            &existing,
            "transfer ownership",
        )
        .await?;

        if !new_owner.is_user() {
            return Err(malformed_error(format!(
                "Registry owner transfer for slug '{}' requires a valid new owner user principal",
                slug
            )));
        }
        if principal_matches_ref(&existing.owner_principal, new_owner) {
            return Err(conflict_error(format!(
                "Registry owner for slug '{}' is already bound to '{}'",
                slug,
                new_owner.label()
            )));
        }
        let normalized_reason = normalize_required_reason(reason, "Registry owner transfer")?;
        let normalized_reason_code = normalize_reason_code(
            reason_code,
            REGISTRY_OWNER_TRANSFER_REASON_CODES,
            "Registry owner transfer",
        )?;

        let previous_owner = principal_from_json(&existing.owner_principal);
        let now = Utc::now();
        let mut active: RegistryModuleOwnerActiveModel = existing.into();
        active.owner_principal = Set(new_owner.to_json_value());
        active.bound_by = Set(authority.principal.to_json_value());
        active.bound_at = Set(now);
        active.updated_at = Set(now);
        let binding = active.update(&self.db).await?;
        self.record_governance_event(
            slug,
            None,
            None,
            "owner_transferred",
            authority_actor(authority),
            Some(new_owner.label()),
            serde_json::json!({
                "owner_transition": {
                    "previous_owner": previous_owner.to_json_value(),
                    "new_owner": principal_from_json(&binding.owner_principal).to_json_value(),
                    "bound_by": principal_from_json(&binding.bound_by).to_json_value(),
                },
                "reason": normalized_reason,
                "reason_code": normalized_reason_code,
            }),
        )
        .await?;
        Ok(binding)
    }

    pub async fn apply_catalog_projection(
        &self,
        modules: Vec<CatalogManifestModule>,
        preferred_locale: Option<&str>,
        fallback_locale: Option<&str>,
    ) -> anyhow::Result<Vec<CatalogManifestModule>> {
        let releases = RegistryModuleReleaseEntity::find()
            .order_by_desc(registry_module_release::Column::PublishedAt)
            .all(&self.db)
            .await?;

        if releases.is_empty() {
            return Ok(modules);
        }

        let mut release_map: HashMap<String, Vec<registry_module_release::Model>> = HashMap::new();
        for release in releases {
            release_map
                .entry(release.slug.clone())
                .or_default()
                .push(release);
        }

        let mut projected = modules;
        for module in &mut projected {
            let Some(releases) = release_map.get(&module.slug) else {
                continue;
            };

            let mut versions = releases
                .iter()
                .map(|release| CatalogModuleVersion {
                    version: release.version.clone(),
                    changelog: None,
                    yanked: release.status == RegistryModuleReleaseStatus::Yanked,
                    published_at: Some(release.published_at.to_rfc3339()),
                    checksum_sha256: release.checksum_sha256.clone(),
                    signature: None,
                })
                .collect::<Vec<_>>();
            versions.sort_by(|left, right| {
                left.yanked
                    .cmp(&right.yanked)
                    .then_with(|| right.published_at.cmp(&left.published_at))
                    .then_with(|| compare_semver_desc(&left.version, &right.version))
                    .then_with(|| right.version.cmp(&left.version))
            });

            if let Some(latest_active) = releases
                .iter()
                .find(|release| release.status == RegistryModuleReleaseStatus::Active)
            {
                let metadata = load_release_metadata(
                    &self.db,
                    &latest_active.id,
                    preferred_locale,
                    fallback_locale.or(Some(latest_active.default_locale.as_str())),
                )
                .await?;
                module.version = Some(latest_active.version.clone());
                module.name = Some(metadata.name);
                module.description = Some(metadata.description);
                module.publisher = Some(principal_display_label(&latest_active.publisher));
                module.checksum_sha256 = latest_active.checksum_sha256.clone();
            }
            module.versions = versions;
        }

        Ok(projected)
    }

    pub async fn lifecycle_snapshot(
        &self,
        slug: &str,
    ) -> anyhow::Result<Option<RegistryModuleLifecycleSnapshot>> {
        let owner_binding = RegistryModuleOwnerEntity::find_by_id(slug)
            .one(&self.db)
            .await?;
        let latest_request = RegistryPublishRequestEntity::find()
            .filter(registry_publish_request::Column::Slug.eq(slug))
            .order_by_desc(registry_publish_request::Column::CreatedAt)
            .one(&self.db)
            .await?;
        let latest_release = RegistryModuleReleaseEntity::find()
            .filter(registry_module_release::Column::Slug.eq(slug))
            .order_by_desc(registry_module_release::Column::PublishedAt)
            .one(&self.db)
            .await?;
        let recent_events = RegistryGovernanceEventEntity::find()
            .filter(registry_governance_event::Column::Slug.eq(slug))
            .order_by_desc(registry_governance_event::Column::CreatedAt)
            .limit(10)
            .all(&self.db)
            .await?;
        let validation_stage_rows = if let Some(request) = latest_request.as_ref() {
            self.validation_stage_rows(&request.id).await?
        } else {
            Vec::new()
        };

        if owner_binding.is_none()
            && latest_request.is_none()
            && latest_release.is_none()
            && recent_events.is_empty()
            && validation_stage_rows.is_empty()
        {
            return Ok(None);
        }

        let validation_stages = derive_validation_stage_snapshots(
            latest_request.as_ref(),
            &recent_events,
            &validation_stage_rows,
        );
        let follow_up_gates = derive_follow_up_gate_snapshots(
            latest_request.as_ref(),
            &recent_events,
            &validation_stages,
        );

        let governance_actions = lifecycle_governance_actions(
            latest_request.as_ref(),
            latest_release.as_ref(),
            owner_binding.as_ref(),
            &validation_stages,
        );

        Ok(Some(RegistryModuleLifecycleSnapshot {
            owner_binding: owner_binding
                .as_ref()
                .map(|binding| RegistryModuleOwnerSnapshot {
                    owner: principal_from_json(&binding.owner_principal),
                    bound_by: principal_from_json(&binding.bound_by),
                    bound_at: binding.bound_at.to_rfc3339(),
                    updated_at: binding.updated_at.to_rfc3339(),
                }),
            latest_request: latest_request
                .as_ref()
                .map(|request| RegistryPublishRequestSnapshot {
                    id: request.id.clone(),
                    status: request_status_label(request.status.clone()).to_string(),
                    requested_by: principal_from_json(&request.requested_by),
                    publisher: optional_principal_from_json(&request.publisher_principal),
                    approved_by: optional_principal_from_json(&request.approved_by),
                    rejected_by: optional_principal_from_json(&request.rejected_by),
                    rejection_reason: request.rejection_reason.clone(),
                    changes_requested_by: optional_principal_from_json(
                        &request.changes_requested_by,
                    ),
                    changes_requested_reason: request.changes_requested_reason.clone(),
                    changes_requested_reason_code: request.changes_requested_reason_code.clone(),
                    changes_requested_at: request
                        .changes_requested_at
                        .map(|value| value.to_rfc3339()),
                    held_by: optional_principal_from_json(&request.held_by),
                    held_reason: request.held_reason.clone(),
                    held_reason_code: request.held_reason_code.clone(),
                    held_at: request.held_at.map(|value| value.to_rfc3339()),
                    held_from_status: request.held_from_status.clone(),
                    warnings: deserialize_message_list(&request.validation_warnings),
                    errors: deserialize_message_list(&request.validation_errors),
                    created_at: request.created_at.to_rfc3339(),
                    updated_at: request.updated_at.to_rfc3339(),
                    published_at: request.published_at.map(|value| value.to_rfc3339()),
                }),
            latest_release: latest_release
                .as_ref()
                .map(|release| RegistryModuleReleaseSnapshot {
                    version: release.version.clone(),
                    status: release_status_label(release.status.clone()).to_string(),
                    publisher: principal_from_json(&release.publisher),
                    checksum_sha256: release.checksum_sha256.clone(),
                    published_at: release.published_at.to_rfc3339(),
                    yanked_reason: release.yanked_reason.clone(),
                    yanked_by: optional_principal_from_json(&release.yanked_by),
                    yanked_at: release.yanked_at.map(|value| value.to_rfc3339()),
                }),
            recent_events: recent_events
                .into_iter()
                .map(|event| RegistryGovernanceEventSnapshot {
                    id: event.id,
                    event_type: event.event_type,
                    actor: principal_from_json(&event.actor),
                    publisher: optional_principal_from_json(&event.publisher),
                    payload: governance_event_payload(&event.details),
                    created_at: event.created_at.to_rfc3339(),
                })
                .collect(),
            follow_up_gates,
            governance_actions,
            validation_stages,
        }))
    }

    pub async fn publish_request_follow_up_snapshot(
        &self,
        request: &registry_publish_request::Model,
    ) -> anyhow::Result<RegistryPublishRequestFollowUpSnapshot> {
        self.publish_request_follow_up_snapshot_for_authority(request, None)
            .await
    }

    pub async fn publish_request_follow_up_snapshot_for_authority(
        &self,
        request: &registry_publish_request::Model,
        authority: Option<&RegistryAuthority>,
    ) -> anyhow::Result<RegistryPublishRequestFollowUpSnapshot> {
        let validation_stage_rows = self.validation_stage_rows(&request.id).await?;
        let validation_stages =
            derive_validation_stage_snapshots(Some(request), &[], &validation_stage_rows);
        let follow_up_gates =
            derive_follow_up_gate_snapshots(Some(request), &[], &validation_stages);
        let approval_override_required = request.status == RegistryPublishRequestStatus::Approved
            && validation_stages
                .iter()
                .any(|stage| !stage.status.eq_ignore_ascii_case("passed"));
        let governance_actions = if let Some(authority) = authority {
            let owner = self.registry_slug_owner(&request.slug).await?;
            publish_request_governance_actions_for_authority(
                request,
                owner.as_ref(),
                &validation_stages,
                approval_override_required,
                authority,
            )
        } else {
            publish_request_governance_actions(
                request,
                &validation_stages,
                approval_override_required,
            )
        };

        Ok(RegistryPublishRequestFollowUpSnapshot {
            follow_up_gates,
            validation_stages,
            approval_override_required,
            governance_actions,
        })
    }

    async fn upsert_release_from_request(
        &self,
        request_id: &str,
        _actor: &str,
        publisher: &str,
        checksum: String,
        stored: StoredRegistryArtifact,
        published_at: chrono::DateTime<Utc>,
        request: &registry_publish_request::Model,
    ) -> anyhow::Result<registry_module_release::Model> {
        let existing = RegistryModuleReleaseEntity::find()
            .filter(registry_module_release::Column::Slug.eq(&request.slug))
            .filter(registry_module_release::Column::Version.eq(&request.version))
            .one(&self.db)
            .await?;

        let marketplace = request.marketplace.clone();
        let ui_packages = request.ui_packages.clone();
        let publisher = normalize_actor(publisher);
        let publisher_principal = actor_principal(&publisher).to_json_value();

        if let Some(existing) = existing {
            let mut active: RegistryModuleReleaseActiveModel = existing.into();
            active.request_id = Set(Some(request_id.to_string()));
            active.crate_name = Set(request.crate_name.clone());
            active.default_locale = Set(request.default_locale.clone());
            active.ownership = Set(request.ownership.clone());
            active.trust_level = Set(request.trust_level.clone());
            active.license = Set(request.license.clone());
            active.entry_type = Set(request.entry_type.clone());
            active.marketplace = Set(marketplace);
            active.ui_packages = Set(ui_packages);
            active.status = Set(RegistryModuleReleaseStatus::Active);
            active.publisher = Set(publisher_principal);
            active.artifact_storage_key = Set(Some(stored.artifact_storage_key));
            active.checksum_sha256 = Set(Some(checksum));
            active.artifact_size = Set(Some(stored.artifact_size));
            active.yanked_reason = Set(None);
            active.yanked_by = Set(None);
            active.yanked_at = Set(None);
            active.published_at = Set(published_at);
            active.updated_at = Set(Utc::now());
            let release = active.update(&self.db).await?;
            self.replace_release_translations_from_request(&release.id, request)
                .await?;
            return Ok(release);
        }

        let id = format!("rrel_{}", uuid::Uuid::new_v4().simple());
        let active = RegistryModuleReleaseActiveModel {
            id: Set(id),
            request_id: Set(Some(request_id.to_string())),
            slug: Set(request.slug.clone()),
            version: Set(request.version.clone()),
            crate_name: Set(request.crate_name.clone()),
            default_locale: Set(request.default_locale.clone()),
            ownership: Set(request.ownership.clone()),
            trust_level: Set(request.trust_level.clone()),
            license: Set(request.license.clone()),
            entry_type: Set(request.entry_type.clone()),
            marketplace: Set(marketplace),
            ui_packages: Set(ui_packages),
            status: Set(RegistryModuleReleaseStatus::Active),
            publisher: Set(publisher_principal),
            artifact_storage_key: Set(Some(stored.artifact_storage_key)),
            checksum_sha256: Set(Some(checksum)),
            artifact_size: Set(Some(stored.artifact_size)),
            yanked_reason: Set(None),
            yanked_by: Set(None),
            yanked_at: Set(None),
            published_at: Set(published_at),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
        };
        let release = active.insert(&self.db).await?;
        self.replace_release_translations_from_request(&release.id, request)
            .await?;
        Ok(release)
    }

    async fn ensure_authority_can_create_publish_request(
        &self,
        authority: &RegistryAuthority,
        slug: &str,
    ) -> anyhow::Result<()> {
        let owner = self.registry_slug_owner(slug).await?;
        if authority_can_create_publish_request(authority, owner.as_ref()) {
            return Ok(());
        }

        Err(forbidden_error(format!(
            "Principal '{}' is not allowed to create registry publish requests for slug '{}'",
            authority_actor(authority),
            slug
        )))
    }

    async fn ensure_authority_can_manage_publish_request(
        &self,
        authority: &RegistryAuthority,
        request: &registry_publish_request::Model,
        action: &str,
    ) -> anyhow::Result<()> {
        let owner = self.registry_slug_owner(&request.slug).await?;
        if authority_can_manage_publish_request(authority, request, owner.as_ref()) {
            return Ok(());
        }

        Err(forbidden_error(format!(
            "Principal '{}' is not allowed to {} registry publish request '{}' for slug '{}'; management actions require either MODULES_MANAGE, the current persisted owner binding, or (before owner binding exists) the original requester identity",
            authority_actor(authority),
            action,
            request.id,
            request.slug
        )))
    }

    async fn ensure_authority_can_review_publish_request(
        &self,
        authority: &RegistryAuthority,
        request: &registry_publish_request::Model,
        action: &str,
    ) -> anyhow::Result<()> {
        let owner = self.registry_slug_owner(&request.slug).await?;
        if authority_can_review_publish_request(authority, owner.as_ref()) {
            return Ok(());
        }

        Err(forbidden_error(format!(
            "Principal '{}' is not allowed to {} registry publish request '{}' for slug '{}'; review actions require either MODULES_MANAGE or the current persisted owner binding",
            authority_actor(authority),
            action,
            request.id,
            request.slug
        )))
    }

    async fn ensure_authority_can_manage_release(
        &self,
        authority: &RegistryAuthority,
        release: &registry_module_release::Model,
        action: &str,
    ) -> anyhow::Result<()> {
        let owner = self.registry_slug_owner(&release.slug).await?;
        if authority_can_manage_release(authority, release, owner.as_ref()) {
            return Ok(());
        }

        Err(forbidden_error(format!(
            "Principal '{}' is not allowed to {} published release '{}@{}'; yank/unpublish actions require either MODULES_MANAGE, the current persisted owner binding, or the published release principal",
            authority_actor(authority),
            action,
            release.slug,
            release.version
        )))
    }

    async fn ensure_authority_can_transfer_registry_owner(
        &self,
        authority: &RegistryAuthority,
        binding: &registry_module_owner::Model,
        action: &str,
    ) -> anyhow::Result<()> {
        if authority_can_transfer_registry_owner(authority, binding) {
            return Ok(());
        }

        Err(forbidden_error(format!(
            "Principal '{}' is not allowed to {} for slug '{}'; owner transfer requires either MODULES_MANAGE or the current persisted owner binding",
            authority_actor(authority),
            action,
            binding.slug
        )))
    }

    async fn resolve_effective_publisher(
        &self,
        request: &registry_publish_request::Model,
        authority: &RegistryAuthority,
    ) -> anyhow::Result<String> {
        if let Some(owner) = self.registry_slug_owner(&request.slug).await? {
            return Ok(principal_display_label(&owner.owner_principal));
        }

        if let Some(publisher) = optional_principal_display_label(&request.publisher_principal) {
            return Ok(publisher);
        }

        Ok(authority.principal.label().to_string())
    }

    async fn bind_registry_slug_owner(
        &self,
        slug: &str,
        owner_principal: &RegistryPrincipalRef,
        bound_by: &RegistryAuthority,
    ) -> anyhow::Result<registry_module_owner::Model> {
        let owner_principal_json = owner_principal.to_json_value();
        let bound_by_principal = bound_by.principal.to_json_value();
        let now = Utc::now();

        if let Some(existing) = RegistryModuleOwnerEntity::find_by_id(slug)
            .one(&self.db)
            .await?
        {
            if principal_matches_ref(&existing.owner_principal, owner_principal) {
                let mut active: RegistryModuleOwnerActiveModel = existing.into();
                active.bound_by = Set(bound_by_principal);
                active.updated_at = Set(now);
                return Ok(active.update(&self.db).await?);
            }

            if !bound_by.can_manage_modules {
                return Err(forbidden_error(format!(
                    "Principal '{}' is not allowed to rebind registry owner for slug '{}'",
                    authority_actor(bound_by),
                    slug
                )));
            }

            let mut active: RegistryModuleOwnerActiveModel = existing.into();
            active.owner_principal = Set(owner_principal_json);
            active.bound_by = Set(bound_by_principal);
            active.bound_at = Set(now);
            active.updated_at = Set(now);
            let binding = active.update(&self.db).await?;
            self.record_governance_event(
                slug,
                None,
                None,
                "owner_bound",
                authority_actor(bound_by),
                Some(owner_principal.label()),
                serde_json::json!({
                    "owner_transition": {
                        "new_owner": principal_from_json(&binding.owner_principal).to_json_value(),
                        "bound_by": principal_from_json(&binding.bound_by).to_json_value(),
                    },
                    "mode": "rebind",
                }),
            )
            .await?;
            return Ok(binding);
        }

        let active = RegistryModuleOwnerActiveModel {
            slug: Set(slug.to_string()),
            owner_principal: Set(owner_principal_json),
            bound_by: Set(bound_by_principal),
            bound_at: Set(now),
            updated_at: Set(now),
        };
        let binding = active.insert(&self.db).await?;
        self.record_governance_event(
            slug,
            None,
            None,
            "owner_bound",
            authority_actor(bound_by),
            Some(owner_principal.label()),
            serde_json::json!({
                "owner_transition": {
                    "new_owner": principal_from_json(&binding.owner_principal).to_json_value(),
                    "bound_by": principal_from_json(&binding.bound_by).to_json_value(),
                },
                "mode": "initial",
            }),
        )
        .await?;
        Ok(binding)
    }

    async fn registry_slug_owner(
        &self,
        slug: &str,
    ) -> anyhow::Result<Option<registry_module_owner::Model>> {
        Ok(RegistryModuleOwnerEntity::find_by_id(slug)
            .one(&self.db)
            .await?)
    }

pub fn release_status_label(status: RegistryModuleReleaseStatus) -> &'static str {
    match status {
        RegistryModuleReleaseStatus::Active => "active",
        RegistryModuleReleaseStatus::Yanked => "yanked",
    }
}

pub fn request_ui_packages(
    request: &registry_publish_request::Model,
) -> RegistryPublishUiPackagesRequest {
    serde_json::from_value(request.ui_packages.clone()).unwrap_or_default()
}

}
