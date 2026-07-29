//! Native Leptos server-function adapter for the shared Translation contract.

use leptos::prelude::*;

#[cfg(feature = "ssr")]
use crate::model::{
    Actor, ActorKind, ApplyResult, Assignment, Cancellation, Glossary, GlossaryBinding,
    GlossaryConcept, GlossaryMatchKind, GlossaryScope, GlossarySummary, GlossaryTermPolicy,
    GlossaryVariant, InventoryResult, Job, JobItem, JobProgress, MemoryEntry, MemoryMatchEvidence,
    MemoryMatchKind, MemoryMutation, MemoryRetentionPolicy, MemorySuggestion, Proposal,
    ProposalOrigin, ProposalValue, ProviderProgress, QaIssue, RequiredProviderProgress, Retry,
    TranslationPolicy, TranslationResourceIdentity, TranslationTarget,
};
use crate::model::{TranslationAdminOperation, TranslationAdminResponse};

#[server(prefix = "/api/fn", endpoint = "translation-admin/execute")]
pub async fn execute_translation_native(
    operation: TranslationAdminOperation,
) -> Result<TranslationAdminResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        execute_ssr(operation).await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = operation;
        Err(ServerFnError::new(
            "translation-admin/execute requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
async fn execute_ssr(
    operation: TranslationAdminOperation,
) -> Result<TranslationAdminResponse, ServerFnError> {
    use std::sync::Arc;

    use leptos::prelude::expect_context;
    use rustok_api::{AuthContext, HostRuntimeContext, RequestContext, TenantContext};
    use rustok_outbox::TransactionalEventBus;
    use rustok_tenant::{TenantLocalePolicyPort, TenantService};
    use rustok_translation_targets::TranslationTargetRegistry;

    let auth = leptos_axum::extract::<AuthContext>()
        .await
        .map_err(ServerFnError::new)?;
    let tenant = leptos_axum::extract::<TenantContext>()
        .await
        .map_err(ServerFnError::new)?;
    let request = leptos_axum::extract::<RequestContext>()
        .await
        .map_err(ServerFnError::new)?;
    if auth.tenant_id != tenant.id || request.tenant_id != tenant.id {
        return Err(ServerFnError::new(
            "Authenticated tenant does not match request tenant",
        ));
    }

    let runtime = expect_context::<HostRuntimeContext>();
    let database = runtime.db_clone();
    let providers = runtime
        .shared_get::<Arc<TranslationTargetRegistry>>()
        .unwrap_or_else(|| Arc::new(TranslationTargetRegistry::default()));
    let event_bus = runtime
        .shared_get::<TransactionalEventBus>()
        .ok_or_else(|| ServerFnError::new("Translation runtime is unavailable"))?;
    let tenant_locale_policies: Arc<dyn TenantLocalePolicyPort> =
        Arc::new(TenantService::new(database.clone()));
    let context = port_context(&auth, &request, operation.idempotency_key())?;

    dispatch(
        operation,
        context,
        database,
        providers,
        tenant_locale_policies,
        event_bus,
    )
    .await
}

#[cfg(feature = "ssr")]
fn port_context(
    auth: &rustok_api::AuthContext,
    request: &rustok_api::RequestContext,
    idempotency_key: Option<&str>,
) -> Result<rustok_api::PortContext, ServerFnError> {
    use rustok_api::{PortActor, PortContext};
    use rustok_core::infer_user_role_from_permissions;

    if matches!(idempotency_key, Some(key) if key.trim().is_empty()) {
        return Err(ServerFnError::new("Idempotency key must not be empty"));
    }
    let mut context = PortContext::new(
        request.tenant_id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        request.locale.as_str(),
        format!("translation-native-{}", rustok_core::generate_id()),
    )
    .with_deadline(std::time::Duration::from_secs(5))
    .with_role(infer_user_role_from_permissions(&auth.permissions).to_string());
    for permission in &auth.permissions {
        context = context.with_claim(permission.to_string());
    }
    if let Some(channel) = request.channel_slug.as_deref() {
        context = context.with_channel(channel);
    }
    if let Some(key) = idempotency_key {
        context = context.with_idempotency_key(key);
    }
    Ok(context)
}

#[cfg(feature = "ssr")]
async fn dispatch(
    operation: TranslationAdminOperation,
    context: rustok_api::PortContext,
    database: sea_orm::DatabaseConnection,
    providers: std::sync::Arc<rustok_translation_targets::TranslationTargetRegistry>,
    tenant_locale_policies: std::sync::Arc<dyn rustok_tenant::TenantLocalePolicyPort>,
    event_bus: rustok_outbox::TransactionalEventBus,
) -> Result<TranslationAdminResponse, ServerFnError> {
    use rustok_translation::{
        AddItemInput, ApplyProposalInput, ApproveProposalInput, AssignItemInput, CancelJobInput,
        CreateGlossaryInput, CreateJobInput, MemoryListInput, MemoryLookupInput, ProposalValue,
        PurgeMemoryEntryInput, RecoverApplyInput, ReplaceGlossaryTermsInput,
        ReplaceRequiredTargetLocalesInput, RetryItemInput, SaveProposalInput,
        SetGlossaryActiveInput, SetMemoryRetentionInput, SubmitProposalInput,
        TombstoneMemoryEntryInput, TranslationGlossaryService, TranslationInventoryService,
        TranslationMemoryService, TranslationPolicyService, TranslationProgressService,
        TranslationWorkflowService, UnassignItemInput, UpdateGlossaryInput,
    };

    let policy = || TranslationPolicyService::new(database.clone(), tenant_locale_policies.clone());
    let glossary =
        || TranslationGlossaryService::new(database.clone(), tenant_locale_policies.clone());
    let memory = || TranslationMemoryService::new(database.clone());
    let progress = || {
        TranslationProgressService::new(
            database.clone(),
            providers.clone(),
            tenant_locale_policies.clone(),
        )
    };
    let inventory = || TranslationInventoryService::new(database.clone(), providers.clone());
    let workflow = || {
        TranslationWorkflowService::new(
            database.clone(),
            providers.clone(),
            tenant_locale_policies.clone(),
            event_bus.clone(),
        )
    };

    let response = match operation {
        TranslationAdminOperation::ReadPolicy => TranslationAdminResponse::Policy(map_policy(
            policy().read_policy(context).await.map_err(public_error)?,
        )),
        TranslationAdminOperation::ListTargets => {
            authorize_target_list(&context)?;
            TranslationAdminResponse::Targets(
                providers
                    .descriptors()
                    .into_iter()
                    .map(map_target)
                    .collect(),
            )
        }
        TranslationAdminOperation::ListGlossaries { limit } => {
            TranslationAdminResponse::Glossaries(
                glossary()
                    .list_glossaries(context, limit)
                    .await
                    .map_err(public_error)?
                    .into_iter()
                    .map(map_glossary_summary)
                    .collect(),
            )
        }
        TranslationAdminOperation::ReadGlossary {
            glossary_id,
            revision,
        } => TranslationAdminResponse::Glossary(map_glossary(
            glossary()
                .read_glossary(context, parse_uuid(&glossary_id, "glossary_id")?, revision)
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::ListMemoryEntries {
            source_locale,
            target_locale,
            include_tombstoned,
            limit,
        } => TranslationAdminResponse::MemoryEntries(
            memory()
                .list_entries(
                    context,
                    MemoryListInput {
                        source_locale: source_locale.map(parse_locale).transpose()?,
                        target_locale: target_locale.map(parse_locale).transpose()?,
                        include_tombstoned,
                        limit,
                    },
                )
                .await
                .map_err(public_error)?
                .into_iter()
                .map(map_memory_entry)
                .collect(),
        ),
        TranslationAdminOperation::ReadMemoryEntry { entry_id } => {
            TranslationAdminResponse::MemoryEntry(map_memory_entry(
                memory()
                    .read_entry(context, parse_uuid(&entry_id, "entry_id")?)
                    .await
                    .map_err(public_error)?,
            ))
        }
        TranslationAdminOperation::LookupMemory {
            source_locale,
            target_locale,
            identity,
            field_key,
            source_text,
            minimum_similarity_basis_points,
            limit,
        } => TranslationAdminResponse::MemorySuggestions(
            memory()
                .lookup(
                    context,
                    MemoryLookupInput {
                        source_locale: parse_locale(source_locale)?,
                        target_locale: parse_locale(target_locale)?,
                        identity: parse_identity(identity)?,
                        field_key: parse_field_key(field_key)?,
                        source_text,
                        minimum_similarity_basis_points,
                        limit,
                    },
                )
                .await
                .map_err(public_error)?
                .into_iter()
                .map(map_memory_suggestion)
                .collect(),
        ),
        TranslationAdminOperation::ReadJobProgress { job_id } => {
            TranslationAdminResponse::JobProgress(map_job_progress(
                progress()
                    .read_job_progress(context, parse_uuid(&job_id, "job_id")?)
                    .await
                    .map_err(public_error)?,
            ))
        }
        TranslationAdminOperation::ReadProviderProgress {
            owner_slug,
            resource_kind,
            source_locale,
            target_locale,
        } => TranslationAdminResponse::ProviderProgress(map_provider_progress(
            progress()
                .read_provider_progress(
                    context,
                    parse_owner_slug(owner_slug)?,
                    parse_resource_kind(resource_kind)?,
                    parse_locale(source_locale)?,
                    parse_locale(target_locale)?,
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::ReadRequiredProviderProgress {
            owner_slug,
            resource_kind,
            source_locale,
        } => TranslationAdminResponse::RequiredProviderProgress(map_required_provider_progress(
            progress()
                .read_required_provider_progress(
                    context,
                    parse_owner_slug(owner_slug)?,
                    parse_resource_kind(resource_kind)?,
                    parse_locale(source_locale)?,
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::ReplacePolicy {
            expected_revision,
            required_target_locales,
            ..
        } => TranslationAdminResponse::Policy(map_policy(
            policy()
                .replace_required_target_locales(
                    context,
                    ReplaceRequiredTargetLocalesInput {
                        expected_revision,
                        required_target_locales: required_target_locales
                            .into_iter()
                            .map(parse_locale)
                            .collect::<Result<Vec<_>, _>>()?,
                    },
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::CreateGlossary {
            name,
            description,
            source_locale,
            target_locale,
            scope,
            ..
        } => TranslationAdminResponse::Glossary(map_glossary(
            glossary()
                .create_glossary(
                    context,
                    CreateGlossaryInput {
                        name,
                        description,
                        source_locale: parse_locale(source_locale)?,
                        target_locale: parse_locale(target_locale)?,
                        scope: parse_glossary_scope(scope)?,
                    },
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::UpdateGlossary {
            glossary_id,
            expected_revision,
            name,
            description,
            ..
        } => TranslationAdminResponse::Glossary(map_glossary(
            glossary()
                .update_glossary(
                    context,
                    UpdateGlossaryInput {
                        glossary_id: parse_uuid(&glossary_id, "glossary_id")?,
                        expected_revision,
                        name,
                        description,
                    },
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::ReplaceGlossaryTerms {
            glossary_id,
            expected_revision,
            concepts,
            ..
        } => TranslationAdminResponse::Glossary(map_glossary(
            glossary()
                .replace_terms(
                    context,
                    ReplaceGlossaryTermsInput {
                        glossary_id: parse_uuid(&glossary_id, "glossary_id")?,
                        expected_revision,
                        concepts: concepts
                            .into_iter()
                            .map(map_glossary_concept_input)
                            .collect(),
                    },
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::SetGlossaryActive {
            glossary_id,
            expected_revision,
            is_active,
            ..
        } => TranslationAdminResponse::Glossary(map_glossary(
            glossary()
                .set_active(
                    context,
                    SetGlossaryActiveInput {
                        glossary_id: parse_uuid(&glossary_id, "glossary_id")?,
                        expected_revision,
                        is_active,
                    },
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::SetMemoryRetention {
            entry_id,
            expected_revision,
            policy,
            retain_until,
            ..
        } => TranslationAdminResponse::MemoryMutation(map_memory_mutation(
            memory()
                .set_retention(
                    context,
                    SetMemoryRetentionInput {
                        entry_id: parse_uuid(&entry_id, "entry_id")?,
                        expected_revision,
                        policy: map_memory_retention_input(policy),
                        retain_until: retain_until
                            .map(|value| {
                                chrono::DateTime::parse_from_rfc3339(&value).map_err(|_| {
                                    ServerFnError::new("retain_until must be an RFC 3339 timestamp")
                                })
                            })
                            .transpose()?,
                    },
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::TombstoneMemoryEntry {
            entry_id,
            expected_revision,
            ..
        } => TranslationAdminResponse::MemoryMutation(map_memory_mutation(
            memory()
                .tombstone_entry(
                    context,
                    TombstoneMemoryEntryInput {
                        entry_id: parse_uuid(&entry_id, "entry_id")?,
                        expected_revision,
                    },
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::PurgeMemoryEntry {
            entry_id,
            expected_revision,
            ..
        } => TranslationAdminResponse::MemoryMutation(map_memory_mutation(
            memory()
                .purge_entry(
                    context,
                    PurgeMemoryEntryInput {
                        entry_id: parse_uuid(&entry_id, "entry_id")?,
                        expected_revision,
                    },
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::CreateJob {
            source_locale,
            target_locale,
            glossary,
            ..
        } => TranslationAdminResponse::Job(map_job(
            workflow()
                .create_job(
                    context,
                    CreateJobInput {
                        source_locale: parse_locale(source_locale)?,
                        target_locale: parse_locale(target_locale)?,
                        glossary: glossary.map(map_glossary_binding_input).transpose()?,
                    },
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::AddItem {
            job_id, identity, ..
        } => TranslationAdminResponse::Item(map_item(
            workflow()
                .add_item(
                    context,
                    AddItemInput {
                        job_id: parse_uuid(&job_id, "job_id")?,
                        identity: parse_identity(identity)?,
                    },
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::SaveProposal {
            item_id,
            origin,
            values,
            ..
        } => TranslationAdminResponse::Proposal(map_proposal(
            workflow()
                .save_proposal(
                    context,
                    SaveProposalInput {
                        item_id: parse_uuid(&item_id, "item_id")?,
                        origin: map_origin_input(origin),
                        values: values
                            .into_iter()
                            .map(|value| {
                                Ok(ProposalValue {
                                    key: parse_field_key(value.key)?,
                                    value: value.value,
                                })
                            })
                            .collect::<Result<Vec<_>, ServerFnError>>()?,
                    },
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::SubmitProposal {
            item_id,
            proposal_id,
            ..
        } => TranslationAdminResponse::Proposal(map_proposal(
            workflow()
                .submit_proposal(
                    context,
                    SubmitProposalInput {
                        item_id: parse_uuid(&item_id, "item_id")?,
                        proposal_id: parse_uuid(&proposal_id, "proposal_id")?,
                    },
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::ApproveProposal {
            item_id,
            proposal_id,
            ..
        } => TranslationAdminResponse::Proposal(map_proposal(
            workflow()
                .approve_proposal(
                    context,
                    ApproveProposalInput {
                        item_id: parse_uuid(&item_id, "item_id")?,
                        proposal_id: parse_uuid(&proposal_id, "proposal_id")?,
                    },
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::ApplyProposal {
            item_id,
            proposal_id,
            ..
        } => TranslationAdminResponse::Apply(map_apply(
            workflow()
                .apply_proposal(
                    context,
                    ApplyProposalInput {
                        item_id: parse_uuid(&item_id, "item_id")?,
                        proposal_id: parse_uuid(&proposal_id, "proposal_id")?,
                    },
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::AssignItem {
            item_id,
            expected_revision,
            assignee,
            ..
        } => TranslationAdminResponse::Assignment(map_assignment(
            workflow()
                .assign_item(
                    context,
                    AssignItemInput {
                        item_id: parse_uuid(&item_id, "item_id")?,
                        expected_revision,
                        assignee: map_actor_input(assignee),
                    },
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::UnassignItem {
            item_id,
            expected_revision,
            ..
        } => TranslationAdminResponse::Assignment(map_assignment(
            workflow()
                .unassign_item(
                    context,
                    UnassignItemInput {
                        item_id: parse_uuid(&item_id, "item_id")?,
                        expected_revision,
                    },
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::CancelJob {
            job_id,
            expected_revision,
            reason,
            ..
        } => TranslationAdminResponse::Cancellation(map_cancellation(
            workflow()
                .cancel_job(
                    context,
                    CancelJobInput {
                        job_id: parse_uuid(&job_id, "job_id")?,
                        expected_revision,
                        reason,
                    },
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::RetryItem {
            item_id,
            expected_revision,
            reason,
            ..
        } => TranslationAdminResponse::Retry(map_retry(
            workflow()
                .retry_item(
                    context,
                    RetryItemInput {
                        item_id: parse_uuid(&item_id, "item_id")?,
                        expected_revision,
                        reason,
                    },
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::RecoverApply {
            operation_id,
            expected_attempt_count,
            reason,
            ..
        } => TranslationAdminResponse::Apply(map_apply(
            workflow()
                .recover_apply(
                    context,
                    RecoverApplyInput {
                        operation_id: parse_uuid(&operation_id, "operation_id")?,
                        expected_attempt_count,
                        reason,
                    },
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::RebuildJobProgress { job_id, .. } => {
            TranslationAdminResponse::JobProgress(map_job_progress(
                progress()
                    .rebuild_job_progress(context, parse_uuid(&job_id, "job_id")?)
                    .await
                    .map_err(public_error)?,
            ))
        }
        TranslationAdminOperation::SyncProviderInventory {
            owner_slug,
            resource_kind,
            limit,
        } => TranslationAdminResponse::Inventory(map_inventory(
            inventory()
                .sync_provider_changes(
                    context,
                    parse_owner_slug(owner_slug)?,
                    parse_resource_kind(resource_kind)?,
                    limit,
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::RebuildProviderInventory {
            owner_slug,
            resource_kind,
            source_locale,
            target_locale,
            page_size,
        } => TranslationAdminResponse::Inventory(map_rebuild_inventory(
            inventory()
                .rebuild_provider_inventory(
                    context,
                    parse_owner_slug(owner_slug)?,
                    parse_resource_kind(resource_kind)?,
                    parse_locale(source_locale)?,
                    parse_locale(target_locale)?,
                    page_size,
                )
                .await
                .map_err(public_error)?,
        )),
    };
    Ok(response)
}

#[cfg(feature = "ssr")]
fn authorize_target_list(context: &rustok_api::PortContext) -> Result<(), ServerFnError> {
    use rustok_api::{Action, PortCallPolicy, Resource};
    use rustok_core::{PermissionScope, SecurityContext};

    context
        .require_policy(PortCallPolicy::read())
        .map_err(|error| ServerFnError::new(error.message))?;
    let security = SecurityContext::try_from_port_context(context)
        .map_err(|error| ServerFnError::new(error.message))?;
    if security.get_scope(Resource::Translations, Action::Read) == PermissionScope::None {
        return Err(ServerFnError::new("Translation permission denied"));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
fn parse_uuid(value: &str, field: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value).map_err(|_| ServerFnError::new(format!("{field} must be a UUID")))
}

#[cfg(feature = "ssr")]
fn parse_locale(value: String) -> Result<rustok_api::TenantLocale, ServerFnError> {
    rustok_api::TenantLocale::new(value).map_err(|error| ServerFnError::new(error.to_string()))
}

#[cfg(feature = "ssr")]
fn parse_owner_slug(value: String) -> Result<rustok_translation_targets::OwnerSlug, ServerFnError> {
    rustok_translation_targets::OwnerSlug::new(value)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[cfg(feature = "ssr")]
fn parse_resource_kind(
    value: String,
) -> Result<rustok_translation_targets::ResourceKind, ServerFnError> {
    rustok_translation_targets::ResourceKind::new(value)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[cfg(feature = "ssr")]
fn parse_field_key(value: String) -> Result<rustok_translation_targets::FieldKey, ServerFnError> {
    rustok_translation_targets::FieldKey::new(value)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[cfg(feature = "ssr")]
fn parse_glossary_scope(
    value: GlossaryScope,
) -> Result<rustok_translation::GlossaryScope, ServerFnError> {
    Ok(rustok_translation::GlossaryScope {
        owner_slug: value.owner_slug.map(parse_owner_slug).transpose()?,
        resource_kind: value.resource_kind.map(parse_resource_kind).transpose()?,
        field_key: value.field_key.map(parse_field_key).transpose()?,
    })
}

#[cfg(feature = "ssr")]
fn map_glossary_binding_input(
    value: GlossaryBinding,
) -> Result<rustok_translation::GlossaryBinding, ServerFnError> {
    Ok(rustok_translation::GlossaryBinding {
        glossary_id: parse_uuid(&value.glossary_id, "glossary_id")?,
        revision: value.revision,
    })
}

#[cfg(feature = "ssr")]
fn map_glossary_concept_input(value: GlossaryConcept) -> rustok_translation::GlossaryConcept {
    rustok_translation::GlossaryConcept {
        concept_key: value.concept_key,
        source_term: value.source_term,
        variants: value
            .variants
            .into_iter()
            .map(|variant| rustok_translation::GlossaryVariant {
                value: variant.value,
                policy: match variant.policy {
                    GlossaryTermPolicy::Preferred => {
                        rustok_translation::GlossaryTermPolicy::Preferred
                    }
                    GlossaryTermPolicy::Allowed => rustok_translation::GlossaryTermPolicy::Allowed,
                    GlossaryTermPolicy::Forbidden => {
                        rustok_translation::GlossaryTermPolicy::Forbidden
                    }
                    GlossaryTermPolicy::DoNotTranslate => {
                        rustok_translation::GlossaryTermPolicy::DoNotTranslate
                    }
                },
            })
            .collect(),
        match_kind: match value.match_kind {
            GlossaryMatchKind::Exact => rustok_translation::GlossaryMatchKind::Exact,
            GlossaryMatchKind::WholeWord => rustok_translation::GlossaryMatchKind::WholeWord,
            GlossaryMatchKind::Substring => rustok_translation::GlossaryMatchKind::Substring,
        },
        case_sensitive: value.case_sensitive,
        notes: value.notes,
    }
}

#[cfg(feature = "ssr")]
fn parse_identity(
    value: TranslationResourceIdentity,
) -> Result<rustok_translation_targets::TranslationResourceIdentity, ServerFnError> {
    Ok(rustok_translation_targets::TranslationResourceIdentity {
        owner_slug: parse_owner_slug(value.owner_slug)?,
        resource_kind: parse_resource_kind(value.resource_kind)?,
        resource_id: rustok_translation_targets::ResourceId::new(value.resource_id)
            .map_err(|error| ServerFnError::new(error.to_string()))?,
        subresource_id: value
            .subresource_id
            .map(rustok_translation_targets::ResourceId::new)
            .transpose()
            .map_err(|error| ServerFnError::new(error.to_string()))?,
    })
}

#[cfg(feature = "ssr")]
fn map_origin_input(value: ProposalOrigin) -> rustok_translation::ProposalOrigin {
    match value {
        ProposalOrigin::Manual => rustok_translation::ProposalOrigin::Manual,
        ProposalOrigin::Import => rustok_translation::ProposalOrigin::Import,
        ProposalOrigin::Memory => rustok_translation::ProposalOrigin::Memory,
        ProposalOrigin::Ai => rustok_translation::ProposalOrigin::Ai,
    }
}

#[cfg(feature = "ssr")]
fn map_memory_retention_input(
    value: MemoryRetentionPolicy,
) -> rustok_translation::MemoryRetentionPolicy {
    match value {
        MemoryRetentionPolicy::OwnerLifecycle => {
            rustok_translation::MemoryRetentionPolicy::OwnerLifecycle
        }
        MemoryRetentionPolicy::RetainUntil => {
            rustok_translation::MemoryRetentionPolicy::RetainUntil
        }
        MemoryRetentionPolicy::LegalHold => rustok_translation::MemoryRetentionPolicy::LegalHold,
    }
}

#[cfg(feature = "ssr")]
fn map_actor_input(value: Actor) -> rustok_api::PortActor {
    match value.kind {
        ActorKind::User => rustok_api::PortActor::user(value.id),
        ActorKind::Service => rustok_api::PortActor::service(value.id),
    }
}

#[cfg(feature = "ssr")]
fn public_error(error: rustok_translation::TranslationError) -> ServerFnError {
    ServerFnError::new(
        rustok_translation::map_translation_public_error(
            &error,
            "native_operation",
            "translation_admin_native",
        )
        .to_string(),
    )
}

#[cfg(feature = "ssr")]
fn map_policy(value: rustok_translation::TranslationPolicyRecord) -> TranslationPolicy {
    TranslationPolicy {
        tenant_id: value.tenant_id.to_string(),
        required_target_locales: locale_strings(value.required_target_locales),
        tenant_locale_policy_revision: value.tenant_locale_policy_revision,
        revision: value.revision,
        freshness: format!("{:?}", value.freshness).to_ascii_lowercase(),
        disabled_required_target_locales: locale_strings(value.disabled_required_target_locales),
    }
}

#[cfg(feature = "ssr")]
fn map_target(
    value: rustok_translation_targets::TranslationTargetProviderDescriptor,
) -> TranslationTarget {
    TranslationTarget {
        owner_slug: value.owner_slug.to_string(),
        resource_kind: value.resource_kind.to_string(),
        display_name: value.display_name,
        capabilities: value
            .capabilities
            .into_iter()
            .filter_map(|capability| {
                serde_json::to_value(capability)
                    .ok()?
                    .as_str()
                    .map(str::to_owned)
            })
            .collect(),
        read_permission_floor: value.read_permission_floor.into_iter().collect(),
        apply_permission_floor: value.apply_permission_floor.into_iter().collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_glossary_summary(value: rustok_translation::GlossarySummaryRecord) -> GlossarySummary {
    GlossarySummary {
        id: value.id.to_string(),
        name: value.name,
        description: value.description,
        source_locale: value.source_locale.as_str().to_string(),
        target_locale: value.target_locale.as_str().to_string(),
        scope: map_glossary_scope(value.scope),
        is_active: value.is_active,
        revision: value.revision,
    }
}

#[cfg(feature = "ssr")]
fn map_glossary(value: rustok_translation::GlossaryRecord) -> Glossary {
    Glossary {
        id: value.id.to_string(),
        name: value.name,
        description: value.description,
        source_locale: value.source_locale.as_str().to_string(),
        target_locale: value.target_locale.as_str().to_string(),
        scope: map_glossary_scope(value.scope),
        is_active: value.is_active,
        revision: value.revision,
        concepts: value
            .concepts
            .into_iter()
            .map(|concept| GlossaryConcept {
                concept_key: concept.concept_key,
                source_term: concept.source_term,
                variants: concept
                    .variants
                    .into_iter()
                    .map(|variant| GlossaryVariant {
                        value: variant.value,
                        policy: match variant.policy {
                            rustok_translation::GlossaryTermPolicy::Preferred => {
                                GlossaryTermPolicy::Preferred
                            }
                            rustok_translation::GlossaryTermPolicy::Allowed => {
                                GlossaryTermPolicy::Allowed
                            }
                            rustok_translation::GlossaryTermPolicy::Forbidden => {
                                GlossaryTermPolicy::Forbidden
                            }
                            rustok_translation::GlossaryTermPolicy::DoNotTranslate => {
                                GlossaryTermPolicy::DoNotTranslate
                            }
                        },
                    })
                    .collect(),
                match_kind: match concept.match_kind {
                    rustok_translation::GlossaryMatchKind::Exact => GlossaryMatchKind::Exact,
                    rustok_translation::GlossaryMatchKind::WholeWord => {
                        GlossaryMatchKind::WholeWord
                    }
                    rustok_translation::GlossaryMatchKind::Substring => {
                        GlossaryMatchKind::Substring
                    }
                },
                case_sensitive: concept.case_sensitive,
                notes: concept.notes,
            })
            .collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_memory_retention(value: rustok_translation::MemoryRetentionPolicy) -> MemoryRetentionPolicy {
    match value {
        rustok_translation::MemoryRetentionPolicy::OwnerLifecycle => {
            MemoryRetentionPolicy::OwnerLifecycle
        }
        rustok_translation::MemoryRetentionPolicy::RetainUntil => {
            MemoryRetentionPolicy::RetainUntil
        }
        rustok_translation::MemoryRetentionPolicy::LegalHold => MemoryRetentionPolicy::LegalHold,
    }
}

#[cfg(feature = "ssr")]
fn map_memory_entry(value: rustok_translation::MemoryEntryRecord) -> MemoryEntry {
    MemoryEntry {
        id: value.id.to_string(),
        tenant_id: value.tenant_id.to_string(),
        source_locale: value.source_locale,
        target_locale: value.target_locale,
        owner_slug: value.owner_slug,
        resource_kind: value.resource_kind,
        resource_id: value.resource_id,
        subresource_id: value.subresource_id,
        field_key: value.field_key,
        source_text: value.source_text,
        target_text: value.target_text,
        source_hash: value.source_hash,
        target_hash: value.target_hash,
        context_fingerprint: value.context_fingerprint,
        segmentation_version: value.segmentation_version,
        origin: value.origin,
        quality_state: value.quality_state,
        reviewer_actor_kind: value.reviewer_actor_kind,
        reviewer_actor_id: value.reviewer_actor_id,
        proposal_id: value.proposal_id.to_string(),
        apply_receipt_id: value.apply_receipt_id.to_string(),
        retention_policy: map_memory_retention(value.retention_policy),
        retain_until: value.retain_until.map(|date| date.to_rfc3339()),
        tombstoned_at: value.tombstoned_at.map(|date| date.to_rfc3339()),
        revision: value.revision,
        created_at: value.created_at.to_rfc3339(),
        updated_at: value.updated_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn map_memory_suggestion(value: rustok_translation::MemorySuggestion) -> MemorySuggestion {
    MemorySuggestion {
        entry_id: value.entry_id.to_string(),
        source_text: value.source_text,
        target_text: value.target_text,
        source_hash: value.source_hash,
        owner_slug: value.owner_slug,
        resource_kind: value.resource_kind,
        resource_id: value.resource_id,
        field_key: value.field_key,
        origin: value.origin,
        proposal_id: value.proposal_id.to_string(),
        apply_receipt_id: value.apply_receipt_id.to_string(),
        evidence: MemoryMatchEvidence {
            kind: match value.evidence.kind {
                rustok_translation::MemoryMatchKind::Exact => MemoryMatchKind::Exact,
                rustok_translation::MemoryMatchKind::ContextualFuzzy => {
                    MemoryMatchKind::ContextualFuzzy
                }
                rustok_translation::MemoryMatchKind::Fuzzy => MemoryMatchKind::Fuzzy,
            },
            source_exact: value.evidence.source_exact,
            context_match: value.evidence.context_match,
            base_similarity_basis_points: value.evidence.base_similarity_basis_points,
            context_bonus_basis_points: value.evidence.context_bonus_basis_points,
            final_similarity_basis_points: value.evidence.final_similarity_basis_points,
            segmentation_version: value.evidence.segmentation_version,
        },
    }
}

#[cfg(feature = "ssr")]
fn map_memory_mutation(value: rustok_translation::MemoryMutationRecord) -> MemoryMutation {
    MemoryMutation {
        entry_id: value.entry_id.to_string(),
        revision: value.revision,
        state: value.state,
        retention_policy: map_memory_retention(value.retention_policy),
        retain_until: value.retain_until.map(|date| date.to_rfc3339()),
        tombstoned_at: value.tombstoned_at.map(|date| date.to_rfc3339()),
    }
}

#[cfg(feature = "ssr")]
fn map_glossary_scope(value: rustok_translation::GlossaryScope) -> GlossaryScope {
    GlossaryScope {
        owner_slug: value.owner_slug.map(|item| item.to_string()),
        resource_kind: value.resource_kind.map(|item| item.to_string()),
        field_key: value.field_key.map(|item| item.to_string()),
    }
}

#[cfg(feature = "ssr")]
fn map_job_progress(value: rustok_translation::JobProgressRecord) -> JobProgress {
    JobProgress {
        job_id: value.job_id.to_string(),
        source_digest: value.source_digest,
        total_items: value.total_items,
        assigned_items: value.assigned_items,
        terminal_items: value.terminal_items,
        missing_items: value.missing_items,
        draft_items: value.draft_items,
        in_review_items: value.in_review_items,
        approved_items: value.approved_items,
        applying_items: value.applying_items,
        applied_items: value.applied_items,
        stale_items: value.stale_items,
        conflict_items: value.conflict_items,
        blocked_items: value.blocked_items,
        excluded_items: value.excluded_items,
        cancelled_items: value.cancelled_items,
        required_units: value.required_units,
        optional_units: value.optional_units,
        applied_required_units: value.applied_required_units,
        applied_optional_units: value.applied_optional_units,
        approved_required_units: value.approved_required_units,
        approved_optional_units: value.approved_optional_units,
        complete_resources: value.complete_resources,
        source_characters: value.source_characters,
        translated_characters: value.translated_characters,
        revision: value.revision,
        updated_at: value.updated_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn map_provider_progress(value: rustok_translation::ProviderProgressRecord) -> ProviderProgress {
    ProviderProgress {
        owner_slug: value.owner_slug.to_string(),
        resource_kind: value.resource_kind.to_string(),
        source_locale: value.source_locale.as_str().to_string(),
        target_locale: value.target_locale.as_str().to_string(),
        required_units: value.facts.required_units,
        exact_required_units: value.facts.exact_required_units,
        optional_units: value.facts.optional_units,
        exact_optional_units: value.facts.exact_optional_units,
        resources: value.facts.resources,
        complete_resources: value.facts.complete_resources,
        owner_change_cursor: value.facts.owner_change_cursor.map(|item| item.to_string()),
        projected_cursor: value.projected_cursor.map(|item| item.to_string()),
        checkpoint_revision: value.checkpoint_revision,
        checkpoint_updated_at: value.checkpoint_updated_at.map(|item| item.to_rfc3339()),
        freshness: format!("{:?}", value.freshness).to_ascii_lowercase(),
    }
}

#[cfg(feature = "ssr")]
fn map_required_provider_progress(
    value: rustok_translation::RequiredProviderProgressRecord,
) -> RequiredProviderProgress {
    RequiredProviderProgress {
        owner_slug: value.owner_slug.to_string(),
        resource_kind: value.resource_kind.to_string(),
        source_locale: value.source_locale.as_str().to_string(),
        required_target_locales: locale_strings(value.required_target_locales),
        translation_policy_revision: value.translation_policy_revision,
        tenant_locale_policy_revision: value.tenant_locale_policy_revision,
        required_units: value.required_units,
        exact_required_units: value.exact_required_units,
        optional_units: value.optional_units,
        exact_optional_units: value.exact_optional_units,
        resource_locale_pairs: value.resource_locale_pairs,
        complete_resource_locale_pairs: value.complete_resource_locale_pairs,
        freshness: format!("{:?}", value.freshness).to_ascii_lowercase(),
        targets: value
            .targets
            .into_iter()
            .map(map_provider_progress)
            .collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_job(value: rustok_translation::JobRecord) -> Job {
    Job {
        id: value.id.to_string(),
        source_locale: value.source_locale.as_str().to_string(),
        target_locale: value.target_locale.as_str().to_string(),
        glossary: value.glossary.map(|binding| GlossaryBinding {
            glossary_id: binding.glossary_id.to_string(),
            revision: binding.revision,
        }),
        status: value.status,
        revision: value.revision,
    }
}

#[cfg(feature = "ssr")]
fn map_item(value: rustok_translation::JobItemRecord) -> JobItem {
    JobItem {
        id: value.id.to_string(),
        job_id: value.job_id.to_string(),
        identity: TranslationResourceIdentity {
            owner_slug: value.identity.owner_slug.to_string(),
            resource_kind: value.identity.resource_kind.to_string(),
            resource_id: value.identity.resource_id.to_string(),
            subresource_id: value.identity.subresource_id.map(|item| item.to_string()),
        },
        status: value.status,
        assignee: value.assignee.map(map_actor),
        source_digest: value.source_digest,
        revision: value.revision,
    }
}

#[cfg(feature = "ssr")]
fn map_actor(value: rustok_api::PortActor) -> Actor {
    Actor {
        kind: match value.kind {
            rustok_api::PortActorKind::User => ActorKind::User,
            rustok_api::PortActorKind::Service => ActorKind::Service,
            rustok_api::PortActorKind::System => ActorKind::Service,
        },
        id: value.id,
    }
}

#[cfg(feature = "ssr")]
fn map_proposal(value: rustok_translation::ProposalRecord) -> Proposal {
    Proposal {
        id: value.id.to_string(),
        item_id: value.item_id.to_string(),
        proposal_revision: value.proposal_revision,
        origin: format!("{:?}", value.origin).to_ascii_lowercase(),
        values: value
            .values
            .into_iter()
            .map(|field| ProposalValue {
                key: field.key.to_string(),
                value: field.value,
                expected_source_hash: field.expected_source_hash,
            })
            .collect(),
        qa_issues: value
            .qa_issues
            .into_iter()
            .map(|issue| QaIssue {
                field: issue.field.map(|field| field.to_string()),
                severity: format!("{:?}", issue.severity).to_ascii_lowercase(),
                code: issue.code,
                message: issue.message,
            })
            .collect(),
        qa_accepted: value.qa_accepted,
        status: value.status,
        approval_receipt_id: value.approval_receipt_id,
    }
}

#[cfg(feature = "ssr")]
fn map_apply(value: rustok_translation::ApplyRecord) -> ApplyResult {
    ApplyResult {
        operation_id: value.operation_id.to_string(),
        item_id: value.item_id.to_string(),
        proposal_id: value.proposal_id.to_string(),
        provider_receipt_id: value.provider_receipt_id,
        resource_revision: value.resource_revision.to_string(),
        target_revision: value.target_revision.to_string(),
        applied_field_keys: value
            .applied_field_keys
            .into_iter()
            .map(|field| field.to_string())
            .collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_assignment(value: rustok_translation::AssignmentRecord) -> Assignment {
    Assignment {
        operation_id: value.operation_id.to_string(),
        item_id: value.item_id.to_string(),
        assignee: value.assignee.map(map_actor),
        item_revision: value.item_revision,
    }
}

#[cfg(feature = "ssr")]
fn map_cancellation(value: rustok_translation::CancellationRecord) -> Cancellation {
    Cancellation {
        cancellation_id: value.cancellation_id.to_string(),
        job_id: value.job_id.to_string(),
        job_revision: value.job_revision,
        cancelled_item_count: value.cancelled_item_count,
    }
}

#[cfg(feature = "ssr")]
fn map_retry(value: rustok_translation::RetryRecord) -> Retry {
    Retry {
        retry_id: value.retry_id.to_string(),
        item_id: value.item_id.to_string(),
        item_revision: value.item_revision,
        status: value.status,
    }
}

#[cfg(feature = "ssr")]
fn map_inventory(value: rustok_translation::TranslationInventorySyncResult) -> InventoryResult {
    InventoryResult {
        observed_resources: value.observed_resources,
        checkpoint: value.checkpoint.map(|item| item.to_string()),
        checkpoint_revision: value.checkpoint_revision,
    }
}

#[cfg(feature = "ssr")]
fn map_rebuild_inventory(
    value: rustok_translation::TranslationInventoryRebuildResult,
) -> InventoryResult {
    InventoryResult {
        observed_resources: value.observed_resources,
        checkpoint: value.checkpoint.map(|item| item.to_string()),
        checkpoint_revision: value.checkpoint_revision,
    }
}

#[cfg(feature = "ssr")]
fn locale_strings(locales: Vec<rustok_api::TenantLocale>) -> Vec<String> {
    locales
        .into_iter()
        .map(|locale| locale.as_str().to_string())
        .collect()
}
