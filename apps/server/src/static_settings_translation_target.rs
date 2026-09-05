//! Runtime Translation target provider for static module Settings.
//!
//! The host owns package discovery and provider registration. All Settings
//! persistence, source-locale provenance, exact reads, change evidence and
//! writes remain behind `rustok-modules` owner services. The neutral adapter
//! remains persistence-free and owns identity/revision/patch mapping.

use std::collections::BTreeSet;

use async_trait::async_trait;
use rustok_api::{Action, PortContext, PortError, Resource, TenantLocale};
use rustok_core::{PermissionScope, SecurityContext};
use rustok_modules::{
    ModuleCommandContext, StaticSettingsLocalizationError, StaticSettingsLocalizationRegistry,
    StaticSettingsLocalizationService, StaticSettingsSourceLocaleError,
    static_settings_translation_read::{
        StaticSettingsChangeReadRequest, StaticSettingsExactLocaleSnapshot,
        StaticSettingsTranslationReadError, StaticSettingsTranslationReadService,
    },
};
use rustok_modules_translation::{
    STATIC_SETTINGS_TRANSLATION_OWNER_SLUG, STATIC_SETTINGS_TRANSLATION_RESOURCE_KIND,
    StaticSettingsTranslationIdentity, StaticSettingsTranslationIdentityError,
    StaticSettingsTranslationPrepareResult,
};
use rustok_outbox::idempotency::{self, Admission};
use rustok_translation_targets::{
    FieldKey, ListTranslationResourcesRequest, OpaqueCursor, OpaqueRevision, OwnerSlug,
    ReadTranslationResourceRequest, ResourceKind, TranslationApplicationReceipt,
    TranslationFieldSnapshot, TranslationPatchRequest, TranslationPatchValidation,
    TranslationResourceLifecycle, TranslationResourcePage, TranslationResourceSnapshot,
    TranslationResourceSummary, TranslationTargetCapability, TranslationTargetChange,
    TranslationTargetChangePage, TranslationTargetChangesRequest, TranslationTargetProgressFacts,
    TranslationTargetProgressRequest, TranslationTargetProvider, TranslationTargetProviderDescriptor,
    provider_support::{
        contract_validation_error, decode_application_receipt, field_hash, validation_to_port_error,
    },
    validate_translation_apply_context, validate_translation_read_context,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::modules::manifest::ManifestManager;
use crate::static_settings_localization_registry::resolve_static_settings_localization_registry;

const PROVIDER_RECEIPT_OWNER: &str = "modules.static_settings_translation_target";
const OPERATION_APPLY_PATCH: &str = "apply_patch";
const RESOURCE_REVISION_PREFIX: &str = "settings-owner-v1";
const CHANGE_CURSOR_VERSION: &str = "v1";
const PROGRESS_STABILITY_ATTEMPTS: usize = 3;

#[derive(Clone)]
pub struct StaticSettingsTranslationTargetProvider {
    db: DatabaseConnection,
}

impl StaticSettingsTranslationTargetProvider {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    fn descriptor_value() -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new(STATIC_SETTINGS_TRANSLATION_OWNER_SLUG)
                .expect("static Settings Translation owner slug must be valid"),
            resource_kind: ResourceKind::new(STATIC_SETTINGS_TRANSLATION_RESOURCE_KIND)
                .expect("static Settings Translation resource kind must be valid"),
            display_name: "Static module settings".to_string(),
            capabilities: BTreeSet::from([
                TranslationTargetCapability::ListResources,
                TranslationTargetCapability::ReadExactResource,
                TranslationTargetCapability::AggregateProgress,
                TranslationTargetCapability::ValidatePatch,
                TranslationTargetCapability::ApplyPatch,
                TranslationTargetCapability::ChangeCursor,
            ]),
            read_permission_floor: BTreeSet::from(["settings:read".to_string()]),
            apply_permission_floor: BTreeSet::from(["settings:update".to_string()]),
        }
    }

    fn candidate_registries(&self) -> Result<Vec<StaticSettingsLocalizationRegistry>, PortError> {
        let manifest = ManifestManager::load().map_err(manifest_error_to_port_error)?;
        let modules =
            ManifestManager::catalog_modules(&manifest).map_err(manifest_error_to_port_error)?;
        let mut registries = Vec::new();
        for module in modules {
            let registry = resolve_static_settings_localization_registry(&module.slug)
                .map_err(manifest_error_to_port_error)?;
            if !registry.localized_fields().is_empty() {
                registries.push(registry);
            }
        }
        Ok(registries)
    }

    fn registry_for_identity(
        &self,
        identity: &rustok_translation_targets::TranslationResourceIdentity,
    ) -> Result<StaticSettingsLocalizationRegistry, PortError> {
        let module_slug = StaticSettingsTranslationIdentity::module_slug_from_identity(identity)
            .map_err(|_| {
                PortError::validation(
                    "settings.translation_identity_invalid",
                    "Settings translation identity must address modules/static_settings without a subresource",
                )
            })?;
        let registry = resolve_static_settings_localization_registry(module_slug)
            .map_err(manifest_error_to_port_error)?;
        if registry.localized_fields().is_empty() {
            return Err(PortError::not_found(
                "settings.translation_resource_not_found",
                "Static Settings translation resource was not found",
            ));
        }
        Ok(registry)
    }

    async fn owner_snapshot(
        &self,
        tenant_id: Uuid,
        registry: &StaticSettingsLocalizationRegistry,
        source_locale: &TenantLocale,
        target_locale: &TenantLocale,
    ) -> Result<StaticSettingsExactLocaleSnapshot, PortError> {
        let snapshot = StaticSettingsTranslationReadService::new(self.db.clone())
            .exact_locale_snapshot(tenant_id, registry, target_locale.as_str())
            .await
            .map_err(translation_read_error_to_port_error)?;
        if snapshot.source_locale != source_locale.as_str() {
            return Err(PortError::not_found(
                "settings.translation_source_not_found",
                "Requested Settings source locale is not the authoritative exact source locale",
            ));
        }
        if snapshot.fields.is_empty() {
            return Err(PortError::not_found(
                "settings.translation_resource_not_found",
                "Static Settings resource has no source-present localizable fields",
            ));
        }
        Ok(snapshot)
    }

    async fn neutral_snapshot(
        &self,
        tenant_id: Uuid,
        registry: &StaticSettingsLocalizationRegistry,
        source_locale: &TenantLocale,
        target_locale: &TenantLocale,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        let owner = self
            .owner_snapshot(tenant_id, registry, source_locale, target_locale)
            .await?;
        snapshot_from_owner(registry, &owner)
    }

    async fn module_highwater(
        &self,
        tenant_id: Uuid,
        registry: &StaticSettingsLocalizationRegistry,
    ) -> Result<Option<u64>, PortError> {
        StaticSettingsTranslationReadService::new(self.db.clone())
            .read_changes(
                registry,
                StaticSettingsChangeReadRequest {
                    tenant_id,
                    after_seq: None,
                    through_seq: None,
                    limit: 1,
                },
            )
            .await
            .map_err(translation_read_error_to_port_error)
            .map(|page| page.through_seq)
    }

    async fn global_highwater(
        &self,
        tenant_id: Uuid,
        registries: &[StaticSettingsLocalizationRegistry],
    ) -> Result<Option<u64>, PortError> {
        let mut highwater = None;
        for registry in registries {
            if let Some(candidate) = self.module_highwater(tenant_id, registry).await? {
                highwater = Some(highwater.map_or(candidate, |current: u64| current.max(candidate)));
            }
        }
        Ok(highwater)
    }

    async fn progress_once(
        &self,
        tenant_id: Uuid,
        registries: &[StaticSettingsLocalizationRegistry],
        request: &TranslationTargetProgressRequest,
    ) -> Result<TranslationTargetProgressFacts, PortError> {
        let mut required_units = 0_u64;
        let mut exact_required_units = 0_u64;
        let mut resources = 0_u64;
        let mut complete_resources = 0_u64;

        for registry in registries {
            let owner = match StaticSettingsTranslationReadService::new(self.db.clone())
                .exact_locale_snapshot(tenant_id, registry, request.target_locale.as_str())
                .await
            {
                Ok(snapshot) if snapshot.source_locale == request.source_locale.as_str() => snapshot,
                Ok(_) => continue,
                Err(error) if source_not_translation_ready(&error) => continue,
                Err(error) => return Err(translation_read_error_to_port_error(error)),
            };
            if owner.fields.is_empty() {
                continue;
            }
            let progress = owner.progress();
            required_units = required_units.checked_add(progress.source_units).ok_or_else(|| {
                PortError::invariant_violation(
                    "settings.translation_progress_overflow",
                    "Settings required progress count overflow",
                )
            })?;
            exact_required_units = exact_required_units
                .checked_add(progress.exact_units)
                .ok_or_else(|| {
                    PortError::invariant_violation(
                        "settings.translation_progress_overflow",
                        "Settings exact progress count overflow",
                    )
                })?;
            resources = resources.checked_add(1).ok_or_else(|| {
                PortError::invariant_violation(
                    "settings.translation_progress_overflow",
                    "Settings resource count overflow",
                )
            })?;
            if progress.complete {
                complete_resources = complete_resources.checked_add(1).ok_or_else(|| {
                    PortError::invariant_violation(
                        "settings.translation_progress_overflow",
                        "Settings complete resource count overflow",
                    )
                })?;
            }
        }

        Ok(TranslationTargetProgressFacts {
            required_units,
            exact_required_units,
            optional_units: 0,
            exact_optional_units: 0,
            resources,
            complete_resources,
            owner_change_cursor: None,
        })
    }

    async fn fail_receipt(&self, lease: idempotency::Lease, error: &PortError) {
        if let Err(receipt_error) = idempotency::fail(&self.db, lease, error).await {
            tracing::error!(
                operation_id = %lease.operation_id,
                error = %receipt_error.message,
                "Failed to persist Settings translation-target failure receipt"
            );
        }
    }
}

