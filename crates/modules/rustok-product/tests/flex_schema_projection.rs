use std::{sync::Arc, time::Duration};

use flex::{
    CreateFieldDefinitionCommand, FieldDefRegistry, FieldDefinitionService, FieldDefinitionView,
    FlexAttachedFieldReadService, FlexVersionedAttachedSchemaReadPort,
    UpdateFieldDefinitionCommand,
};
use rustok_api::{PortActor, PortContext};
use rustok_core::field_schema::FlexError;
use rustok_events::EventEnvelope;
use rustok_product::{
    migrations::m20261116_000042_add_product_flex_schema_projection::Migration,
    services::flex_schema_projection::{
        ProductFlexSchemaApplyOutcome::*, ProductFlexSchemaProjection,
    },
};
use rustok_test_utils::{
    connect_postgres, create_postgres_database, drop_postgres_database_if_exists,
    postgres_database_url, unique_postgres_database_name,
};
use sea_orm::{
    ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement, TransactionTrait,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use serde_json::json;
use uuid::Uuid;

struct FixtureDefinitions;

#[async_trait::async_trait]
impl FieldDefinitionService for FixtureDefinitions {
    fn entity_type(&self) -> &'static str {
        "product"
    }

    async fn list_all(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<Vec<FieldDefinitionView>, FlexError> {
        let rows = db
            .query_all_raw(Statement::from_sql_and_values(
                db.get_database_backend(),
                "SELECT id, position FROM product_field_definitions WHERE tenant_id = $1",
                [tenant_id.into()],
            ))
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|row| FieldDefinitionView {
                id: row.try_get("", "id").unwrap(),
                field_key: "tagline".into(),
                field_type: "text".into(),
                label: json!({"en": "Tagline"}),
                description: None,
                is_localized: true,
                is_required: false,
                default_value: None,
                validation: None,
                position: row.try_get("", "position").unwrap(),
                is_active: true,
                created_at: "2026-11-16T12:00:00Z".into(),
                updated_at: "2026-11-16T12:00:00Z".into(),
            })
            .collect())
    }

    async fn find_by_id(
        &self,
        _: &DatabaseConnection,
        _: Uuid,
        id: Uuid,
    ) -> Result<Option<FieldDefinitionView>, FlexError> {
        Err(FlexError::NotFound(id))
    }

    async fn reorder(
        &self,
        _: &DatabaseConnection,
        _: Uuid,
        _: &[Uuid],
    ) -> Result<Vec<FieldDefinitionView>, FlexError> {
        Err(FlexError::UnknownEntityType(self.entity_type().into()))
    }

    async fn create(
        &self,
        _: &DatabaseConnection,
        _: Uuid,
        _: Option<Uuid>,
        _: CreateFieldDefinitionCommand,
    ) -> Result<(FieldDefinitionView, EventEnvelope), FlexError> {
        Err(FlexError::UnknownEntityType(self.entity_type().into()))
    }

    async fn update(
        &self,
        _: &DatabaseConnection,
        _: Uuid,
        _: Option<Uuid>,
        id: Uuid,
        _: UpdateFieldDefinitionCommand,
    ) -> Result<(FieldDefinitionView, EventEnvelope), FlexError> {
        Err(FlexError::NotFound(id))
    }

    async fn deactivate(
        &self,
        _: &DatabaseConnection,
        _: Uuid,
        _: Option<Uuid>,
        id: Uuid,
    ) -> Result<EventEnvelope, FlexError> {
        Err(FlexError::NotFound(id))
    }
}

fn context(tenant_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        "test-flex-projection",
    )
    .with_deadline(Duration::from_secs(5))
}

