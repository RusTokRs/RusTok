use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{PortActor, PortContext, PortError, TenantLocale};
use rustok_translation::{
    TranslationError, TranslationInventoryService,
    entities::{inventory_resource, memory_entry, provider_checkpoint},
    migrations,
};
use rustok_translation_targets::{
    ListTranslationResourcesRequest, OpaqueCursor, OpaqueRevision, OwnerSlug,
    ReadTranslationResourceRequest, ResourceId, ResourceKind, TranslationApplicationReceipt,
    TranslationPatchRequest, TranslationPatchValidation, TranslationResourceIdentity,
    TranslationResourceLifecycle, TranslationResourcePage, TranslationResourceSnapshot,
    TranslationResourceSummary, TranslationTargetCapability, TranslationTargetChange,
    TranslationTargetChangePage, TranslationTargetChangesRequest, TranslationTargetProvider,
    TranslationTargetProviderDescriptor, TranslationTargetRegistry,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
    DbBackend, EntityTrait, PaginatorTrait, QueryFilter, Set, Statement,
};
use sea_orm_migration::SchemaManager;
use tokio::{
    process::Command,
    sync::{Barrier, Notify},
};
use uuid::Uuid;

#[derive(Clone, Copy)]
enum ChangePageFixture {
    Valid,
    Deleted,
    MissingCursor,
    WrongOwner,
    ProviderOutage,
}

#[derive(Clone)]
struct ProviderGate {
    entered: Arc<Notify>,
    resume: Arc<Notify>,
}

struct CursorProvider {
    page: ChangePageFixture,
    change_gate: Option<ProviderGate>,
    change_barrier: Option<Arc<Barrier>>,
    list_gate: Option<ProviderGate>,
}

struct SqliteTestFileGuard(PathBuf);

