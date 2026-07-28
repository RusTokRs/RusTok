use std::{collections::BTreeSet, sync::Arc, time::Duration};

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError};
use rustok_translation::{
    TranslationError, TranslationInventoryService,
    entities::{inventory_resource, provider_checkpoint},
    migrations,
};
use rustok_translation_targets::{
    ListTranslationResourcesRequest, OpaqueCursor, OpaqueRevision, OwnerSlug,
    ReadTranslationResourceRequest, ResourceId, ResourceKind, TranslationApplicationReceipt,
    TranslationPatchRequest, TranslationPatchValidation, TranslationResourceIdentity,
    TranslationResourceLifecycle, TranslationResourcePage, TranslationResourceSnapshot,
    TranslationTargetCapability, TranslationTargetChange, TranslationTargetChangePage,
    TranslationTargetChangesRequest, TranslationTargetProvider,
    TranslationTargetProviderDescriptor, TranslationTargetRegistry,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait,
    PaginatorTrait, QueryFilter, Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[derive(Clone, Copy)]
enum ChangePageFixture {
    Valid,
    MissingCursor,
    WrongOwner,
    ProviderOutage,
}

struct CursorProvider(ChangePageFixture);

#[async_trait]
impl TranslationTargetProvider for CursorProvider {
    fn descriptor(&self) -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new("media").unwrap(),
            resource_kind: ResourceKind::new("asset").unwrap(),
            display_name: "Media asset metadata".to_string(),
            capabilities: BTreeSet::from([TranslationTargetCapability::ChangeCursor]),
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
        if matches!(self.0, ChangePageFixture::ProviderOutage) {
            return Err(PortError::unavailable(
                "media.translation_changes_unavailable",
                "provider is unavailable",
            ));
        }
        if matches!(self.0, ChangePageFixture::Valid) && request.after.is_some() {
            return Ok(TranslationTargetChangePage {
                changes: Vec::new(),
                next_cursor: None,
            });
        }
        let owner_slug = match self.0 {
            ChangePageFixture::WrongOwner => OwnerSlug::new("product").unwrap(),
            ChangePageFixture::Valid
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
                lifecycle: TranslationResourceLifecycle::Active,
            }],
            next_cursor: match self.0 {
                ChangePageFixture::MissingCursor => None,
                ChangePageFixture::Valid
                | ChangePageFixture::WrongOwner
                | ChangePageFixture::ProviderOutage => Some(OpaqueCursor::new("cursor-1").unwrap()),
            },
        })
    }
}

fn unavailable() -> PortError {
    PortError::unavailable("translation.test_unavailable", "not used by this fixture")
}

async fn fixture(
    page: ChangePageFixture,
) -> (DatabaseConnection, TranslationInventoryService, PortContext) {
    let database = Database::connect("sqlite::memory:").await.unwrap();
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
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenants (id) VALUES (?)",
            [tenant_id.into()],
        ))
        .await
        .unwrap();
    let mut registry = TranslationTargetRegistry::default();
    registry.register(CursorProvider(page)).unwrap();
    let service = TranslationInventoryService::new(database.clone(), Arc::new(registry));
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        "translation-inventory-sync",
    )
    .with_deadline(Duration::from_secs(5));
    (database, service, context)
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
        .execute(Statement::from_sql_and_values(
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