#[async_trait]
impl TranslationTargetProvider for StaticSettingsTranslationTargetProvider {
    fn descriptor(&self) -> TranslationTargetProviderDescriptor {
        Self::descriptor_value()
    }

    async fn list_resources(
        &self,
        context: PortContext,
        request: ListTranslationResourcesRequest,
    ) -> Result<TranslationResourcePage, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Read)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let after = request.cursor.as_ref().map(OpaqueCursor::as_str);
        let mut resources = Vec::new();

        for registry in self.candidate_registries()? {
            if after.is_some_and(|after| registry.module_slug() <= after) {
                continue;
            }
            let owner = match StaticSettingsTranslationReadService::new(self.db.clone())
                .exact_locale_snapshot(tenant_id, &registry, request.target_locale.as_str())
                .await
            {
                Ok(snapshot) if snapshot.source_locale == request.source_locale.as_str() => snapshot,
                Ok(_) => continue,
                Err(error) if source_not_translation_ready(&error) => continue,
                Err(error) => return Err(translation_read_error_to_port_error(error)),
            };
            if owner.fields.is_empty() {
                continue;
            }
            resources.push(summary_from_owner(&registry, &owner)?);
            if resources.len() > usize::from(request.limit) {
                break;
            }
        }

        let has_more = resources.len() > usize::from(request.limit);
        if has_more {
            resources.truncate(usize::from(request.limit));
        }
        let next_cursor = has_more
            .then(|| resources.last())
            .flatten()
            .map(|summary| {
                OpaqueCursor::new(summary.identity.resource_id.as_str()).expect(
                    "Settings module slug resource ID must satisfy the opaque cursor contract",
                )
            });

        Ok(TranslationResourcePage {
            resources,
            next_cursor,
        })
    }

    async fn read_resource(
        &self,
        context: PortContext,
        request: ReadTranslationResourceRequest,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Read)?;
        if request.source_locale == request.target_locale {
            return Err(PortError::validation(
                "translation.equal_source_target_locale",
                "source and target locale must differ",
            ));
        }
        let tenant_id = parse_tenant_id(&context)?;
        let registry = self.registry_for_identity(&request.identity)?;
        self.neutral_snapshot(
            tenant_id,
            &registry,
            &request.source_locale,
            &request.target_locale,
        )
        .await
    }

    async fn validate_patch(
        &self,
        context: PortContext,
        request: TranslationPatchRequest,
    ) -> Result<TranslationPatchValidation, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Update)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let registry = self.registry_for_identity(&request.identity)?;
        let owner = self
            .owner_snapshot(
                tenant_id,
                &registry,
                &request.source_locale,
                &request.target_locale,
            )
            .await?;
        let identity = StaticSettingsTranslationIdentity::from_registry(&registry)
            .map_err(adapter_error_to_port_error)?;
        identity
            .validate_patch_against_snapshot(&request, &owner)
            .map_err(adapter_error_to_port_error)
    }

    async fn apply_patch(
        &self,
        context: PortContext,
        request: TranslationPatchRequest,
    ) -> Result<TranslationApplicationReceipt, PortError> {
        validate_translation_apply_context(&context)?;
        authorize(&context, Action::Update)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let command_context = owner_command_context(&context, tenant_id)?;
        let idempotency_key = context.idempotency_key.as_deref().unwrap_or_default();
        let lease = match idempotency::admit(
            &self.db,
            idempotency::OwnerOperationScope::Tenant(tenant_id),
            PROVIDER_RECEIPT_OWNER,
            idempotency_key,
            OPERATION_APPLY_PATCH,
            &request,
        )
        .await?
        {
            Admission::Run(lease) => lease,
            Admission::Replay(value) => return decode_application_receipt(value),
            Admission::ReplayError(error) => return Err(error),
        };

        let result = async {
            let registry = self.registry_for_identity(&request.identity)?;
            let owner = self
                .owner_snapshot(
                    tenant_id,
                    &registry,
                    &request.source_locale,
                    &request.target_locale,
                )
                .await?;
            let identity = StaticSettingsTranslationIdentity::from_registry(&registry)
                .map_err(adapter_error_to_port_error)?;
            let plan = match identity
                .prepare_apply_plan(&request, &owner, &command_context)
                .map_err(adapter_error_to_port_error)?
            {
                StaticSettingsTranslationPrepareResult::Ready(plan) => plan,
                StaticSettingsTranslationPrepareResult::Rejected(validation) => {
                    return Err(validation_to_port_error(&validation));
                }
            };

            let service = StaticSettingsLocalizationService::new(self.db.clone());
            for command in plan.commands {
                service
                    .apply_exact(&registry, command)
                    .await
                    .map_err(localization_error_to_port_error)?;
            }

            let final_owner = self
                .owner_snapshot(
                    tenant_id,
                    &registry,
                    &request.source_locale,
                    &request.target_locale,
                )
                .await?;
            if final_owner.owner_revision != plan.final_owner_revision {
                return Err(PortError::conflict(
                    "settings.translation_owner_revision_conflict",
                    "Settings owner revision changed during translation apply",
                ));
            }
            let final_revisions = identity
                .revisions_for_snapshot(&final_owner)
                .map_err(adapter_error_to_port_error)?;
            let target_revision = final_revisions.target_revision.ok_or_else(|| {
                PortError::invariant_violation(
                    "settings.translation_target_revision_missing",
                    "Settings translation apply completed without an exact target revision",
                )
            })?;
            let receipt = TranslationApplicationReceipt {
                provider_receipt_id: format!("settings:{}", lease.operation_id),
                resource_revision: final_revisions.resource_revision,
                target_revision,
                applied_field_keys: request
                    .fields
                    .iter()
                    .map(|field| field.key.clone())
                    .collect(),
            };
            idempotency::complete(&self.db, lease, &receipt).await?;
            Ok(receipt)
        }
        .await;

        if let Err(error) = &result {
            // Owner field writes are individually durable and replay-safe. If a
            // multi-field provider operation is interrupted after a committed
            // prefix, the reclaimed provider receipt fails closed here instead
            // of re-planning against stale neutral preconditions. The caller
            // must re-read current owner state and submit a fresh proposal.
            self.fail_receipt(lease, error).await;
        }
        result
    }

    async fn read_progress(
        &self,
        context: PortContext,
        request: TranslationTargetProgressRequest,
    ) -> Result<TranslationTargetProgressFacts, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Read)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let registries = self.candidate_registries()?;

        for _ in 0..PROGRESS_STABILITY_ATTEMPTS {
            let before = self.global_highwater(tenant_id, &registries).await?;
            let mut facts = self.progress_once(tenant_id, &registries, &request).await?;
            let after = self.global_highwater(tenant_id, &registries).await?;
            if before == after {
                // Progress and change polling share one cursor contract. A
                // stable high-water mark is therefore a tail checkpoint.
                facts.owner_change_cursor = after
                    .map(|value| change_cursor(value, value))
                    .transpose()?;
                facts.validate().map_err(|error| {
                    PortError::invariant_violation(
                        "settings.translation_progress_invalid",
                        error.to_string(),
                    )
                })?;
                return Ok(facts);
            }
        }

        Err(PortError::unavailable(
            "settings.translation_progress_unstable",
            "Settings translation progress changed while it was being aggregated",
        ))
    }

    async fn read_changes(
        &self,
        context: PortContext,
        request: TranslationTargetChangesRequest,
    ) -> Result<TranslationTargetChangePage, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Read)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let registries = self.candidate_registries()?;
        let parsed = request
            .after
            .as_ref()
            .map(parse_change_cursor)
            .transpose()?;
        let (through, after) = match parsed {
            Some((through, after)) if through == after => {
                let current = self
                    .global_highwater(tenant_id, &registries)
                    .await?
                    .unwrap_or(after)
                    .max(after);
                (current, after)
            }
            Some(cursor) => cursor,
            None => (self.global_highwater(tenant_id, &registries).await?.unwrap_or(0), 0),
        };
        if through == 0 {
            return Ok(TranslationTargetChangePage {
                changes: Vec::new(),
                next_cursor: None,
            });
        }

        let service = StaticSettingsTranslationReadService::new(self.db.clone());
        let mut changes = Vec::<(u64, TranslationTargetChange)>::new();
        for registry in &registries {
            let Some(current_highwater) = self.module_highwater(tenant_id, registry).await? else {
                continue;
            };
            let bound = through.min(current_highwater);
            if after >= bound {
                continue;
            }
            let page = service
                .read_changes(
                    registry,
                    StaticSettingsChangeReadRequest {
                        tenant_id,
                        after_seq: (after > 0).then_some(after),
                        through_seq: Some(bound),
                        limit: request.limit,
                    },
                )
                .await
                .map_err(translation_read_error_to_port_error)?;
            let identity = StaticSettingsTranslationIdentity::from_registry(registry)
                .map_err(adapter_error_to_port_error)?;
            for change in page.changes {
                changes.push((
                    change.change_seq,
                    TranslationTargetChange {
                        identity: identity.resource().clone(),
                        resource_revision: resource_revision_from_owner(change.owner_revision)?,
                        lifecycle: TranslationResourceLifecycle::Active,
                    },
                ));
            }
        }
        changes.sort_by_key(|(change_seq, _)| *change_seq);
        if changes.len() > usize::from(request.limit) {
            changes.truncate(usize::from(request.limit));
        }
        let last_seq = changes.last().map(|(change_seq, _)| *change_seq);
        let next_cursor = Some(match last_seq {
            Some(last_seq) if last_seq < through => change_cursor(through, last_seq)?,
            _ => change_cursor(through, through)?,
        });

        Ok(TranslationTargetChangePage {
            changes: changes.into_iter().map(|(_, change)| change).collect(),
            next_cursor,
        })
    }
}

