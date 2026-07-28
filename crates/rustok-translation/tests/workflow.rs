use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, Utc};
use rustok_api::{Action, Permission, PortActor, PortContext, PortError, Resource, TenantLocale};
use rustok_outbox::{OutboxTransport, SysEvents, SysEventsMigration, TransactionalEventBus};
use rustok_translation::{
    AddItemInput, ApplyProposalInput, ApproveProposalInput, AssignItemInput, CancelJobInput,
    CreateJobInput, ProposalOrigin, ProposalValue, RecoverApplyInput, RetryItemInput,
    SaveProposalInput, SubmitProposalInput, TranslationError, TranslationProgressService,
    TranslationWorkflowService, UnassignItemInput,
    entities::{
        apply_operation, apply_receipt, apply_recovery, assignment, cancellation, job, job_item,
        job_progress, proposal, retry,
    },
    migrations,
};
use rustok_translation_targets::{
    FieldKey, ListTranslationResourcesRequest, OpaqueRevision, OwnerSlug,
    ReadTranslationResourceRequest, ResourceId, ResourceKind, TranslationApplicationReceipt,
    TranslationDataClassification, TranslationFieldDescriptor, TranslationFieldSnapshot,
    TranslationPatchRequest, TranslationPatchValidation, TranslationResourceIdentity,
    TranslationResourceLifecycle, TranslationResourcePage, TranslationResourceSnapshot,
    TranslationResourceSummary, TranslationStrategy, TranslationTargetCapability,
    TranslationTargetProvider, TranslationTargetProviderDescriptor, TranslationTargetRegistry,
    TranslationValueProfile,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait,
    PaginatorTrait, QueryFilter, Statement, sea_query::Expr,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Default)]
struct ApplyProviderState {
    calls: AtomicUsize,
    fail_after_commit: AtomicBool,
    next_error: Mutex<Option<PortError>>,
    receipts: Mutex<BTreeMap<String, TranslationApplicationReceipt>>,
}

struct SnapshotProvider {
    apply_state: Arc<ApplyProviderState>,
}

#[async_trait]
impl TranslationTargetProvider for SnapshotProvider {
    fn descriptor(&self) -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new("media").unwrap(),
            resource_kind: ResourceKind::new("asset").unwrap(),
            display_name: "Media asset metadata".to_string(),
            capabilities: BTreeSet::from([
                TranslationTargetCapability::ReadExactResource,
                TranslationTargetCapability::ValidatePatch,
                TranslationTargetCapability::ApplyPatch,
            ]),
            read_permission_floor: BTreeSet::from(["media:read".to_string()]),
            apply_permission_floor: BTreeSet::from(["media:update".to_string()]),
        }
    }

    async fn list_resources(
        &self,
        _context: PortContext,
        _request: ListTranslationResourcesRequest,
    ) -> Result<TranslationResourcePage, PortError> {
        Err(unavailable())
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
                resource_revision: OpaqueRevision::new("resource-7").unwrap(),
                exact_locales: vec![request.source_locale.clone()],
            },
            source_locale: request.source_locale,
            target_locale: request.target_locale,
            rendered_fallback_locale: None,
            source_revision: OpaqueRevision::new("source-3").unwrap(),
            target_revision: None,
            fields: vec![TranslationFieldSnapshot {
                descriptor: TranslationFieldDescriptor {
                    key: FieldKey::new("title").unwrap(),
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
                source_hash: "sha256:hero".to_string(),
            }],
        })
    }

    async fn validate_patch(
        &self,
        _context: PortContext,
        request: TranslationPatchRequest,
    ) -> Result<TranslationPatchValidation, PortError> {
        request
            .validate()
            .map_err(|error| PortError::validation("translation.test_patch", error.to_string()))?;
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
        request
            .validate()
            .map_err(|error| PortError::validation("translation.test_patch", error.to_string()))?;
        self.apply_state.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(error) = self.apply_state.next_error.lock().await.take() {
            return Err(error);
        }
        let idempotency_key = context.idempotency_key.as_deref().unwrap_or_default();
        let mut receipts = self.apply_state.receipts.lock().await;
        if let Some(existing) = receipts.get(idempotency_key) {
            return Ok(existing.clone());
        }
        let receipt = TranslationApplicationReceipt {
            provider_receipt_id: format!("provider:{idempotency_key}"),
            resource_revision: OpaqueRevision::new("resource-8").unwrap(),
            target_revision: OpaqueRevision::new("target-1").unwrap(),
            applied_field_keys: request
                .fields
                .iter()
                .map(|field| field.key.clone())
                .collect(),
        };
        receipts.insert(idempotency_key.to_string(), receipt.clone());
        drop(receipts);
        if self
            .apply_state
            .fail_after_commit
            .swap(false, Ordering::SeqCst)
        {
            return Err(PortError::timeout(
                "translation.test_unknown_outcome",
                "owner committed but the response was lost",
            ));
        }
        Ok(receipt)
    }
}

fn unavailable() -> PortError {
    PortError::unavailable("translation.test_unavailable", "not used by this fixture")
}

async fn fixture_with_apply_state() -> (
    DatabaseConnection,
    TranslationWorkflowService,
    Uuid,
    Arc<ApplyProviderState>,
) {
    let database = Database::connect("sqlite::memory:").await.unwrap();
    database
        .execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .unwrap();
    database
        .execute_unprepared("CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL)")
        .await
        .unwrap();
    let manager = SchemaManager::new(&database);
    SysEventsMigration.up(&manager).await.unwrap();
    for migration in migrations::migrations() {
        migration.up(&manager).await.unwrap();
    }
    let tenant_id = Uuid::new_v4();
    seed_tenant(&database, tenant_id).await;
    let apply_state = Arc::new(ApplyProviderState::default());
    let mut registry = TranslationTargetRegistry::default();
    registry
        .register(SnapshotProvider {
            apply_state: Arc::clone(&apply_state),
        })
        .unwrap();
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(database.clone())));
    (
        database.clone(),
        TranslationWorkflowService::new(database, Arc::new(registry), event_bus),
        tenant_id,
        apply_state,
    )
}

async fn fixture() -> (DatabaseConnection, TranslationWorkflowService, Uuid) {
    let (database, service, tenant_id, _) = fixture_with_apply_state().await;
    (database, service, tenant_id)
}

