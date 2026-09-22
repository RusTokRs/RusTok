use std::collections::BTreeSet;

use alloy::storage::{
    ScriptPresentationTranslationChangeLifecycle, ScriptPresentationTranslationChangeOwnerPort,
    ScriptPresentationTranslationError, ScriptPresentationTranslationOperationContext,
    ScriptPresentationTranslationResourceSummary, SeaOrmScriptPresentationTranslationStore,
    SeaOrmScriptPresentationTranslationTargetOwner,
};
use async_trait::async_trait;
use rustok_api::{Action, PortContext, PortError, Resource, StoredLocale, TenantLocale};
use rustok_core::{ModuleRuntimeExtensions, PermissionScope, SecurityContext};
use rustok_translation_targets::{
    FieldKey, ListTranslationResourcesRequest, OpaqueCursor, OpaqueRevision, OwnerSlug,
    ReadTranslationResourceRequest, ResourceId, ResourceKind, TranslationApplicationReceipt,
    TranslationDataClassification, TranslationFieldDescriptor, TranslationFieldSnapshot,
    TranslationPatchRequest, TranslationPatchValidation, TranslationResourceIdentity,
    TranslationResourceLifecycle, TranslationResourcePage, TranslationResourceSnapshot,
    TranslationResourceSummary, TranslationStrategy, TranslationTargetCapability,
    TranslationTargetChange, TranslationTargetChangePage, TranslationTargetChangesRequest,
    TranslationTargetProgressFacts, TranslationTargetProgressRequest, TranslationTargetProvider,
    TranslationTargetProviderDescriptor, TranslationTargetRegistryError, TranslationValueProfile,
    provider_support::{
        contract_validation_error, field_hash, merged_patch_values,
        normalize_optional_target_value, opaque_positive_revision, parse_positive_revision,
        read_request_from_patch, validate_patch_against_snapshot, validation_to_port_error,
    },
    register_translation_target_provider, validate_translation_apply_context,
    validate_translation_read_context,
};
use sea_orm::DatabaseConnection;
use sha2::{Digest, Sha256};
use uuid::Uuid;

const TRANSLATION_OWNER_SLUG: &str = "alloy";
const TRANSLATION_RESOURCE_KIND: &str = "script_presentation";
const DESCRIPTION_FIELD_KEY: &str = "description";

#[derive(Clone)]
pub struct AlloyScriptPresentationTranslationTargetProvider {
    db: DatabaseConnection,
}