fn summary_from_owner(
    registry: &StaticSettingsLocalizationRegistry,
    owner: &StaticSettingsExactLocaleSnapshot,
) -> Result<TranslationResourceSummary, PortError> {
    let identity = StaticSettingsTranslationIdentity::from_registry(registry)
        .map_err(adapter_error_to_port_error)?;
    let revisions = identity
        .revisions_for_snapshot(owner)
        .map_err(adapter_error_to_port_error)?;
    let source_locale = TenantLocale::new(owner.source_locale.clone()).map_err(|error| {
        PortError::invariant_violation(
            "settings.translation_source_locale_invalid",
            error.to_string(),
        )
    })?;
    let target_locale = TenantLocale::new(owner.target_locale.clone()).map_err(|error| {
        PortError::invariant_violation(
            "settings.translation_target_locale_invalid",
            error.to_string(),
        )
    })?;
    let mut exact_locales = vec![source_locale];
    if owner.progress().complete {
        exact_locales.push(target_locale);
    }
    Ok(TranslationResourceSummary {
        identity: identity.resource().clone(),
        display_label: registry.module_slug().to_string(),
        lifecycle: TranslationResourceLifecycle::Active,
        resource_revision: revisions.resource_revision,
        exact_locales,
    })
}

fn snapshot_from_owner(
    registry: &StaticSettingsLocalizationRegistry,
    owner: &StaticSettingsExactLocaleSnapshot,
) -> Result<TranslationResourceSnapshot, PortError> {
    if owner.fields.is_empty() {
        return Err(PortError::not_found(
            "settings.translation_resource_not_found",
            "Static Settings resource has no source-present localizable fields",
        ));
    }
    let identity = StaticSettingsTranslationIdentity::from_registry(registry)
        .map_err(adapter_error_to_port_error)?;
    let revisions = identity
        .revisions_for_snapshot(owner)
        .map_err(adapter_error_to_port_error)?;
    let summary = summary_from_owner(registry, owner)?;
    let source_locale = TenantLocale::new(owner.source_locale.clone()).map_err(|error| {
        PortError::invariant_violation(
            "settings.translation_source_locale_invalid",
            error.to_string(),
        )
    })?;
    let target_locale = TenantLocale::new(owner.target_locale.clone()).map_err(|error| {
        PortError::invariant_violation(
            "settings.translation_target_locale_invalid",
            error.to_string(),
        )
    })?;
    let mut fields = Vec::with_capacity(owner.fields.len());
    for field in &owner.fields {
        let key = FieldKey::new(field.field_id.clone()).map_err(|error| {
            PortError::invariant_violation(
                "settings.translation_field_key_invalid",
                error.to_string(),
            )
        })?;
        let descriptor = identity.descriptor_for_field(&key).ok_or_else(|| {
            PortError::invariant_violation(
                "settings.translation_field_unadmitted",
                "Settings owner snapshot exposed a field outside the localization registry",
            )
        })?;
        fields.push(TranslationFieldSnapshot {
            descriptor,
            source_value: field.source_value.clone(),
            exact_target_value: field.exact_target_value.clone(),
            source_hash: field_hash(&field.source_value),
            protected_tokens: Vec::new(),
        });
    }
    let snapshot = TranslationResourceSnapshot {
        summary,
        source_locale,
        target_locale,
        rendered_fallback_locale: None,
        source_revision: revisions.source_revision,
        target_revision: revisions.target_revision,
        fields,
    };
    snapshot.validate().map_err(|error| {
        PortError::invariant_violation(
            "settings.translation_snapshot_invalid",
            error.to_string(),
        )
    })?;
    Ok(snapshot)
}

