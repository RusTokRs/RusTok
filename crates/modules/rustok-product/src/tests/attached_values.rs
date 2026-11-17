use std::{sync::Arc, time::Duration};

use rustok_api::{PortActor, PortErrorKind};
use rustok_outbox_storage::{OutboxTransport, SysEvents, SysEventsMigration, TransactionalEventBus};
use sea_orm::{Database, PaginatorTrait, Schema};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use serde_json::json;

use super::*;
use crate::services::catalog::helpers::product_field_definitions_storage as field;

fn read_context(tenant: Uuid, locale: &str) -> PortContext {
    PortContext::new(
        tenant.to_string(),
        PortActor::system(),
        locale,
        "test-attached-values",
    )
    .with_deadline(Duration::from_secs(5))
}

#[tokio::test]
async fn attached_values_preserve_owner_metadata_and_commit_with_revision_and_outbox() {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let manager = SchemaManager::new(&db);
    let schema = Schema::new(db.get_database_backend());
    manager
        .create_table(schema.create_table_from_entity(product::Entity))
        .await
        .unwrap();
    manager
        .create_table(schema.create_table_from_entity(field::Entity))
        .await
        .unwrap();
    db.execute_unprepared("CREATE UNIQUE INDEX uq_products_tenant_id ON products (tenant_id, id)")
        .await
        .unwrap();
    crate::migrations::m20261116_000041_move_product_attached_localized_values::Migration
        .up(&manager)
        .await
        .unwrap();
    SysEventsMigration.up(&manager).await.unwrap();
    rustok_outbox_storage::migration::create_owner_operation_receipts_table(&manager)
        .await
        .unwrap();

    let tenant = Uuid::new_v4();
    let id = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let write = |tenant: Uuid, locale: &str, key: &str| {
        let mut context = read_context(tenant, locale).with_idempotency_key(key);
        context.actor = PortActor::user(actor.to_string());
        context
    };
    let now = Utc::now();
    product::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant),
        status: Set(product::ProductStatus::Draft),
        metadata: Set(json!({"source": "erp", "shipping_profile": "standard"})),
        revision: Set(1),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    for (key, localized) in [
        ("badge", false),
        ("tagline", true),
        ("shipping_profile", false),
    ] {
        field::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant),
            field_key: Set(key.to_owned()),
            field_type: Set("text".to_owned()),
            label: Set(json!({"en": key})),
            is_localized: Set(localized),
            is_required: Set(false),
            position: Set(0),
            is_active: Set(true),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
    }
    let catalog = CatalogService::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone()))),
    );
    let mut no_deadline = read_context(tenant, "en");
    no_deadline.deadline_ms = None;
    assert_eq!(
        catalog
            .read_attached_values(no_deadline, id, "en")
            .await
            .unwrap_err()
            .code,
        "port.deadline_required",
    );
    assert_eq!(
        catalog
            .read_attached_values(read_context(Uuid::new_v4(), "en"), id, "en")
            .await
            .unwrap_err()
            .kind,
        PortErrorKind::NotFound,
    );
    assert_eq!(
        catalog
            .read_attached_values(read_context(tenant, "en"), id, "en")
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        catalog
            .update_attached_values(
                write(tenant, "en", "initial"),
                id,
                Some(json!({"badge": "gold", "tagline": "Hello"})),
            )
            .await
            .unwrap(),
        Some(json!({"badge": "gold", "tagline": "Hello"}))
    );
    catalog
        .update_attached_values(
            write(tenant, "ru", "add-ru"),
            id,
            Some(json!({"tagline": "Привет"})),
        )
        .await
        .unwrap();
    assert_eq!(
        catalog
            .read_attached_values(read_context(tenant, "fr"), id, "ru")
            .await
            .unwrap(),
        Some(json!({"badge": "gold", "tagline": "Привет"}))
    );
    assert_eq!(
        catalog
            .read_attached_values(read_context(tenant, "en"), id, "ru")
            .await
            .unwrap(),
        Some(json!({"badge": "gold", "tagline": "Hello"}))
    );
    assert_eq!(
        catalog
            .update_attached_values(
                write(tenant, "en", "reserved-field"),
                id,
                Some(json!({"shipping_profile": "bypass"})),
            )
            .await
            .unwrap_err()
            .kind,
        PortErrorKind::Validation
    );
    assert_eq!(
        catalog
            .delete_attached_values(write(Uuid::new_v4(), "en", "foreign-delete"), id)
            .await
            .unwrap_err()
            .kind,
        PortErrorKind::NotFound
    );
    let row = product::Entity::find_by_id(id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.revision, 3);
    assert_eq!(
        row.metadata,
        json!({"source": "erp", "shipping_profile": "standard", "badge": "gold"})
    );
    assert_eq!(SysEvents::find().count(&db).await.unwrap(), 2);

    db.execute_unprepared(
        "CREATE TRIGGER reject_product_event BEFORE INSERT ON sys_events
         BEGIN SELECT RAISE(ABORT, 'outbox unavailable'); END",
    )
    .await
    .unwrap();
    assert!(
        catalog
            .delete_attached_values(write(tenant, "en", "failed-delete"), id)
            .await
            .is_err()
    );
    let row_after_failure = product::Entity::find_by_id(id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row_after_failure.revision, 3);
    assert_eq!(
        catalog
            .read_attached_values(read_context(tenant, "ru"), id, "en")
            .await
            .unwrap(),
        Some(json!({"badge": "gold", "tagline": "Привет"}))
    );
    db.execute_unprepared("DROP TRIGGER reject_product_event")
        .await
        .unwrap();

    catalog
        .delete_attached_values(write(tenant, "en", "delete"), id)
        .await
        .unwrap();
    assert_eq!(
        catalog
            .read_attached_values(read_context(tenant, "ru"), id, "en")
            .await
            .unwrap(),
        None
    );
    let row = product::Entity::find_by_id(id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.revision, 4);
    assert_eq!(
        row.metadata,
        json!({"source": "erp", "shipping_profile": "standard"})
    );
    assert_eq!(SysEvents::find().count(&db).await.unwrap(), 3);

    let latest = Some(json!({"badge": "platinum", "tagline": "New"}));
    assert_eq!(
        catalog
            .update_attached_values(write(tenant, "en", "repopulate"), id, latest.clone())
            .await
            .unwrap(),
        latest
    );
    let mut original = write(tenant, "EN", "initial");
    original.correlation_id = "retry".to_owned();
    let original_values = Some(json!({"tagline": "Hello", "badge": "gold"}));
    assert_eq!(
        catalog
            .update_attached_values(original, id, original_values.clone())
            .await
            .unwrap(),
        original_values
    );
    let conflict = catalog
        .update_attached_values(write(tenant, "en", "initial"), id, latest.clone())
        .await
        .unwrap_err();
    assert_eq!(conflict.code, "product.idempotency_conflict");
    let mut foreign_actor = write(tenant, "en", "initial");
    foreign_actor.actor = PortActor::service(Uuid::new_v4().to_string());
    assert_eq!(
        catalog
            .update_attached_values(foreign_actor, id, original_values)
            .await
            .unwrap_err()
            .kind,
        PortErrorKind::Conflict
    );
    catalog
        .delete_attached_values(write(tenant, "ru", "delete"), id)
        .await
        .unwrap();
    assert_eq!(
        catalog
            .read_attached_values(read_context(tenant, "en"), id, "en")
            .await
            .unwrap(),
        latest
    );
    assert_eq!(
        product::Entity::find_by_id(id)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .revision,
        5
    );
    assert_eq!(SysEvents::find().count(&db).await.unwrap(), 4);
}