impl AlloyScriptPresentationTranslationTargetProvider {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    fn descriptor_value() -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
                .expect("static Alloy owner slug must satisfy Translation target contract"),
            resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
                .expect("static Alloy Script presentation kind must satisfy Translation contract"),
            display_name: "Alloy Script Presentations".to_string(),
            capabilities: BTreeSet::from([
                TranslationTargetCapability::ListResources,
                TranslationTargetCapability::ReadExactResource,
                TranslationTargetCapability::AggregateProgress,
                TranslationTargetCapability::ValidatePatch,
                TranslationTargetCapability::ApplyPatch,
                TranslationTargetCapability::ChangeCursor,
            ]),
            read_permission_floor: BTreeSet::from(["scripts:manage".to_string()]),
            apply_permission_floor: BTreeSet::from(["scripts:manage".to_string()]),
        }
    }

    fn owner(
        &self,
        tenant_id: Uuid,
    ) -> Result<SeaOrmScriptPresentationTranslationTargetOwner, PortError> {
        SeaOrmScriptPresentationTranslationTargetOwner::new(self.db.clone(), tenant_id)
            .map_err(owner_error_to_port_error)
    }

    fn change_owner(
        &self,
        tenant_id: Uuid,
    ) -> Result<SeaOrmScriptPresentationTranslationStore, PortError> {
        SeaOrmScriptPresentationTranslationStore::new(self.db.clone(), tenant_id)
            .map_err(owner_error_to_port_error)
    }

    async fn load_snapshot(
        &self,
        tenant_id: Uuid,
        request: &ReadTranslationResourceRequest,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        if request.source_locale == request.target_locale {
            return Err(PortError::validation(
                "translation.equal_source_target_locale",
                "source and target locale must differ",
            ));
        }
        let script_id = parse_identity(&request.identity)?;
        let source_locale = StoredLocale::from(request.source_locale.clone());
        let target_locale = StoredLocale::from(request.target_locale.clone());
        let exact = self
            .owner(tenant_id)?
            .read_exact_resource(script_id, &source_locale, &target_locale)
            .await
            .map_err(owner_error_to_port_error)?;
        let source_value = exact.source.description.clone().unwrap_or_default();
        let exact_locales = exact
            .exact_locales
            .into_iter()
            .map(tenant_locale_from_stored)
            .collect::<Result<Vec<_>, _>>()?;
        let snapshot = TranslationResourceSnapshot {
            summary: TranslationResourceSummary {
                identity: script_presentation_identity(exact.script_id),
                display_label: format!("Alloy Script {}", exact.script_id),
                lifecycle: TranslationResourceLifecycle::Active,
                resource_revision: opaque_revision(exact.resource_revision, "resource_revision")?,
                exact_locales,
            },
            source_locale: request.source_locale.clone(),
            target_locale: request.target_locale.clone(),
            rendered_fallback_locale: None,
            source_revision: opaque_positive_revision(
                exact.source.copy_revision,
                "source_revision",
            )?,
            target_revision: exact
                .target
                .as_ref()
                .map(|target| opaque_positive_revision(target.copy_revision, "target_revision"))
                .transpose()?,
            fields: vec![TranslationFieldSnapshot {
                descriptor: description_descriptor(),
                source_hash: field_hash(&source_value),
                source_value,
                exact_target_value: exact.target.and_then(|target| target.description),
                protected_tokens: Vec::new(),
            }],
        };
        snapshot.validate().map_err(|error| {
            PortError::invariant_violation("alloy.translation_snapshot_invalid", error.to_string())
        })?;
        Ok(snapshot)
    }
}

pub fn register_alloy_script_presentation_translation_target_provider(
    extensions: &mut ModuleRuntimeExtensions,
    db: DatabaseConnection,
) -> Result<(), TranslationTargetRegistryError> {
    register_translation_target_provider(
        extensions,
        AlloyScriptPresentationTranslationTargetProvider::new(db),
    )
}

#[async_trait]
impl TranslationTargetProvider for AlloyScriptPresentationTranslationTargetProvider {
    fn descriptor(&self) -> TranslationTargetProviderDescriptor {
        Self::descriptor_value()
    }

    async fn list_resources(
        &self,
        context: PortContext,
        request: ListTranslationResourcesRequest,
    ) -> Result<TranslationResourcePage, PortError> {
        validate_translation_read_context(&context)?;
        authorize_manage(&context)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let after = request
            .cursor
            .as_ref()
            .map(|cursor| {
                Uuid::parse_str(cursor.as_str()).map_err(|_| {
                    PortError::validation(
                        "alloy.translation_cursor_invalid",
                        "Alloy Script presentation list cursor must be a Script UUID",
                    )
                })
            })
            .transpose()?;
        let source_locale = StoredLocale::from(request.source_locale);
        let page = self
            .owner(tenant_id)?
            .list_exact_resources(&source_locale, after, request.limit)
            .await
            .map_err(owner_error_to_port_error)?;
        let resources = page
            .resources
            .into_iter()
            .map(summary_from_owner)
            .collect::<Result<Vec<_>, _>>()?;
        let next_cursor = page
            .next_after
            .map(|script_id| OpaqueCursor::new(script_id.to_string()))
            .transpose()
            .map_err(|error| {
                PortError::invariant_violation(
                    "alloy.translation_cursor_invalid",
                    error.to_string(),
                )
            })?;
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
        authorize_manage(&context)?;
        let tenant_id = parse_tenant_id(&context)?;
        self.load_snapshot(tenant_id, &request).await
    }

