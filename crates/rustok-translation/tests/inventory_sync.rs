use std::{collections::BTreeSet, sync::Arc, time::Duration};

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError};
use rustok_translation::{
    TranslationInventoryService,
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
use sea_orm::{ConnectionTrait, Database, DbBackend, EntityTrait, PaginatorTrait, Statement};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

struct CursorProvider;

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
        if request.after.is_some() {
            return Ok(TranslationTargetChangePage {
                changes: Vec::new(),
                next_cursor: None,
            });
        }
        Ok(TranslationTargetChangePage {
            changes: vec![TranslationTargetChange {
                identity: TranslationResourceIdentity {
                    owner_slug: OwnerSlug::new("media").unwrap(),
                    resource_kind: ResourceKind::new("asset").unwrap(),
                    resource_id: ResourceId::new("asset-1").unwrap(),
                    subresource_id: None,
                },
                resource_revision: OpaqueRevision::new("revision-7").unwrap(),
                lifecycle: TranslationResourceLifecycle::Active,
            }],
            next_cursor: Some(OpaqueCursor::new("cursor-1").unwrap()),
        })
    }
}

fn unavailable() -> PortError {
    PortError::unavailable("translation.test_unavailable", "not used by this fixture")
}

#[tokio::test]
async fn sync_replays_provider_cursor_without_losing_checkpoint() {
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
    registry.register(CursorProvider).unwrap();
    let service = TranslationInventoryService::new(database.clone(), Arc::new(registry));
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        "translation-inventory-sync",
    )
    .with_deadline(Duration::from_secs(5));

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