fn resource_revision_from_owner(owner_revision: u64) -> Result<OpaqueRevision, PortError> {
    if owner_revision == 0 {
        return Err(PortError::invariant_violation(
            "settings.translation_resource_revision_invalid",
            "Settings owner revision must be positive",
        ));
    }
    OpaqueRevision::new(format!("{RESOURCE_REVISION_PREFIX}:{owner_revision}")).map_err(|error| {
        PortError::invariant_violation(
            "settings.translation_resource_revision_invalid",
            error.to_string(),
        )
    })
}

fn change_cursor(through: u64, after: u64) -> Result<OpaqueCursor, PortError> {
    OpaqueCursor::new(format!("{CHANGE_CURSOR_VERSION}:{through}:{after}")).map_err(|error| {
        PortError::invariant_violation(
            "settings.translation_change_cursor_invalid",
            error.to_string(),
        )
    })
}

fn parse_change_cursor(cursor: &OpaqueCursor) -> Result<(u64, u64), PortError> {
    let mut parts = cursor.as_str().split(':');
    let version = parts.next();
    let through = parts.next().and_then(|value| value.parse::<u64>().ok());
    let after = parts.next().and_then(|value| value.parse::<u64>().ok());
    if version != Some(CHANGE_CURSOR_VERSION)
        || parts.next().is_some()
        || through.is_none()
        || after.is_none()
    {
        return Err(PortError::validation(
            "settings.translation_change_cursor_invalid",
            "Settings translation change cursor is invalid",
        ));
    }
    let through = through.unwrap_or_default();
    let after = after.unwrap_or_default();
    if through == 0 || after == 0 || after > through {
        return Err(PortError::validation(
            "settings.translation_change_cursor_invalid",
            "Settings translation change cursor bounds are invalid",
        ));
    }
    Ok((through, after))
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    parse_non_nil_uuid(
        &context.tenant_id,
        "settings.invalid_tenant_id",
        "Settings translation target context must carry a non-nil UUID tenant_id",
    )
}

