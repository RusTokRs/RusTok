use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

#[cfg(feature = "graphql")]
use async_graphql::{EmptySubscription, Request, Schema, Variables};
use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, Utc};
use rustok_api::{
    Action, Permission, PortActor, PortContext, PortError, Resource, TenantLocale,
    manifest_hash::hash_manifest,
};
#[cfg(feature = "graphql")]
use rustok_api::{AuthContext, RequestContext};
use rustok_core::RetentionPolicy;
use rustok_outbox::{OutboxTransport, SysEvents, SysEventsMigration, TransactionalEventBus};
use rustok_storage::{
    LocalStorageConfig, StorageRuntime,
    object_store::{ObjectStoreExt, path::Path},
};
use rustok_tenant::{
    ReplaceTenantLocalePolicyRequest, TenantLocalePolicyEntry, TenantLocalePolicyPort,
    TenantLocalePolicyProjection,
};
use rustok_translation::{
    AddItemInput, ApplyProposalInput, ApproveProposalInput, AssignItemInput, CancelJobInput,
    CreateGlossaryInput, CreateInterchangeExportArtifactInput, CreateJobInput,
    CreateWorkflowNoteInput, ExportTranslationJobInput, GlossaryBinding, GlossaryConcept,
    GlossaryMatchKind, GlossaryScope, GlossaryTermPolicy, GlossaryVariant,
    ImportTranslationItemInput, ListInterchangeArtifactsInput, ListWorkflowNotesInput,
    MemoryListInput, MemoryLookupInput, MemoryMatchKind, ProcessInterchangeImportArtifactInput,
    ProposalOrigin, ProposalValue, PurgeMemoryEntryInput, ReadInterchangeArtifactInput,
    RecoverApplyInput, ReplaceGlossaryTermsInput, ResolveWorkflowNoteInput, RetryItemInput,
    ReviewerQueueInput, ReviewerWorkloadInput, SaveProposalInput, SetMemoryRetentionInput,
    StoreInterchangeImportArtifactInput, SubmitProposalInput, TombstoneMemoryEntryInput,
    TranslationError, TranslationExchangeService, TranslationGlossaryService,
    TranslationInterchangeArtifactStatus, TranslationMemoryService, TranslationProgressService,
    TranslationWorkflowService, UnassignItemInput,
    entities::{
        apply_operation, apply_receipt, apply_recovery, assignment, cancellation, exchange_job,
        job, job_item, job_progress, memory_entry, memory_receipt, proposal, retry,
    },
    migrations, parse_artifact_document,
};
use rustok_translation_targets::{
    FieldKey, ListTranslationResourcesRequest, OpaqueRevision, OwnerSlug,
    ReadTranslationResourceRequest, ResourceId, ResourceKind, TranslationApplicationReceipt,
    TranslationDataClassification, TranslationFieldDescriptor, TranslationFieldSnapshot,
    TranslationPatchIssue, TranslationPatchIssueSeverity, TranslationPatchRequest,
    TranslationPatchValidation, TranslationResourceIdentity, TranslationResourceLifecycle,
    TranslationResourcePage, TranslationResourceSnapshot, TranslationResourceSummary,
    TranslationStrategy, TranslationTargetCapability, TranslationTargetProvider,
    TranslationTargetProviderDescriptor, TranslationTargetRegistry, TranslationValueProfile,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait,
    PaginatorTrait, QueryFilter, Statement, sea_query::Expr,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use tempfile::TempDir;
use tokio::sync::Mutex;
use uuid::Uuid;

#[cfg(feature = "graphql")]
use rustok_translation::{
    graphql::{TranslationMutation, TranslationQuery},
    graphql_runtime::TranslationGraphqlRuntimeData,
};

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

struct TestTenantLocalePolicies;

#[async_trait]
impl TenantLocalePolicyPort for TestTenantLocalePolicies {
    async fn read_locale_policy(
        &self,
        context: PortContext,
    ) -> Result<TenantLocalePolicyProjection, PortError> {
        let tenant_id = Uuid::parse_str(&context.tenant_id).unwrap();
        Ok(TenantLocalePolicyProjection {
            tenant_id,
            revision: 7,
            default_locale: TenantLocale::new("en").unwrap(),
            locales: ["en", "de", "fr"]
                .into_iter()
                .map(|locale| TenantLocalePolicyEntry {
                    locale: TenantLocale::new(locale).unwrap(),
                    name: locale.to_string(),
                    native_name: locale.to_string(),
                    is_default: locale == "en",
                    is_enabled: true,
                    fallback_locale: (locale != "en").then(|| TenantLocale::new("en").unwrap()),
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
                protected_tokens: Vec::new(),
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
    let (database, tenant_id) = test_database().await;
    let apply_state = Arc::new(ApplyProviderState::default());
    let registry = snapshot_registry(Arc::clone(&apply_state));
    let event_bus = test_event_bus(&database);
    (
        database.clone(),
        TranslationWorkflowService::new(
            database,
            registry,
            Arc::new(TestTenantLocalePolicies),
            event_bus,
        ),
        tenant_id,
        apply_state,
    )
}

async fn test_database() -> (DatabaseConnection, Uuid) {
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
    (database, tenant_id)
}

fn snapshot_registry(apply_state: Arc<ApplyProviderState>) -> Arc<TranslationTargetRegistry> {
    let mut registry = TranslationTargetRegistry::default();
    registry.register(SnapshotProvider { apply_state }).unwrap();
    Arc::new(registry)
}

fn test_event_bus(database: &DatabaseConnection) -> TransactionalEventBus {
    TransactionalEventBus::new(Arc::new(OutboxTransport::new(database.clone())))
}

async fn fixture() -> (DatabaseConnection, TranslationWorkflowService, Uuid) {
    let (database, service, tenant_id, _) = fixture_with_apply_state().await;
    (database, service, tenant_id)
}

async fn exchange_fixture() -> (
    DatabaseConnection,
    TranslationWorkflowService,
    TranslationExchangeService,
    StorageRuntime,
    TempDir,
    Uuid,
) {
    let (database, tenant_id) = test_database().await;
    let apply_state = Arc::new(ApplyProviderState::default());
    let registry = snapshot_registry(apply_state);
    let storage_directory = tempfile::tempdir().unwrap();
    let storage = StorageRuntime::local(&LocalStorageConfig {
        base_dir: storage_directory.path().display().to_string(),
        base_url: "/media".to_string(),
        fsync: false,
    })
    .unwrap();
    let workflow = TranslationWorkflowService::new(
        database.clone(),
        Arc::clone(&registry),
        Arc::new(TestTenantLocalePolicies),
        test_event_bus(&database),
    );
    let exchange = TranslationExchangeService::new(
        database.clone(),
        registry,
        Arc::new(TestTenantLocalePolicies),
        test_event_bus(&database),
        storage.clone(),
    );
    (
        database,
        workflow,
        exchange,
        storage,
        storage_directory,
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

#[cfg(feature = "graphql")]
type TranslationSchema = Schema<TranslationQuery, TranslationMutation, EmptySubscription>;

#[cfg(feature = "graphql")]
async fn graphql_fixture() -> (DatabaseConnection, TranslationSchema, Uuid) {
    let (database, tenant_id) = test_database().await;
    let registry = snapshot_registry(Arc::new(ApplyProviderState::default()));
    let event_bus = test_event_bus(&database);
    let runtime = TranslationGraphqlRuntimeData::new(
        database.clone(),
        registry,
        Arc::new(TestTenantLocalePolicies),
        event_bus,
        None,
        None,
        None,
    );
    let schema = Schema::build(TranslationQuery, TranslationMutation, EmptySubscription)
        .data(runtime)
        .finish();
    (database, schema, tenant_id)
}

#[cfg(feature = "graphql")]
async fn graphql_interchange_artifact_fixture()
-> (DatabaseConnection, TranslationSchema, TempDir, Uuid) {
    let (database, tenant_id) = test_database().await;
    let registry = snapshot_registry(Arc::new(ApplyProviderState::default()));
    let event_bus = test_event_bus(&database);
    let storage_directory = tempfile::tempdir().unwrap();
    let storage = StorageRuntime::local(&LocalStorageConfig {
        base_dir: storage_directory.path().display().to_string(),
        base_url: "/private".to_string(),
        fsync: false,
    })
    .unwrap();
    let runtime = TranslationGraphqlRuntimeData::new(
        database.clone(),
        registry,
        Arc::new(TestTenantLocalePolicies),
        event_bus,
        Some(storage),
        None,
        None,
    );
    let schema = Schema::build(TranslationQuery, TranslationMutation, EmptySubscription)
        .data(runtime)
        .finish();
    (database, schema, storage_directory, tenant_id)
}

#[cfg(feature = "graphql")]
fn graphql_request(
    query: &str,
    variables: serde_json::Value,
    auth_tenant_id: Uuid,
    request_tenant_id: Uuid,
) -> Request {
    let user_id = Uuid::new_v4();
    let permissions = [
        Action::Create,
        Action::Read,
        Action::Update,
        Action::Resolve,
        Action::Import,
        Action::Export,
    ]
    .into_iter()
    .map(|action| Permission::new(Resource::Translations, action))
    .collect();
    Request::new(query)
        .variables(Variables::from_json(variables))
        .data(AuthContext {
            user_id,
            session_id: Uuid::new_v4(),
            tenant_id: auth_tenant_id,
            permissions,
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
        })
        .data(RequestContext {
            tenant_id: request_tenant_id,
            user_id: Some(user_id),
            channel_id: None,
            channel_slug: None,
            channel_resolution_source: None,
            locale: "en".to_string(),
        })
}

#[cfg(feature = "graphql")]
fn assert_graphql_error_code(response: &async_graphql::Response, expected: &str) {
    assert_eq!(response.errors.len(), 1, "{:?}", response.errors);
    assert_eq!(
        response.errors[0]
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("code")),
        Some(&async_graphql::Value::from(expected))
    );
}

fn job_input(target_locale: &str) -> CreateJobInput {
    CreateJobInput {
        source_locale: TenantLocale::new("en").unwrap(),
        target_locale: TenantLocale::new(target_locale).unwrap(),
        glossary: None,
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

async fn submit_item(
    service: &TranslationWorkflowService,
    tenant_id: Uuid,
    actor_id: Uuid,
    item_id: Uuid,
    suffix: &str,
) -> Uuid {
    let proposal = service
        .save_proposal(
            user_write_context(
                tenant_id,
                actor_id,
                Action::Update,
                &format!("save-reviewer-{suffix}"),
            ),
            SaveProposalInput {
                item_id,
                origin: ProposalOrigin::Manual,
                values: vec![ProposalValue {
                    key: FieldKey::new("title").unwrap(),
                    value: format!("Translated {suffix}"),
                }],
            },
        )
        .await
        .unwrap();
    service
        .submit_proposal(
            user_write_context(
                tenant_id,
                actor_id,
                Action::Update,
                &format!("submit-reviewer-{suffix}"),
            ),
            SubmitProposalInput {
                item_id,
                proposal_id: proposal.id,
            },
        )
        .await
        .unwrap();
    proposal.id
}

#[tokio::test]
async fn bounded_interchange_exports_owner_snapshot_and_imports_through_canonical_qa() {
    let (_database, service, tenant_id) = fixture().await;
    let job = service
        .create_job(
            write_context(tenant_id, "interchange-create-job"),
            job_input("de"),
        )
        .await
        .unwrap();
    let item = service
        .add_item(
            write_context(tenant_id, "interchange-add-item"),
            AddItemInput {
                job_id: job.id,
                identity: identity("asset-interchange"),
            },
        )
        .await
        .unwrap();
    let interchange = service.interchange_service();
    assert!(matches!(
        interchange
            .export_job(
                read_context(tenant_id),
                ExportTranslationJobInput {
                    job_id: job.id,
                    max_items: 0,
                },
            )
            .await
            .unwrap_err(),
        TranslationError::InvalidRequest(_)
    ));
    let document = interchange
        .export_job(
            read_context(tenant_id),
            ExportTranslationJobInput {
                job_id: job.id,
                max_items: 10,
            },
        )
        .await
        .unwrap();
    assert_eq!(document.schema_version, 1);
    assert_eq!(document.items.len(), 1);
    assert_eq!(document.items[0].item_id, item.id);
    assert_eq!(
        document.items[0]
            .fields
            .iter()
            .map(|field| field.key.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["title"])
    );

    assert!(matches!(
        interchange
            .import_item(
                write_context(tenant_id, "interchange-import-stale"),
                ImportTranslationItemInput {
                    schema_version: document.schema_version,
                    job_id: document.job_id,
                    item_id: item.id,
                    identity: document.items[0].identity.clone(),
                    source_digest: "stale-source-digest".to_string(),
                    values: vec![ProposalValue {
                        key: FieldKey::new("title").unwrap(),
                        value: "Titel".to_string(),
                    }],
                },
            )
            .await
            .unwrap_err(),
        TranslationError::WorkflowRevisionConflict
    ));

    let proposal = interchange
        .import_item(
            write_context(tenant_id, "interchange-import-item"),
            ImportTranslationItemInput {
                schema_version: document.schema_version,
                job_id: document.job_id,
                item_id: item.id,
                identity: document.items[0].identity.clone(),
                source_digest: document.items[0].source_digest.clone(),
                values: vec![ProposalValue {
                    key: FieldKey::new("title").unwrap(),
                    value: "Titel".to_string(),
                }],
            },
        )
        .await
        .unwrap();
    assert_eq!(proposal.origin, ProposalOrigin::Import);
    assert_eq!(proposal.status, "draft");
}

#[tokio::test]
async fn interchange_artifacts_are_tenant_scoped_expiring_and_report_conflicts() {
    let (database, workflow, exchange, storage, _storage_directory, tenant_id) =
        exchange_fixture().await;
    assert!(matches!(
        parse_artifact_document(
            &"x".repeat(rustok_translation::MAX_INTERCHANGE_ARTIFACT_BYTES + 1)
        ),
        Err(TranslationError::InvalidRequest(_))
    ));
    let job = workflow
        .create_job(
            write_context(tenant_id, "artifact-create-job"),
            job_input("de"),
        )
        .await
        .unwrap();
    let item = workflow
        .add_item(
            write_context(tenant_id, "artifact-add-item"),
            AddItemInput {
                job_id: job.id,
                identity: identity("asset-interchange-artifact"),
            },
        )
        .await
        .unwrap();

    let export_context = write_context(tenant_id, "artifact-export");
    let exported = exchange
        .create_export_artifact(
            export_context.clone(),
            CreateInterchangeExportArtifactInput {
                job_id: job.id,
                max_items: 10,
                expires_in_seconds: 300,
            },
        )
        .await
        .unwrap();
    assert_eq!(exported.status, TranslationInterchangeArtifactStatus::Ready);
    let exported_replay = exchange
        .create_export_artifact(
            export_context,
            CreateInterchangeExportArtifactInput {
                job_id: job.id,
                max_items: 10,
                expires_in_seconds: 300,
            },
        )
        .await
        .unwrap();
    assert_eq!(exported_replay.id, exported.id);

    let exported_content = exchange
        .read_artifact(
            read_context(tenant_id),
            ReadInterchangeArtifactInput {
                artifact_id: exported.id,
            },
        )
        .await
        .unwrap();
    assert_eq!(exported_content.document.items.len(), 1);
    assert_eq!(exported_content.document.items[0].item_id, item.id);
    assert_eq!(
        exported_content.document.items[0].fields[0].proposed_value,
        None
    );

    let other_tenant_id = Uuid::new_v4();
    seed_tenant(&database, other_tenant_id).await;
    let isolated = exchange
        .read_artifact(
            read_context(other_tenant_id),
            ReadInterchangeArtifactInput {
                artifact_id: exported.id,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        isolated,
        TranslationError::InterchangeArtifactNotFound
    ));

    let mut import_document = exported_content.document.clone();
    import_document.items[0].fields[0].proposed_value = Some("Titel".to_string());
    let import = exchange
        .store_import_artifact(
            write_context(tenant_id, "artifact-store-import"),
            StoreInterchangeImportArtifactInput {
                job_id: job.id,
                document: import_document,
                expires_in_seconds: 300,
            },
        )
        .await
        .unwrap();
    let process_context = write_context(tenant_id, "artifact-process-import");
    let process_input = ProcessInterchangeImportArtifactInput {
        artifact_id: import.id,
    };
    let process_request_hash = hash_manifest(&process_input).unwrap();
    exchange_job::Entity::update_many()
        .col_expr(
            exchange_job::Column::Status,
            Expr::value(TranslationInterchangeArtifactStatus::Processing.as_str()),
        )
        .col_expr(
            exchange_job::Column::ProcessingIdempotencyKey,
            Expr::value(process_context.idempotency_key.clone()),
        )
        .col_expr(
            exchange_job::Column::ProcessingRequestHash,
            Expr::value(process_request_hash),
        )
        .col_expr(
            exchange_job::Column::ProcessedByActorKind,
            Expr::value("system"),
        )
        .col_expr(
            exchange_job::Column::ProcessedByActorId,
            Expr::value(process_context.actor.id.clone()),
        )
        .col_expr(
            exchange_job::Column::ProcessingLeaseToken,
            Expr::value(Some(Uuid::new_v4())),
        )
        .col_expr(
            exchange_job::Column::ProcessingLeaseExpiresAt,
            Expr::value(Some(
                (Utc::now() + ChronoDuration::seconds(60)).fixed_offset(),
            )),
        )
        .filter(exchange_job::Column::Id.eq(import.id))
        .exec(&database)
        .await
        .unwrap();
    let in_progress = exchange
        .process_import_artifact(process_context.clone(), process_input.clone())
        .await
        .unwrap_err();
    assert!(matches!(
        in_progress,
        TranslationError::InterchangeArtifactInProgress
    ));
    exchange_job::Entity::update_many()
        .col_expr(
            exchange_job::Column::ProcessingLeaseExpiresAt,
            Expr::value(Some(
                (Utc::now() - ChronoDuration::seconds(1)).fixed_offset(),
            )),
        )
        .filter(exchange_job::Column::Id.eq(import.id))
        .exec(&database)
        .await
        .unwrap();
    let completed = exchange
        .process_import_artifact(process_context.clone(), process_input.clone())
        .await
        .unwrap();
    assert_eq!(
        completed.status,
        TranslationInterchangeArtifactStatus::Completed
    );
    let report = completed.report.as_ref().unwrap();
    assert_eq!(report.total_items, 1);
    assert_eq!(report.accepted_items, 1);
    assert_eq!(report.conflict_items, 0);
    assert_eq!(report.outcomes[0].status, "imported");
    let completed_replay = exchange
        .process_import_artifact(process_context, process_input)
        .await
        .unwrap();
    assert_eq!(completed_replay, completed);

    let mut stale_document = exported_content.document;
    stale_document.items[0].source_digest = "stale-source-digest".to_string();
    stale_document.items[0].fields[0].proposed_value = Some("Veraltet".to_string());
    let stale_import = exchange
        .store_import_artifact(
            write_context(tenant_id, "artifact-store-stale"),
            StoreInterchangeImportArtifactInput {
                job_id: job.id,
                document: stale_document,
                expires_in_seconds: 300,
            },
        )
        .await
        .unwrap();
    let stale_completed = exchange
        .process_import_artifact(
            write_context(tenant_id, "artifact-process-stale"),
            ProcessInterchangeImportArtifactInput {
                artifact_id: stale_import.id,
            },
        )
        .await
        .unwrap();
    let stale_report = stale_completed.report.as_ref().unwrap();
    assert_eq!(stale_report.accepted_items, 0);
    assert_eq!(stale_report.conflict_items, 1);
    assert_eq!(stale_report.rejected_items, 0);
    assert_eq!(stale_report.outcomes[0].status, "source_conflict");

    let export_model = exchange_job::Entity::find_by_id(exported.id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    exchange_job::Entity::update_many()
        .col_expr(
            exchange_job::Column::ExpiresAt,
            Expr::value((Utc::now() - ChronoDuration::seconds(1)).fixed_offset()),
        )
        .filter(exchange_job::Column::Id.eq(exported.id))
        .exec(&database)
        .await
        .unwrap();
    let expired = exchange
        .read_artifact(
            read_context(tenant_id),
            ReadInterchangeArtifactInput {
                artifact_id: exported.id,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        expired,
        TranslationError::InterchangeArtifactExpired
    ));
    assert!(matches!(
        storage
            .objects
            .head(&Path::from(export_model.object_key.as_str()))
            .await,
        Err(rustok_storage::object_store::Error::NotFound { .. })
    ));
    let visible = exchange
        .list_artifacts(
            read_context(tenant_id),
            ListInterchangeArtifactsInput {
                job_id: Some(job.id),
                include_expired: false,
                limit: 10,
            },
        )
        .await
        .unwrap();
    assert!(!visible.iter().any(|artifact| artifact.id == exported.id));
    let including_expired = exchange
        .list_artifacts(
            read_context(tenant_id),
            ListInterchangeArtifactsInput {
                job_id: Some(job.id),
                include_expired: true,
                limit: 10,
            },
        )
        .await
        .unwrap();
    assert!(including_expired.iter().any(|artifact| {
        artifact.id == exported.id
            && artifact.status == TranslationInterchangeArtifactStatus::Expired
    }));
}

#[cfg(feature = "graphql")]
#[tokio::test]
async fn authenticated_graphql_interchange_enforces_validation_and_tenant_isolation() {
    const CREATE_JOB: &str = r#"
        mutation CreateJob($input: CreateTranslationJobInput!) {
            createTranslationJob(input: $input) { id }
        }
    "#;
    const ADD_ITEM: &str = r#"
        mutation AddItem($input: AddTranslationJobItemInput!) {
            addTranslationJobItem(input: $input) { id }
        }
    "#;
    const EXPORT_JOB: &str = r#"
        query ExportJob($input: ExportTranslationJobInput!) {
            exportTranslationJob(input: $input) {
                schemaVersion
                jobId
                items {
                    itemId
                    identity { ownerSlug resourceKind resourceId subresourceId }
                    sourceDigest
                    fields { key sourceValue }
                }
            }
        }
    "#;
    const IMPORT_ITEM: &str = r#"
        mutation ImportItem($input: ImportTranslationItemInput!) {
            importTranslationItem(input: $input) {
                itemId
                origin
                status
                qaAccepted
                values { key value expectedSourceHash }
            }
        }
    "#;

    let (database, schema, tenant_id) = graphql_fixture().await;
    let create = schema
        .execute(graphql_request(
            CREATE_JOB,
            serde_json::json!({
                "input": {
                    "sourceLocale": "en",
                    "targetLocale": "de",
                    "idempotencyKey": "graphql-interchange-create"
                }
            }),
            tenant_id,
            tenant_id,
        ))
        .await;
    assert!(create.errors.is_empty(), "{:?}", create.errors);
    let create_data = create.data.into_json().unwrap();
    let job_id =
        Uuid::parse_str(create_data["createTranslationJob"]["id"].as_str().unwrap()).unwrap();

    let add = schema
        .execute(graphql_request(
            ADD_ITEM,
            serde_json::json!({
                "input": {
                    "jobId": job_id,
                    "identity": {
                        "ownerSlug": "media",
                        "resourceKind": "asset",
                        "resourceId": "asset-graphql-interchange"
                    },
                    "idempotencyKey": "graphql-interchange-add"
                }
            }),
            tenant_id,
            tenant_id,
        ))
        .await;
    assert!(add.errors.is_empty(), "{:?}", add.errors);
    let add_data = add.data.into_json().unwrap();
    let item_id =
        Uuid::parse_str(add_data["addTranslationJobItem"]["id"].as_str().unwrap()).unwrap();

    let invalid_export = schema
        .execute(graphql_request(
            EXPORT_JOB,
            serde_json::json!({"input": {"jobId": job_id, "maxItems": 0}}),
            tenant_id,
            tenant_id,
        ))
        .await;
    assert_graphql_error_code(&invalid_export, "BAD_USER_INPUT");

    let export = schema
        .execute(graphql_request(
            EXPORT_JOB,
            serde_json::json!({"input": {"jobId": job_id, "maxItems": 10}}),
            tenant_id,
            tenant_id,
        ))
        .await;
    assert!(export.errors.is_empty(), "{:?}", export.errors);
    let export_data = export.data.into_json().unwrap();
    let document = &export_data["exportTranslationJob"];
    assert_eq!(document["schemaVersion"], 1);
    assert_eq!(document["jobId"], job_id.to_string());
    assert_eq!(document["items"].as_array().unwrap().len(), 1);
    let exported_item = &document["items"][0];
    assert_eq!(exported_item["itemId"], item_id.to_string());
    assert_eq!(exported_item["fields"][0]["key"], "title");
    assert_eq!(exported_item["fields"][0]["sourceValue"], "Hero");

    let stale_import = schema
        .execute(graphql_request(
            IMPORT_ITEM,
            serde_json::json!({
                "input": {
                    "schemaVersion": document["schemaVersion"],
                    "jobId": job_id,
                    "itemId": item_id,
                    "identity": exported_item["identity"],
                    "sourceDigest": "stale-source-digest",
                    "values": [{"key": "title", "value": "Titel"}],
                    "idempotencyKey": "graphql-interchange-stale"
                }
            }),
            tenant_id,
            tenant_id,
        ))
        .await;
    assert_graphql_error_code(&stale_import, "BAD_USER_INPUT");

    let import = schema
        .execute(graphql_request(
            IMPORT_ITEM,
            serde_json::json!({
                "input": {
                    "schemaVersion": document["schemaVersion"],
                    "jobId": job_id,
                    "itemId": item_id,
                    "identity": exported_item["identity"],
                    "sourceDigest": exported_item["sourceDigest"],
                    "values": [{"key": "title", "value": "Titel"}],
                    "idempotencyKey": "graphql-interchange-import"
                }
            }),
            tenant_id,
            tenant_id,
        ))
        .await;
    assert!(import.errors.is_empty(), "{:?}", import.errors);
    let import_data = import.data.into_json().unwrap();
    let proposal = &import_data["importTranslationItem"];
    assert_eq!(proposal["itemId"], item_id.to_string());
    assert_eq!(proposal["origin"], "import");
    assert_eq!(proposal["status"], "draft");
    assert_eq!(proposal["qaAccepted"], true);
    assert_eq!(proposal["values"][0]["key"], "title");
    assert_eq!(proposal["values"][0]["value"], "Titel");

    let other_tenant_id = Uuid::new_v4();
    seed_tenant(&database, other_tenant_id).await;
    let isolated_export = schema
        .execute(graphql_request(
            EXPORT_JOB,
            serde_json::json!({"input": {"jobId": job_id, "maxItems": 10}}),
            other_tenant_id,
            other_tenant_id,
        ))
        .await;
    assert_graphql_error_code(&isolated_export, "NOT_FOUND");

    let mismatched_context = schema
        .execute(graphql_request(
            EXPORT_JOB,
            serde_json::json!({"input": {"jobId": job_id, "maxItems": 10}}),
            tenant_id,
            other_tenant_id,
        ))
        .await;
    assert_graphql_error_code(&mismatched_context, "PERMISSION_DENIED");
}

#[cfg(feature = "graphql")]
#[tokio::test]
async fn authenticated_graphql_interchange_artifacts_use_private_storage_and_report_conflicts() {
    const CREATE_JOB: &str = r#"
        mutation CreateJob($input: CreateTranslationJobInput!) {
            createTranslationJob(input: $input) { id }
        }
    "#;
    const ADD_ITEM: &str = r#"
        mutation AddItem($input: AddTranslationJobItemInput!) {
            addTranslationJobItem(input: $input) { id }
        }
    "#;
    const CREATE_EXPORT: &str = r#"
        mutation CreateExport($input: CreateTranslationInterchangeExportArtifactInput!) {
            createTranslationInterchangeExportArtifact(input: $input) {
                id
                jobId
                direction
                status
            }
        }
    "#;
    const LIST_ARTIFACTS: &str = r#"
        query ListArtifacts($input: TranslationInterchangeArtifactsInput!) {
            translationInterchangeArtifacts(input: $input) {
                id
                jobId
                direction
                status
            }
        }
    "#;
    const READ_ARTIFACT: &str = r#"
        query ReadArtifact($input: ReadTranslationInterchangeArtifactInput!) {
            translationInterchangeArtifact(input: $input) {
                artifact { id jobId direction status }
                document {
                    schemaVersion
                    jobId
                    sourceLocale
                    targetLocale
                    items {
                        itemId
                        identity { ownerSlug resourceKind resourceId subresourceId }
                        sourceDigest
                        sourceRevision
                        targetRevision
                        fields {
                            key
                            sourceValue
                            exactTargetValue
                            proposedValue
                            sourceHash
                            required
                            maxCharacters
                            protectedTokens
                        }
                    }
                }
            }
        }
    "#;
    const STORE_IMPORT: &str = r#"
        mutation StoreImport($input: StoreTranslationInterchangeImportArtifactInput!) {
            storeTranslationInterchangeImportArtifact(input: $input) {
                id
                jobId
                direction
                status
            }
        }
    "#;
    const PROCESS_IMPORT: &str = r#"
        mutation ProcessImport($input: ProcessTranslationInterchangeImportArtifactInput!) {
            processTranslationInterchangeImportArtifact(input: $input) {
                id
                status
                processedAt
                report {
                    totalItems
                    acceptedItems
                    conflictItems
                    rejectedItems
                    outcomes { itemId status }
                }
            }
        }
    "#;

    let (database, schema, _storage_directory, tenant_id) =
        graphql_interchange_artifact_fixture().await;
    let create = schema
        .execute(graphql_request(
            CREATE_JOB,
            serde_json::json!({
                "input": {
                    "sourceLocale": "en",
                    "targetLocale": "de",
                    "idempotencyKey": "graphql-artifact-create-job"
                }
            }),
            tenant_id,
            tenant_id,
        ))
        .await;
    assert!(create.errors.is_empty(), "{:?}", create.errors);
    let create_data = create.data.into_json().unwrap();
    let job_id = create_data["createTranslationJob"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let add = schema
        .execute(graphql_request(
            ADD_ITEM,
            serde_json::json!({
                "input": {
                    "jobId": job_id,
                    "identity": {
                        "ownerSlug": "media",
                        "resourceKind": "asset",
                        "resourceId": "asset-graphql-artifact"
                    },
                    "idempotencyKey": "graphql-artifact-add-item"
                }
            }),
            tenant_id,
            tenant_id,
        ))
        .await;
    assert!(add.errors.is_empty(), "{:?}", add.errors);
    let add_data = add.data.into_json().unwrap();
    let item_id = add_data["addTranslationJobItem"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let create_export = schema
        .execute(graphql_request(
            CREATE_EXPORT,
            serde_json::json!({
                "input": {
                    "jobId": job_id,
                    "maxItems": 10,
                    "expiresInSeconds": 86400,
                    "idempotencyKey": "graphql-artifact-create-export"
                }
            }),
            tenant_id,
            tenant_id,
        ))
        .await;
    assert!(
        create_export.errors.is_empty(),
        "{:?}",
        create_export.errors
    );
    let export_data = create_export.data.into_json().unwrap();
    let exported = &export_data["createTranslationInterchangeExportArtifact"];
    let export_id = exported["id"].as_str().unwrap().to_string();
    assert_eq!(exported["jobId"], job_id);
    assert_eq!(exported["direction"], "export");
    assert_eq!(exported["status"], "ready");

    let listed = schema
        .execute(graphql_request(
            LIST_ARTIFACTS,
            serde_json::json!({
                "input": {"jobId": job_id, "includeExpired": false, "limit": 10}
            }),
            tenant_id,
            tenant_id,
        ))
        .await;
    assert!(listed.errors.is_empty(), "{:?}", listed.errors);
    let listed_data = listed.data.into_json().unwrap();
    assert_eq!(
        listed_data["translationInterchangeArtifacts"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        listed_data["translationInterchangeArtifacts"][0]["id"],
        export_id
    );

    let read = schema
        .execute(graphql_request(
            READ_ARTIFACT,
            serde_json::json!({"input": {"artifactId": export_id}}),
            tenant_id,
            tenant_id,
        ))
        .await;
    assert!(read.errors.is_empty(), "{:?}", read.errors);
    let read_data = read.data.into_json().unwrap();
    let content = &read_data["translationInterchangeArtifact"];
    assert_eq!(content["artifact"]["id"], export_id);
    let mut import_document = content["document"].clone();
    assert_eq!(import_document["items"][0]["itemId"], item_id);
    import_document["items"][0]["fields"][0]["proposedValue"] =
        serde_json::Value::String("Held".to_string());
    let import_document = serde_json::to_string(&import_document).unwrap();

    let store_import = schema
        .execute(graphql_request(
            STORE_IMPORT,
            serde_json::json!({
                "input": {
                    "jobId": job_id,
                    "documentJson": import_document,
                    "expiresInSeconds": 86400,
                    "idempotencyKey": "graphql-artifact-store-import"
                }
            }),
            tenant_id,
            tenant_id,
        ))
        .await;
    assert!(store_import.errors.is_empty(), "{:?}", store_import.errors);
    let store_data = store_import.data.into_json().unwrap();
    let imported = &store_data["storeTranslationInterchangeImportArtifact"];
    let import_id = imported["id"].as_str().unwrap().to_string();
    assert_eq!(imported["direction"], "import");
    assert_eq!(imported["status"], "ready");

    let processed = schema
        .execute(graphql_request(
            PROCESS_IMPORT,
            serde_json::json!({
                "input": {
                    "artifactId": import_id,
                    "idempotencyKey": "graphql-artifact-process-import"
                }
            }),
            tenant_id,
            tenant_id,
        ))
        .await;
    assert!(processed.errors.is_empty(), "{:?}", processed.errors);
    let processed_data = processed.data.into_json().unwrap();
    let report = &processed_data["processTranslationInterchangeImportArtifact"]["report"];
    assert_eq!(
        processed_data["processTranslationInterchangeImportArtifact"]["status"],
        "completed"
    );
    assert_eq!(report["totalItems"], 1);
    assert_eq!(report["acceptedItems"], 1);
    assert_eq!(report["conflictItems"], 0);
    assert_eq!(report["rejectedItems"], 0);
    assert_eq!(report["outcomes"][0]["itemId"], item_id);
    assert_eq!(report["outcomes"][0]["status"], "imported");

    let other_tenant_id = Uuid::new_v4();
    seed_tenant(&database, other_tenant_id).await;
    let isolated = schema
        .execute(graphql_request(
            READ_ARTIFACT,
            serde_json::json!({"input": {"artifactId": import_id}}),
            other_tenant_id,
            other_tenant_id,
        ))
        .await;
    assert_graphql_error_code(&isolated, "NOT_FOUND");
}

#[cfg(feature = "graphql")]
#[tokio::test]
async fn authenticated_graphql_workflow_notes_are_private_tenant_scoped_and_resolvable() {
    const CREATE_JOB: &str = r#"
        mutation CreateJob($input: CreateTranslationJobInput!) {
            createTranslationJob(input: $input) { id }
        }
    "#;
    const CREATE_NOTE: &str = r#"
        mutation CreateNote($input: CreateTranslationWorkflowNoteInput!) {
            createTranslationWorkflowNote(input: $input) {
                id
                jobId
                itemId
                body
                revision
                resolvedAt
            }
        }
    "#;
    const LIST_NOTES: &str = r#"
        query ListNotes($input: TranslationWorkflowNotesInput!) {
            translationWorkflowNotes(input: $input) {
                id
                jobId
                itemId
                body
                revision
                resolvedAt
            }
        }
    "#;
    const RESOLVE_NOTE: &str = r#"
        mutation ResolveNote($input: ResolveTranslationWorkflowNoteInput!) {
            resolveTranslationWorkflowNote(input: $input) {
                id
                revision
                resolvedAt
            }
        }
    "#;

    let (database, schema, tenant_id) = graphql_fixture().await;
    let created_job = schema
        .execute(graphql_request(
            CREATE_JOB,
            serde_json::json!({
                "input": {
                    "sourceLocale": "en",
                    "targetLocale": "de",
                    "idempotencyKey": "graphql-workflow-note-job"
                }
            }),
            tenant_id,
            tenant_id,
        ))
        .await;
    assert!(created_job.errors.is_empty(), "{:?}", created_job.errors);
    let created_job_data = created_job.data.into_json().unwrap();
    let job_id = Uuid::parse_str(
        created_job_data["createTranslationJob"]["id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();

    let note_body = "Use the approved terminology before review.";
    let created_note = schema
        .execute(graphql_request(
            CREATE_NOTE,
            serde_json::json!({
                "input": {
                    "jobId": job_id,
                    "body": note_body,
                    "idempotencyKey": "graphql-workflow-note-create"
                }
            }),
            tenant_id,
            tenant_id,
        ))
        .await;
    assert!(created_note.errors.is_empty(), "{:?}", created_note.errors);
    let created_note_data = created_note.data.into_json().unwrap();
    let note = &created_note_data["createTranslationWorkflowNote"];
    let note_id = Uuid::parse_str(note["id"].as_str().unwrap()).unwrap();
    assert_eq!(note["jobId"], job_id.to_string());
    assert!(note["itemId"].is_null());
    assert_eq!(note["body"], note_body);
    assert_eq!(note["revision"], 0);
    assert!(note["resolvedAt"].is_null());

    let listed = schema
        .execute(graphql_request(
            LIST_NOTES,
            serde_json::json!({
                "input": {
                    "jobId": job_id,
                    "includeResolved": false,
                    "limit": 50
                }
            }),
            tenant_id,
            tenant_id,
        ))
        .await;
    assert!(listed.errors.is_empty(), "{:?}", listed.errors);
    let listed_data = listed.data.into_json().unwrap();
    let listed_notes = listed_data["translationWorkflowNotes"].as_array().unwrap();
    assert_eq!(listed_notes.len(), 1);
    assert_eq!(listed_notes[0]["id"], note_id.to_string());
    assert_eq!(listed_notes[0]["body"], note_body);

    let resolved = schema
        .execute(graphql_request(
            RESOLVE_NOTE,
            serde_json::json!({
                "input": {
                    "noteId": note_id,
                    "expectedRevision": 0,
                    "idempotencyKey": "graphql-workflow-note-resolve"
                }
            }),
            tenant_id,
            tenant_id,
        ))
        .await;
    assert!(resolved.errors.is_empty(), "{:?}", resolved.errors);
    let resolved_data = resolved.data.into_json().unwrap();
    let resolved_note = &resolved_data["resolveTranslationWorkflowNote"];
    assert_eq!(resolved_note["id"], note_id.to_string());
    assert_eq!(resolved_note["revision"], 1);
    assert!(resolved_note["resolvedAt"].as_str().is_some());

    let open_notes = schema
        .execute(graphql_request(
            LIST_NOTES,
            serde_json::json!({
                "input": {
                    "jobId": job_id,
                    "includeResolved": false,
                    "limit": 50
                }
            }),
            tenant_id,
            tenant_id,
        ))
        .await;
    assert!(open_notes.errors.is_empty(), "{:?}", open_notes.errors);
    let open_notes_data = open_notes.data.into_json().unwrap();
    assert!(
        open_notes_data["translationWorkflowNotes"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let other_tenant_id = Uuid::new_v4();
    seed_tenant(&database, other_tenant_id).await;
    let isolated = schema
        .execute(graphql_request(
            LIST_NOTES,
            serde_json::json!({
                "input": {
                    "jobId": job_id,
                    "includeResolved": true,
                    "limit": 50
                }
            }),
            other_tenant_id,
            other_tenant_id,
        ))
        .await;
    assert_graphql_error_code(&isolated, "NOT_FOUND");
}

#[tokio::test]
async fn applied_human_approved_segments_enter_tenant_scoped_deterministic_memory() {
    let (database, service, tenant_id) = fixture().await;
    let memory = TranslationMemoryService::new(database.clone());
    let (item_id, proposal_id) = create_approved_item(&service, tenant_id, "memory").await;
    let apply_context = write_context(tenant_id, "apply-memory");
    let apply_input = ApplyProposalInput {
        item_id,
        proposal_id,
    };
    service
        .apply_proposal(apply_context.clone(), apply_input.clone())
        .await
        .unwrap();
    service
        .apply_proposal(apply_context, apply_input)
        .await
        .unwrap();

    assert_eq!(
        memory_entry::Entity::find()
            .filter(memory_entry::Column::TenantId.eq(tenant_id))
            .count(&database)
            .await
            .unwrap(),
        1
    );
    let exact = memory
        .lookup(
            read_context(tenant_id),
            MemoryLookupInput {
                source_locale: TenantLocale::new("en").unwrap(),
                target_locale: TenantLocale::new("de").unwrap(),
                identity: identity("another-asset"),
                field_key: FieldKey::new("title").unwrap(),
                source_text: "  HERO  ".to_string(),
                minimum_similarity_basis_points: 10_000,
                limit: 10,
            },
        )
        .await
        .unwrap();
    assert_eq!(exact.len(), 1);
    assert_eq!(exact[0].target_text, "Held");
    assert_eq!(exact[0].evidence.kind, MemoryMatchKind::Exact);
    assert!(exact[0].evidence.context_match);
    assert_eq!(exact[0].proposal_id, proposal_id);

    let fuzzy = memory
        .lookup(
            read_context(tenant_id),
            MemoryLookupInput {
                source_locale: TenantLocale::new("en").unwrap(),
                target_locale: TenantLocale::new("de").unwrap(),
                identity: identity("fuzzy-asset"),
                field_key: FieldKey::new("title").unwrap(),
                source_text: "Hero returns".to_string(),
                minimum_similarity_basis_points: 7_000,
                limit: 10,
            },
        )
        .await
        .unwrap();
    assert_eq!(fuzzy.len(), 1);
    assert_eq!(fuzzy[0].evidence.kind, MemoryMatchKind::ContextualFuzzy);
    assert_eq!(fuzzy[0].evidence.final_similarity_basis_points, 7_166);

    let other_tenant_id = Uuid::new_v4();
    seed_tenant(&database, other_tenant_id).await;
    let isolated = memory
        .lookup(
            read_context(other_tenant_id),
            MemoryLookupInput {
                source_locale: TenantLocale::new("en").unwrap(),
                target_locale: TenantLocale::new("de").unwrap(),
                identity: identity("other-tenant"),
                field_key: FieldKey::new("title").unwrap(),
                source_text: "Hero".to_string(),
                minimum_similarity_basis_points: 0,
                limit: 10,
            },
        )
        .await
        .unwrap();
    assert!(isolated.is_empty());
}

#[tokio::test]
async fn memory_retention_tombstone_and_purge_are_revisioned_and_replay_safe() {
    let (database, service, tenant_id) = fixture().await;
    let memory = TranslationMemoryService::new(database.clone());
    let (item_id, proposal_id) =
        create_approved_item(&service, tenant_id, "memory-lifecycle").await;
    service
        .apply_proposal(
            write_context(tenant_id, "apply-memory-lifecycle"),
            ApplyProposalInput {
                item_id,
                proposal_id,
            },
        )
        .await
        .unwrap();
    let entry = memory
        .list_entries(
            read_context(tenant_id),
            MemoryListInput {
                source_locale: None,
                target_locale: None,
                include_tombstoned: false,
                limit: 10,
            },
        )
        .await
        .unwrap()
        .pop()
        .unwrap();

    let legal_hold = memory
        .set_retention(
            write_context(tenant_id, "memory-legal-hold"),
            SetMemoryRetentionInput {
                entry_id: entry.id,
                expected_revision: entry.revision,
                policy: RetentionPolicy::LegalHold,
                retain_until: None,
            },
        )
        .await
        .unwrap();
    let replay = memory
        .set_retention(
            write_context(tenant_id, "memory-legal-hold"),
            SetMemoryRetentionInput {
                entry_id: entry.id,
                expected_revision: entry.revision,
                policy: RetentionPolicy::LegalHold,
                retain_until: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(replay, legal_hold);
    assert_eq!(legal_hold.revision, 2);

    let legal_hold_error = memory
        .tombstone_entry(
            write_context(tenant_id, "memory-tombstone-held"),
            TombstoneMemoryEntryInput {
                entry_id: entry.id,
                expected_revision: legal_hold.revision,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        legal_hold_error,
        TranslationError::MemoryRetentionConflict(_)
    ));

    let owner_lifecycle = memory
        .set_retention(
            write_context(tenant_id, "memory-owner-lifecycle"),
            SetMemoryRetentionInput {
                entry_id: entry.id,
                expected_revision: legal_hold.revision,
                policy: RetentionPolicy::OwnerLifecycle,
                retain_until: None,
            },
        )
        .await
        .unwrap();
    let tombstoned = memory
        .tombstone_entry(
            write_context(tenant_id, "memory-tombstone"),
            TombstoneMemoryEntryInput {
                entry_id: entry.id,
                expected_revision: owner_lifecycle.revision,
            },
        )
        .await
        .unwrap();
    assert_eq!(tombstoned.state, "tombstoned");
    assert!(
        memory
            .lookup(
                read_context(tenant_id),
                MemoryLookupInput {
                    source_locale: TenantLocale::new("en").unwrap(),
                    target_locale: TenantLocale::new("de").unwrap(),
                    identity: identity("memory-after-tombstone"),
                    field_key: FieldKey::new("title").unwrap(),
                    source_text: "Hero".to_string(),
                    minimum_similarity_basis_points: 0,
                    limit: 10,
                },
            )
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        memory
            .list_entries(
                read_context(tenant_id),
                MemoryListInput {
                    source_locale: None,
                    target_locale: None,
                    include_tombstoned: true,
                    limit: 10,
                },
            )
            .await
            .unwrap()
            .len(),
        1
    );

    let purge_input = PurgeMemoryEntryInput {
        entry_id: entry.id,
        expected_revision: tombstoned.revision,
    };
    let purge_context = write_context(tenant_id, "memory-purge");
    let purged = memory
        .purge_entry(purge_context.clone(), purge_input.clone())
        .await
        .unwrap();
    let purge_replay = memory
        .purge_entry(purge_context, purge_input)
        .await
        .unwrap();
    assert_eq!(purge_replay, purged);
    assert_eq!(purged.state, "purged");
    assert!(matches!(
        memory
            .read_entry(read_context(tenant_id), entry.id)
            .await
            .unwrap_err(),
        TranslationError::MemoryEntryNotFound
    ));
    assert_eq!(
        memory_receipt::Entity::find()
            .filter(memory_receipt::Column::TenantId.eq(tenant_id))
            .count(&database)
            .await
            .unwrap(),
        4
    );
}

#[tokio::test]
async fn jobs_reject_tenant_disabled_locales() {
    let (_database, service, tenant_id) = fixture().await;
    let error = service
        .create_job(
            write_context(tenant_id, "create-job-disabled-locale"),
            job_input("es"),
        )
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        TranslationError::DisabledJobLocale {
            role: "target",
            locale
        } if locale == "es"
    ));
}

#[tokio::test]
async fn deterministic_qa_is_persisted_on_save_and_blocks_review_submission() {
    let (database, service, tenant_id) = fixture().await;
    let job = service
        .create_job(write_context(tenant_id, "qa-create-job"), job_input("de"))
        .await
        .unwrap();
    let item = service
        .add_item(
            write_context(tenant_id, "qa-add-item"),
            AddItemInput {
                job_id: job.id,
                identity: identity("qa-asset"),
            },
        )
        .await
        .unwrap();
    let proposal_record = service
        .save_proposal(
            write_context(tenant_id, "qa-save-proposal"),
            SaveProposalInput {
                item_id: item.id,
                origin: ProposalOrigin::Manual,
                values: vec![ProposalValue {
                    key: FieldKey::new("title").unwrap(),
                    value: "x".repeat(201),
                }],
            },
        )
        .await
        .unwrap();
    assert!(!proposal_record.qa_accepted);
    assert!(proposal_record.qa_issues.iter().any(|issue| {
        issue.severity == TranslationPatchIssueSeverity::Error
            && issue.code == "translation.qa.max_characters_exceeded"
    }));

    let error = service
        .submit_proposal(
            write_context(tenant_id, "qa-submit-proposal"),
            SubmitProposalInput {
                item_id: item.id,
                proposal_id: proposal_record.id,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(error, TranslationError::ProposalValidationFailed));

    let persisted = proposal::Entity::find_by_id(proposal_record.id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert!(persisted.submitted_at.is_none());
    let issues: Vec<TranslationPatchIssue> = serde_json::from_value(persisted.qa_issues).unwrap();
    assert!(
        issues
            .iter()
            .any(|issue| issue.code == "translation.qa.max_characters_exceeded")
    );
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
        Arc::new(TestTenantLocalePolicies),
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
        Arc::new(TestTenantLocalePolicies),
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
async fn reviewer_queue_and_workload_are_assignment_scoped_and_bounded() {
    let (database, service, tenant_id) = fixture().await;
    let progress_service = TranslationProgressService::new(
        database.clone(),
        Arc::new(TranslationTargetRegistry::default()),
        Arc::new(TestTenantLocalePolicies),
    );
    let (approved_item_id, _) =
        create_approved_item(&service, tenant_id, "reviewer-approved").await;
    let job_id = job_item::Entity::find_by_id(approved_item_id)
        .one(&database)
        .await
        .unwrap()
        .unwrap()
        .job_id;

    let unassigned_item = service
        .add_item(
            write_context(tenant_id, "add-reviewer-unassigned"),
            AddItemInput {
                job_id,
                identity: identity("asset-reviewer-unassigned"),
            },
        )
        .await
        .unwrap();
    let assigned_item = service
        .add_item(
            write_context(tenant_id, "add-reviewer-assigned"),
            AddItemInput {
                job_id,
                identity: identity("asset-reviewer-assigned"),
            },
        )
        .await
        .unwrap();
    let reviewer_id = Uuid::new_v4();
    service
        .assign_item(
            user_write_context(
                tenant_id,
                Uuid::new_v4(),
                Action::Manage,
                "assign-reviewer-item",
            ),
            AssignItemInput {
                item_id: assigned_item.id,
                expected_revision: assigned_item.revision,
                assignee: PortActor::user(reviewer_id.to_string()),
            },
        )
        .await
        .unwrap();
    let unassigned_proposal = submit_item(
        &service,
        tenant_id,
        Uuid::new_v4(),
        unassigned_item.id,
        "unassigned",
    )
    .await;
    let assigned_proposal = submit_item(
        &service,
        tenant_id,
        reviewer_id,
        assigned_item.id,
        "assigned",
    )
    .await;

    let reviewer = PortActor::user(reviewer_id.to_string());
    let assigned_queue = progress_service
        .list_reviewer_queue(
            read_context(tenant_id),
            ReviewerQueueInput {
                job_id,
                assignee: Some(reviewer.clone()),
                include_unassigned: false,
                limit: 50,
            },
        )
        .await
        .unwrap();
    assert_eq!(assigned_queue.len(), 1);
    assert_eq!(assigned_queue[0].item.id, assigned_item.id);
    assert_eq!(assigned_queue[0].proposal_id, assigned_proposal);
    assert_eq!(assigned_queue[0].item.assignee, Some(reviewer.clone()));

    let all_queue = progress_service
        .list_reviewer_queue(
            read_context(tenant_id),
            ReviewerQueueInput {
                job_id,
                assignee: None,
                include_unassigned: true,
                limit: 50,
            },
        )
        .await
        .unwrap();
    assert_eq!(all_queue.len(), 2);
    assert!(
        all_queue
            .iter()
            .any(|item| item.proposal_id == unassigned_proposal)
    );
    assert!(
        all_queue
            .iter()
            .any(|item| item.proposal_id == assigned_proposal)
    );

    let assigned_only_queue = progress_service
        .list_reviewer_queue(
            read_context(tenant_id),
            ReviewerQueueInput {
                job_id,
                assignee: None,
                include_unassigned: false,
                limit: 50,
            },
        )
        .await
        .unwrap();
    assert_eq!(assigned_only_queue.len(), 1);
    assert_eq!(assigned_only_queue[0].item.id, assigned_item.id);

    let invalid_limit = progress_service
        .list_reviewer_queue(
            read_context(tenant_id),
            ReviewerQueueInput {
                job_id,
                assignee: None,
                include_unassigned: true,
                limit: 0,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(invalid_limit, TranslationError::InvalidRequest(_)));

    let workloads = progress_service
        .list_reviewer_workload(read_context(tenant_id), ReviewerWorkloadInput { job_id })
        .await
        .unwrap();
    assert_eq!(workloads.len(), 2);
    let unassigned = workloads
        .iter()
        .find(|workload| workload.assignee.is_none())
        .unwrap();
    assert_eq!(unassigned.open_items, 2);
    assert_eq!(unassigned.in_review_items, 1);
    assert_eq!(unassigned.approved_items, 1);
    assert_eq!(unassigned.source_characters, 8);
    let assigned = workloads
        .iter()
        .find(|workload| workload.assignee.as_ref() == Some(&reviewer))
        .unwrap();
    assert_eq!(assigned.open_items, 1);
    assert_eq!(assigned.in_review_items, 1);
    assert_eq!(assigned.approved_items, 0);
    assert_eq!(assigned.source_characters, 4);

    let other_tenant_id = Uuid::new_v4();
    seed_tenant(&database, other_tenant_id).await;
    let isolated = progress_service
        .list_reviewer_workload(
            read_context(other_tenant_id),
            ReviewerWorkloadInput { job_id },
        )
        .await
        .unwrap_err();
    assert!(matches!(isolated, TranslationError::JobNotFound));
}

#[tokio::test]
async fn workflow_notes_are_private_bounded_and_actor_bound() {
    let (database, service, tenant_id) = fixture().await;
    let collaboration = service.collaboration_service();
    let job = service
        .create_job(write_context(tenant_id, "create-note-job"), job_input("de"))
        .await
        .unwrap();
    let item = service
        .add_item(
            write_context(tenant_id, "add-note-item"),
            AddItemInput {
                job_id: job.id,
                identity: identity("asset-workflow-note"),
            },
        )
        .await
        .unwrap();

    let translator_id = Uuid::new_v4();
    let denied_item_note = collaboration
        .create_workflow_note(
            user_write_context(tenant_id, translator_id, Action::Update, "denied-item-note"),
            CreateWorkflowNoteInput {
                job_id: job.id,
                item_id: Some(item.id),
                body: "This translator is not assigned to the item".to_string(),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(denied_item_note, TranslationError::Forbidden));

    let job_note_input = CreateWorkflowNoteInput {
        job_id: job.id,
        item_id: None,
        body: "Please confirm the release terminology before review.".to_string(),
    };
    let job_note_context =
        user_write_context(tenant_id, translator_id, Action::Update, "create-job-note");
    let job_note = collaboration
        .create_workflow_note(job_note_context.clone(), job_note_input.clone())
        .await
        .unwrap();
    let job_note_replay = collaboration
        .create_workflow_note(job_note_context, job_note_input)
        .await
        .unwrap();
    assert_eq!(job_note_replay, job_note);
    assert_eq!(job_note.author, PortActor::user(translator_id.to_string()));
    assert_eq!(job_note.revision, 0);
    assert!(job_note.resolved_at.is_none());

    let other_actor = collaboration
        .create_workflow_note(
            user_write_context(tenant_id, Uuid::new_v4(), Action::Update, "create-job-note"),
            CreateWorkflowNoteInput {
                job_id: job.id,
                item_id: None,
                body: "Please confirm the release terminology before review.".to_string(),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        other_actor,
        TranslationError::IdempotencyActorMismatch
    ));

    let item_note = collaboration
        .create_workflow_note(
            write_context(tenant_id, "create-item-note"),
            CreateWorkflowNoteInput {
                job_id: job.id,
                item_id: Some(item.id),
                body: "The protected brand token must remain unchanged.".to_string(),
            },
        )
        .await
        .unwrap();
    assert_eq!(item_note.item_id, Some(item.id));

    let unresolved = collaboration
        .list_workflow_notes(
            read_context(tenant_id),
            ListWorkflowNotesInput {
                job_id: job.id,
                item_id: None,
                include_resolved: false,
                limit: 50,
            },
        )
        .await
        .unwrap();
    assert_eq!(unresolved.len(), 2);
    assert_eq!(unresolved[0].id, item_note.id);

    let invalid_limit = collaboration
        .list_workflow_notes(
            read_context(tenant_id),
            ListWorkflowNotesInput {
                job_id: job.id,
                item_id: None,
                include_resolved: false,
                limit: 0,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(invalid_limit, TranslationError::InvalidRequest(_)));

    let reviewer_id = Uuid::new_v4();
    let resolve_context =
        user_write_context(tenant_id, reviewer_id, Action::Resolve, "resolve-job-note");
    let resolved = collaboration
        .resolve_workflow_note(
            resolve_context.clone(),
            ResolveWorkflowNoteInput {
                note_id: job_note.id,
                expected_revision: 0,
            },
        )
        .await
        .unwrap();
    let resolved_replay = collaboration
        .resolve_workflow_note(
            resolve_context,
            ResolveWorkflowNoteInput {
                note_id: job_note.id,
                expected_revision: 0,
            },
        )
        .await
        .unwrap();
    assert_eq!(resolved_replay, resolved);
    assert_eq!(resolved.revision, 1);
    assert_eq!(
        resolved.resolved_by,
        Some(PortActor::user(reviewer_id.to_string()))
    );

    let still_open = collaboration
        .list_workflow_notes(
            read_context(tenant_id),
            ListWorkflowNotesInput {
                job_id: job.id,
                item_id: None,
                include_resolved: false,
                limit: 50,
            },
        )
        .await
        .unwrap();
    assert_eq!(still_open.len(), 1);
    assert_eq!(still_open[0].id, item_note.id);

    let all_notes = collaboration
        .list_workflow_notes(
            read_context(tenant_id),
            ListWorkflowNotesInput {
                job_id: job.id,
                item_id: None,
                include_resolved: true,
                limit: 50,
            },
        )
        .await
        .unwrap();
    assert_eq!(all_notes.len(), 2);

    let other_tenant_id = Uuid::new_v4();
    seed_tenant(&database, other_tenant_id).await;
    let isolated = collaboration
        .list_workflow_notes(
            read_context(other_tenant_id),
            ListWorkflowNotesInput {
                job_id: job.id,
                item_id: None,
                include_resolved: true,
                limit: 50,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(isolated, TranslationError::JobNotFound));
}

#[tokio::test]
async fn blocked_item_retry_is_explicit_actor_bound_and_does_not_retry_conflicts() {
    let (database, service, tenant_id, apply_state) = fixture_with_apply_state().await;
    let progress_service = TranslationProgressService::new(
        database.clone(),
        Arc::new(TranslationTargetRegistry::default()),
        Arc::new(TestTenantLocalePolicies),
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
async fn proposal_qa_uses_the_glossary_revision_captured_by_the_job() {
    let (database, service, tenant_id) = fixture().await;
    let glossary_service =
        TranslationGlossaryService::new(database, Arc::new(TestTenantLocalePolicies));
    let glossary = glossary_service
        .create_glossary(
            write_context(tenant_id, "create-glossary-snapshot"),
            CreateGlossaryInput {
                name: "Media terminology".to_string(),
                description: String::new(),
                source_locale: TenantLocale::new("en").unwrap(),
                target_locale: TenantLocale::new("de").unwrap(),
                scope: GlossaryScope {
                    owner_slug: Some(OwnerSlug::new("media").unwrap()),
                    resource_kind: Some(ResourceKind::new("asset").unwrap()),
                    field_key: Some(FieldKey::new("title").unwrap()),
                },
            },
        )
        .await
        .unwrap();
    let bound_revision = glossary_service
        .replace_terms(
            write_context(tenant_id, "terms-glossary-snapshot-2"),
            ReplaceGlossaryTermsInput {
                glossary_id: glossary.id,
                expected_revision: glossary.revision,
                concepts: vec![glossary_concept("Held")],
            },
        )
        .await
        .unwrap();
    let job = service
        .create_job(
            write_context(tenant_id, "create-job-glossary-snapshot"),
            CreateJobInput {
                source_locale: TenantLocale::new("en").unwrap(),
                target_locale: TenantLocale::new("de").unwrap(),
                glossary: Some(GlossaryBinding {
                    glossary_id: glossary.id,
                    revision: bound_revision.revision,
                }),
            },
        )
        .await
        .unwrap();
    glossary_service
        .replace_terms(
            write_context(tenant_id, "terms-glossary-snapshot-3"),
            ReplaceGlossaryTermsInput {
                glossary_id: glossary.id,
                expected_revision: bound_revision.revision,
                concepts: vec![glossary_concept("Hauptfigur")],
            },
        )
        .await
        .unwrap();
    let item = service
        .add_item(
            write_context(tenant_id, "add-item-glossary-snapshot"),
            AddItemInput {
                job_id: job.id,
                identity: identity("asset-glossary-snapshot"),
            },
        )
        .await
        .unwrap();

    let captured_revision_proposal = service
        .save_proposal(
            write_context(tenant_id, "save-captured-glossary-revision"),
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
    assert!(captured_revision_proposal.qa_accepted);
    assert!(captured_revision_proposal.qa_issues.is_empty());

    let current_revision_proposal = service
        .save_proposal(
            write_context(tenant_id, "save-current-glossary-revision"),
            SaveProposalInput {
                item_id: item.id,
                origin: ProposalOrigin::Manual,
                values: vec![ProposalValue {
                    key: FieldKey::new("title").unwrap(),
                    value: "Hauptfigur".to_string(),
                }],
            },
        )
        .await
        .unwrap();
    assert!(!current_revision_proposal.qa_accepted);
    assert_eq!(
        current_revision_proposal.qa_issues[0].code,
        "translation.glossary.preferred_term_missing"
    );
}

fn glossary_concept(preferred_term: &str) -> GlossaryConcept {
    GlossaryConcept {
        concept_key: "hero".to_string(),
        source_term: "Hero".to_string(),
        variants: vec![GlossaryVariant {
            value: preferred_term.to_string(),
            policy: GlossaryTermPolicy::Preferred,
        }],
        match_kind: GlossaryMatchKind::WholeWord,
        case_sensitive: false,
        notes: String::new(),
    }
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
