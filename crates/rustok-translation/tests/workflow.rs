use std::{collections::BTreeSet, sync::Arc, time::Duration};

use async_trait::async_trait;
use rustok_api::{Action, Permission, PortActor, PortContext, PortError, Resource, TenantLocale};
use rustok_translation::{
    AddItemInput, ApproveProposalInput, CreateJobInput, ProposalOrigin, ProposalValue,
    SaveProposalInput, SubmitProposalInput, TranslationError, TranslationWorkflowService,
    entities::{job, job_item, proposal},
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
    ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait, PaginatorTrait,
    Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

struct SnapshotProvider;

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
        _context: PortContext,
        _request: TranslationPatchRequest,
    ) -> Result<TranslationApplicationReceipt, PortError> {
        Err(unavailable())
    }
}

fn unavailable() -> PortError {
    PortError::unavailable("translation.test_unavailable", "not used by this fixture")
}

async fn fixture() -> (DatabaseConnection, TranslationWorkflowService, Uuid) {
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
    for migration in migrations::migrations() {
        migration.up(&manager).await.unwrap();
    }
    let tenant_id = Uuid::new_v4();
    seed_tenant(&database, tenant_id).await;
    let mut registry = TranslationTargetRegistry::default();
    registry.register(SnapshotProvider).unwrap();
    (
        database.clone(),
        TranslationWorkflowService::new(database, Arc::new(registry)),
        tenant_id,
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