fn owner_command_context(
    context: &PortContext,
    tenant_id: Uuid,
) -> Result<ModuleCommandContext, PortError> {
    let actor_id = parse_non_nil_uuid(
        &context.actor.id,
        "settings.invalid_actor_id",
        "Settings translation owner command requires a UUID actor id",
    )?;
    let correlation_id = parse_non_nil_uuid(
        &context.correlation_id,
        "settings.invalid_correlation_id",
        "Settings translation owner command requires a UUID correlation id",
    )?;
    let idempotency_key = parse_non_nil_uuid(
        context.idempotency_key.as_deref().unwrap_or_default(),
        "settings.invalid_idempotency_key",
        "Settings translation owner command requires a UUID idempotency key",
    )?;
    let trace_id = context
        .traceparent
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(context.correlation_id.as_str())
        .to_string();
    let command = ModuleCommandContext {
        actor_id,
        tenant_id: Some(tenant_id),
        trace_id,
        correlation_id,
        idempotency_key,
    };
    command.validate().map_err(|error| {
        PortError::validation(
            "settings.invalid_command_context",
            error.to_string(),
        )
    })?;
    Ok(command)
}

fn parse_non_nil_uuid(value: &str, code: &str, message: &str) -> Result<Uuid, PortError> {
    Uuid::parse_str(value)
        .ok()
        .filter(|value| !value.is_nil())
        .ok_or_else(|| PortError::validation(code, message))
}