async fn seed_tenant(database: &DatabaseConnection, tenant_id: Uuid) {
    database
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenants (id) VALUES (?)",
            [tenant_id.into()],
        ))
        .await
        .unwrap();
}

fn write_context(tenant_id: Uuid, idempotency_key: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        format!("translation-workflow-{idempotency_key}"),
    )
    .with_idempotency_key(idempotency_key)
    .with_deadline(Duration::from_secs(5))
}

fn read_context(tenant_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        "translation-progress-read",
    )
    .with_deadline(Duration::from_secs(5))
}

fn user_write_context(
    tenant_id: Uuid,
    actor_id: Uuid,
    action: Action,
    idempotency_key: &str,
) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        format!("translation-workflow-{idempotency_key}"),
    )
    .with_claim(Permission::new(Resource::Translations, action).to_string())
    .with_role("manager")
    .with_idempotency_key(idempotency_key)
    .with_deadline(Duration::from_secs(5))
}

fn recovery_context(tenant_id: Uuid, actor_id: Uuid, idempotency_key: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        format!("translation-recovery-{idempotency_key}"),
    )
    .with_claim(Permission::new(Resource::Translations, Action::Manage).to_string())
    .with_claim(Permission::new(Resource::Translations, Action::Publish).to_string())
    .with_role("manager")
    .with_idempotency_key(idempotency_key)
    .with_deadline(Duration::from_secs(5))
}

fn job_input(target_locale: &str) -> CreateJobInput {
    CreateJobInput {
        source_locale: TenantLocale::new("en").unwrap(),
        target_locale: TenantLocale::new(target_locale).unwrap(),
    }
}

fn identity(resource_id: &str) -> TranslationResourceIdentity {
    TranslationResourceIdentity {
        owner_slug: OwnerSlug::new("media").unwrap(),
        resource_kind: ResourceKind::new("asset").unwrap(),
        resource_id: ResourceId::new(resource_id).unwrap(),
        subresource_id: None,
    }
}

async fn create_approved_item(
    service: &TranslationWorkflowService,
    tenant_id: Uuid,
    suffix: &str,
) -> (Uuid, Uuid) {
    let job = service
        .create_job(
            write_context(tenant_id, &format!("create-job-{suffix}")),
            job_input("de"),
        )
        .await
        .unwrap();
    let item = service
        .add_item(
            write_context(tenant_id, &format!("add-item-{suffix}")),
            AddItemInput {
                job_id: job.id,
                identity: identity(&format!("asset-{suffix}")),
            },
        )
        .await
        .unwrap();
    let translator_id = Uuid::new_v4();
    let reviewer_id = Uuid::new_v4();
    let proposal = service
        .save_proposal(
            user_write_context(
                tenant_id,
                translator_id,
                Action::Update,
                &format!("save-proposal-{suffix}"),
            ),
            SaveProposalInput {
                item_id: item.id,
                origin: ProposalOrigin::Manual,
                values: vec![ProposalValue {
                    key: FieldKey::new("title").unwrap(),
                    value: "Held".to_string(),
                }],
            },
        )
        .await
        .unwrap();
    service
        .submit_proposal(
            user_write_context(
                tenant_id,
                translator_id,
                Action::Update,
                &format!("submit-proposal-{suffix}"),
            ),
            SubmitProposalInput {
                item_id: item.id,
                proposal_id: proposal.id,
            },
        )
        .await
        .unwrap();
    service
        .approve_proposal(
            user_write_context(
                tenant_id,
                reviewer_id,
                Action::Resolve,
                &format!("approve-proposal-{suffix}"),
            ),
            ApproveProposalInput {
                item_id: item.id,
                proposal_id: proposal.id,
            },
        )
        .await
        .unwrap();
    (item.id, proposal.id)
}

async fn event_count(database: &DatabaseConnection, event_type: &str) -> u64 {
    SysEvents::find()
        .filter(rustok_outbox::entity::Column::EventType.eq(event_type))
        .count(database)
        .await
        .unwrap()
}

async fn event_payloads(database: &DatabaseConnection) -> Vec<serde_json::Value> {
    SysEvents::find()
        .all(database)
        .await
        .unwrap()
        .into_iter()
        .map(|event| event.payload)
        .collect()
}