async fn verify_projection(
    source: DatabaseConnection,
    target: DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    let tenant = Uuid::new_v4();
    let other_tenant = Uuid::new_v4();
    source.execute_unprepared(
        "CREATE TABLE product_field_definitions (id UUID PRIMARY KEY NOT NULL, tenant_id UUID NOT NULL, position INT NOT NULL)",
    ).await?;
    source
        .execute_raw(Statement::from_sql_and_values(
            source.get_database_backend(),
            "INSERT INTO product_field_definitions (id, tenant_id, position) VALUES ($1, $2, 0)",
            [Uuid::new_v4().into(), tenant.into()],
        ))
        .await?;

    let source_manager = SchemaManager::new(&source);
    let target_manager = SchemaManager::new(&target);
    Migration.up(&source_manager).await?;
    Migration.up(&target_manager).await?;
    assert!(
        !target_manager
            .has_table("product_field_definitions")
            .await?
    );
    assert!(
        !target_manager
            .has_table("flex_product_attached_schema_revisions")
            .await?
    );

    let mut registry = FieldDefRegistry::new();
    registry.register(Arc::new(FixtureDefinitions));
    let provider = FlexAttachedFieldReadService::new(source.clone(), registry);
    let store = ProductFlexSchemaProjection::new(target.clone());
    let initial = provider
        .read_versioned_attached_schema(context(tenant), "product")
        .await?;
    assert_eq!(initial.revision, 0);
    assert_eq!(initial.schema.definitions.len(), 1);
    assert_eq!(store.refresh(context(tenant), &provider).await?, Advanced);
    assert_eq!(store.refresh(context(tenant), &provider).await?, Replay);

    let txn = source.begin().await?;
    txn.execute_unprepared("UPDATE product_field_definitions SET position = 4")
        .await?;
    txn.rollback().await?;
    assert_eq!(
        provider
            .read_versioned_attached_schema(context(tenant), "product")
            .await?,
        initial
    );

    source
        .execute_unprepared("UPDATE product_field_definitions SET position = 3")
        .await?;
    let reordered = provider
        .read_versioned_attached_schema(context(tenant), "product")
        .await?;
    assert_eq!(reordered.revision, 1);
    assert_eq!(reordered.schema.definitions[0].position, 3);
    assert_eq!(store.refresh(context(tenant), &provider).await?, Advanced);
    assert_eq!(store.apply(tenant, initial.clone()).await?, Stale);
    assert!(store.apply(other_tenant, reordered.clone()).await.is_err());
    let mut conflicting = reordered.clone();
    conflicting.schema.definitions[0].position = 5;
    assert_eq!(
        store.apply(tenant, conflicting).await.unwrap_err().code,
        "product.flex_schema_revision_conflict"
    );
    assert_eq!(store.apply(tenant, reordered).await?, Replay);

    if source.get_database_backend() == DatabaseBackend::Postgres {
        source
            .execute_unprepared("TRUNCATE product_field_definitions")
            .await?;
    } else {
        source
            .execute_unprepared("DELETE FROM product_field_definitions")
            .await?;
    }
    let removed = provider
        .read_versioned_attached_schema(context(tenant), "product")
        .await?;
    assert_eq!(removed.revision, 2);
    assert!(removed.schema.definitions.is_empty());
    assert_eq!(store.apply(tenant, removed).await?, Advanced);
    let untouched = provider
        .read_versioned_attached_schema(context(other_tenant), "product")
        .await?;
    assert_eq!(untouched.revision, 0);
    assert!(untouched.schema.definitions.is_empty());
    assert_eq!(store.apply(other_tenant, untouched).await?, Advanced);

    Migration.down(&target_manager).await?;
    Migration.down(&source_manager).await?;
    assert!(
        !target_manager
            .has_table("product_flex_schema_projections")
            .await?
    );
    assert!(
        !source_manager
            .has_table("flex_product_attached_schema_revisions")
            .await?
    );
    source
        .execute_unprepared("DELETE FROM product_field_definitions")
        .await?;
    Ok(())
}

#[tokio::test]
async fn sqlite_imports_versioned_flex_schemas_across_databases()
-> Result<(), Box<dyn std::error::Error>> {
    verify_projection(
        Database::connect("sqlite::memory:").await?,
        Database::connect("sqlite::memory:").await?,
    )
    .await
}

#[tokio::test]
#[ignore = "requires RUSTOK_MIGRATION_SMOKE_ADMIN_URL for disposable PostgreSQL database creation"]
async fn postgres_imports_versioned_flex_schemas_across_databases()
-> Result<(), Box<dyn std::error::Error>> {
    let admin_url = std::env::var("RUSTOK_MIGRATION_SMOKE_ADMIN_URL")?;
    rustok_test_utils::assert_postgres_url(&admin_url);
    let admin = connect_postgres(&admin_url).await?;
    let source_name = unique_postgres_database_name("flex_snapshot_source");
    let target_name = unique_postgres_database_name("product_snapshot_target");
    create_postgres_database(&admin, &source_name).await?;
    let result = async {
        create_postgres_database(&admin, &target_name).await?;
        let source = connect_postgres(&postgres_database_url(&admin_url, &source_name)).await?;
        let target = connect_postgres(&postgres_database_url(&admin_url, &target_name)).await?;
        verify_projection(source, target).await
    }
    .await;
    let source_cleanup = drop_postgres_database_if_exists(&admin, &source_name).await;
    let target_cleanup = drop_postgres_database_if_exists(&admin, &target_name).await;
    result?;
    source_cleanup?;
    target_cleanup?;
    Ok(())
}