fn authorize(context: &PortContext, action: Action) -> Result<(), PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Settings, action) == PermissionScope::None {
        return Err(PortError::forbidden(
            "settings.translation_permission_denied",
            format!("settings:{action} permission is required"),
        ));
    }
    Ok(())
}

fn source_not_translation_ready(error: &StaticSettingsTranslationReadError) -> bool {
    matches!(
        error,
        StaticSettingsTranslationReadError::SourceLocale(
            StaticSettingsSourceLocaleError::SourceLocaleUnassigned(_)
                | StaticSettingsSourceLocaleError::SourceLocaleStale { .. }
        )
    )
}

fn manifest_error_to_port_error(error: crate::modules::manifest::ManifestError) -> PortError {
    tracing::error!(error = %error, "Settings translation package registry resolution failed");
    PortError::unavailable(
        "settings.translation_registry_unavailable",
        "Settings translation package registry is unavailable",
    )
}

fn adapter_error_to_port_error(error: StaticSettingsTranslationIdentityError) -> PortError {
    match error {
        StaticSettingsTranslationIdentityError::ForeignIdentity => PortError::validation(
            "settings.translation_identity_invalid",
            "Settings translation identity belongs to another owner or resource kind",
        ),
        error => PortError::invariant_violation(
            "settings.translation_adapter_invalid",
            error.to_string(),
        ),
    }
}