    async fn validate_patch(
        &self,
        context: PortContext,
        request: TranslationPatchRequest,
    ) -> Result<TranslationPatchValidation, PortError> {
        validate_translation_read_context(&context)?;
        authorize_manage(&context)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let snapshot = self
            .load_snapshot(tenant_id, &read_request_from_patch(&request))
            .await?;
        Ok(validate_patch_against_snapshot(&request, &snapshot))
    }

    async fn apply_patch(
        &self,
        context: PortContext,
        request: TranslationPatchRequest,
    ) -> Result<TranslationApplicationReceipt, PortError> {
        validate_translation_apply_context(&context)?;
        authorize_manage(&context)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let script_id = parse_identity(&request.identity)?;
        let idempotency_key = context
            .idempotency_key
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                PortError::validation(
                    "alloy.translation_idempotency_required",
                    "Alloy Script presentation apply requires a nonblank idempotency key",
                )
            })?
            .to_string();
        let request_fingerprint = patch_fingerprint(&request)?;
        let target_owner = self.owner(tenant_id)?;
        if let Some(replay) = target_owner
            .replay_completed_apply(script_id, &idempotency_key, &request_fingerprint)
            .await
            .map_err(owner_error_to_port_error)?
        {
            return Ok(TranslationApplicationReceipt {
                provider_receipt_id: replay.operation_id.to_string(),
                resource_revision: opaque_revision(replay.resource_revision, "resource_revision")?,
                target_revision: opaque_positive_revision(
                    replay.target_copy_revision,
                    "target_revision",
                )?,
                applied_field_keys: request
                    .fields
                    .iter()
                    .map(|field| field.key.clone())
                    .collect(),
            });
        }

        let snapshot = self
            .load_snapshot(tenant_id, &read_request_from_patch(&request))
            .await?;
        let validation = validate_patch_against_snapshot(&request, &snapshot);
        if !validation.accepted {
            return Err(validation_to_port_error(&validation));
        }
        let mut target_values = merged_patch_values(&request, &snapshot);
        let target_description = target_values
            .remove(DESCRIPTION_FIELD_KEY)
            .flatten()
            .and_then(normalize_optional_target_value);
        let expected_source_copy_revision =
            parse_positive_revision(&request.expected_source_revision, "source_revision")?;
        let expected_target_copy_revision = request
            .expected_target_revision
            .as_ref()
            .map(|revision| parse_positive_revision(revision, "target_revision"))
            .transpose()?;
        let applied = self
            .change_owner(tenant_id)?
            .apply_exact_locale(
                script_id,
                alloy::storage::ScriptPresentationTranslationApply {
                    operation: ScriptPresentationTranslationOperationContext {
                        idempotency_key,
                        proposal_id: request.proposal_id.clone(),
                        approval_receipt_id: request.approval_receipt_id.clone(),
                        request_fingerprint,
                    },
                    source_locale: request.source_locale.as_str().to_string(),
                    target_locale: request.target_locale.as_str().to_string(),
                    target_description,
                    expected_resource_revision: request
                        .expected_resource_revision
                        .as_str()
                        .to_string(),
                    expected_source_copy_revision,
                    expected_target_copy_revision,
                },
            )
            .await
            .map_err(owner_error_to_port_error)?;
        Ok(TranslationApplicationReceipt {
            provider_receipt_id: applied.operation_id.to_string(),
            resource_revision: opaque_revision(applied.resource_revision, "resource_revision")?,
            target_revision: opaque_positive_revision(
                applied.target_copy_revision,
                "target_revision",
            )?,
            applied_field_keys: request
                .fields
                .iter()
                .map(|field| field.key.clone())
                .collect(),
        })
    }

    async fn read_progress(
        &self,
        context: PortContext,
        request: TranslationTargetProgressRequest,
    ) -> Result<TranslationTargetProgressFacts, PortError> {
        validate_translation_read_context(&context)?;
        authorize_manage(&context)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let progress = self
            .change_owner(tenant_id)?
            .read_exact_progress(
                request.source_locale.as_str(),
                request.target_locale.as_str(),
            )
            .await
            .map_err(owner_error_to_port_error)?;
        let facts = TranslationTargetProgressFacts {
            required_units: progress.facts.required_units,
            exact_required_units: progress.facts.exact_required_units,
            optional_units: progress.facts.optional_units,
            exact_optional_units: progress.facts.exact_optional_units,
            resources: progress.facts.resources,
            complete_resources: progress.facts.complete_resources,
            owner_change_cursor: progress
                .owner_change_cursor
                .map(OpaqueCursor::new)
                .transpose()
                .map_err(|error| {
                    PortError::invariant_violation(
                        "alloy.translation_change_cursor_invalid",
                        error.to_string(),
                    )
                })?,
        };
        facts.validate().map_err(|error| {
            PortError::invariant_violation("alloy.translation_progress_invalid", error.to_string())
        })?;
        Ok(facts)
    }

    async fn read_changes(
        &self,
        context: PortContext,
        request: TranslationTargetChangesRequest,
    ) -> Result<TranslationTargetChangePage, PortError> {
        validate_translation_read_context(&context)?;
        authorize_manage(&context)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let page = self
            .change_owner(tenant_id)?
            .read_changes(
                request.after.as_ref().map(OpaqueCursor::as_str),
                request.limit,
            )
            .await
            .map_err(owner_error_to_port_error)?;
        let changes = page
            .changes
            .into_iter()
            .map(|change| {
                Ok(TranslationTargetChange {
                    identity: script_presentation_identity(change.script_id),
                    resource_revision: opaque_revision(
                        change.resource_revision,
                        "resource_revision",
                    )?,
                    lifecycle: match change.lifecycle {
                        ScriptPresentationTranslationChangeLifecycle::Active => {
                            TranslationResourceLifecycle::Active
                        }
                        ScriptPresentationTranslationChangeLifecycle::Deleted => {
                            TranslationResourceLifecycle::Deleted
                        }
                    },
                })
            })
            .collect::<Result<Vec<_>, PortError>>()?;
        let next_cursor = page
            .next_cursor
            .map(OpaqueCursor::new)
            .transpose()
            .map_err(|error| {
                PortError::invariant_violation(
                    "alloy.translation_change_cursor_invalid",
                    error.to_string(),
                )
            })?;
        Ok(TranslationTargetChangePage {
            changes,
            next_cursor,
        })
    }
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "alloy.invalid_tenant_id",
            "Alloy Script presentation Translation context must carry a UUID tenant_id",
        )
    })
}

