//! Native Leptos server-function adapter for the shared Translation contract.

use leptos::prelude::*;

#[cfg(feature = "ssr")]
use crate::model::MachineTranslationEstimate;
#[cfg(feature = "ssr")]
use crate::model::{
    Actor, ActorKind, ApplyResult, Assignment, Cancellation, Glossary, GlossaryBinding,
    GlossaryConcept, GlossaryMatchKind, GlossaryScope, GlossarySummary, GlossaryTermPolicy,
    GlossaryVariant, InterchangeArtifact, InterchangeArtifactContent,
    InterchangeArtifactItemOutcome, InterchangeConflictReport, InterchangeDocument,
    InterchangeField, InterchangeItem, InventoryResult, Job, JobItem, JobProgress,
    MachineCancellation, MachineOperationStatus, MachineProposal, MachineTranslationAttempt,
    MachineTranslationDiagnostic, MachineTranslationUsage, MemoryEntry, MemoryMatchEvidence,
    MemoryMatchKind, MemoryMutation, MemoryRetentionPolicy, MemorySuggestion, Proposal,
    ProposalOrigin, ProposalValue, ProviderProgress, QaIssue, RequiredProviderProgress, Retry,
    ReviewerQueueItem, ReviewerWorkload, TranslationPolicy, TranslationResourceIdentity,
    TranslationTarget, WorkflowNote,
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
    use leptos::prelude::expect_context;
    use rustok_api::{AuthContext, HostRuntimeContext, RequestContext, TenantContext};

    let auth = leptos_axum::extract::<AuthContext>()
        .await
        .map_err(ServerFnError::new)?;
    let tenant = leptos_axum::extract::<TenantContext>()
        .await
        .map_err(ServerFnError::new)?;
    let request = leptos_axum::extract::<RequestContext>()
        .await
        .map_err(ServerFnError::new)?;
    let runtime = expect_context::<HostRuntimeContext>();
    execute_with_runtime(operation, &auth, &tenant, &request, &runtime).await
}