fn translation_read_error_to_port_error(error: StaticSettingsTranslationReadError) -> PortError {
    match error {
        StaticSettingsTranslationReadError::InvalidIdentity
        | StaticSettingsTranslationReadError::InvalidLocale
        | StaticSettingsTranslationReadError::EqualSourceAndTargetLocale
        | StaticSettingsTranslationReadError::InvalidPageLimit
        | StaticSettingsTranslationReadError::InvalidCursorBounds => PortError::validation(
            "settings.translation_read_invalid",
            error.to_string(),
        ),
        StaticSettingsTranslationReadError::OwnerOperationInProgress(_)
        | StaticSettingsTranslationReadError::SnapshotUnstable
        | StaticSettingsTranslationReadError::Database(_) => PortError::unavailable(
            "settings.translation_read_unavailable",
            error.to_string(),
        ),
        StaticSettingsTranslationReadError::InconsistentState(_) => PortError::invariant_violation(
            "settings.translation_read_inconsistent",
            error.to_string(),
        ),
        StaticSettingsTranslationReadError::SourceLocale(source) => {
            source_locale_error_to_port_error(source)
        }
    }
}

fn source_locale_error_to_port_error(error: StaticSettingsSourceLocaleError) -> PortError {
    match error {
        StaticSettingsSourceLocaleError::SourceLocaleUnassigned(_)
        | StaticSettingsSourceLocaleError::SourceLocaleStale { .. } => PortError::not_found(
            "settings.translation_source_not_found",
            "Static Settings authoritative source locale is not available",
        ),
        StaticSettingsSourceLocaleError::InvalidIdentity
        | StaticSettingsSourceLocaleError::InvalidLocale => PortError::validation(
            "settings.translation_source_invalid",
            error.to_string(),
        ),
        StaticSettingsSourceLocaleError::OwnerRevisionConflict { .. } => PortError::conflict(
            "settings.translation_owner_revision_conflict",
            error.to_string(),
        ),
        StaticSettingsSourceLocaleError::OwnerOperationInProgress(_)
        | StaticSettingsSourceLocaleError::SourceSnapshotUnstable
        | StaticSettingsSourceLocaleError::Database(_) => PortError::unavailable(
            "settings.translation_source_unavailable",
            error.to_string(),
        ),
        StaticSettingsSourceLocaleError::InconsistentState(_) => PortError::invariant_violation(
            "settings.translation_source_inconsistent",
            error.to_string(),
        ),
        StaticSettingsSourceLocaleError::Localization(error) => {
            localization_error_to_port_error(error)
        }
        StaticSettingsSourceLocaleError::OperationReceipt(error) => error,
    }
}