fn authorize_manage(context: &PortContext) -> Result<(), PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Scripts, Action::Manage) == PermissionScope::None {
        return Err(PortError::forbidden(
            "alloy.translation_permission_denied",
            "scripts:manage permission is required for Alloy Script presentation Translation",
        ));
    }
    Ok(())
}

fn parse_identity(identity: &TranslationResourceIdentity) -> Result<Uuid, PortError> {
    if identity.owner_slug.as_str() != TRANSLATION_OWNER_SLUG
        || identity.resource_kind.as_str() != TRANSLATION_RESOURCE_KIND
        || identity.subresource_id.is_some()
    {
        return Err(PortError::validation(
            "alloy.translation_identity_invalid",
            "Alloy Translation identity must address alloy/script_presentation without a subresource",
        ));
    }
    Uuid::parse_str(identity.resource_id.as_str()).map_err(|_| {
        PortError::validation(
            "alloy.translation_resource_id_invalid",
            "Alloy Script presentation resource id must be a UUID",
        )
    })
}

fn script_presentation_identity(script_id: Uuid) -> TranslationResourceIdentity {
    TranslationResourceIdentity {
        owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
            .expect("static Alloy owner slug must satisfy Translation target contract"),
        resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
            .expect("static Alloy resource kind must satisfy Translation target contract"),
        resource_id: ResourceId::new(script_id.to_string())
            .expect("UUID Script id must satisfy Translation target contract"),
        subresource_id: None,
    }
}