impl Drop for SqliteTestFileGuard {
    fn drop(&mut self) {
        for path in sqlite_test_files(&self.0) {
            if path.exists() {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

#[async_trait]
impl TranslationTargetProvider for CursorProvider {
    fn descriptor(&self) -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new("media").unwrap(),
            resource_kind: ResourceKind::new("asset").unwrap(),
            display_name: "Media asset metadata".to_string(),
            capabilities: BTreeSet::from([
                TranslationTargetCapability::ListResources,
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
        if request.cursor.is_none()
            && let Some(gate) = &self.list_gate
        {
            gate.entered.notify_one();
            gate.resume.notified().await;
        }
        let (resource_id, next_cursor) = match request.cursor.as_ref().map(OpaqueCursor::as_str) {
            None => (
                "scan-asset-1",
                Some(OpaqueCursor::new("scan-page-1").unwrap()),
            ),
            Some("scan-page-1") => ("scan-asset-2", None),
            Some(_) => {
                return Err(PortError::validation(
                    "translation.test_scan_cursor_invalid",
                    "unexpected scan cursor",
                ));
            }
        };
        Ok(TranslationResourcePage {
            resources: vec![TranslationResourceSummary {
                identity: TranslationResourceIdentity {
                    owner_slug: OwnerSlug::new("media").unwrap(),
                    resource_kind: ResourceKind::new("asset").unwrap(),
                    resource_id: ResourceId::new(resource_id).unwrap(),
                    subresource_id: None,
                },
                display_label: resource_id.to_string(),
                lifecycle: if matches!(self.page, ChangePageFixture::Deleted) {
                    TranslationResourceLifecycle::Deleted
                } else {
                    TranslationResourceLifecycle::Active
                },
                resource_revision: OpaqueRevision::new("scan-revision-1").unwrap(),
                exact_locales: vec![request.source_locale],
            }],
            next_cursor,
        })
    }

    async fn read_resource(
        &self,
        _context: PortContext,
        _request: ReadTranslationResourceRequest,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        Err(unavailable())
    }

    async fn validate_patch(
        &self,
        _context: PortContext,
        _request: TranslationPatchRequest,
    ) -> Result<TranslationPatchValidation, PortError> {
        Err(unavailable())
    }

    async fn apply_patch(
        &self,
        _context: PortContext,
        _request: TranslationPatchRequest,
    ) -> Result<TranslationApplicationReceipt, PortError> {
        Err(unavailable())
    }

    async fn read_changes(
        &self,
        _context: PortContext,
        request: TranslationTargetChangesRequest,
    ) -> Result<TranslationTargetChangePage, PortError> {
        if let Some(barrier) = &self.change_barrier {
            barrier.wait().await;
        }
        if let Some(gate) = &self.change_gate {
            gate.entered.notify_one();
            gate.resume.notified().await;
        }
        if matches!(self.page, ChangePageFixture::ProviderOutage) {
            return Err(PortError::unavailable(
                "media.translation_changes_unavailable",
                "provider is unavailable",
            ));
        }
        if matches!(
            self.page,
            ChangePageFixture::Valid | ChangePageFixture::Deleted
        ) && request.after.is_some()
        {
            return Ok(TranslationTargetChangePage {
                changes: Vec::new(),
                next_cursor: None,
            });
        }
        let owner_slug = match self.page {
            ChangePageFixture::WrongOwner => OwnerSlug::new("product").unwrap(),
            ChangePageFixture::Valid
            | ChangePageFixture::Deleted
            | ChangePageFixture::MissingCursor
            | ChangePageFixture::ProviderOutage => OwnerSlug::new("media").unwrap(),
        };
        Ok(TranslationTargetChangePage {
            changes: vec![TranslationTargetChange {
                identity: TranslationResourceIdentity {
                    owner_slug,
                    resource_kind: ResourceKind::new("asset").unwrap(),
                    resource_id: ResourceId::new("asset-1").unwrap(),
                    subresource_id: None,
                },
                resource_revision: OpaqueRevision::new("revision-7").unwrap(),
                lifecycle: if matches!(self.page, ChangePageFixture::Deleted) {
                    TranslationResourceLifecycle::Deleted
                } else {
                    TranslationResourceLifecycle::Active
                },
            }],
            next_cursor: match self.page {
                ChangePageFixture::MissingCursor => None,
                ChangePageFixture::Valid
                | ChangePageFixture::Deleted
                | ChangePageFixture::WrongOwner
                | ChangePageFixture::ProviderOutage => Some(OpaqueCursor::new("cursor-1").unwrap()),
            },
        })
    }
}

async fn insert_memory_entry(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    resource_id: &str,
) -> Uuid {
    let id = Uuid::new_v4();
    let now = Utc::now().fixed_offset();
    memory_entry::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        source_locale: Set("en".to_string()),
        target_locale: Set("de".to_string()),
        owner_slug: Set("media".to_string()),
        resource_kind: Set("asset".to_string()),
        resource_id: Set(resource_id.to_string()),
        subresource_id: Set(None),
        field_key: Set("title".to_string()),
        source_text: Set("Source".to_string()),
        target_text: Set("Target".to_string()),
        source_key: Set(Uuid::new_v4().simple().to_string()),
        source_hash: Set(Uuid::new_v4().simple().to_string()),
        target_hash: Set(Uuid::new_v4().simple().to_string()),
        context_fingerprint: Set(Uuid::new_v4().simple().to_string()),
        segmentation_version: Set("owner-field-v1".to_string()),
        origin: Set("manual".to_string()),
        quality_state: Set("human_approved_applied".to_string()),
        reviewer_actor_kind: Set("system".to_string()),
        reviewer_actor_id: Set("inventory-test".to_string()),
        proposal_id: Set(Uuid::new_v4()),
        apply_receipt_id: Set(Uuid::new_v4()),
        retention_policy: Set("owner_lifecycle".to_string()),
        retain_until: Set(None),
        owner_lifecycle_revision: Set(None),
        owner_deleted_at: Set(None),
        tombstoned_at: Set(None),
        revision: Set(1),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(database)
    .await
    .unwrap();
    id
}

fn unavailable() -> PortError {
    PortError::unavailable("translation.test_unavailable", "not used by this fixture")
}

async fn fixture(
    page: ChangePageFixture,
) -> (DatabaseConnection, TranslationInventoryService, PortContext) {
    fixture_with_provider(CursorProvider {
        page,
        change_gate: None,
        change_barrier: None,
        list_gate: None,
    })
    .await
}

async fn fixture_with_provider(
    provider: CursorProvider,
) -> (DatabaseConnection, TranslationInventoryService, PortContext) {
    let database = Database::connect("sqlite::memory:").await.unwrap();
    initialize_fixture(database, provider).await
}

async fn fixture_at(
    database_path: &Path,
    provider: CursorProvider,
) -> (DatabaseConnection, TranslationInventoryService, PortContext) {
    let database = connect_file(database_path, true).await;
    initialize_fixture(database, provider).await
}

async fn connect_file(database_path: &Path, create_if_missing: bool) -> DatabaseConnection {
    let database_path = database_path.to_path_buf();
    let mut options =
        ConnectOptions::new("sqlite://translation-inventory-placeholder.sqlite?mode=rwc");
    options
        .max_connections(4)
        .min_connections(1)
        .sqlx_logging(false)
        .map_sqlx_sqlite_opts(move |options| {
            options
                .filename(database_path.clone())
                .create_if_missing(create_if_missing)
                .busy_timeout(Duration::from_secs(5))
        });
    let database = Database::connect(options).await.unwrap();
    database
        .execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .unwrap();
    database
}

async fn initialize_fixture(
    database: DatabaseConnection,
    provider: CursorProvider,
) -> (DatabaseConnection, TranslationInventoryService, PortContext) {
    database
        .execute_unprepared("CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL)")
        .await
        .unwrap();
    let manager = SchemaManager::new(&database);
    for migration in migrations::migrations() {
        migration.up(&manager).await.unwrap();
    }
    let tenant_id = Uuid::new_v4();
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenants (id) VALUES (?)",
            [tenant_id.into()],
        ))
        .await
        .unwrap();
    let (service, context) = service_and_context(database.clone(), tenant_id, provider);
    (database, service, context)
}

fn service_and_context(
    database: DatabaseConnection,
    tenant_id: Uuid,
    provider: CursorProvider,
) -> (TranslationInventoryService, PortContext) {
    let mut registry = TranslationTargetRegistry::default();
    registry.register(provider).unwrap();
    let service = TranslationInventoryService::new(database.clone(), Arc::new(registry));
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        "translation-inventory-sync",
    )
    .with_deadline(Duration::from_secs(5));
    (service, context)
}

fn sqlite_test_files(database_path: &Path) -> [PathBuf; 4] {
    [
        database_path.to_path_buf(),
        PathBuf::from(format!("{}-journal", database_path.display())),
        PathBuf::from(format!("{}-shm", database_path.display())),
        PathBuf::from(format!("{}-wal", database_path.display())),
    ]
}

fn inventory_test_database() -> (PathBuf, SqliteTestFileGuard) {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace path");
    let evidence_dir = workspace.join("target/translation-inventory-process-tests");
    std::fs::create_dir_all(&evidence_dir)
        .expect("Translation inventory process evidence directory");
    let evidence_dir = evidence_dir
        .canonicalize()
        .expect("Translation inventory process evidence path");
    assert!(evidence_dir.starts_with(workspace.join("target")));
    let database_path = evidence_dir.join(format!(
        "rustok-translation-inventory-{}.sqlite",
        Uuid::new_v4()
    ));
    let guard = SqliteTestFileGuard(database_path.clone());
    (database_path, guard)
}

#[tokio::test]
async fn sync_replays_provider_cursor_without_losing_checkpoint() {
    let (database, service, context) = fixture(ChangePageFixture::Valid).await;
    let first = service
        .sync_provider_changes(
            context.clone(),
            OwnerSlug::new("media").unwrap(),
            ResourceKind::new("asset").unwrap(),
            10,
        )
        .await
        .unwrap();
    assert_eq!(first.observed_resources, 1);
    assert_eq!(first.checkpoint.as_ref().unwrap().as_str(), "cursor-1");
    assert_eq!(first.checkpoint_revision, 1);

    let second = service
        .sync_provider_changes(
            context,
            OwnerSlug::new("media").unwrap(),
            ResourceKind::new("asset").unwrap(),
            10,
        )
        .await
        .unwrap();
    assert_eq!(second.observed_resources, 0);
    assert_eq!(second.checkpoint.as_ref().unwrap().as_str(), "cursor-1");
    assert_eq!(second.checkpoint_revision, 2);
    assert_eq!(
        inventory_resource::Entity::find()
            .count(&database)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        provider_checkpoint::Entity::find()
            .count(&database)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn deleted_owner_resource_atomically_marks_matching_memory_for_retention() {
    let (database, service, context) = fixture(ChangePageFixture::Deleted).await;
    let tenant_id = Uuid::parse_str(&context.tenant_id).unwrap();
    let entry_id = insert_memory_entry(&database, tenant_id, "asset-1").await;

    service
        .sync_provider_changes(
            context,
            OwnerSlug::new("media").unwrap(),
            ResourceKind::new("asset").unwrap(),
            10,
        )
        .await
        .unwrap();

    let entry = memory_entry::Entity::find_by_id(entry_id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        entry.owner_lifecycle_revision.as_deref(),
        Some("revision-7")
    );
    assert!(entry.owner_deleted_at.is_some());
    assert_eq!(entry.revision, 2);
    assert!(entry.tombstoned_at.is_none());
}

#[tokio::test]
async fn deleted_owner_resource_in_full_rebuild_marks_matching_memory() {
    let (database, service, context) = fixture(ChangePageFixture::Deleted).await;
    let tenant_id = Uuid::parse_str(&context.tenant_id).unwrap();
    let entry_id = insert_memory_entry(&database, tenant_id, "scan-asset-1").await;

    service
        .rebuild_provider_inventory(
            context,
            OwnerSlug::new("media").unwrap(),
            ResourceKind::new("asset").unwrap(),
            TenantLocale::new("en").unwrap(),
            TenantLocale::new("de").unwrap(),
            1,
        )
        .await
        .unwrap();

    let entry = memory_entry::Entity::find_by_id(entry_id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        entry.owner_lifecycle_revision.as_deref(),
        Some("scan-revision-1")
    );
    assert!(entry.owner_deleted_at.is_some());
    assert_eq!(entry.revision, 2);
}

#[tokio::test]
async fn sync_rejects_changes_without_a_checkpoint_cursor() {
    let (database, service, context) = fixture(ChangePageFixture::MissingCursor).await;

    let error = service
        .sync_provider_changes(
            context,
            OwnerSlug::new("media").unwrap(),
            ResourceKind::new("asset").unwrap(),
            10,
        )
        .await
        .unwrap_err();

    assert!(matches!(error, TranslationError::MissingCheckpointCursor));
    assert_eq!(
        inventory_resource::Entity::find()
            .count(&database)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        provider_checkpoint::Entity::find()
            .count(&database)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn sync_rejects_changes_owned_by_another_provider() {
    let (database, service, context) = fixture(ChangePageFixture::WrongOwner).await;

    let error = service
        .sync_provider_changes(
            context,
            OwnerSlug::new("media").unwrap(),
            ResourceKind::new("asset").unwrap(),
            10,
        )
        .await
        .unwrap_err();

    assert!(matches!(error, TranslationError::ProviderIdentityMismatch));
    assert_eq!(
        inventory_resource::Entity::find()
            .count(&database)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        provider_checkpoint::Entity::find()
            .count(&database)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn sync_rejects_invalid_page_bounds_before_calling_the_provider() {
    let (database, service, context) = fixture(ChangePageFixture::ProviderOutage).await;

    let error = service
        .sync_provider_changes(
            context,
            OwnerSlug::new("media").unwrap(),
            ResourceKind::new("asset").unwrap(),
            0,
        )
        .await
        .unwrap_err();

    assert!(matches!(error, TranslationError::InvalidRequest(_)));
    assert_eq!(
        provider_checkpoint::Entity::find()
            .count(&database)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn provider_outage_does_not_advance_inventory_or_checkpoint() {
    let (database, service, context) = fixture(ChangePageFixture::ProviderOutage).await;

    let error = service
        .sync_provider_changes(
            context,
            OwnerSlug::new("media").unwrap(),
            ResourceKind::new("asset").unwrap(),
            10,
        )
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        TranslationError::Provider {
            ref code,
            retryable: true,
            ..
        } if code == "media.translation_changes_unavailable"
    ));
    assert_eq!(
        inventory_resource::Entity::find()
            .count(&database)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        provider_checkpoint::Entity::find()
            .count(&database)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn provider_inventory_and_checkpoints_are_tenant_isolated() {
    let (database, service, first_context) = fixture(ChangePageFixture::Valid).await;
    let first_tenant_id = Uuid::parse_str(&first_context.tenant_id).unwrap();
    let second_tenant_id = Uuid::new_v4();
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenants (id) VALUES (?)",
            [second_tenant_id.into()],
        ))
        .await
        .unwrap();
    let second_context = PortContext::new(
        second_tenant_id.to_string(),
        PortActor::system(),
        "en",
        "translation-inventory-sync-second-tenant",
    )
    .with_deadline(Duration::from_secs(5));

    for context in [first_context, second_context] {
        service
            .sync_provider_changes(
                context,
                OwnerSlug::new("media").unwrap(),
                ResourceKind::new("asset").unwrap(),
                10,
            )
            .await
            .unwrap();
    }

    for tenant_id in [first_tenant_id, second_tenant_id] {
        assert_eq!(
            inventory_resource::Entity::find()
                .filter(inventory_resource::Column::TenantId.eq(tenant_id))
                .count(&database)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            provider_checkpoint::Entity::find()
                .filter(provider_checkpoint::Column::TenantId.eq(tenant_id))
                .count(&database)
                .await
                .unwrap(),
            1
        );
    }
}

#[tokio::test]
async fn stale_checkpoint_is_rejected_before_inventory_persistence() {
    let gate = ProviderGate {
        entered: Arc::new(Notify::new()),
        resume: Arc::new(Notify::new()),
    };
    let (database, service, context) = fixture_with_provider(CursorProvider {
        page: ChangePageFixture::Valid,
        change_gate: Some(gate.clone()),
        change_barrier: None,
        list_gate: None,
    })
    .await;
    let tenant_id = Uuid::parse_str(&context.tenant_id).unwrap();
    let sync = tokio::spawn(async move {
        service
            .sync_provider_changes(
                context,
                OwnerSlug::new("media").unwrap(),
                ResourceKind::new("asset").unwrap(),
                10,
            )
            .await
    });

    gate.entered.notified().await;
    provider_checkpoint::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        owner_slug: Set("media".to_string()),
        resource_kind: Set("asset".to_string()),
        cursor: Set(Some("cursor-concurrent".to_string())),
        revision: Set(7),
        updated_at: Set(Utc::now().fixed_offset()),
    }
    .insert(&database)
    .await
    .unwrap();
    gate.resume.notify_one();

    let error = sync.await.unwrap().unwrap_err();
    assert!(matches!(error, TranslationError::CheckpointConflict));
    assert_eq!(
        inventory_resource::Entity::find()
            .count(&database)
            .await
            .unwrap(),
        0
    );
    let checkpoint = provider_checkpoint::Entity::find()
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(checkpoint.cursor.as_deref(), Some("cursor-concurrent"));
    assert_eq!(checkpoint.revision, 7);
}

#[tokio::test]
async fn full_rescan_atomically_replaces_provider_inventory_after_cursor_drain() {
    let (database, service, context) = fixture(ChangePageFixture::Valid).await;

    let result = service
        .rebuild_provider_inventory(
            context,
            OwnerSlug::new("media").unwrap(),
            ResourceKind::new("asset").unwrap(),
            TenantLocale::new("en").unwrap(),
            TenantLocale::new("de").unwrap(),
            1,
        )
        .await
        .unwrap();

    assert_eq!(result.observed_resources, 2);
    assert_eq!(result.checkpoint.as_ref().unwrap().as_str(), "cursor-1");
    assert_eq!(result.checkpoint_revision, 3);
    let rows = inventory_resource::Entity::find()
        .all(&database)
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows.iter()
            .map(|row| row.resource_id.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["scan-asset-1", "scan-asset-2"])
    );
}

#[tokio::test]
async fn full_rescan_rolls_back_when_checkpoint_advances_during_listing() {
    let gate = ProviderGate {
        entered: Arc::new(Notify::new()),
        resume: Arc::new(Notify::new()),
    };
    let (database, service, context) = fixture_with_provider(CursorProvider {
        page: ChangePageFixture::Valid,
        change_gate: None,
        change_barrier: None,
        list_gate: Some(gate.clone()),
    })
    .await;
    let rebuild = tokio::spawn(async move {
        service
            .rebuild_provider_inventory(
                context,
                OwnerSlug::new("media").unwrap(),
                ResourceKind::new("asset").unwrap(),
                TenantLocale::new("en").unwrap(),
                TenantLocale::new("de").unwrap(),
                1,
            )
            .await
    });

    gate.entered.notified().await;
    provider_checkpoint::Entity::update_many()
        .col_expr(
            provider_checkpoint::Column::Cursor,
            sea_orm::sea_query::Expr::value("cursor-concurrent"),
        )
        .col_expr(
            provider_checkpoint::Column::Revision,
            sea_orm::sea_query::Expr::value(7_i64),
        )
        .exec(&database)
        .await
        .unwrap();
    gate.resume.notify_one();

    let error = rebuild.await.unwrap().unwrap_err();
    assert!(matches!(error, TranslationError::CheckpointConflict));
    let rows = inventory_resource::Entity::find()
        .all(&database)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].resource_id, "asset-1");
    let checkpoint = provider_checkpoint::Entity::find()
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(checkpoint.cursor.as_deref(), Some("cursor-concurrent"));
    assert_eq!(checkpoint.revision, 7);
}

#[tokio::test]
async fn independent_replica_pools_converge_on_one_inventory_checkpoint() {
    let (database_path, _database_cleanup) = inventory_test_database();
    let (database, service, context) = fixture_at(
        &database_path,
        CursorProvider {
            page: ChangePageFixture::Valid,
            change_gate: None,
            change_barrier: None,
            list_gate: None,
        },
    )
    .await;
    let tenant_id = Uuid::parse_str(&context.tenant_id).unwrap();
    drop(service);
    database.close().await.unwrap();

    let barrier = Arc::new(Barrier::new(2));
    let first_database = connect_file(&database_path, false).await;
    let second_database = connect_file(&database_path, false).await;
    let (first, first_context) = service_and_context(
        first_database.clone(),
        tenant_id,
        CursorProvider {
            page: ChangePageFixture::Valid,
            change_gate: None,
            change_barrier: Some(barrier.clone()),
            list_gate: None,
        },
    );
    let (second, second_context) = service_and_context(
        second_database.clone(),
        tenant_id,
        CursorProvider {
            page: ChangePageFixture::Valid,
            change_gate: None,
            change_barrier: Some(barrier),
            list_gate: None,
        },
    );

    let (first_result, second_result) = tokio::join!(
        first.sync_provider_changes(
            first_context,
            OwnerSlug::new("media").unwrap(),
            ResourceKind::new("asset").unwrap(),
            10,
        ),
        second.sync_provider_changes(
            second_context,
            OwnerSlug::new("media").unwrap(),
            ResourceKind::new("asset").unwrap(),
            10,
        )
    );
    let mut completed = 0;
    let mut conflicts = 0;
    for result in [first_result, second_result] {
        match result {
            Ok(result) => {
                completed += 1;
                assert_eq!(result.checkpoint_revision, 1);
            }
            Err(TranslationError::CheckpointConflict) => conflicts += 1,
            Err(error) => panic!("unexpected replica sync failure: {error}"),
        }
    }
    assert_eq!(completed, 1);
    assert_eq!(conflicts, 1);
    assert_eq!(
        inventory_resource::Entity::find()
            .filter(inventory_resource::Column::TenantId.eq(tenant_id))
            .count(&first_database)
            .await
            .unwrap(),
        1
    );
    let checkpoint = provider_checkpoint::Entity::find()
        .filter(provider_checkpoint::Column::TenantId.eq(tenant_id))
        .one(&first_database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(checkpoint.cursor.as_deref(), Some("cursor-1"));
    assert_eq!(checkpoint.revision, 1);
    first_database.close().await.unwrap();
    second_database.close().await.unwrap();
}

#[tokio::test]
async fn separate_process_recovers_inventory_after_outage_and_rebuilds_atomically() {
    let (database_path, _database_cleanup) = inventory_test_database();
    let (database, service, context) = fixture_at(
        &database_path,
        CursorProvider {
            page: ChangePageFixture::ProviderOutage,
            change_gate: None,
            change_barrier: None,
            list_gate: None,
        },
    )
    .await;
    let tenant_id = Uuid::parse_str(&context.tenant_id).unwrap();
    let error = service
        .sync_provider_changes(
            context,
            OwnerSlug::new("media").unwrap(),
            ResourceKind::new("asset").unwrap(),
            10,
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        TranslationError::Provider {
            ref code,
            retryable: true,
            ..
        } if code == "media.translation_changes_unavailable"
    ));
    assert_eq!(
        inventory_resource::Entity::find()
            .count(&database)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        provider_checkpoint::Entity::find()
            .count(&database)
            .await
            .unwrap(),
        0
    );
    drop(service);
    database.close().await.unwrap();

    for action in ["sync", "rebuild"] {
        let output = Command::new(std::env::current_exe().expect("Translation test executable"))
            .args([
                "--exact",
                "inventory_process_recovery_child",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("RUSTOK_TRANSLATION_TEST_INVENTORY_DB_PATH", &database_path)
            .env(
                "RUSTOK_TRANSLATION_TEST_INVENTORY_TENANT_ID",
                tenant_id.to_string(),
            )
            .env("RUSTOK_TRANSLATION_TEST_INVENTORY_ACTION", action)
            .output()
            .await
            .expect("Translation inventory child process");
        assert!(
            output.status.success(),
            "Translation inventory child failed for action {action}:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let database = connect_file(&database_path, false).await;
    let rows = inventory_resource::Entity::find()
        .filter(inventory_resource::Column::TenantId.eq(tenant_id))
        .all(&database)
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows.iter()
            .map(|row| row.resource_id.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["scan-asset-1", "scan-asset-2"])
    );
    let checkpoint = provider_checkpoint::Entity::find()
        .filter(provider_checkpoint::Column::TenantId.eq(tenant_id))
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(checkpoint.cursor.as_deref(), Some("cursor-1"));
    assert_eq!(checkpoint.revision, 3);
    database.close().await.unwrap();
}

#[tokio::test]
#[ignore = "internal child process for Translation inventory recovery evidence"]
async fn inventory_process_recovery_child() {
    let Some(database_path) = std::env::var_os("RUSTOK_TRANSLATION_TEST_INVENTORY_DB_PATH") else {
        return;
    };
    let tenant_id = Uuid::parse_str(
        &std::env::var("RUSTOK_TRANSLATION_TEST_INVENTORY_TENANT_ID")
            .expect("Translation inventory child tenant id"),
    )
    .expect("Translation inventory child tenant UUID");
    let action = std::env::var("RUSTOK_TRANSLATION_TEST_INVENTORY_ACTION")
        .expect("Translation inventory child action");
    let database = connect_file(Path::new(&database_path), false).await;
    let (service, context) = service_and_context(
        database.clone(),
        tenant_id,
        CursorProvider {
            page: ChangePageFixture::Valid,
            change_gate: None,
            change_barrier: None,
            list_gate: None,
        },
    );
    match action.as_str() {
        "sync" => {
            let result = service
                .sync_provider_changes(
                    context,
                    OwnerSlug::new("media").unwrap(),
                    ResourceKind::new("asset").unwrap(),
                    10,
                )
                .await
                .unwrap();
            assert_eq!(result.observed_resources, 1);
            assert_eq!(result.checkpoint_revision, 1);
        }
        "rebuild" => {
            let result = service
                .rebuild_provider_inventory(
                    context,
                    OwnerSlug::new("media").unwrap(),
                    ResourceKind::new("asset").unwrap(),
                    TenantLocale::new("en").unwrap(),
                    TenantLocale::new("de").unwrap(),
                    1,
                )
                .await
                .unwrap();
            assert_eq!(result.observed_resources, 2);
            assert_eq!(result.checkpoint_revision, 3);
        }
        _ => panic!("unknown Translation inventory child action"),
    }
    drop(service);
    database.close().await.unwrap();
}