fn localization_error_to_port_error(error: StaticSettingsLocalizationError) -> PortError {
    match error {
        StaticSettingsLocalizationError::InvalidIdentity
        | StaticSettingsLocalizationError::InvalidLocale
        | StaticSettingsLocalizationError::UnknownField(_)
        | StaticSettingsLocalizationError::InvalidValue { .. }
        | StaticSettingsLocalizationError::Metadata(_) => PortError::validation(
            "settings.translation_owner_validation",
            error.to_string(),
        ),
        StaticSettingsLocalizationError::TargetRevisionConflict { .. }
        | StaticSettingsLocalizationError::OwnerRevisionConflict { .. } => PortError::conflict(
            "settings.translation_owner_conflict",
            error.to_string(),
        ),
        StaticSettingsLocalizationError::OwnerOperationInProgress(_)
        | StaticSettingsLocalizationError::SourceSnapshotUnstable
        | StaticSettingsLocalizationError::Database(_) => PortError::unavailable(
            "settings.translation_owner_unavailable",
            error.to_string(),
        ),
        StaticSettingsLocalizationError::InconsistentState(_) => PortError::invariant_violation(
            "settings.translation_owner_inconsistent",
            error.to_string(),
        ),
        StaticSettingsLocalizationError::OperationReceipt(error) => error,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet, HashMap};
    use std::time::Duration;

    use rustok_api::PortActor;
    use rustok_modules::{ModuleSettingSpec, static_settings_translation_read::StaticSettingsExactLocaleField};

    use super::*;

    fn registry() -> StaticSettingsLocalizationRegistry {
        StaticSettingsLocalizationRegistry::new(
            "storefront",
            HashMap::from([(
                "title".to_string(),
                ModuleSettingSpec {
                    value_type: "string".to_string(),
                    ..Default::default()
                },
            )]),
            BTreeMap::from([("storefront.title".to_string(), "title".to_string())]),
            BTreeSet::new(),
        )
        .expect("registry")
    }

    fn owner_snapshot() -> StaticSettingsExactLocaleSnapshot {
        StaticSettingsExactLocaleSnapshot {
            tenant_id: Uuid::new_v4(),
            module_slug: "storefront".to_string(),
            source_locale: "en".to_string(),
            target_locale: "de".to_string(),
            owner_revision: 9,
            owner_change_seq: Some(17),
            fields: vec![StaticSettingsExactLocaleField {
                field_id: "storefront.title".to_string(),
                source_value: "Welcome".to_string(),
                exact_target_value: Some("Willkommen".to_string()),
                target_revision: Some(2),
                target_owner_revision: Some(8),
            }],
        }
    }

    #[test]
    fn descriptor_exposes_owner_runtime_capabilities() {
        let descriptor = StaticSettingsTranslationTargetProvider::descriptor_value();
        descriptor.validate().expect("descriptor");
        assert_eq!(descriptor.owner_slug.as_str(), "modules");
        assert_eq!(descriptor.resource_kind.as_str(), "static_settings");
        assert!(descriptor.capabilities.contains(&TranslationTargetCapability::ChangeCursor));
        assert!(descriptor.capabilities.contains(&TranslationTargetCapability::ApplyPatch));
    }

    #[test]
    fn owner_snapshot_maps_through_neutral_adapter_contract() {
        let registry = registry();
        let owner = owner_snapshot();
        let snapshot = snapshot_from_owner(&registry, &owner).expect("snapshot");
        assert_eq!(snapshot.summary.identity.resource_id.as_str(), "storefront");
        assert_eq!(snapshot.summary.exact_locales.len(), 2);
        assert_eq!(snapshot.fields[0].source_hash, field_hash("Welcome"));
        let adapter = StaticSettingsTranslationIdentity::from_registry(&registry).unwrap();
        assert_eq!(
            snapshot.summary.resource_revision,
            adapter.revisions_for_snapshot(&owner).unwrap().resource_revision
        );
        assert_eq!(
            snapshot.summary.resource_revision,
            resource_revision_from_owner(owner.owner_revision).unwrap()
        );
    }

    #[test]
    fn bounded_change_cursor_round_trips() {
        let cursor = change_cursor(42, 17).expect("cursor");
        assert_eq!(parse_change_cursor(&cursor).unwrap(), (42, 17));
        let tail = change_cursor(42, 42).expect("tail cursor");
        assert_eq!(parse_change_cursor(&tail).unwrap(), (42, 42));
        assert!(parse_change_cursor(&OpaqueCursor::new("42").unwrap()).is_err());
    }

    #[test]
    fn owner_command_bridge_requires_uuid_evidence() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let correlation_id = Uuid::new_v4();
        let idempotency_key = Uuid::new_v4();
        let context = PortContext::new(
            tenant_id.to_string(),
            PortActor::user(actor_id.to_string()),
            "en",
            correlation_id.to_string(),
        )
        .with_idempotency_key(idempotency_key.to_string())
        .with_deadline(Duration::from_secs(5));
        let owner = owner_command_context(&context, tenant_id).expect("owner context");
        assert_eq!(owner.actor_id, actor_id);
        assert_eq!(owner.correlation_id, correlation_id);
        assert_eq!(owner.idempotency_key, idempotency_key);
        assert_eq!(owner.tenant_id, Some(tenant_id));
    }
}