fn summary_from_owner(
    summary: ScriptPresentationTranslationResourceSummary,
) -> Result<TranslationResourceSummary, PortError> {
    Ok(TranslationResourceSummary {
        identity: script_presentation_identity(summary.script_id),
        display_label: format!("Alloy Script {}", summary.script_id),
        lifecycle: TranslationResourceLifecycle::Active,
        resource_revision: opaque_revision(summary.resource_revision, "resource_revision")?,
        exact_locales: summary
            .exact_locales
            .into_iter()
            .map(tenant_locale_from_stored)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn tenant_locale_from_stored(locale: StoredLocale) -> Result<TenantLocale, PortError> {
    TenantLocale::new(locale.as_str()).map_err(|error| {
        PortError::invariant_violation(
            "alloy.translation_locale_invalid",
            format!("Alloy exact locale cannot enter Translation contract: {error}"),
        )
    })
}

fn description_descriptor() -> TranslationFieldDescriptor {
    TranslationFieldDescriptor {
        key: FieldKey::new(DESCRIPTION_FIELD_KEY)
            .expect("static Alloy description field key must satisfy Translation target contract"),
        profile: TranslationValueProfile::PlainText,
        strategy: TranslationStrategy::Translate,
        classification: TranslationDataClassification::TenantPrivate,
        required: false,
        ai_export_allowed: false,
        max_characters: None,
        preserves_whitespace: false,
    }
}

fn opaque_revision(value: String, field: &'static str) -> Result<OpaqueRevision, PortError> {
    OpaqueRevision::new(value).map_err(|error| {
        PortError::invariant_violation(
            "alloy.translation_revision_invalid",
            format!("Alloy {field} is invalid: {error}"),
        )
    })
}

fn patch_fingerprint(request: &TranslationPatchRequest) -> Result<String, PortError> {
    let payload = serde_json::to_vec(request).map_err(|error| {
        PortError::invariant_violation(
            "alloy.translation_request_fingerprint_failed",
            error.to_string(),
        )
    })?;
    let mut digest = Sha256::new();
    digest.update(b"alloy/script-presentation-translation-patch/v1");
    digest.update(payload);
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}

fn owner_error_to_port_error(error: ScriptPresentationTranslationError) -> PortError {
    match error {
        ScriptPresentationTranslationError::Invalid(message) => {
            PortError::validation("alloy.translation_owner_validation", message)
        }
        ScriptPresentationTranslationError::UnsupportedBackend => PortError::unavailable(
            "alloy.translation_backend_unavailable",
            "Alloy Script presentation Translation target requires PostgreSQL",
        ),
        ScriptPresentationTranslationError::ScriptNotFound(_) => PortError::not_found(
            "alloy.translation_resource_not_found",
            "Alloy Script presentation resource was not found",
        ),
        ScriptPresentationTranslationError::SourceLocaleNotFound { .. } => PortError::not_found(
            "alloy.translation_source_not_found",
            "Exact source Alloy Script presentation locale was not found",
        ),
        ScriptPresentationTranslationError::RevisionConflict { .. } => PortError::conflict(
            "alloy.translation_revision_conflict",
            "Alloy Script presentation state conflicts with the request",
        ),
        ScriptPresentationTranslationError::IdempotencyConflict => PortError::conflict(
            "alloy.translation_idempotency_conflict",
            "Alloy Script presentation idempotency key was reused for another request",
        ),
        ScriptPresentationTranslationError::OwnerInvariant(message) => {
            PortError::invariant_violation("alloy.translation_owner_invariant", message)
        }
        ScriptPresentationTranslationError::Storage(message) => PortError::unavailable(
            "alloy.translation_owner_unavailable",
            format!("Alloy Script presentation storage is temporarily unavailable: {message}"),
        ),
    }
}