#[cfg(feature = "ssr")]
async fn execute_with_runtime(
    operation: TranslationAdminOperation,
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    request: &rustok_api::RequestContext,
    runtime: &rustok_api::HostRuntimeContext,
) -> Result<TranslationAdminResponse, ServerFnError> {
    use std::sync::Arc;

    use rustok_outbox::TransactionalEventBus;
    use rustok_tenant::{TenantLocalePolicyPort, TenantService};
    use rustok_translation_targets::TranslationTargetRegistry;

    if auth.tenant_id != tenant.id || request.tenant_id != tenant.id {
        return Err(ServerFnError::new(
            "Authenticated tenant does not match request tenant",
        ));
    }

    let database = runtime.db_clone();
    let providers = runtime
        .shared_get::<Arc<TranslationTargetRegistry>>()
        .unwrap_or_else(|| Arc::new(TranslationTargetRegistry::default()));
    let event_bus = runtime
        .shared_get::<TransactionalEventBus>()
        .ok_or_else(|| ServerFnError::new("Translation runtime is unavailable"))?;
    let tenant_locale_policies = runtime
        .shared_get::<Arc<dyn TenantLocalePolicyPort>>()
        .unwrap_or_else(|| Arc::new(TenantService::new(database.clone())));
    let storage = runtime.shared_get::<rustok_storage::StorageRuntime>();
    let mut context = port_context(auth, request, operation.idempotency_key())?;
    if matches!(
        &operation,
        TranslationAdminOperation::EstimateMachineTranslation { .. }
            | TranslationAdminOperation::GenerateMachineProposal { .. }
            | TranslationAdminOperation::RecoverMachineOperation { .. }
    ) {
        context.deadline_ms = Some(120_000);
    }
    let machine_port = if matches!(
        &operation,
        TranslationAdminOperation::EstimateMachineTranslation { .. }
            | TranslationAdminOperation::GenerateMachineProposal { .. }
            | TranslationAdminOperation::CancelMachineOperation { .. }
            | TranslationAdminOperation::RecoverMachineOperation { .. }
            | TranslationAdminOperation::ReadMachineOperationStatus { .. }
    ) {
        rustok_translation::machine_translation_port_from_context(runtime)
            .map_err(|error| ServerFnError::new(error.message))?
    } else {
        None
    };

    dispatch(
        operation,
        context,
        TranslationNativeDependencies {
            database,
            providers,
            tenant_locale_policies,
            event_bus,
            machine_port,
            storage,
        },
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
struct TranslationNativeDependencies {
    database: sea_orm::DatabaseConnection,
    providers: std::sync::Arc<rustok_translation_targets::TranslationTargetRegistry>,
    tenant_locale_policies: std::sync::Arc<dyn rustok_tenant::TenantLocalePolicyPort>,
    event_bus: rustok_outbox::TransactionalEventBus,
    machine_port: Option<std::sync::Arc<dyn rustok_translation::MachineTranslationPort>>,
    storage: Option<rustok_storage::StorageRuntime>,
}

#[cfg(feature = "ssr")]
async fn dispatch(
    operation: TranslationAdminOperation,
    context: rustok_api::PortContext,
    dependencies: TranslationNativeDependencies,
) -> Result<TranslationAdminResponse, ServerFnError> {
    use rustok_translation::{
        AddItemInput, ApplyProposalInput, ApproveProposalInput, AssignItemInput, CancelJobInput,
        CancelMachineOperationInput, CreateGlossaryInput, CreateInterchangeExportArtifactInput,
        CreateJobInput, CreateWorkflowNoteInput, ExportTranslationJobInput,
        GenerateMachineProposalInput, ImportTranslationItemInput, ListInterchangeArtifactsInput,
        ListWorkflowNotesInput, MemoryListInput, MemoryLookupInput,
        ProcessInterchangeImportArtifactInput, ProposalValue, PurgeMemoryEntryInput,
        ReadInterchangeArtifactInput, RecoverApplyInput, RecoverMachineOperationInput,
        ReplaceGlossaryTermsInput, ReplaceRequiredTargetLocalesInput, ResolveWorkflowNoteInput,
        RetryItemInput, ReviewerQueueInput, ReviewerWorkloadInput, SaveProposalInput,
        SetGlossaryActiveInput, SetMemoryRetentionInput, StoreInterchangeImportArtifactInput,
        SubmitProposalInput, TombstoneMemoryEntryInput, TranslationExchangeService,
        TranslationGlossaryService, TranslationInventoryService, TranslationMachineControlService,
        TranslationMachineService, TranslationMemoryService, TranslationPolicyService,
        TranslationProgressService, TranslationWorkflowService, UnassignItemInput,
        UpdateGlossaryInput,
    };

    let TranslationNativeDependencies {
        database,
        providers,
        tenant_locale_policies,
        event_bus,
        machine_port,
        storage,
    } = dependencies;

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
    let interchange = || workflow().interchange_service();
    let exchange = || {
        storage
            .clone()
            .map(|storage| {
                TranslationExchangeService::new(
                    database.clone(),
                    providers.clone(),
                    tenant_locale_policies.clone(),
                    event_bus.clone(),
                    storage,
                )
            })
            .ok_or_else(|| ServerFnError::new("Translation interchange storage is unavailable"))
    };
    let collaboration = || workflow().collaboration_service();
    let machine = || {
        machine_port
            .as_ref()
            .cloned()
            .map(|port| {
                TranslationMachineService::new(
                    database.clone(),
                    providers.clone(),
                    tenant_locale_policies.clone(),
                    event_bus.clone(),
                    port,
                )
            })
            .ok_or_else(|| ServerFnError::new("Machine translation provider is unavailable"))
    };
    let machine_control =
        || TranslationMachineControlService::new(database.clone(), machine_port.clone());

    let response = match operation {
        TranslationAdminOperation::ReadPolicy => TranslationAdminResponse::Policy(map_policy(
            policy().read_policy(context).await.map_err(public_error)?,
        )),
        TranslationAdminOperation::ReadMachineOperationStatus { operation_id } => {
            TranslationAdminResponse::MachineOperationStatus(map_machine_operation_status(
                machine_control()
                    .operation_status(context, parse_uuid(&operation_id, "operation_id")?)
                    .await
                    .map_err(public_error)?,
            ))
        }
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
        TranslationAdminOperation::ReadReviewerQueue {
            job_id,
            assignee,
            include_unassigned,
            limit,
        } => TranslationAdminResponse::ReviewerQueue(
            progress()
                .list_reviewer_queue(
                    context,
                    ReviewerQueueInput {
                        job_id: parse_uuid(&job_id, "job_id")?,
                        assignee: assignee.map(map_actor_input),
                        include_unassigned,
                        limit,
                    },
                )
                .await
                .map_err(public_error)?
                .into_iter()
                .map(map_reviewer_queue_item)
                .collect(),
        ),
        TranslationAdminOperation::ReadReviewerWorkload { job_id } => {
            TranslationAdminResponse::ReviewerWorkloads(
                progress()
                    .list_reviewer_workload(
                        context,
                        ReviewerWorkloadInput {
                            job_id: parse_uuid(&job_id, "job_id")?,
                        },
                    )
                    .await
                    .map_err(public_error)?
                    .into_iter()
                    .map(map_reviewer_workload)
                    .collect(),
            )
        }
        TranslationAdminOperation::ListWorkflowNotes {
            job_id,
            item_id,
            include_resolved,
            limit,
        } => TranslationAdminResponse::WorkflowNotes(
            collaboration()
                .list_workflow_notes(
                    context,
                    ListWorkflowNotesInput {
                        job_id: parse_uuid(&job_id, "job_id")?,
                        item_id: item_id
                            .map(|value| parse_uuid(&value, "item_id"))
                            .transpose()?,
                        include_resolved,
                        limit,
                    },
                )
                .await
                .map_err(public_error)?
                .into_iter()
                .map(map_workflow_note)
                .collect(),
        ),
        TranslationAdminOperation::ExportJob { job_id, max_items } => {
            TranslationAdminResponse::InterchangeDocument(map_interchange_document(
                interchange()
                    .export_job(
                        context,
                        ExportTranslationJobInput {
                            job_id: parse_uuid(&job_id, "job_id")?,
                            max_items,
                        },
                    )
                    .await
                    .map_err(public_error)?,
            ))
        }
        TranslationAdminOperation::ListInterchangeArtifacts {
            job_id,
            include_expired,
            limit,
        } => TranslationAdminResponse::InterchangeArtifacts(
            exchange()?
                .list_artifacts(
                    context,
                    ListInterchangeArtifactsInput {
                        job_id: job_id
                            .map(|value| parse_uuid(&value, "job_id"))
                            .transpose()?,
                        include_expired,
                        limit,
                    },
                )
                .await
                .map_err(public_error)?
                .into_iter()
                .map(map_interchange_artifact)
                .collect(),
        ),
        TranslationAdminOperation::ReadInterchangeArtifact { artifact_id } => {
            TranslationAdminResponse::InterchangeArtifactContent(map_interchange_artifact_content(
                exchange()?
                    .read_artifact(
                        context,
                        ReadInterchangeArtifactInput {
                            artifact_id: parse_uuid(&artifact_id, "interchange_artifact_id")?,
                        },
                    )
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
        TranslationAdminOperation::CreateWorkflowNote {
            job_id,
            item_id,
            body,
            ..
        } => TranslationAdminResponse::WorkflowNote(map_workflow_note(
            collaboration()
                .create_workflow_note(
                    context,
                    CreateWorkflowNoteInput {
                        job_id: parse_uuid(&job_id, "job_id")?,
                        item_id: item_id
                            .map(|value| parse_uuid(&value, "item_id"))
                            .transpose()?,
                        body,
                    },
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::ResolveWorkflowNote {
            note_id,
            expected_revision,
            ..
        } => TranslationAdminResponse::WorkflowNote(map_workflow_note(
            collaboration()
                .resolve_workflow_note(
                    context,
                    ResolveWorkflowNoteInput {
                        note_id: parse_uuid(&note_id, "workflow_note_id")?,
                        expected_revision,
                    },
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::CreateInterchangeExportArtifact {
            job_id,
            max_items,
            expires_in_seconds,
            ..
        } => TranslationAdminResponse::InterchangeArtifact(map_interchange_artifact(
            exchange()?
                .create_export_artifact(
                    context,
                    CreateInterchangeExportArtifactInput {
                        job_id: parse_uuid(&job_id, "job_id")?,
                        max_items,
                        expires_in_seconds,
                    },
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::StoreInterchangeImportArtifact {
            job_id,
            document_json,
            expires_in_seconds,
            ..
        } => TranslationAdminResponse::InterchangeArtifact(map_interchange_artifact(
            exchange()?
                .store_import_artifact(
                    context,
                    StoreInterchangeImportArtifactInput {
                        job_id: parse_uuid(&job_id, "job_id")?,
                        document: rustok_translation::parse_artifact_document(&document_json)
                            .map_err(public_error)?,
                        expires_in_seconds,
                    },
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::ProcessInterchangeImportArtifact { artifact_id, .. } => {
            TranslationAdminResponse::InterchangeArtifact(map_interchange_artifact(
                exchange()?
                    .process_import_artifact(
                        context,
                        ProcessInterchangeImportArtifactInput {
                            artifact_id: parse_uuid(&artifact_id, "interchange_artifact_id")?,
                        },
                    )
                    .await
                    .map_err(public_error)?,
            ))
        }
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
        TranslationAdminOperation::ImportItem {
            schema_version,
            job_id,
            item_id,
            identity,
            source_digest,
            values,
            ..
        } => TranslationAdminResponse::Proposal(map_proposal(
            interchange()
                .import_item(
                    context,
                    ImportTranslationItemInput {
                        schema_version,
                        job_id: parse_uuid(&job_id, "job_id")?,
                        item_id: parse_uuid(&item_id, "item_id")?,
                        identity: parse_identity(identity)?,
                        source_digest,
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
        TranslationAdminOperation::EstimateMachineTranslation {
            item_id,
            field_keys,
            minimum_memory_similarity_basis_points,
            tone,
            domain,
            style,
            ..
        } => TranslationAdminResponse::MachineEstimate(map_machine_estimate(
            machine()?
                .estimate_proposal(
                    context,
                    GenerateMachineProposalInput {
                        item_id: parse_uuid(&item_id, "item_id")?,
                        field_keys: field_keys
                            .into_iter()
                            .map(parse_field_key)
                            .collect::<Result<Vec<_>, _>>()?,
                        minimum_memory_similarity_basis_points,
                        tone,
                        domain,
                        style,
                    },
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::GenerateMachineProposal {
            item_id,
            field_keys,
            minimum_memory_similarity_basis_points,
            tone,
            domain,
            style,
            ..
        } => map_machine_proposal_outcome(
            machine()?
                .generate_proposal(
                    context,
                    GenerateMachineProposalInput {
                        item_id: parse_uuid(&item_id, "item_id")?,
                        field_keys: field_keys
                            .into_iter()
                            .map(parse_field_key)
                            .collect::<Result<Vec<_>, ServerFnError>>()?,
                        minimum_memory_similarity_basis_points,
                        tone,
                        domain,
                        style,
                    },
                )
                .await
                .map_err(public_error)?,
        ),
        TranslationAdminOperation::CancelMachineOperation {
            operation_id,
            reason,
            ..
        } => TranslationAdminResponse::MachineCancellation(map_machine_cancellation(
            machine_control()
                .cancel_operation(
                    context,
                    CancelMachineOperationInput {
                        operation_id: parse_uuid(&operation_id, "operation_id")?,
                        reason,
                    },
                )
                .await
                .map_err(public_error)?,
        )),
        TranslationAdminOperation::RecoverMachineOperation {
            operation_id,
            expected_updated_at,
            item_id,
            field_keys,
            minimum_memory_similarity_basis_points,
            tone,
            domain,
            style,
            reason,
            ..
        } => TranslationAdminResponse::MachineProposal(map_machine_proposal(
            machine()?
                .recover_operation(
                    context,
                    RecoverMachineOperationInput {
                        operation_id: parse_uuid(&operation_id, "operation_id")?,
                        expected_updated_at: chrono::DateTime::parse_from_rfc3339(
                            &expected_updated_at,
                        )
                        .map_err(|_| {
                            ServerFnError::new("expected_updated_at must be an RFC 3339 timestamp")
                        })?,
                        proposal: GenerateMachineProposalInput {
                            item_id: parse_uuid(&item_id, "item_id")?,
                            field_keys: field_keys
                                .into_iter()
                                .map(parse_field_key)
                                .collect::<Result<Vec<_>, ServerFnError>>()?,
                            minimum_memory_similarity_basis_points,
                            tone,
                            domain,
                            style,
                        },
                        reason,
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
fn map_memory_retention_input(value: MemoryRetentionPolicy) -> rustok_core::RetentionPolicy {
    match value {
        MemoryRetentionPolicy::OwnerLifecycle => rustok_core::RetentionPolicy::OwnerLifecycle,
        MemoryRetentionPolicy::RetainUntil => rustok_core::RetentionPolicy::RetainUntil,
        MemoryRetentionPolicy::LegalHold => rustok_core::RetentionPolicy::LegalHold,
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
fn map_memory_retention(value: rustok_core::RetentionPolicy) -> MemoryRetentionPolicy {
    match value {
        rustok_core::RetentionPolicy::OwnerLifecycle => MemoryRetentionPolicy::OwnerLifecycle,
        rustok_core::RetentionPolicy::RetainUntil => MemoryRetentionPolicy::RetainUntil,
        rustok_core::RetentionPolicy::LegalHold => MemoryRetentionPolicy::LegalHold,
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
fn map_interchange_document(
    value: rustok_translation::TranslationInterchangeDocument,
) -> InterchangeDocument {
    InterchangeDocument {
        schema_version: value.schema_version,
        job_id: value.job_id.to_string(),
        source_locale: value.source_locale.as_str().to_string(),
        target_locale: value.target_locale.as_str().to_string(),
        items: value
            .items
            .into_iter()
            .map(|item| InterchangeItem {
                item_id: item.item_id.to_string(),
                identity: TranslationResourceIdentity {
                    owner_slug: item.identity.owner_slug.to_string(),
                    resource_kind: item.identity.resource_kind.to_string(),
                    resource_id: item.identity.resource_id.to_string(),
                    subresource_id: item.identity.subresource_id.map(|id| id.to_string()),
                },
                source_digest: item.source_digest,
                source_revision: item.source_revision.to_string(),
                target_revision: item.target_revision.map(|revision| revision.to_string()),
                fields: item
                    .fields
                    .into_iter()
                    .map(|field| InterchangeField {
                        key: field.key.to_string(),
                        source_value: field.source_value,
                        exact_target_value: field.exact_target_value,
                        proposed_value: field.proposed_value,
                        source_hash: field.source_hash,
                        required: field.required,
                        max_characters: field.max_characters,
                        protected_tokens: field.protected_tokens,
                    })
                    .collect(),
            })
            .collect(),
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
fn map_reviewer_queue_item(value: rustok_translation::ReviewerQueueRecord) -> ReviewerQueueItem {
    ReviewerQueueItem {
        item: map_item(value.item),
        proposal_id: value.proposal_id.to_string(),
        proposal_revision: value.proposal_revision,
        submitted_at: value.submitted_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn map_reviewer_workload(value: rustok_translation::ReviewerWorkloadRecord) -> ReviewerWorkload {
    ReviewerWorkload {
        job_id: value.job_id.to_string(),
        assignee: value.assignee.map(map_actor),
        open_items: value.open_items,
        missing_items: value.missing_items,
        draft_items: value.draft_items,
        in_review_items: value.in_review_items,
        approved_items: value.approved_items,
        applying_items: value.applying_items,
        rebase_required_items: value.rebase_required_items,
        blocked_items: value.blocked_items,
        source_characters: value.source_characters,
    }
}

#[cfg(feature = "ssr")]
fn map_interchange_artifact(
    value: rustok_translation::TranslationInterchangeArtifactRecord,
) -> InterchangeArtifact {
    InterchangeArtifact {
        id: value.id.to_string(),
        job_id: value.job_id.to_string(),
        direction: value.direction.as_str().to_string(),
        status: value.status.as_str().to_string(),
        content_length: value.content_length,
        checksum_sha256: value.checksum_sha256,
        expires_at: value.expires_at.to_rfc3339(),
        processed_at: value.processed_at.map(|value| value.to_rfc3339()),
        report: value.report.map(|report| InterchangeConflictReport {
            total_items: report.total_items,
            accepted_items: report.accepted_items,
            conflict_items: report.conflict_items,
            rejected_items: report.rejected_items,
            outcomes: report
                .outcomes
                .into_iter()
                .map(|outcome| InterchangeArtifactItemOutcome {
                    item_id: outcome.item_id.to_string(),
                    status: outcome.status,
                })
                .collect(),
        }),
        created_at: value.created_at.to_rfc3339(),
        updated_at: value.updated_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn map_interchange_artifact_content(
    value: rustok_translation::TranslationInterchangeArtifactContent,
) -> InterchangeArtifactContent {
    InterchangeArtifactContent {
        artifact: map_interchange_artifact(value.artifact),
        document: map_interchange_document(value.document),
    }
}

#[cfg(feature = "ssr")]
fn map_workflow_note(value: rustok_translation::WorkflowNoteRecord) -> WorkflowNote {
    WorkflowNote {
        id: value.id.to_string(),
        job_id: value.job_id.to_string(),
        item_id: value.item_id.map(|item_id| item_id.to_string()),
        body: value.body,
        author: map_actor(value.author),
        revision: value.revision,
        resolved_at: value.resolved_at.map(|timestamp| timestamp.to_rfc3339()),
        resolved_by: value.resolved_by.map(map_actor),
        created_at: value.created_at.to_rfc3339(),
        updated_at: value.updated_at.to_rfc3339(),
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
fn map_machine_proposal(value: rustok_translation::MachineProposalRecord) -> MachineProposal {
    MachineProposal {
        operation_id: value.operation_id.to_string(),
        item_id: value.item_id.to_string(),
        proposal_id: value.proposal_id.to_string(),
        adapter_slug: value.adapter_slug,
        provider_slug: value.provider_slug,
        provider_policy_digest: value.provider_policy_digest,
        machine_request_digest: value.machine_request_digest,
        glossary_revision: value.glossary_revision,
        glossary_digest: value.glossary_digest,
        memory_digest: value.memory_digest,
        execution_id: value.execution_id,
        execution_request_digest: value.execution_request_digest,
        prompt_policy_digest: value.prompt_policy_digest,
        attempts: value
            .attempts
            .into_iter()
            .map(|attempt| MachineTranslationAttempt {
                attempt: attempt.attempt,
                provider_profile_id: attempt.provider_profile_id,
                provider_slug: attempt.provider_slug,
                model: attempt.model,
                fallback: attempt.fallback,
            })
            .collect(),
        usage: MachineTranslationUsage {
            input_tokens: value.usage.input_tokens,
            output_tokens: value.usage.output_tokens,
            total_tokens: value.usage.total_tokens,
            cost_minor_units: value.usage.cost_minor_units,
            currency_code: value.usage.currency_code,
            price_snapshot_digest: value.usage.price_snapshot_digest,
        },
        diagnostics: value
            .diagnostics
            .into_iter()
            .map(|diagnostic| MachineTranslationDiagnostic {
                code: diagnostic.code,
                blocking: diagnostic.blocking,
                unit_id: diagnostic.unit_id,
            })
            .collect(),
        review_required: value.review_required,
        created_at: value.created_at.to_rfc3339(),
        updated_at: value.updated_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn map_machine_proposal_outcome(
    value: rustok_translation::MachineProposalOutcome,
) -> TranslationAdminResponse {
    match value {
        rustok_translation::MachineProposalOutcome::Completed(proposal) => {
            TranslationAdminResponse::MachineProposal(map_machine_proposal(*proposal))
        }
        rustok_translation::MachineProposalOutcome::InProgress(status) => {
            TranslationAdminResponse::MachineOperationStatus(map_machine_operation_status(status))
        }
    }
}

#[cfg(feature = "ssr")]
fn map_machine_cancellation(
    value: rustok_translation::MachineCancellationRecord,
) -> MachineCancellation {
    MachineCancellation {
        cancellation_id: value.cancellation_id.to_string(),
        operation_id: value.operation_id.to_string(),
        status: value.status,
        provider_execution_id: value.provider_execution_id,
        provider_status: value.provider_status,
        provider_error_code: value.provider_error_code,
        provider_observed_at: value.provider_observed_at.to_rfc3339(),
        created_at: value.created_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn map_machine_operation_status(
    value: rustok_translation::MachineOperationStatusRecord,
) -> MachineOperationStatus {
    MachineOperationStatus {
        operation_id: value.operation_id.to_string(),
        item_id: value.item_id.to_string(),
        status: value.status,
        provider_execution_id: value.provider_execution_id,
        provider_status: value.provider_status,
        provider_error_code: value.provider_error_code,
        updated_at: value.updated_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn map_machine_estimate(
    value: rustok_translation::MachineTranslationEstimate,
) -> MachineTranslationEstimate {
    MachineTranslationEstimate {
        input_tokens_upper_bound: value.input_tokens_upper_bound,
        output_tokens_upper_bound: value.output_tokens_upper_bound,
        attempts_upper_bound: value.attempts_upper_bound,
        cost_minor_units_upper_bound: value.cost_minor_units_upper_bound,
        currency_code: value.currency_code,
        price_snapshot_digest: value.price_snapshot_digest,
        review_required: value.review_required,
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

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    use async_trait::async_trait;
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode, header::CONTENT_TYPE},
        response::IntoResponse,
    };
    use leptos::{prelude::provide_context, server_fn::ServerFn};
    use rustok_api::{
        Action, AuthContext, AuthContextExtension, HostRuntimeContext, Permission, PortContext,
        PortError, RequestContext, Resource, TenantContext, TenantContextExtension, TenantLocale,
    };
    use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
    use rustok_storage::{LocalStorageConfig, StorageRuntime};
    use rustok_tenant::{
        ReplaceTenantLocalePolicyRequest, TenantLocalePolicyEntry, TenantLocalePolicyPort,
        TenantLocalePolicyProjection,
    };
    use rustok_translation::{
        MachineTranslationAttemptEvidence, MachineTranslationBatchExecution,
        MachineTranslationBatchRequest, MachineTranslationBatchResult,
        MachineTranslationDiagnostic, MachineTranslationEstimate,
        MachineTranslationExecutionEvidence, MachineTranslationExecutionStatus,
        MachineTranslationExecutionStatusEvidence, MachineTranslationPort,
        MachineTranslationPortFactory, MachineTranslationProviderDescriptor,
        MachineTranslationProviderHealth, MachineTranslationProviderState,
        MachineTranslationUnitResult, MachineTranslationUsage, SharedMachineTranslationPortFactory,
        entities::{apply_operation, job_item, machine_operation, proposal},
    };
    use rustok_translation_targets::{
        FieldKey, ListTranslationResourcesRequest, OpaqueCursor, OpaqueRevision, OwnerSlug,
        ReadTranslationResourceRequest, ResourceId, ResourceKind, TranslationApplicationReceipt,
        TranslationDataClassification, TranslationFieldDescriptor, TranslationFieldSnapshot,
        TranslationPatchRequest, TranslationPatchValidation, TranslationResourceLifecycle,
        TranslationResourcePage, TranslationResourceSnapshot, TranslationResourceSummary,
        TranslationStrategy, TranslationTargetCapability, TranslationTargetChange,
        TranslationTargetChangePage, TranslationTargetChangesRequest,
        TranslationTargetProgressFacts, TranslationTargetProgressRequest,
        TranslationTargetProvider, TranslationTargetProviderDescriptor, TranslationTargetRegistry,
        TranslationValueProfile,
    };
    use sea_orm::{
        ColumnTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait,
        QueryFilter, Statement,
    };
    use sea_orm_migration::{MigrationTrait, SchemaManager};
    use tempfile::TempDir;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    use super::execute_with_runtime;
    use crate::model::{
        Actor, ActorKind, GlossaryConcept, GlossaryMatchKind, GlossaryScope, GlossaryTermPolicy,
        GlossaryVariant, Job, JobItem, MemoryRetentionPolicy, Proposal, ProposalOrigin,
        ProposalValueInput, TranslationAdminOperation, TranslationAdminResponse,
        TranslationResourceIdentity,
    };

    #[derive(Default)]
    struct NativeProviderState {
        fail_after_commit: AtomicBool,
        next_error: Mutex<Option<PortError>>,
        receipts: Mutex<BTreeMap<String, TranslationApplicationReceipt>>,
    }

    struct NativeSnapshotProvider {
        state: Arc<NativeProviderState>,
    }

    struct NativeMachinePort {
        descriptor: MachineTranslationProviderDescriptor,
        health: Mutex<MachineTranslationProviderState>,
        execution_status: Mutex<MachineTranslationExecutionStatus>,
    }

    struct NativeMachinePortFactory {
        port: Arc<NativeMachinePort>,
    }

    struct NativeTenantLocalePolicies;

    impl NativeMachinePort {
        fn new() -> Self {
            Self {
                descriptor: MachineTranslationProviderDescriptor {
                    slug: "native-machine".to_string(),
                    display_name: "Native machine fixture".to_string(),
                    policy_digest: "b".repeat(64),
                    supported_profiles: vec![TranslationValueProfile::PlainText],
                    supported_classifications: vec![TranslationDataClassification::Public],
                    max_batch_units: 100,
                    max_batch_characters: 10_000,
                    review_required: true,
                },
                health: Mutex::new(MachineTranslationProviderState::Available),
                execution_status: Mutex::new(MachineTranslationExecutionStatus::Completed),
            }
        }

        async fn set_health(&self, state: MachineTranslationProviderState) {
            *self.health.lock().await = state;
        }

        async fn set_execution_status(&self, status: MachineTranslationExecutionStatus) {
            *self.execution_status.lock().await = status;
        }

        fn result(request: &MachineTranslationBatchRequest) -> MachineTranslationBatchResult {
            MachineTranslationBatchResult {
                provider_slug: "native-provider".to_string(),
                units: request
                    .units
                    .iter()
                    .map(|unit| MachineTranslationUnitResult {
                        unit_id: unit.unit_id.clone(),
                        translated_value: "Held".to_string(),
                        protected_tokens: unit.protected_tokens.clone(),
                        diagnostics: vec![MachineTranslationDiagnostic {
                            code: "translation.machine.review_required".to_string(),
                            blocking: false,
                            unit_id: Some(unit.unit_id.clone()),
                        }],
                    })
                    .collect(),
                execution: MachineTranslationExecutionEvidence {
                    execution_id: "native-execution".to_string(),
                    request_digest: "c".repeat(64),
                    prompt_policy_digest: request.adapter_policy_digest.clone(),
                    attempts: vec![MachineTranslationAttemptEvidence {
                        attempt: 1,
                        provider_profile_id: "native-profile".to_string(),
                        provider_slug: "native-provider".to_string(),
                        model: "native-model".to_string(),
                        fallback: false,
                    }],
                    usage: MachineTranslationUsage {
                        input_tokens: 4,
                        output_tokens: 2,
                        total_tokens: 6,
                        cost_minor_units: 1,
                        currency_code: "USD".to_string(),
                        price_snapshot_digest: "e".repeat(64),
                    },
                },
                review_required: true,
            }
        }
    }

    #[async_trait]
    impl MachineTranslationPort for NativeMachinePort {
        fn descriptor(&self) -> &MachineTranslationProviderDescriptor {
            &self.descriptor
        }

        async fn health(
            &self,
            _context: PortContext,
        ) -> Result<MachineTranslationProviderHealth, PortError> {
            let state = *self.health.lock().await;
            Ok(MachineTranslationProviderHealth {
                state,
                reason_code: (state == MachineTranslationProviderState::Unavailable)
                    .then(|| "translation.machine.test_unavailable".to_string()),
                retry_after_ms: (state != MachineTranslationProviderState::Available)
                    .then_some(1_000),
            })
        }

        async fn estimate_batch(
            &self,
            context: PortContext,
            request: MachineTranslationBatchRequest,
        ) -> Result<MachineTranslationEstimate, PortError> {
            request.validate(&context)?;
            Ok(MachineTranslationEstimate {
                input_tokens_upper_bound: 64,
                output_tokens_upper_bound: 1_048_576,
                attempts_upper_bound: 1,
                cost_minor_units_upper_bound: 1,
                currency_code: "USD".to_string(),
                price_snapshot_digest: "e".repeat(64),
                review_required: true,
            })
        }

        async fn translate_batch(
            &self,
            context: PortContext,
            request: MachineTranslationBatchRequest,
        ) -> Result<MachineTranslationBatchExecution, PortError> {
            request.validate(&context)?;
            let status = *self.execution_status.lock().await;
            if matches!(
                status,
                MachineTranslationExecutionStatus::Queued
                    | MachineTranslationExecutionStatus::Running
            ) {
                return Ok(MachineTranslationBatchExecution::InProgress(
                    MachineTranslationExecutionStatusEvidence {
                        execution_id: Some("native-execution".to_string()),
                        status,
                    },
                ));
            }
            Ok(MachineTranslationBatchExecution::Completed(Self::result(
                &request,
            )))
        }

        async fn execution_status(
            &self,
            _context: PortContext,
            _execution_idempotency_key: String,
        ) -> Result<MachineTranslationExecutionStatusEvidence, PortError> {
            Ok(MachineTranslationExecutionStatusEvidence {
                execution_id: Some("native-execution".to_string()),
                status: MachineTranslationExecutionStatus::Completed,
            })
        }

        async fn recover_batch(
            &self,
            context: PortContext,
            _execution_idempotency_key: String,
            request: MachineTranslationBatchRequest,
        ) -> Result<Option<MachineTranslationBatchResult>, PortError> {
            request.validate(&context)?;
            Ok(Some(Self::result(&request)))
        }

        async fn cancel_execution(
            &self,
            _context: PortContext,
            _execution_idempotency_key: String,
        ) -> Result<MachineTranslationExecutionStatusEvidence, PortError> {
            Ok(MachineTranslationExecutionStatusEvidence {
                execution_id: Some("native-execution".to_string()),
                status: MachineTranslationExecutionStatus::CancellationRequested,
            })
        }
    }

    impl MachineTranslationPortFactory for NativeMachinePortFactory {
        fn create(
            &self,
            _context: &HostRuntimeContext,
        ) -> Result<Option<Arc<dyn MachineTranslationPort>>, PortError> {
            let port: Arc<dyn MachineTranslationPort> = self.port.clone();
            Ok(Some(port))
        }
    }

    #[async_trait]
    impl TenantLocalePolicyPort for NativeTenantLocalePolicies {
        async fn read_locale_policy(
            &self,
            context: PortContext,
        ) -> Result<TenantLocalePolicyProjection, PortError> {
            let tenant_id = Uuid::parse_str(&context.tenant_id).map_err(|error| {
                PortError::validation("translation.test_tenant", error.to_string())
            })?;
            Ok(TenantLocalePolicyProjection {
                tenant_id,
                revision: 7,
                default_locale: TenantLocale::new("en").expect("valid locale"),
                locales: ["en", "de", "fr"]
                    .into_iter()
                    .map(|locale| TenantLocalePolicyEntry {
                        locale: TenantLocale::new(locale).expect("valid locale"),
                        name: locale.to_string(),
                        native_name: locale.to_string(),
                        is_default: locale == "en",
                        is_enabled: true,
                        fallback_locale: (locale != "en")
                            .then(|| TenantLocale::new("en").expect("valid locale")),
                    })
                    .collect(),
            })
        }

        async fn replace_locale_policy(
            &self,
            _context: PortContext,
            _request: ReplaceTenantLocalePolicyRequest,
        ) -> Result<TenantLocalePolicyProjection, PortError> {
            Err(unavailable())
        }
    }

    #[async_trait]
    impl TranslationTargetProvider for NativeSnapshotProvider {
        fn descriptor(&self) -> TranslationTargetProviderDescriptor {
            TranslationTargetProviderDescriptor {
                owner_slug: OwnerSlug::new("media").expect("valid owner slug"),
                resource_kind: ResourceKind::new("asset").expect("valid resource kind"),
                display_name: "Media asset metadata".to_string(),
                capabilities: BTreeSet::from([
                    TranslationTargetCapability::ListResources,
                    TranslationTargetCapability::ReadExactResource,
                    TranslationTargetCapability::AggregateProgress,
                    TranslationTargetCapability::ValidatePatch,
                    TranslationTargetCapability::ApplyPatch,
                    TranslationTargetCapability::ChangeCursor,
                ]),
                read_permission_floor: BTreeSet::from(["media:read".to_string()]),
                apply_permission_floor: BTreeSet::from(["media:update".to_string()]),
            }
        }

        async fn list_resources(
            &self,
            _context: PortContext,
            request: ListTranslationResourcesRequest,
        ) -> Result<TranslationResourcePage, PortError> {
            request.validate().map_err(|error| {
                PortError::validation("translation.test_list", error.to_string())
            })?;
            Ok(TranslationResourcePage {
                resources: vec![resource_summary("hero", request.source_locale)],
                next_cursor: None,
            })
        }

        async fn read_resource(
            &self,
            _context: PortContext,
            request: ReadTranslationResourceRequest,
        ) -> Result<TranslationResourceSnapshot, PortError> {
            Ok(TranslationResourceSnapshot {
                summary: TranslationResourceSummary {
                    identity: request.identity,
                    display_label: "Hero".to_string(),
                    lifecycle: TranslationResourceLifecycle::Active,
                    resource_revision: OpaqueRevision::new("resource-7").expect("valid revision"),
                    exact_locales: vec![request.source_locale.clone()],
                },
                source_locale: request.source_locale,
                target_locale: request.target_locale,
                rendered_fallback_locale: None,
                source_revision: OpaqueRevision::new("source-3").expect("valid revision"),
                target_revision: None,
                fields: vec![TranslationFieldSnapshot {
                    descriptor: TranslationFieldDescriptor {
                        key: FieldKey::new("title").expect("valid field key"),
                        profile: TranslationValueProfile::PlainText,
                        strategy: TranslationStrategy::Translate,
                        classification: TranslationDataClassification::Public,
                        required: true,
                        ai_export_allowed: true,
                        max_characters: Some(200),
                        preserves_whitespace: false,
                    },
                    source_value: "Hero".to_string(),
                    exact_target_value: None,
                    source_hash: "a".repeat(64),
                    protected_tokens: Vec::new(),
                }],
            })
        }

        async fn validate_patch(
            &self,
            _context: PortContext,
            request: TranslationPatchRequest,
        ) -> Result<TranslationPatchValidation, PortError> {
            request.validate().map_err(|error| {
                PortError::validation("translation.test_patch", error.to_string())
            })?;
            Ok(TranslationPatchValidation {
                accepted: true,
                issues: Vec::new(),
            })
        }

        async fn apply_patch(
            &self,
            context: PortContext,
            request: TranslationPatchRequest,
        ) -> Result<TranslationApplicationReceipt, PortError> {
            context.require_write_semantics()?;
            request.validate().map_err(|error| {
                PortError::validation("translation.test_patch", error.to_string())
            })?;
            if let Some(error) = self.state.next_error.lock().await.take() {
                return Err(error);
            }
            let idempotency_key = context.idempotency_key.as_deref().unwrap_or("missing");
            let mut receipts = self.state.receipts.lock().await;
            if let Some(existing) = receipts.get(idempotency_key) {
                return Ok(existing.clone());
            }
            let receipt = TranslationApplicationReceipt {
                provider_receipt_id: format!("native-provider:{}", idempotency_key),
                resource_revision: OpaqueRevision::new("resource-8").expect("valid revision"),
                target_revision: OpaqueRevision::new("target-1").expect("valid revision"),
                applied_field_keys: request.fields.into_iter().map(|field| field.key).collect(),
            };
            receipts.insert(idempotency_key.to_string(), receipt.clone());
            drop(receipts);
            if self.state.fail_after_commit.swap(false, Ordering::SeqCst) {
                return Err(PortError::timeout(
                    "translation.test_unknown_outcome",
                    "owner committed but the response was lost",
                ));
            }
            Ok(receipt)
        }

        async fn read_progress(
            &self,
            _context: PortContext,
            request: TranslationTargetProgressRequest,
        ) -> Result<TranslationTargetProgressFacts, PortError> {
            request.validate().map_err(|error| {
                PortError::validation("translation.test_progress", error.to_string())
            })?;
            Ok(TranslationTargetProgressFacts {
                required_units: 1,
                exact_required_units: 1,
                optional_units: 0,
                exact_optional_units: 0,
                resources: 1,
                complete_resources: 1,
                owner_change_cursor: Some(OpaqueCursor::new("cursor-1").expect("valid cursor")),
            })
        }

        async fn read_changes(
            &self,
            _context: PortContext,
            request: TranslationTargetChangesRequest,
        ) -> Result<TranslationTargetChangePage, PortError> {
            request.validate().map_err(|error| {
                PortError::validation("translation.test_changes", error.to_string())
            })?;
            if request.after.is_some() {
                return Ok(TranslationTargetChangePage {
                    changes: Vec::new(),
                    next_cursor: None,
                });
            }
            Ok(TranslationTargetChangePage {
                changes: vec![TranslationTargetChange {
                    identity: target_identity("hero"),
                    resource_revision: OpaqueRevision::new("resource-7").expect("valid revision"),
                    lifecycle: TranslationResourceLifecycle::Active,
                }],
                next_cursor: Some(OpaqueCursor::new("cursor-1").expect("valid cursor")),
            })
        }
    }

    fn target_identity(
        resource_id: &str,
    ) -> rustok_translation_targets::TranslationResourceIdentity {
        rustok_translation_targets::TranslationResourceIdentity {
            owner_slug: OwnerSlug::new("media").expect("valid owner slug"),
            resource_kind: ResourceKind::new("asset").expect("valid resource kind"),
            resource_id: ResourceId::new(resource_id).expect("valid resource id"),
            subresource_id: None,
        }
    }

    fn resource_summary(
        resource_id: &str,
        source_locale: TenantLocale,
    ) -> TranslationResourceSummary {
        TranslationResourceSummary {
            identity: target_identity(resource_id),
            display_label: "Hero".to_string(),
            lifecycle: TranslationResourceLifecycle::Active,
            resource_revision: OpaqueRevision::new("resource-7").expect("valid revision"),
            exact_locales: vec![source_locale],
        }
    }

    fn unavailable() -> PortError {
        PortError::unavailable("translation.test_unavailable", "not used by this fixture")
    }

    async fn native_fixture() -> (
        HostRuntimeContext,
        Uuid,
        Uuid,
        AuthContext,
        TenantContext,
        RequestContext,
    ) {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("connect test database");
        database
            .execute_unprepared("PRAGMA foreign_keys = ON")
            .await
            .expect("enable foreign keys");
        database
            .execute_unprepared("CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL)")
            .await
            .expect("create tenant table");
        let manager = SchemaManager::new(&database);
        SysEventsMigration
            .up(&manager)
            .await
            .expect("migrate outbox");
        for migration in rustok_translation::migrations::migrations() {
            migration.up(&manager).await.expect("migrate Translation");
        }

        let first_tenant_id = Uuid::new_v4();
        let second_tenant_id = Uuid::new_v4();
        seed_tenant(&database, first_tenant_id).await;
        seed_tenant(&database, second_tenant_id).await;

        let provider_state = Arc::new(NativeProviderState::default());
        let mut registry = TranslationTargetRegistry::default();
        registry
            .register(NativeSnapshotProvider {
                state: Arc::clone(&provider_state),
            })
            .expect("register provider");
        let registry = Arc::new(registry);
        let event_bus =
            TransactionalEventBus::new(Arc::new(OutboxTransport::new(database.clone())));
        let locale_policies: Arc<dyn TenantLocalePolicyPort> = Arc::new(NativeTenantLocalePolicies);
        let machine_port = Arc::new(NativeMachinePort::new());
        let machine_factory =
            SharedMachineTranslationPortFactory(Arc::new(NativeMachinePortFactory {
                port: Arc::clone(&machine_port),
            }));
        let runtime = HostRuntimeContext::new(database)
            .with_shared_value(registry)
            .with_shared_value(event_bus)
            .with_shared_value(locale_policies)
            .with_shared_value(provider_state)
            .with_shared_value(machine_factory)
            .with_shared_value(machine_port);
        let user_id = Uuid::new_v4();
        let auth = auth_context(first_tenant_id, user_id);
        let tenant = tenant_context(first_tenant_id);
        let request = request_context(first_tenant_id, user_id);

        (
            runtime,
            first_tenant_id,
            second_tenant_id,
            auth,
            tenant,
            request,
        )
    }

    async fn native_interchange_artifact_fixture() -> (
        HostRuntimeContext,
        Uuid,
        Uuid,
        AuthContext,
        TenantContext,
        RequestContext,
        TempDir,
    ) {
        let (runtime, first_tenant_id, second_tenant_id, auth, tenant, request) =
            native_fixture().await;
        let storage_directory = tempfile::tempdir().expect("create artifact storage directory");
        let storage = StorageRuntime::local(&LocalStorageConfig {
            base_dir: storage_directory.path().display().to_string(),
            base_url: "/private".to_string(),
            fsync: false,
        })
        .expect("create artifact storage runtime");
        (
            runtime.with_shared_value(storage),
            first_tenant_id,
            second_tenant_id,
            auth,
            tenant,
            request,
            storage_directory,
        )
    }

    async fn seed_tenant(database: &DatabaseConnection, tenant_id: Uuid) {
        database
            .execute(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO tenants (id) VALUES (?)",
                [tenant_id.into()],
            ))
            .await
            .expect("seed tenant");
    }

    fn auth_context(tenant_id: Uuid, user_id: Uuid) -> AuthContext {
        AuthContext {
            user_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: [
                Resource::Translations,
                Resource::TranslationMemory,
                Resource::TranslationGlossaries,
            ]
            .into_iter()
            .flat_map(|resource| {
                [
                    Action::Create,
                    Action::Delete,
                    Action::List,
                    Action::Manage,
                    Action::Publish,
                    Action::Read,
                    Action::Resolve,
                    Action::Run,
                    Action::Update,
                    Action::Import,
                    Action::Export,
                ]
                .into_iter()
                .map(move |action| Permission::new(resource, action))
            })
            .collect(),
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
        }
    }

    fn tenant_context(tenant_id: Uuid) -> TenantContext {
        TenantContext {
            id: tenant_id,
            name: "Translation test tenant".to_string(),
            slug: format!("tenant-{tenant_id}"),
            domain: None,
            settings: serde_json::json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        }
    }

    fn request_context(tenant_id: Uuid, user_id: Uuid) -> RequestContext {
        RequestContext {
            tenant_id,
            user_id: Some(user_id),
            channel_id: None,
            channel_slug: None,
            channel_resolution_source: None,
            locale: "en".to_string(),
        }
    }

    fn identity() -> TranslationResourceIdentity {
        TranslationResourceIdentity {
            owner_slug: "media".to_string(),
            resource_kind: "asset".to_string(),
            resource_id: "hero".to_string(),
            subresource_id: None,
        }
    }

    async fn execute_over_http(
        runtime: &HostRuntimeContext,
        auth: &AuthContext,
        tenant: &TenantContext,
        operation: TranslationAdminOperation,
    ) -> (StatusCode, Vec<u8>) {
        let body = serde_qs::to_string(&super::ExecuteTranslationNative { operation })
            .expect("encode server-function payload");
        let mut request = Request::builder()
            .method("POST")
            .uri(<super::ExecuteTranslationNative as ServerFn>::PATH)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(body))
            .expect("build server-function request");
        request
            .extensions_mut()
            .insert(AuthContextExtension(auth.clone()));
        request
            .extensions_mut()
            .insert(TenantContextExtension(tenant.clone()));
        let runtime = runtime.clone();
        let response = leptos_axum::handle_server_fns_with_context(
            move || provide_context(runtime.clone()),
            request,
        )
        .await
        .into_response();
        let status = response.status();
        let body = to_bytes(response.into_body(), 1024 * 1024)
            .await
            .expect("read server-function response")
            .to_vec();
        (status, body)
    }

    async fn execute_http_ok(
        runtime: &HostRuntimeContext,
        auth: &AuthContext,
        tenant: &TenantContext,
        operation: TranslationAdminOperation,
    ) -> TranslationAdminResponse {
        let operation_name = format!("{operation:?}");
        let (status, body) = execute_over_http(runtime, auth, tenant, operation).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "{operation_name}: {}",
            String::from_utf8_lossy(&body)
        );
        serde_json::from_slice(&body).expect("decode server-function response")
    }

    async fn execute_http_error(
        runtime: &HostRuntimeContext,
        auth: &AuthContext,
        tenant: &TenantContext,
        operation: TranslationAdminOperation,
    ) -> String {
        let (status, body) = execute_over_http(runtime, auth, tenant, operation).await;
        assert_eq!(
            status,
            StatusCode::INTERNAL_SERVER_ERROR,
            "{}",
            String::from_utf8_lossy(&body)
        );
        String::from_utf8(body).expect("server-function error is UTF-8")
    }

    async fn create_approved_http_item(
        runtime: &HostRuntimeContext,
        translator: &AuthContext,
        reviewer: &AuthContext,
        tenant: &TenantContext,
        key: &str,
    ) -> (Job, JobItem, Proposal) {
        let (job, item) = create_http_job_item(runtime, translator, tenant, key).await;
        let response = execute_http_ok(
            runtime,
            translator,
            tenant,
            TranslationAdminOperation::SaveProposal {
                item_id: item.id.clone(),
                origin: ProposalOrigin::Manual,
                values: vec![ProposalValueInput {
                    key: "title".to_string(),
                    value: "Held".to_string(),
                }],
                idempotency_key: format!("{key}-save-proposal"),
            },
        )
        .await;
        let TranslationAdminResponse::Proposal(proposal) = response else {
            panic!("expected proposal response");
        };
        execute_http_ok(
            runtime,
            translator,
            tenant,
            TranslationAdminOperation::SubmitProposal {
                item_id: item.id.clone(),
                proposal_id: proposal.id.clone(),
                idempotency_key: format!("{key}-submit-proposal"),
            },
        )
        .await;
        let response = execute_http_ok(
            runtime,
            reviewer,
            tenant,
            TranslationAdminOperation::ApproveProposal {
                item_id: item.id.clone(),
                proposal_id: proposal.id.clone(),
                idempotency_key: format!("{key}-approve-proposal"),
            },
        )
        .await;
        let TranslationAdminResponse::Proposal(approved) = response else {
            panic!("expected proposal response");
        };
        (job, item, approved)
    }

    async fn create_http_job_item(
        runtime: &HostRuntimeContext,
        actor: &AuthContext,
        tenant: &TenantContext,
        key: &str,
    ) -> (Job, JobItem) {
        let response = execute_http_ok(
            runtime,
            actor,
            tenant,
            TranslationAdminOperation::CreateJob {
                source_locale: "en".to_string(),
                target_locale: "de".to_string(),
                glossary: None,
                idempotency_key: format!("{key}-create-job"),
            },
        )
        .await;
        let TranslationAdminResponse::Job(job) = response else {
            panic!("expected job response");
        };
        let response = execute_http_ok(
            runtime,
            actor,
            tenant,
            TranslationAdminOperation::AddItem {
                job_id: job.id.clone(),
                identity: identity(),
                idempotency_key: format!("{key}-add-item"),
            },
        )
        .await;
        let TranslationAdminResponse::Item(item) = response else {
            panic!("expected item response");
        };
        (job, item)
    }

    #[tokio::test]
    async fn native_server_fn_endpoint_extracts_authenticated_host_context() {
        let (runtime, _, second_tenant_id, auth, tenant, _) = native_fixture().await;
        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ReadPolicy,
        )
        .await;
        let TranslationAdminResponse::Policy(policy) = response else {
            panic!("expected policy response");
        };
        assert_eq!(policy.tenant_id, tenant.id.to_string());
        assert_eq!(policy.tenant_locale_policy_revision, 7);

        let error = execute_http_error(
            &runtime,
            &auth,
            &tenant_context(second_tenant_id),
            TranslationAdminOperation::ReadPolicy,
        )
        .await;
        assert!(error.contains("Authenticated tenant does not match request tenant"));
    }

    #[tokio::test]
    async fn native_machine_operations_execute_authenticated_http_parity() {
        use sea_orm::PaginatorTrait;

        let (runtime, tenant_id, _, auth, tenant, _) = native_fixture().await;
        let machine_port = runtime
            .shared_get::<Arc<NativeMachinePort>>()
            .expect("native machine port");

        let (_, completed_item) =
            create_http_job_item(&runtime, &auth, &tenant, "native-machine-completed").await;
        let operation_count = machine_operation::Entity::find()
            .count(runtime.db())
            .await
            .unwrap();
        let proposal_count = proposal::Entity::find().count(runtime.db()).await.unwrap();
        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::EstimateMachineTranslation {
                item_id: completed_item.id.clone(),
                field_keys: vec!["title".to_string()],
                minimum_memory_similarity_basis_points: 0,
                tone: Some("neutral".to_string()),
                domain: Some("media".to_string()),
                style: None,
                idempotency_key: "native-machine-estimate".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::MachineEstimate(estimate) = response else {
            panic!("expected machine estimate response");
        };
        assert_eq!(estimate.cost_minor_units_upper_bound, 1);
        assert!(estimate.review_required);
        assert_eq!(
            machine_operation::Entity::find()
                .count(runtime.db())
                .await
                .unwrap(),
            operation_count
        );
        assert_eq!(
            proposal::Entity::find().count(runtime.db()).await.unwrap(),
            proposal_count
        );
        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::GenerateMachineProposal {
                item_id: completed_item.id.clone(),
                field_keys: vec!["title".to_string()],
                minimum_memory_similarity_basis_points: 0,
                tone: Some("neutral".to_string()),
                domain: Some("media".to_string()),
                style: None,
                idempotency_key: "native-machine-generate".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::MachineProposal(completed) = response else {
            panic!("expected machine proposal response");
        };
        assert_eq!(completed.item_id, completed_item.id);
        assert_eq!(completed.adapter_slug, "native-machine");
        assert_eq!(completed.provider_slug, "native-provider");
        assert_eq!(completed.execution_id, "native-execution");
        assert!(completed.review_required);

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ReadMachineOperationStatus {
                operation_id: completed.operation_id.clone(),
            },
        )
        .await;
        let TranslationAdminResponse::MachineOperationStatus(status) = response else {
            panic!("expected machine operation status response");
        };
        assert_eq!(status.item_id, completed_item.id);
        assert_eq!(status.status, "completed");
        assert_eq!(status.provider_status, "completed");
        assert_eq!(
            status.provider_execution_id.as_deref(),
            Some("native-execution")
        );

        machine_port
            .set_execution_status(MachineTranslationExecutionStatus::Running)
            .await;
        let (_, in_progress_item) =
            create_http_job_item(&runtime, &auth, &tenant, "native-machine-in-progress").await;
        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::GenerateMachineProposal {
                item_id: in_progress_item.id.clone(),
                field_keys: vec!["title".to_string()],
                minimum_memory_similarity_basis_points: 0,
                tone: None,
                domain: None,
                style: None,
                idempotency_key: "native-machine-in-progress-generate".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::MachineOperationStatus(in_progress) = response else {
            panic!("expected a pollable in-progress machine operation response");
        };
        assert_eq!(in_progress.item_id, in_progress_item.id);
        assert_eq!(in_progress.status, "registered");
        assert_eq!(in_progress.provider_status, "running");
        assert_eq!(
            in_progress.provider_execution_id.as_deref(),
            Some("native-execution")
        );
        let persisted_in_progress = machine_operation::Entity::find_by_id(
            Uuid::parse_str(&in_progress.operation_id).expect("valid machine operation id"),
        )
        .one(runtime.db())
        .await
        .expect("read in-progress machine operation")
        .expect("in-progress machine operation");
        assert_eq!(persisted_in_progress.status, "registered");
        assert!(persisted_in_progress.proposal_id.is_none());

        machine_port
            .set_execution_status(MachineTranslationExecutionStatus::Completed)
            .await;
        machine_port
            .set_health(MachineTranslationProviderState::Unavailable)
            .await;
        let (_, cancelled_item) =
            create_http_job_item(&runtime, &auth, &tenant, "native-machine-cancelled").await;
        let error = execute_http_error(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::GenerateMachineProposal {
                item_id: cancelled_item.id.clone(),
                field_keys: vec!["title".to_string()],
                minimum_memory_similarity_basis_points: 0,
                tone: None,
                domain: None,
                style: None,
                idempotency_key: "native-machine-cancel-generate".to_string(),
            },
        )
        .await;
        assert!(
            error.contains("TRANSLATION_TEMPORARILY_UNAVAILABLE"),
            "{error}"
        );
        let cancelled_item_id =
            Uuid::parse_str(&cancelled_item.id).expect("valid cancelled item id");
        let registered = machine_operation::Entity::find()
            .filter(machine_operation::Column::TenantId.eq(tenant_id))
            .filter(machine_operation::Column::ItemId.eq(cancelled_item_id))
            .one(runtime.db())
            .await
            .expect("read registered machine operation")
            .expect("registered machine operation");
        assert_eq!(registered.status, "registered");

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ReadMachineOperationStatus {
                operation_id: registered.id.to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::MachineOperationStatus(status) = response else {
            panic!("expected registered machine operation status response");
        };
        assert_eq!(status.status, "registered");
        assert_eq!(status.provider_status, "completed");

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::CancelMachineOperation {
                operation_id: registered.id.to_string(),
                reason: "Cancel the unavailable machine request".to_string(),
                idempotency_key: "native-machine-cancel".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::MachineCancellation(cancellation) = response else {
            panic!("expected machine cancellation response");
        };
        assert_eq!(cancellation.operation_id, registered.id.to_string());
        assert_eq!(cancellation.status, "cancelled");
        assert_eq!(cancellation.provider_status, "cancellation_requested");
        assert_eq!(
            cancellation.provider_execution_id.as_deref(),
            Some("native-execution")
        );

        let (_, recovery_item) =
            create_http_job_item(&runtime, &auth, &tenant, "native-machine-recovery").await;
        let error = execute_http_error(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::GenerateMachineProposal {
                item_id: recovery_item.id.clone(),
                field_keys: vec!["title".to_string()],
                minimum_memory_similarity_basis_points: 0,
                tone: None,
                domain: None,
                style: None,
                idempotency_key: "native-machine-recovery-generate".to_string(),
            },
        )
        .await;
        assert!(
            error.contains("TRANSLATION_TEMPORARILY_UNAVAILABLE"),
            "{error}"
        );
        let recovery_item_id = Uuid::parse_str(&recovery_item.id).expect("valid recovery item id");
        let recovery_operation = machine_operation::Entity::find()
            .filter(machine_operation::Column::TenantId.eq(tenant_id))
            .filter(machine_operation::Column::ItemId.eq(recovery_item_id))
            .one(runtime.db())
            .await
            .expect("read recovery machine operation")
            .expect("recovery machine operation");
        assert_eq!(recovery_operation.status, "registered");
        let saving_at = chrono::Utc::now().fixed_offset();
        let update = machine_operation::Entity::update_many()
            .col_expr(
                machine_operation::Column::Status,
                sea_orm::sea_query::Expr::value("saving"),
            )
            .col_expr(
                machine_operation::Column::UpdatedAt,
                sea_orm::sea_query::Expr::value(saving_at),
            )
            .filter(machine_operation::Column::TenantId.eq(tenant_id))
            .filter(machine_operation::Column::Id.eq(recovery_operation.id))
            .filter(machine_operation::Column::Status.eq("registered"))
            .exec(runtime.db())
            .await
            .expect("move machine operation to recovery state");
        assert_eq!(update.rows_affected, 1);
        let saving = machine_operation::Entity::find_by_id(recovery_operation.id)
            .one(runtime.db())
            .await
            .expect("read saving machine operation")
            .expect("saving machine operation");
        assert_eq!(saving.status, "saving");

        machine_port
            .set_health(MachineTranslationProviderState::Available)
            .await;
        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::RecoverMachineOperation {
                operation_id: saving.id.to_string(),
                expected_updated_at: saving.updated_at.to_rfc3339(),
                item_id: recovery_item.id.clone(),
                field_keys: vec!["title".to_string()],
                minimum_memory_similarity_basis_points: 0,
                tone: None,
                domain: None,
                style: None,
                reason: "Recover the interrupted machine proposal save".to_string(),
                idempotency_key: "native-machine-recover".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::MachineProposal(recovered) = response else {
            panic!("expected recovered machine proposal response");
        };
        assert_eq!(recovered.operation_id, saving.id.to_string());
        assert_eq!(recovered.item_id, recovery_item.id);
        assert_eq!(recovered.provider_slug, "native-provider");
        assert_eq!(recovered.execution_id, "native-execution");
        let persisted = machine_operation::Entity::find_by_id(saving.id)
            .one(runtime.db())
            .await
            .expect("read recovered machine operation")
            .expect("recovered machine operation");
        assert_eq!(persisted.status, "completed");
        assert_eq!(
            persisted.proposal_id.map(|id| id.to_string()).as_deref(),
            Some(recovered.proposal_id.as_str())
        );
    }

    fn glossary_concept() -> GlossaryConcept {
        GlossaryConcept {
            concept_key: "hero".to_string(),
            source_term: "Hero".to_string(),
            variants: vec![GlossaryVariant {
                value: "Held".to_string(),
                policy: GlossaryTermPolicy::Preferred,
            }],
            match_kind: GlossaryMatchKind::WholeWord,
            case_sensitive: false,
            notes: "Preferred media terminology".to_string(),
        }
    }

    #[tokio::test]
    async fn native_policy_and_glossary_execute_authenticated_http_parity() {
        let (runtime, _, _, auth, tenant, _) = native_fixture().await;

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ReadPolicy,
        )
        .await;
        let TranslationAdminResponse::Policy(initial_policy) = response else {
            panic!("expected policy response");
        };
        assert_eq!(initial_policy.revision, 0);

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ReplacePolicy {
                expected_revision: initial_policy.revision,
                required_target_locales: vec!["de".to_string()],
                idempotency_key: "native-replace-policy".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::Policy(policy) = response else {
            panic!("expected policy response");
        };
        assert_eq!(policy.revision, 1);
        assert_eq!(policy.required_target_locales, ["de"]);

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::CreateGlossary {
                name: "Media terminology".to_string(),
                description: "Approved media terms".to_string(),
                source_locale: "en".to_string(),
                target_locale: "de".to_string(),
                scope: GlossaryScope {
                    owner_slug: Some("media".to_string()),
                    resource_kind: Some("asset".to_string()),
                    field_key: Some("title".to_string()),
                },
                idempotency_key: "native-create-glossary".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::Glossary(created) = response else {
            panic!("expected glossary response");
        };
        assert_eq!(created.revision, 1);
        assert!(created.is_active);

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::UpdateGlossary {
                glossary_id: created.id.clone(),
                expected_revision: created.revision,
                name: "Media terminology v2".to_string(),
                description: "Reviewed media terms".to_string(),
                idempotency_key: "native-update-glossary".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::Glossary(updated) = response else {
            panic!("expected glossary response");
        };
        assert_eq!(updated.revision, 2);

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ReplaceGlossaryTerms {
                glossary_id: updated.id.clone(),
                expected_revision: updated.revision,
                concepts: vec![glossary_concept()],
                idempotency_key: "native-replace-glossary-terms".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::Glossary(with_terms) = response else {
            panic!("expected glossary response");
        };
        assert_eq!(with_terms.revision, 3);
        assert_eq!(with_terms.concepts.len(), 1);

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::SetGlossaryActive {
                glossary_id: with_terms.id.clone(),
                expected_revision: with_terms.revision,
                is_active: false,
                idempotency_key: "native-disable-glossary".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::Glossary(disabled) = response else {
            panic!("expected glossary response");
        };
        assert_eq!(disabled.revision, 4);
        assert!(!disabled.is_active);

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ReadGlossary {
                glossary_id: disabled.id.clone(),
                revision: Some(disabled.revision),
            },
        )
        .await;
        let TranslationAdminResponse::Glossary(read) = response else {
            panic!("expected glossary response");
        };
        assert_eq!(read, disabled);

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ListGlossaries { limit: 10 },
        )
        .await;
        let TranslationAdminResponse::Glossaries(glossaries) = response else {
            panic!("expected glossaries response");
        };
        assert_eq!(glossaries.len(), 1);
        assert_eq!(glossaries[0].id, disabled.id);
    }

    #[tokio::test]
    async fn native_human_workflow_memory_and_progress_execute_http_parity() {
        let (runtime, _, _, auth, tenant, _) = native_fixture().await;
        let reviewer = auth_context(tenant.id, Uuid::new_v4());

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::CreateJob {
                source_locale: "en".to_string(),
                target_locale: "de".to_string(),
                glossary: None,
                idempotency_key: "native-workflow-create-job".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::Job(job) = response else {
            panic!("expected job response");
        };

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::AddItem {
                job_id: job.id.clone(),
                identity: identity(),
                idempotency_key: "native-workflow-add-item".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::Item(item) = response else {
            panic!("expected item response");
        };

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ReadJobProgress {
                job_id: job.id.clone(),
            },
        )
        .await;
        let TranslationAdminResponse::JobProgress(initial_progress) = response else {
            panic!("expected job progress response");
        };
        assert_eq!(initial_progress.total_items, 1);
        assert_eq!(initial_progress.missing_items, 1);

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::CreateWorkflowNote {
                job_id: job.id.clone(),
                item_id: Some(item.id.clone()),
                body: "Private translator context".to_string(),
                idempotency_key: "native-workflow-note-create".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::WorkflowNote(note) = response else {
            panic!("expected workflow note response");
        };
        assert_eq!(note.body, "Private translator context");
        assert_eq!(note.item_id.as_deref(), Some(item.id.as_str()));
        assert!(note.resolved_at.is_none());

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ListWorkflowNotes {
                job_id: job.id.clone(),
                item_id: Some(item.id.clone()),
                include_resolved: false,
                limit: 10,
            },
        )
        .await;
        let TranslationAdminResponse::WorkflowNotes(notes) = response else {
            panic!("expected workflow notes response");
        };
        assert_eq!(notes, vec![note.clone()]);

        let response = execute_http_ok(
            &runtime,
            &reviewer,
            &tenant,
            TranslationAdminOperation::ResolveWorkflowNote {
                note_id: note.id.clone(),
                expected_revision: note.revision,
                idempotency_key: "native-workflow-note-resolve".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::WorkflowNote(resolved_note) = response else {
            panic!("expected resolved workflow note response");
        };
        assert_eq!(resolved_note.revision, note.revision + 1);
        assert!(resolved_note.resolved_at.is_some());
        assert_eq!(
            resolved_note
                .resolved_by
                .as_ref()
                .map(|actor| actor.id.as_str()),
            Some(reviewer.user_id.to_string().as_str())
        );

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ListWorkflowNotes {
                job_id: job.id.clone(),
                item_id: Some(item.id.clone()),
                include_resolved: false,
                limit: 10,
            },
        )
        .await;
        let TranslationAdminResponse::WorkflowNotes(open_notes) = response else {
            panic!("expected workflow notes response");
        };
        assert!(open_notes.is_empty());

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::AssignItem {
                item_id: item.id.clone(),
                expected_revision: item.revision,
                assignee: Actor {
                    kind: ActorKind::User,
                    id: auth.user_id.to_string(),
                },
                idempotency_key: "native-workflow-assign".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::Assignment(assigned) = response else {
            panic!("expected assignment response");
        };
        assert_eq!(
            assigned.assignee.as_ref().map(|actor| actor.id.as_str()),
            Some(auth.user_id.to_string().as_str())
        );

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::UnassignItem {
                item_id: item.id.clone(),
                expected_revision: assigned.item_revision,
                idempotency_key: "native-workflow-unassign".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::Assignment(unassigned) = response else {
            panic!("expected assignment response");
        };
        assert!(unassigned.assignee.is_none());

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::SaveProposal {
                item_id: item.id.clone(),
                origin: ProposalOrigin::Manual,
                values: vec![ProposalValueInput {
                    key: "title".to_string(),
                    value: "Held".to_string(),
                }],
                idempotency_key: "native-workflow-save".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::Proposal(draft) = response else {
            panic!("expected proposal response");
        };
        assert!(draft.qa_accepted);
        assert_eq!(draft.status, "draft");

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::SubmitProposal {
                item_id: item.id.clone(),
                proposal_id: draft.id.clone(),
                idempotency_key: "native-workflow-submit".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::Proposal(submitted) = response else {
            panic!("expected proposal response");
        };
        assert_eq!(submitted.status, "in_review");

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ReadReviewerQueue {
                job_id: job.id.clone(),
                assignee: None,
                include_unassigned: true,
                limit: 10,
            },
        )
        .await;
        let TranslationAdminResponse::ReviewerQueue(queue) = response else {
            panic!("expected reviewer queue response");
        };
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].item.id, item.id);
        assert_eq!(queue[0].proposal_id, draft.id);

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ReadReviewerWorkload {
                job_id: job.id.clone(),
            },
        )
        .await;
        let TranslationAdminResponse::ReviewerWorkloads(workloads) = response else {
            panic!("expected reviewer workload response");
        };
        assert_eq!(workloads.len(), 1);
        assert!(workloads[0].assignee.is_none());
        assert_eq!(workloads[0].open_items, 1);
        assert_eq!(workloads[0].in_review_items, 1);

        let response = execute_http_ok(
            &runtime,
            &reviewer,
            &tenant,
            TranslationAdminOperation::ApproveProposal {
                item_id: item.id.clone(),
                proposal_id: draft.id.clone(),
                idempotency_key: "native-workflow-approve".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::Proposal(approved) = response else {
            panic!("expected proposal response");
        };
        assert_eq!(approved.status, "approved");
        assert!(approved.approval_receipt_id.is_some());

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ApplyProposal {
                item_id: item.id.clone(),
                proposal_id: draft.id,
                idempotency_key: "native-workflow-apply".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::Apply(applied) = response else {
            panic!("expected apply response");
        };
        assert_eq!(applied.item_id, item.id);
        assert_eq!(applied.applied_field_keys, ["title"]);

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ReadJobProgress {
                job_id: job.id.clone(),
            },
        )
        .await;
        let TranslationAdminResponse::JobProgress(completed_progress) = response else {
            panic!("expected job progress response");
        };
        assert_eq!(completed_progress.applied_items, 1);
        assert_eq!(completed_progress.complete_resources, 1);

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::RebuildJobProgress {
                job_id: job.id,
                idempotency_key: "native-workflow-rebuild-progress".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::JobProgress(rebuilt_progress) = response else {
            panic!("expected job progress response");
        };
        assert_eq!(rebuilt_progress.applied_items, 1);

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ListMemoryEntries {
                source_locale: Some("en".to_string()),
                target_locale: Some("de".to_string()),
                include_tombstoned: false,
                limit: 10,
            },
        )
        .await;
        let TranslationAdminResponse::MemoryEntries(mut entries) = response else {
            panic!("expected memory entries response");
        };
        assert_eq!(entries.len(), 1);
        let entry = entries.pop().expect("memory entry");

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ReadMemoryEntry {
                entry_id: entry.id.clone(),
            },
        )
        .await;
        let TranslationAdminResponse::MemoryEntry(read_entry) = response else {
            panic!("expected memory entry response");
        };
        assert_eq!(read_entry.target_text, "Held");

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::LookupMemory {
                source_locale: "en".to_string(),
                target_locale: "de".to_string(),
                identity: identity(),
                field_key: "title".to_string(),
                source_text: "Hero".to_string(),
                minimum_similarity_basis_points: 10_000,
                limit: 10,
            },
        )
        .await;
        let TranslationAdminResponse::MemorySuggestions(suggestions) = response else {
            panic!("expected memory suggestions response");
        };
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].target_text, "Held");

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::SetMemoryRetention {
                entry_id: entry.id.clone(),
                expected_revision: entry.revision,
                policy: MemoryRetentionPolicy::LegalHold,
                retain_until: None,
                idempotency_key: "native-memory-legal-hold".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::MemoryMutation(legal_hold) = response else {
            panic!("expected memory mutation response");
        };

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::SetMemoryRetention {
                entry_id: entry.id.clone(),
                expected_revision: legal_hold.revision,
                policy: MemoryRetentionPolicy::OwnerLifecycle,
                retain_until: None,
                idempotency_key: "native-memory-owner-lifecycle".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::MemoryMutation(owner_lifecycle) = response else {
            panic!("expected memory mutation response");
        };

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::TombstoneMemoryEntry {
                entry_id: entry.id.clone(),
                expected_revision: owner_lifecycle.revision,
                idempotency_key: "native-memory-tombstone".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::MemoryMutation(tombstoned) = response else {
            panic!("expected memory mutation response");
        };
        assert_eq!(tombstoned.state, "tombstoned");

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::PurgeMemoryEntry {
                entry_id: entry.id,
                expected_revision: tombstoned.revision,
                idempotency_key: "native-memory-purge".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::MemoryMutation(purged) = response else {
            panic!("expected memory mutation response");
        };
        assert_eq!(purged.state, "purged");
    }

    #[tokio::test]
    async fn native_qa_rejection_and_job_cancellation_execute_http_parity() {
        let (runtime, _, _, auth, tenant, _) = native_fixture().await;

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::CreateJob {
                source_locale: "en".to_string(),
                target_locale: "de".to_string(),
                glossary: None,
                idempotency_key: "native-qa-create-job".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::Job(job) = response else {
            panic!("expected job response");
        };

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::AddItem {
                job_id: job.id.clone(),
                identity: identity(),
                idempotency_key: "native-qa-add-item".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::Item(item) = response else {
            panic!("expected item response");
        };

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::SaveProposal {
                item_id: item.id.clone(),
                origin: ProposalOrigin::Manual,
                values: vec![ProposalValueInput {
                    key: "title".to_string(),
                    value: "x".repeat(201),
                }],
                idempotency_key: "native-qa-save-invalid".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::Proposal(invalid) = response else {
            panic!("expected proposal response");
        };
        assert!(!invalid.qa_accepted);
        assert!(
            invalid
                .qa_issues
                .iter()
                .any(|issue| issue.code == "translation.qa.max_characters_exceeded")
        );

        let error = execute_http_error(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::SubmitProposal {
                item_id: item.id,
                proposal_id: invalid.id,
                idempotency_key: "native-qa-submit-invalid".to_string(),
            },
        )
        .await;
        assert!(error.contains("TRANSLATION_REQUEST_INVALID"), "{error}");

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::CancelJob {
                job_id: job.id,
                expected_revision: 1,
                reason: "The target locale is no longer required".to_string(),
                idempotency_key: "native-cancel-job".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::Cancellation(cancellation) = response else {
            panic!("expected cancellation response");
        };
        assert_eq!(cancellation.cancelled_item_count, 1);
        assert_eq!(cancellation.job_revision, 2);
    }

    #[tokio::test]
    async fn native_retry_and_apply_recovery_execute_http_parity() {
        let (runtime, _, _, auth, tenant, _) = native_fixture().await;
        let reviewer = auth_context(tenant.id, Uuid::new_v4());
        let provider_state = runtime
            .shared_get::<Arc<NativeProviderState>>()
            .expect("provider state");

        let (_, retry_item, retry_proposal) =
            create_approved_http_item(&runtime, &auth, &reviewer, &tenant, "native-retry").await;
        provider_state
            .next_error
            .lock()
            .await
            .replace(PortError::forbidden(
                "translation.test_blocked",
                "operator intervention is required",
            ));
        let error = execute_http_error(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ApplyProposal {
                item_id: retry_item.id.clone(),
                proposal_id: retry_proposal.id.clone(),
                idempotency_key: "native-retry-apply-fail".to_string(),
            },
        )
        .await;
        assert!(error.contains("TRANSLATION_OPERATION_FAILED"), "{error}");

        let retry_item_id = Uuid::parse_str(&retry_item.id).expect("valid item id");
        let blocked = job_item::Entity::find_by_id(retry_item_id)
            .one(runtime.db())
            .await
            .expect("read blocked item")
            .expect("blocked item");
        assert_eq!(blocked.status, "blocked");
        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::RetryItem {
                item_id: retry_item.id.clone(),
                expected_revision: blocked.revision,
                reason: "The owner policy issue has been resolved".to_string(),
                idempotency_key: "native-retry-item".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::Retry(retried) = response else {
            panic!("expected retry response");
        };
        assert_eq!(retried.status, "approved");

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ApplyProposal {
                item_id: retry_item.id,
                proposal_id: retry_proposal.id,
                idempotency_key: "native-retry-apply-success".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::Apply(applied) = response else {
            panic!("expected apply response");
        };
        assert_eq!(applied.applied_field_keys, ["title"]);

        let (_, recovery_item, recovery_proposal) =
            create_approved_http_item(&runtime, &auth, &reviewer, &tenant, "native-recovery").await;
        provider_state
            .fail_after_commit
            .store(true, Ordering::SeqCst);
        let error = execute_http_error(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ApplyProposal {
                item_id: recovery_item.id.clone(),
                proposal_id: recovery_proposal.id,
                idempotency_key: "native-recovery-apply".to_string(),
            },
        )
        .await;
        assert!(
            error.contains("TRANSLATION_TEMPORARILY_UNAVAILABLE"),
            "{error}"
        );

        let recovery_item_id = Uuid::parse_str(&recovery_item.id).expect("valid item id");
        let pending = apply_operation::Entity::find()
            .filter(apply_operation::Column::ItemId.eq(recovery_item_id))
            .one(runtime.db())
            .await
            .expect("read pending apply")
            .expect("pending apply");
        assert_eq!(pending.status, "pending");
        assert_eq!(pending.attempt_count, 1);

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::RecoverApply {
                operation_id: pending.id.to_string(),
                expected_attempt_count: pending.attempt_count,
                reason: "Recover an owner response lost after commit".to_string(),
                idempotency_key: "native-recover-apply".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::Apply(recovered) = response else {
            panic!("expected apply response");
        };
        assert_eq!(recovered.item_id, recovery_item.id);
        assert_eq!(
            recovered.provider_receipt_id,
            "native-provider:native-recovery-apply"
        );
        let completed = apply_operation::Entity::find_by_id(pending.id)
            .one(runtime.db())
            .await
            .expect("read completed apply")
            .expect("completed apply");
        assert_eq!(completed.status, "completed");
        assert_eq!(completed.attempt_count, 2);
    }

    #[tokio::test]
    async fn native_inventory_and_provider_progress_execute_http_parity() {
        let (runtime, _, _, auth, tenant, _) = native_fixture().await;

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ListTargets,
        )
        .await;
        let TranslationAdminResponse::Targets(targets) = response else {
            panic!("expected targets response");
        };
        assert_eq!(targets.len(), 1);
        assert!(
            targets[0]
                .capabilities
                .iter()
                .any(|value| value == "change_cursor")
        );

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::SyncProviderInventory {
                owner_slug: "media".to_string(),
                resource_kind: "asset".to_string(),
                limit: 10,
            },
        )
        .await;
        let TranslationAdminResponse::Inventory(synced) = response else {
            panic!("expected inventory response");
        };
        assert_eq!(synced.observed_resources, 1);
        assert_eq!(synced.checkpoint.as_deref(), Some("cursor-1"));

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::RebuildProviderInventory {
                owner_slug: "media".to_string(),
                resource_kind: "asset".to_string(),
                source_locale: "en".to_string(),
                target_locale: "de".to_string(),
                page_size: 10,
            },
        )
        .await;
        let TranslationAdminResponse::Inventory(rebuilt) = response else {
            panic!("expected inventory response");
        };
        assert_eq!(rebuilt.observed_resources, 1);

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ReadProviderProgress {
                owner_slug: "media".to_string(),
                resource_kind: "asset".to_string(),
                source_locale: "en".to_string(),
                target_locale: "de".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::ProviderProgress(progress) = response else {
            panic!("expected provider progress response");
        };
        assert_eq!(progress.required_units, 1);
        assert_eq!(progress.exact_required_units, 1);
        assert_eq!(progress.freshness, "current");

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ReplacePolicy {
                expected_revision: 0,
                required_target_locales: vec!["de".to_string()],
                idempotency_key: "native-inventory-replace-policy".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::Policy(policy) = response else {
            panic!("expected policy response");
        };
        assert_eq!(policy.required_target_locales, ["de"]);

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ReadRequiredProviderProgress {
                owner_slug: "media".to_string(),
                resource_kind: "asset".to_string(),
                source_locale: "en".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::RequiredProviderProgress(required) = response else {
            panic!("expected required provider progress response");
        };
        assert_eq!(required.required_target_locales, ["de"]);
        assert_eq!(required.targets.len(), 1);
        assert_eq!(required.complete_resource_locale_pairs, 1);
    }

    #[tokio::test]
    async fn native_interchange_executes_authenticated_http_parity() {
        let (runtime, _, second_tenant_id, auth, tenant, _) = native_fixture().await;

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::CreateJob {
                source_locale: "en".to_string(),
                target_locale: "de".to_string(),
                glossary: None,
                idempotency_key: "native-create-job".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::Job(job) = response else {
            panic!("expected job response");
        };

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::AddItem {
                job_id: job.id.clone(),
                identity: identity(),
                idempotency_key: "native-add-item".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::Item(item) = response else {
            panic!("expected item response");
        };

        let malformed = execute_http_error(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ExportJob {
                job_id: job.id.clone(),
                max_items: 0,
            },
        )
        .await;
        assert!(
            malformed.contains("TRANSLATION_REQUEST_INVALID"),
            "{malformed}"
        );

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ExportJob {
                job_id: job.id.clone(),
                max_items: 10,
            },
        )
        .await;
        let TranslationAdminResponse::InterchangeDocument(document) = response else {
            panic!("expected interchange document");
        };
        assert_eq!(document.schema_version, 1);
        assert_eq!(document.items.len(), 1);
        assert_eq!(document.items[0].fields[0].source_value, "Hero");

        let stale = execute_http_error(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ImportItem {
                schema_version: document.schema_version,
                job_id: job.id.clone(),
                item_id: item.id.clone(),
                identity: document.items[0].identity.clone(),
                source_digest: "0".repeat(64),
                values: vec![ProposalValueInput {
                    key: "title".to_string(),
                    value: "Held".to_string(),
                }],
                idempotency_key: "native-import-stale".to_string(),
            },
        )
        .await;
        assert!(stale.contains("TRANSLATION_REQUEST_INVALID"), "{stale}");

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ImportItem {
                schema_version: document.schema_version,
                job_id: job.id.clone(),
                item_id: item.id,
                identity: document.items[0].identity.clone(),
                source_digest: document.items[0].source_digest.clone(),
                values: vec![ProposalValueInput {
                    key: "title".to_string(),
                    value: "Held".to_string(),
                }],
                idempotency_key: "native-import-valid".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::Proposal(proposal) = response else {
            panic!("expected proposal response");
        };
        assert_eq!(proposal.origin, "import");
        assert_eq!(proposal.status, "draft");
        assert!(proposal.qa_accepted);

        let second_user_id = Uuid::new_v4();
        let second_auth = auth_context(second_tenant_id, second_user_id);
        let second_tenant = tenant_context(second_tenant_id);
        let isolated = execute_http_error(
            &runtime,
            &second_auth,
            &second_tenant,
            TranslationAdminOperation::ExportJob {
                job_id: job.id,
                max_items: 10,
            },
        )
        .await;
        assert!(
            isolated.contains("TRANSLATION_RESOURCE_NOT_FOUND"),
            "{isolated}"
        );
    }

    #[tokio::test]
    async fn native_interchange_artifacts_execute_authenticated_http_parity() {
        let (runtime, _, second_tenant_id, auth, tenant, _, _storage_directory) =
            native_interchange_artifact_fixture().await;
        let (job, item) = create_http_job_item(&runtime, &auth, &tenant, "native-artifact").await;

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::CreateInterchangeExportArtifact {
                job_id: job.id.clone(),
                max_items: 10,
                expires_in_seconds: 86_400,
                idempotency_key: "native-artifact-export".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::InterchangeArtifact(exported) = response else {
            panic!("expected export artifact response");
        };
        assert_eq!(exported.direction, "export");
        assert_eq!(exported.status, "ready");

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ListInterchangeArtifacts {
                job_id: Some(job.id.clone()),
                include_expired: false,
                limit: 10,
            },
        )
        .await;
        let TranslationAdminResponse::InterchangeArtifacts(artifacts) = response else {
            panic!("expected interchange artifacts response");
        };
        assert_eq!(artifacts, vec![exported.clone()]);

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ReadInterchangeArtifact {
                artifact_id: exported.id.clone(),
            },
        )
        .await;
        let TranslationAdminResponse::InterchangeArtifactContent(mut export_content) = response
        else {
            panic!("expected interchange artifact content response");
        };
        assert_eq!(export_content.artifact.id, exported.id);
        assert_eq!(export_content.document.items.len(), 1);
        assert_eq!(export_content.document.items[0].item_id, item.id);
        export_content.document.items[0].fields[0].proposed_value = Some("Held".to_string());
        let import_document = serde_json::to_string(&export_content.document)
            .expect("serialize import artifact document");

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::StoreInterchangeImportArtifact {
                job_id: job.id.clone(),
                document_json: import_document,
                expires_in_seconds: 86_400,
                idempotency_key: "native-artifact-store".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::InterchangeArtifact(imported) = response else {
            panic!("expected import artifact response");
        };
        assert_eq!(imported.direction, "import");
        assert_eq!(imported.status, "ready");

        let response = execute_http_ok(
            &runtime,
            &auth,
            &tenant,
            TranslationAdminOperation::ProcessInterchangeImportArtifact {
                artifact_id: imported.id.clone(),
                idempotency_key: "native-artifact-process".to_string(),
            },
        )
        .await;
        let TranslationAdminResponse::InterchangeArtifact(processed) = response else {
            panic!("expected processed import artifact response");
        };
        assert_eq!(processed.status, "completed");
        let report = processed
            .report
            .expect("expected aggregate conflict report");
        assert_eq!(report.total_items, 1);
        assert_eq!(report.accepted_items, 1);
        assert_eq!(report.conflict_items, 0);
        assert_eq!(report.rejected_items, 0);
        assert_eq!(report.outcomes[0].item_id, item.id);
        assert_eq!(report.outcomes[0].status, "imported");

        let second_auth = auth_context(second_tenant_id, Uuid::new_v4());
        let second_tenant = tenant_context(second_tenant_id);
        let isolated = execute_http_error(
            &runtime,
            &second_auth,
            &second_tenant,
            TranslationAdminOperation::ReadInterchangeArtifact {
                artifact_id: imported.id,
            },
        )
        .await;
        assert!(
            isolated.contains("TRANSLATION_RESOURCE_NOT_FOUND"),
            "{isolated}"
        );
    }

    #[tokio::test]
    async fn native_runtime_rejects_invalid_context_and_missing_dependencies() {
        let (runtime, _, second_tenant_id, auth, tenant, request) = native_fixture().await;
        let mismatched_tenant = tenant_context(second_tenant_id);
        let mismatch = execute_with_runtime(
            TranslationAdminOperation::ReadPolicy,
            &auth,
            &mismatched_tenant,
            &request,
            &runtime,
        )
        .await
        .expect_err("mismatched tenant context must fail");
        assert!(
            mismatch
                .to_string()
                .contains("Authenticated tenant does not match request tenant")
        );

        let empty_key = execute_with_runtime(
            TranslationAdminOperation::CreateJob {
                source_locale: "en".to_string(),
                target_locale: "de".to_string(),
                glossary: None,
                idempotency_key: String::new(),
            },
            &auth,
            &tenant,
            &request,
            &runtime,
        )
        .await
        .expect_err("empty idempotency key must fail");
        assert!(
            empty_key
                .to_string()
                .contains("Idempotency key must not be empty")
        );

        let incomplete_runtime = HostRuntimeContext::new(runtime.db_clone());
        let unavailable = execute_with_runtime(
            TranslationAdminOperation::ReadPolicy,
            &auth,
            &tenant,
            &request,
            &incomplete_runtime,
        )
        .await
        .expect_err("missing runtime dependencies must fail");
        assert!(
            unavailable
                .to_string()
                .contains("Translation runtime is unavailable")
        );
    }
}