#[tokio::test]
async fn assignment_is_actor_bound_cas_guarded_and_replay_safe() {
    let (database, service, tenant_id) = fixture().await;
    let job = service
        .create_job(
            write_context(tenant_id, "create-job-assignment"),
            job_input("de"),
        )
        .await
        .unwrap();
    let item = service
        .add_item(
            write_context(tenant_id, "add-item-assignment"),
            AddItemInput {
                job_id: job.id,
                identity: identity("asset-assignment"),
            },
        )
        .await
        .unwrap();
    let manager_id = Uuid::new_v4();
    let assignee_id = Uuid::new_v4();
    let assign_input = AssignItemInput {
        item_id: item.id,
        expected_revision: 0,
        assignee: PortActor::user(assignee_id.to_string()),
    };
    let unauthorized = service
        .assign_item(
            user_write_context(
                tenant_id,
                manager_id,
                Action::Update,
                "assign-item-without-manage",
            ),
            assign_input.clone(),
        )
        .await
        .unwrap_err();
    assert!(matches!(unauthorized, TranslationError::Forbidden));
    let assign_context = user_write_context(tenant_id, manager_id, Action::Manage, "assign-item");
    let assigned = service
        .assign_item(assign_context.clone(), assign_input.clone())
        .await
        .unwrap();
    let replay = service
        .assign_item(assign_context.clone(), assign_input.clone())
        .await
        .unwrap();
    assert_eq!(replay, assigned);
    assert_eq!(assigned.assignee, Some(assign_input.assignee.clone()));
    assert_eq!(assigned.item_revision, 1);
    assert_eq!(
        assignment::Entity::find().count(&database).await.unwrap(),
        1
    );
    assert_eq!(event_count(&database, "translation.item.assigned").await, 1);
    let non_assignee = service
        .save_proposal(
            user_write_context(
                tenant_id,
                Uuid::new_v4(),
                Action::Update,
                "save-assigned-by-other",
            ),
            SaveProposalInput {
                item_id: item.id,
                origin: ProposalOrigin::Manual,
                values: vec![ProposalValue {
                    key: FieldKey::new("title").unwrap(),
                    value: "Denied".to_string(),
                }],
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        non_assignee,
        TranslationError::ItemAssignedToAnotherActor
    ));
    service
        .save_proposal(
            user_write_context(
                tenant_id,
                assignee_id,
                Action::Update,
                "save-assigned-by-assignee",
            ),
            SaveProposalInput {
                item_id: item.id,
                origin: ProposalOrigin::Manual,
                values: vec![ProposalValue {
                    key: FieldKey::new("title").unwrap(),
                    value: "Assigned".to_string(),
                }],
            },
        )
        .await
        .unwrap();

    let changed_request = service
        .assign_item(
            assign_context,
            AssignItemInput {
                assignee: PortActor::service("translation-worker"),
                ..assign_input.clone()
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        changed_request,
        TranslationError::IdempotencyConflict
    ));
    let other_actor = service
        .assign_item(
            user_write_context(tenant_id, Uuid::new_v4(), Action::Manage, "assign-item"),
            assign_input,
        )
        .await
        .unwrap_err();
    assert!(matches!(
        other_actor,
        TranslationError::IdempotencyActorMismatch
    ));

    let stale_revision = service
        .unassign_item(
            user_write_context(tenant_id, manager_id, Action::Manage, "unassign-item-stale"),
            UnassignItemInput {
                item_id: item.id,
                expected_revision: 0,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        stale_revision,
        TranslationError::WorkflowRevisionConflict
    ));
    let unassign_input = UnassignItemInput {
        item_id: item.id,
        expected_revision: 2,
    };
    let unassign_context =
        user_write_context(tenant_id, manager_id, Action::Manage, "unassign-item");
    let unassigned = service
        .unassign_item(unassign_context.clone(), unassign_input.clone())
        .await
        .unwrap();
    let unassign_replay = service
        .unassign_item(unassign_context, unassign_input)
        .await
        .unwrap();
    assert_eq!(unassign_replay, unassigned);
    assert_eq!(unassigned.assignee, None);
    assert_eq!(unassigned.item_revision, 3);
    assert_eq!(
        event_count(&database, "translation.item.unassigned").await,
        1
    );
    let persisted = job_item::Entity::find_by_id(item.id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert!(persisted.assigned_actor_kind.is_none());
    assert!(persisted.assigned_actor_id.is_none());
    assert_eq!(persisted.revision, 3);
}

#[tokio::test]
async fn job_cancellation_preserves_applied_items_and_cancels_remaining_work_once() {
    let (database, service, tenant_id) = fixture().await;
    let job = service
        .create_job(
            write_context(tenant_id, "create-job-cancel"),
            job_input("de"),
        )
        .await
        .unwrap();
    let applied_item = service
        .add_item(
            write_context(tenant_id, "add-item-cancel-applied"),
            AddItemInput {
                job_id: job.id,
                identity: identity("asset-cancel-applied"),
            },
        )
        .await
        .unwrap();
    let pending_item = service
        .add_item(
            write_context(tenant_id, "add-item-cancel-pending"),
            AddItemInput {
                job_id: job.id,
                identity: identity("asset-cancel-pending"),
            },
        )
        .await
        .unwrap();
    service
        .assign_item(
            write_context(tenant_id, "assign-item-cancel-pending"),
            AssignItemInput {
                item_id: pending_item.id,
                expected_revision: 0,
                assignee: PortActor::service("translation-worker"),
            },
        )
        .await
        .unwrap();
    let translator_id = Uuid::new_v4();
    let reviewer_id = Uuid::new_v4();
    let proposal = service
        .save_proposal(
            user_write_context(
                tenant_id,
                translator_id,
                Action::Update,
                "save-cancel-applied",
            ),
            SaveProposalInput {
                item_id: applied_item.id,
                origin: ProposalOrigin::Manual,
                values: vec![ProposalValue {
                    key: FieldKey::new("title").unwrap(),
                    value: "Applied".to_string(),
                }],
            },
        )
        .await
        .unwrap();
    service
        .submit_proposal(
            user_write_context(
                tenant_id,
                translator_id,
                Action::Update,
                "submit-cancel-applied",
            ),
            SubmitProposalInput {
                item_id: applied_item.id,
                proposal_id: proposal.id,
            },
        )
        .await
        .unwrap();
    service
        .approve_proposal(
            user_write_context(
                tenant_id,
                reviewer_id,
                Action::Resolve,
                "approve-cancel-applied",
            ),
            ApproveProposalInput {
                item_id: applied_item.id,
                proposal_id: proposal.id,
            },
        )
        .await
        .unwrap();
    service
        .apply_proposal(
            write_context(tenant_id, "apply-cancel-applied"),
            ApplyProposalInput {
                item_id: applied_item.id,
                proposal_id: proposal.id,
            },
        )
        .await
        .unwrap();
    let persisted_job = job::Entity::find_by_id(job.id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(persisted_job.revision, 2);

    let manager_id = Uuid::new_v4();
    let cancel_input = CancelJobInput {
        job_id: job.id,
        expected_revision: 2,
        reason: "The target locale is no longer required".to_string(),
    };
    let invalid_reason = service
        .cancel_job(
            user_write_context(
                tenant_id,
                Uuid::new_v4(),
                Action::Manage,
                "cancel-job-invalid-reason",
            ),
            CancelJobInput {
                reason: " ".to_string(),
                ..cancel_input.clone()
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        invalid_reason,
        TranslationError::InvalidCancellationReason
    ));
    let unauthorized = service
        .cancel_job(
            user_write_context(
                tenant_id,
                Uuid::new_v4(),
                Action::Update,
                "cancel-job-without-manage",
            ),
            cancel_input.clone(),
        )
        .await
        .unwrap_err();
    assert!(matches!(unauthorized, TranslationError::Forbidden));
    let cancel_context = user_write_context(tenant_id, manager_id, Action::Manage, "cancel-job");
    let cancelled = service
        .cancel_job(cancel_context.clone(), cancel_input.clone())
        .await
        .unwrap();
    let replay = service
        .cancel_job(cancel_context.clone(), cancel_input.clone())
        .await
        .unwrap();
    assert_eq!(replay, cancelled);
    assert_eq!(cancelled.cancelled_item_count, 1);
    assert_eq!(cancelled.job_revision, 3);
    assert_eq!(
        cancellation::Entity::find().count(&database).await.unwrap(),
        1
    );
    assert_eq!(event_count(&database, "translation.job.cancelled").await, 1);
    let applied = job_item::Entity::find_by_id(applied_item.id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    let pending = job_item::Entity::find_by_id(pending_item.id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(applied.status, "applied");
    assert_eq!(pending.status, "cancelled");
    assert_eq!(pending.revision, 2);
    assert!(pending.assigned_actor_kind.is_none());
    assert!(pending.assigned_actor_id.is_none());

    let changed_reason = service
        .cancel_job(
            cancel_context,
            CancelJobInput {
                reason: "A different reason".to_string(),
                ..cancel_input
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        changed_reason,
        TranslationError::IdempotencyConflict
    ));
}

#[tokio::test]
async fn cancellation_rejects_an_unknown_owner_outcome() {
    let (database, service, tenant_id, apply_state) = fixture_with_apply_state().await;
    let job = service
        .create_job(
            write_context(tenant_id, "create-job-cancel-applying"),
            job_input("de"),
        )
        .await
        .unwrap();
    let item = service
        .add_item(
            write_context(tenant_id, "add-item-cancel-applying"),
            AddItemInput {
                job_id: job.id,
                identity: identity("asset-cancel-applying"),
            },
        )
        .await
        .unwrap();
    let translator_id = Uuid::new_v4();
    let proposal = service
        .save_proposal(
            user_write_context(
                tenant_id,
                translator_id,
                Action::Update,
                "save-cancel-applying",
            ),
            SaveProposalInput {
                item_id: item.id,
                origin: ProposalOrigin::Manual,
                values: vec![ProposalValue {
                    key: FieldKey::new("title").unwrap(),
                    value: "Applying".to_string(),
                }],
            },
        )
        .await
        .unwrap();
    service
        .submit_proposal(
            user_write_context(
                tenant_id,
                translator_id,
                Action::Update,
                "submit-cancel-applying",
            ),
            SubmitProposalInput {
                item_id: item.id,
                proposal_id: proposal.id,
            },
        )
        .await
        .unwrap();
    service
        .approve_proposal(
            user_write_context(
                tenant_id,
                Uuid::new_v4(),
                Action::Resolve,
                "approve-cancel-applying",
            ),
            ApproveProposalInput {
                item_id: item.id,
                proposal_id: proposal.id,
            },
        )
        .await
        .unwrap();
    apply_state.fail_after_commit.store(true, Ordering::SeqCst);
    service
        .apply_proposal(
            write_context(tenant_id, "apply-cancel-applying"),
            ApplyProposalInput {
                item_id: item.id,
                proposal_id: proposal.id,
            },
        )
        .await
        .unwrap_err();

    let error = service
        .cancel_job(
            user_write_context(tenant_id, Uuid::new_v4(), Action::Manage, "cancel-applying"),
            CancelJobInput {
                job_id: job.id,
                expected_revision: 1,
                reason: "Do not race an unresolved owner mutation".to_string(),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(error, TranslationError::JobCancellationInProgress));
    assert_eq!(
        cancellation::Entity::find().count(&database).await.unwrap(),
        0
    );
    assert_eq!(event_count(&database, "translation.job.cancelled").await, 0);
}

#[tokio::test]
async fn outbox_failure_rolls_back_the_workflow_mutation() {
    let (database, service, tenant_id) = fixture().await;
    database
        .execute_unprepared("DROP TABLE sys_events")
        .await
        .unwrap();

    let error = service
        .create_job(
            write_context(tenant_id, "create-job-without-outbox"),
            job_input("de"),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, TranslationError::Event(_)));
    assert_eq!(job::Entity::find().count(&database).await.unwrap(), 0);
}

#[tokio::test]
async fn workflow_events_are_content_free_and_replay_once() {
    let (database, service, tenant_id, _) = fixture_with_apply_state().await;
    let (item_id, proposal_id) = create_approved_item(&service, tenant_id, "event-flow").await;
    let input = ApplyProposalInput {
        item_id,
        proposal_id,
    };
    let context = write_context(tenant_id, "apply-event-flow");
    service
        .apply_proposal(context.clone(), input.clone())
        .await
        .unwrap();
    service.apply_proposal(context, input).await.unwrap();

    for (event_type, expected) in [
        ("translation.job.created", 1),
        ("translation.proposal.submitted", 1),
        ("translation.proposal.approved", 1),
        ("translation.apply.requested", 1),
        ("translation.apply.completed", 1),
        ("translation.job.completed", 1),
    ] {
        assert_eq!(event_count(&database, event_type).await, expected);
    }
    let serialized = serde_json::to_string(&event_payloads(&database).await).unwrap();
    assert!(!serialized.contains("Hero"));
    assert!(!serialized.contains("Held"));
    assert!(!serialized.contains("source_snapshot"));
    assert!(!serialized.contains("values"));
}

#[tokio::test]
async fn successful_terminal_apply_completes_the_job_and_updates_progress_atomically() {
    let (database, service, tenant_id, _) = fixture_with_apply_state().await;
    let progress_service = TranslationProgressService::new(
        database.clone(),
        Arc::new(TranslationTargetRegistry::default()),
    );
    let (item_id, proposal_id) =
        create_approved_item(&service, tenant_id, "progress-completion").await;
    let item = job_item::Entity::find_by_id(item_id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();

    let approved = progress_service
        .read_job_progress(read_context(tenant_id), item.job_id)
        .await
        .unwrap();
    assert_eq!(approved.total_items, 1);
    assert_eq!(approved.approved_items, 1);
    assert_eq!(approved.required_units, 1);
    assert_eq!(approved.approved_required_units, 1);
    assert_eq!(approved.applied_required_units, 0);
    assert_eq!(approved.source_characters, 4);
    assert_eq!(approved.complete_resources, 0);

    service
        .apply_proposal(
            write_context(tenant_id, "apply-progress-completion"),
            ApplyProposalInput {
                item_id,
                proposal_id,
            },
        )
        .await
        .unwrap();

    let completed_job = job::Entity::find_by_id(item.job_id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completed_job.status, "completed");
    assert_eq!(completed_job.revision, 2);
    let completed = progress_service
        .read_job_progress(read_context(tenant_id), item.job_id)
        .await
        .unwrap();
    assert_eq!(completed.total_items, 1);
    assert_eq!(completed.terminal_items, 1);
    assert_eq!(completed.applied_items, 1);
    assert_eq!(completed.approved_items, 0);
    assert_eq!(completed.applied_required_units, 1);
    assert_eq!(completed.approved_required_units, 0);
    assert_eq!(completed.complete_resources, 1);
    assert_eq!(completed.translated_characters, 4);
    assert!(completed.revision > approved.revision);
    assert_eq!(event_count(&database, "translation.job.completed").await, 1);
}

#[tokio::test]
async fn progress_rebuild_repairs_drift_is_idempotent_and_tenant_isolated() {
    let (database, service, tenant_id) = fixture().await;
    let progress_service = TranslationProgressService::new(
        database.clone(),
        Arc::new(TranslationTargetRegistry::default()),
    );
    let job = service
        .create_job(
            write_context(tenant_id, "create-job-progress-rebuild"),
            job_input("de"),
        )
        .await
        .unwrap();
    service
        .add_item(
            write_context(tenant_id, "add-item-progress-rebuild"),
            AddItemInput {
                job_id: job.id,
                identity: identity("asset-progress-rebuild"),
            },
        )
        .await
        .unwrap();
    let before = progress_service
        .read_job_progress(read_context(tenant_id), job.id)
        .await
        .unwrap();
    assert_eq!(before.missing_items, 1);
    assert_eq!(before.source_characters, 4);

    job_progress::Entity::update_many()
        .col_expr(job_progress::Column::SourceCharacters, Expr::value(999_i64))
        .filter(job_progress::Column::TenantId.eq(tenant_id))
        .filter(job_progress::Column::JobId.eq(job.id))
        .exec(&database)
        .await
        .unwrap();
    let drifted = progress_service
        .read_job_progress(read_context(tenant_id), job.id)
        .await
        .unwrap();
    assert_eq!(drifted.source_characters, 999);

    let repaired = progress_service
        .rebuild_job_progress(
            user_write_context(
                tenant_id,
                Uuid::new_v4(),
                Action::Manage,
                "rebuild-job-progress",
            ),
            job.id,
        )
        .await
        .unwrap();
    assert_eq!(repaired.source_characters, 4);
    assert_eq!(repaired.source_digest, before.source_digest);
    assert_eq!(repaired.revision, before.revision + 1);
    let replay = progress_service
        .rebuild_job_progress(
            user_write_context(
                tenant_id,
                Uuid::new_v4(),
                Action::Manage,
                "rebuild-job-progress-again",
            ),
            job.id,
        )
        .await
        .unwrap();
    assert_eq!(replay.revision, repaired.revision);

    let other_tenant_id = Uuid::new_v4();
    seed_tenant(&database, other_tenant_id).await;
    let error = progress_service
        .read_job_progress(read_context(other_tenant_id), job.id)
        .await
        .unwrap_err();
    assert!(matches!(error, TranslationError::JobNotFound));
}

#[tokio::test]
async fn blocked_item_retry_is_explicit_actor_bound_and_does_not_retry_conflicts() {
    let (database, service, tenant_id, apply_state) = fixture_with_apply_state().await;
    let progress_service = TranslationProgressService::new(
        database.clone(),
        Arc::new(TranslationTargetRegistry::default()),
    );
    let (item_id, proposal_id) = create_approved_item(&service, tenant_id, "retry-blocked").await;
    apply_state
        .next_error
        .lock()
        .await
        .replace(PortError::forbidden(
            "media.translation_temporarily_blocked",
            "operator intervention is required",
        ));
    service
        .apply_proposal(
            write_context(tenant_id, "apply-retry-blocked"),
            ApplyProposalInput {
                item_id,
                proposal_id,
            },
        )
        .await
        .unwrap_err();
    let blocked = job_item::Entity::find_by_id(item_id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(blocked.status, "blocked");
    let progress = progress_service
        .read_job_progress(read_context(tenant_id), blocked.job_id)
        .await
        .unwrap();
    assert_eq!(progress.blocked_items, 1);
    assert_eq!(progress.approved_items, 0);

    let retry_actor = Uuid::new_v4();
    let retry_input = RetryItemInput {
        item_id,
        expected_revision: blocked.revision,
        reason: "The owner policy issue has been resolved".to_string(),
    };
    let invalid_reason = service
        .retry_item(
            user_write_context(
                tenant_id,
                Uuid::new_v4(),
                Action::Manage,
                "retry-blocked-invalid-reason",
            ),
            RetryItemInput {
                reason: " ".to_string(),
                ..retry_input.clone()
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        invalid_reason,
        TranslationError::InvalidRetryReason
    ));
    let unauthorized = service
        .retry_item(
            user_write_context(
                tenant_id,
                Uuid::new_v4(),
                Action::Update,
                "retry-blocked-without-manage",
            ),
            retry_input.clone(),
        )
        .await
        .unwrap_err();
    assert!(matches!(unauthorized, TranslationError::Forbidden));
    let retry_context = user_write_context(tenant_id, retry_actor, Action::Manage, "retry-blocked");
    let retried = service
        .retry_item(retry_context.clone(), retry_input.clone())
        .await
        .unwrap();
    assert_eq!(retried.status, "approved");
    assert_eq!(retried.item_revision, blocked.revision + 1);
    let replay = service
        .retry_item(retry_context.clone(), retry_input.clone())
        .await
        .unwrap();
    assert_eq!(replay, retried);
    let changed = service
        .retry_item(
            retry_context,
            RetryItemInput {
                reason: "A different retry reason".to_string(),
                ..retry_input.clone()
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(changed, TranslationError::IdempotencyConflict));
    let other_actor = service
        .retry_item(
            user_write_context(tenant_id, Uuid::new_v4(), Action::Manage, "retry-blocked"),
            retry_input,
        )
        .await
        .unwrap_err();
    assert!(matches!(
        other_actor,
        TranslationError::IdempotencyActorMismatch
    ));
    assert_eq!(retry::Entity::find().count(&database).await.unwrap(), 1);
    assert_eq!(
        event_count(&database, "translation.item.retry_requested").await,
        1
    );
    let serialized = serde_json::to_string(&event_payloads(&database).await).unwrap();
    assert!(!serialized.contains("The owner policy issue has been resolved"));
    let retried_progress = progress_service
        .read_job_progress(read_context(tenant_id), blocked.job_id)
        .await
        .unwrap();
    assert_eq!(retried_progress.blocked_items, 0);
    assert_eq!(retried_progress.approved_items, 1);

    service
        .apply_proposal(
            write_context(tenant_id, "apply-after-explicit-retry"),
            ApplyProposalInput {
                item_id,
                proposal_id,
            },
        )
        .await
        .unwrap();
    let completed = job::Entity::find_by_id(blocked.job_id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completed.status, "completed");

    let (conflict_item_id, conflict_proposal_id) =
        create_approved_item(&service, tenant_id, "retry-conflict").await;
    apply_state
        .next_error
        .lock()
        .await
        .replace(PortError::conflict(
            "media.translation_patch_conflict",
            "live owner revisions changed",
        ));
    service
        .apply_proposal(
            write_context(tenant_id, "apply-retry-conflict"),
            ApplyProposalInput {
                item_id: conflict_item_id,
                proposal_id: conflict_proposal_id,
            },
        )
        .await
        .unwrap_err();
    let conflict = job_item::Entity::find_by_id(conflict_item_id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    let error = service
        .retry_item(
            user_write_context(tenant_id, Uuid::new_v4(), Action::Manage, "retry-conflict"),
            RetryItemInput {
                item_id: conflict_item_id,
                expected_revision: conflict.revision,
                reason: "Do not blindly reuse stale owner revisions".to_string(),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        TranslationError::ItemNotRetryable(status) if status == "conflict"
    ));
}

#[tokio::test]
async fn job_creation_replays_exact_request_and_rejects_changed_hash() {
    let (database, service, tenant_id) = fixture().await;
    let context = write_context(tenant_id, "create-job-1");

    let first = service
        .create_job(context.clone(), job_input("de"))
        .await
        .unwrap();
    let replay = service
        .create_job(context.clone(), job_input("de"))
        .await
        .unwrap();
    assert_eq!(replay, first);

    let error = service
        .create_job(context, job_input("fr"))
        .await
        .unwrap_err();
    assert!(matches!(error, TranslationError::IdempotencyConflict));
    assert_eq!(job::Entity::find().count(&database).await.unwrap(), 1);
}

#[tokio::test]
async fn adding_item_snapshots_owner_resource_and_replays_before_provider_call() {
    let (database, service, tenant_id) = fixture().await;
    let job = service
        .create_job(write_context(tenant_id, "create-job-2"), job_input("de"))
        .await
        .unwrap();
    let input = AddItemInput {
        job_id: job.id,
        identity: identity("asset-1"),
    };
    let context = write_context(tenant_id, "add-item-1");

    let first = service
        .add_item(context.clone(), input.clone())
        .await
        .unwrap();
    let replay = service.add_item(context.clone(), input).await.unwrap();
    assert_eq!(replay, first);

    let error = service
        .add_item(
            context,
            AddItemInput {
                job_id: job.id,
                identity: identity("asset-2"),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(error, TranslationError::IdempotencyConflict));
    assert_eq!(job_item::Entity::find().count(&database).await.unwrap(), 1);
    let persisted_job = job::Entity::find_by_id(job.id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(persisted_job.status, "in_progress");
    assert_eq!(persisted_job.revision, 1);
    let persisted_item = job_item::Entity::find()
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(persisted_item.source_digest.len(), 64);
    assert_eq!(
        persisted_item.source_snapshot["fields"][0]["source_value"],
        "Hero"
    );
}

#[tokio::test]
async fn job_item_creation_is_tenant_isolated() {
    let (database, service, first_tenant_id) = fixture().await;
    let second_tenant_id = Uuid::new_v4();
    seed_tenant(&database, second_tenant_id).await;
    let job = service
        .create_job(
            write_context(first_tenant_id, "create-job-tenant"),
            job_input("de"),
        )
        .await
        .unwrap();

    let error = service
        .add_item(
            write_context(second_tenant_id, "add-item-foreign-job"),
            AddItemInput {
                job_id: job.id,
                identity: identity("asset-1"),
            },
        )
        .await
        .unwrap_err();

    assert!(matches!(error, TranslationError::JobNotFound));
    assert_eq!(job_item::Entity::find().count(&database).await.unwrap(), 0);
}

#[tokio::test]
async fn proposal_moves_through_draft_review_and_separated_approval() {
    let (database, service, tenant_id) = fixture().await;
    let job = service
        .create_job(
            write_context(tenant_id, "create-job-proposal"),
            job_input("de"),
        )
        .await
        .unwrap();
    let item = service
        .add_item(
            write_context(tenant_id, "add-item-proposal"),
            AddItemInput {
                job_id: job.id,
                identity: identity("asset-proposal"),
            },
        )
        .await
        .unwrap();
    let translator_id = Uuid::new_v4();
    let reviewer_id = Uuid::new_v4();
    let save_input = SaveProposalInput {
        item_id: item.id,
        origin: ProposalOrigin::Manual,
        values: vec![ProposalValue {
            key: FieldKey::new("title").unwrap(),
            value: "Held".to_string(),
        }],
    };
    let draft = service
        .save_proposal(
            user_write_context(tenant_id, translator_id, Action::Update, "save-proposal-1"),
            save_input.clone(),
        )
        .await
        .unwrap();
    let replay = service
        .save_proposal(
            user_write_context(tenant_id, translator_id, Action::Update, "save-proposal-1"),
            save_input,
        )
        .await
        .unwrap();
    assert_eq!(replay, draft);
    assert_eq!(draft.status, "draft");
    assert_eq!(draft.values[0].expected_source_hash, "sha256:hero");

    let submitted = service
        .submit_proposal(
            user_write_context(
                tenant_id,
                translator_id,
                Action::Update,
                "submit-proposal-1",
            ),
            SubmitProposalInput {
                item_id: item.id,
                proposal_id: draft.id,
            },
        )
        .await
        .unwrap();
    assert_eq!(submitted.status, "in_review");

    let separation_error = service
        .approve_proposal(
            user_write_context(
                tenant_id,
                translator_id,
                Action::Resolve,
                "approve-proposal-self",
            ),
            ApproveProposalInput {
                item_id: item.id,
                proposal_id: draft.id,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        separation_error,
        TranslationError::ReviewerSeparationRequired
    ));

    let approved = service
        .approve_proposal(
            user_write_context(
                tenant_id,
                reviewer_id,
                Action::Resolve,
                "approve-proposal-1",
            ),
            ApproveProposalInput {
                item_id: item.id,
                proposal_id: draft.id,
            },
        )
        .await
        .unwrap();
    let approval_replay = service
        .approve_proposal(
            user_write_context(
                tenant_id,
                reviewer_id,
                Action::Resolve,
                "approve-proposal-1",
            ),
            ApproveProposalInput {
                item_id: item.id,
                proposal_id: draft.id,
            },
        )
        .await
        .unwrap();
    assert_eq!(approval_replay, approved);
    assert_eq!(approved.status, "approved");
    assert!(approved.approval_receipt_id.is_some());

    let persisted_item = job_item::Entity::find_by_id(item.id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(persisted_item.status, "approved");
    assert_eq!(persisted_item.revision, 3);
    assert_eq!(proposal::Entity::find().count(&database).await.unwrap(), 1);
}

#[tokio::test]
async fn proposal_rejects_unknown_snapshot_fields_without_persistence() {
    let (database, service, tenant_id) = fixture().await;
    let job = service
        .create_job(
            write_context(tenant_id, "create-job-invalid-proposal"),
            job_input("de"),
        )
        .await
        .unwrap();
    let item = service
        .add_item(
            write_context(tenant_id, "add-item-invalid-proposal"),
            AddItemInput {
                job_id: job.id,
                identity: identity("asset-invalid-proposal"),
            },
        )
        .await
        .unwrap();

    let error = service
        .save_proposal(
            write_context(tenant_id, "save-invalid-proposal"),
            SaveProposalInput {
                item_id: item.id,
                origin: ProposalOrigin::Manual,
                values: vec![ProposalValue {
                    key: FieldKey::new("not-owned").unwrap(),
                    value: "Invalid".to_string(),
                }],
            },
        )
        .await
        .unwrap_err();

    assert!(matches!(error, TranslationError::InvalidRequest(_)));
    assert_eq!(proposal::Entity::find().count(&database).await.unwrap(), 0);
}

#[tokio::test]
async fn apply_persists_owner_receipt_and_replays_without_another_owner_call() {
    let (database, service, tenant_id, apply_state) = fixture_with_apply_state().await;
    let (item_id, proposal_id) = create_approved_item(&service, tenant_id, "apply-success").await;
    let input = ApplyProposalInput {
        item_id,
        proposal_id,
    };
    let context = write_context(tenant_id, "apply-success");

    let applied = service
        .apply_proposal(context.clone(), input.clone())
        .await
        .unwrap();
    let replay = service
        .apply_proposal(context.clone(), input.clone())
        .await
        .unwrap();
    assert_eq!(replay, applied);
    assert_eq!(apply_state.calls.load(Ordering::SeqCst), 1);
    assert_eq!(applied.resource_revision.as_str(), "resource-8");
    assert_eq!(applied.target_revision.as_str(), "target-1");
    assert_eq!(
        applied.applied_field_keys,
        [FieldKey::new("title").unwrap()]
    );

    let changed_request = service
        .apply_proposal(
            context.clone(),
            ApplyProposalInput {
                item_id,
                proposal_id: Uuid::new_v4(),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        changed_request,
        TranslationError::IdempotencyConflict
    ));
    let other_actor = service
        .apply_proposal(
            user_write_context(tenant_id, Uuid::new_v4(), Action::Publish, "apply-success"),
            input,
        )
        .await
        .unwrap_err();
    assert!(matches!(
        other_actor,
        TranslationError::IdempotencyActorMismatch
    ));
    assert_eq!(apply_state.calls.load(Ordering::SeqCst), 1);

    let operation = apply_operation::Entity::find()
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(operation.status, "completed");
    assert_eq!(operation.attempt_count, 1);
    assert!(operation.completed_at.is_some());
    assert_eq!(
        apply_receipt::Entity::find()
            .count(&database)
            .await
            .unwrap(),
        1
    );
    let item = job_item::Entity::find_by_id(item_id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(item.status, "applied");
    assert_eq!(item.revision, 5);
    assert!(item.active_apply_operation_id.is_none());
}

#[tokio::test]
async fn apply_reconciles_an_unknown_owner_outcome_with_the_same_intent() {
    let (database, service, tenant_id, apply_state) = fixture_with_apply_state().await;
    let (item_id, proposal_id) = create_approved_item(&service, tenant_id, "apply-unknown").await;
    apply_state.fail_after_commit.store(true, Ordering::SeqCst);
    let input = ApplyProposalInput {
        item_id,
        proposal_id,
    };
    let context = write_context(tenant_id, "apply-unknown");

    let first_error = service
        .apply_proposal(context.clone(), input.clone())
        .await
        .unwrap_err();
    assert!(matches!(
        first_error,
        TranslationError::Provider {
            retryable: true,
            ..
        }
    ));
    let pending = apply_operation::Entity::find()
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pending.status, "pending");
    assert_eq!(
        pending.last_error_code.as_deref(),
        Some("translation.test_unknown_outcome")
    );
    assert_eq!(pending.last_error_retryable, Some(true));
    let applying_item = job_item::Entity::find_by_id(item_id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(applying_item.status, "applying");
    assert_eq!(applying_item.active_apply_operation_id, Some(pending.id));
    assert_eq!(
        apply_receipt::Entity::find()
            .count(&database)
            .await
            .unwrap(),
        0
    );
    assert_eq!(event_count(&database, "translation.apply.failed").await, 1);

    let reconciled = service.apply_proposal(context, input).await.unwrap();
    assert_eq!(reconciled.provider_receipt_id, "provider:apply-unknown");
    assert_eq!(apply_state.calls.load(Ordering::SeqCst), 2);
    let completed = apply_operation::Entity::find_by_id(pending.id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completed.status, "completed");
    assert_eq!(completed.attempt_count, 2);
    assert_eq!(
        apply_receipt::Entity::find()
            .count(&database)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        event_count(&database, "translation.apply.completed").await,
        1
    );
}

#[tokio::test]
async fn privileged_recovery_actor_takes_over_the_original_owner_identity_once() {
    let (database, service, tenant_id, apply_state) = fixture_with_apply_state().await;
    let (item_id, proposal_id) = create_approved_item(&service, tenant_id, "apply-takeover").await;
    apply_state.fail_after_commit.store(true, Ordering::SeqCst);
    let apply_input = ApplyProposalInput {
        item_id,
        proposal_id,
    };
    service
        .apply_proposal(
            write_context(tenant_id, "apply-takeover"),
            apply_input.clone(),
        )
        .await
        .unwrap_err();
    let pending = apply_operation::Entity::find()
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pending.status, "pending");
    assert_eq!(pending.attempt_count, 1);
    assert!(pending.lease_token.is_none());

    let stale_attempt = service
        .recover_apply(
            recovery_context(tenant_id, Uuid::new_v4(), "recover-takeover-stale"),
            RecoverApplyInput {
                operation_id: pending.id,
                expected_attempt_count: 0,
                reason: "Recover an owner response lost after commit".to_string(),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        stale_attempt,
        TranslationError::ApplyRecoveryAttemptMismatch
    ));
    assert_eq!(
        apply_recovery::Entity::find()
            .count(&database)
            .await
            .unwrap(),
        0
    );

    let recovery_actor = Uuid::new_v4();
    let recovery_input = RecoverApplyInput {
        operation_id: pending.id,
        expected_attempt_count: 1,
        reason: "Recover an owner response lost after commit".to_string(),
    };
    let unauthorized = service
        .recover_apply(
            user_write_context(
                tenant_id,
                Uuid::new_v4(),
                Action::Publish,
                "recover-takeover-without-manage",
            ),
            recovery_input.clone(),
        )
        .await
        .unwrap_err();
    assert!(matches!(unauthorized, TranslationError::Forbidden));
    let recovery_call = recovery_context(tenant_id, recovery_actor, "recover-takeover");
    let recovered = service
        .recover_apply(recovery_call.clone(), recovery_input.clone())
        .await
        .unwrap();
    assert_eq!(recovered.provider_receipt_id, "provider:apply-takeover");
    let replay = service
        .recover_apply(recovery_call, recovery_input.clone())
        .await
        .unwrap();
    assert_eq!(replay, recovered);
    assert_eq!(apply_state.calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        apply_recovery::Entity::find()
            .count(&database)
            .await
            .unwrap(),
        1
    );
    let recovery = apply_recovery::Entity::find()
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recovery.requested_by_actor_id, recovery_actor.to_string());
    assert_eq!(recovery.observed_attempt_count, 1);
    assert_eq!(
        event_count(&database, "translation.apply.recovery_requested").await,
        1
    );

    let changed_request = service
        .recover_apply(
            recovery_context(tenant_id, recovery_actor, "recover-takeover"),
            RecoverApplyInput {
                reason: "Different recovery reason".to_string(),
                ..recovery_input.clone()
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        changed_request,
        TranslationError::IdempotencyConflict
    ));
    let other_actor = service
        .recover_apply(
            recovery_context(tenant_id, Uuid::new_v4(), "recover-takeover"),
            recovery_input,
        )
        .await
        .unwrap_err();
    assert!(matches!(
        other_actor,
        TranslationError::IdempotencyActorMismatch
    ));
    assert_eq!(apply_state.calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn recovery_cannot_steal_an_unexpired_apply_lease() {
    let (database, service, tenant_id, apply_state) = fixture_with_apply_state().await;
    let (item_id, proposal_id) = create_approved_item(&service, tenant_id, "apply-leased").await;
    apply_state.fail_after_commit.store(true, Ordering::SeqCst);
    service
        .apply_proposal(
            write_context(tenant_id, "apply-leased"),
            ApplyProposalInput {
                item_id,
                proposal_id,
            },
        )
        .await
        .unwrap_err();
    let operation = apply_operation::Entity::find()
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    let simulated_lease = Uuid::new_v4();
    apply_operation::Entity::update_many()
        .col_expr(
            apply_operation::Column::LeaseToken,
            Expr::value(Some(simulated_lease)),
        )
        .col_expr(
            apply_operation::Column::LeaseOwnerActorKind,
            Expr::value(Some("service".to_string())),
        )
        .col_expr(
            apply_operation::Column::LeaseOwnerActorId,
            Expr::value(Some(Uuid::new_v4().to_string())),
        )
        .col_expr(
            apply_operation::Column::LeaseExpiresAt,
            Expr::value(Some(Utc::now().fixed_offset() + ChronoDuration::minutes(1))),
        )
        .filter(apply_operation::Column::Id.eq(operation.id))
        .exec(&database)
        .await
        .unwrap();

    let recovery_error = service
        .recover_apply(
            recovery_context(tenant_id, Uuid::new_v4(), "recover-leased"),
            RecoverApplyInput {
                operation_id: operation.id,
                expected_attempt_count: 1,
                reason: "Recover only after the active executor lease expires".to_string(),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(recovery_error, TranslationError::ApplyInProgress));
    assert_eq!(
        apply_recovery::Entity::find()
            .count(&database)
            .await
            .unwrap(),
        0
    );
    assert_eq!(apply_state.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn non_retryable_owner_conflict_is_terminal_and_releases_the_item() {
    let (database, service, tenant_id, apply_state) = fixture_with_apply_state().await;
    let (item_id, proposal_id) = create_approved_item(&service, tenant_id, "apply-conflict").await;
    apply_state
        .next_error
        .lock()
        .await
        .replace(PortError::conflict(
            "media.translation_patch_conflict",
            "live owner revisions changed",
        ));
    let input = ApplyProposalInput {
        item_id,
        proposal_id,
    };
    let context = write_context(tenant_id, "apply-conflict");

    let first_error = service
        .apply_proposal(context.clone(), input.clone())
        .await
        .unwrap_err();
    assert!(matches!(
        first_error,
        TranslationError::Provider {
            retryable: false,
            ..
        }
    ));
    let operation = apply_operation::Entity::find()
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(operation.status, "conflict");
    assert_eq!(
        operation.last_error_code.as_deref(),
        Some("media.translation_patch_conflict")
    );
    let item = job_item::Entity::find_by_id(item_id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(item.status, "conflict");
    assert_eq!(item.revision, 5);
    assert!(item.active_apply_operation_id.is_none());
    assert_eq!(event_count(&database, "translation.apply.failed").await, 1);

    let replay_error = service.apply_proposal(context, input).await.unwrap_err();
    assert!(matches!(
        replay_error,
        TranslationError::ApplyOperationTerminal { status, code }
            if status == "conflict" && code == "media.translation_patch_conflict"
    ));
    assert_eq!(apply_state.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        apply_receipt::Entity::find()
            .count(&database)
            .await
            .unwrap(),
        0
    );
}
