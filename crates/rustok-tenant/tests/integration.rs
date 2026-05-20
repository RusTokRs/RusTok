use std::sync::Arc;

use rustok_outbox::entity as outbox_entity;
use rustok_outbox::{OutboxTransport, SysEvents, TransactionalEventBus};
use rustok_tenant::{
    CreateTenantInput, TenantError, TenantService, ToggleModuleInput, UpdateTenantInput,
    entities::{tenant, tenant_module},
};
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait, QueryOrder, Schema,
    sea_query::TableCreateStatement,
};

async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("failed to connect in-memory sqlite");

    let builder = db.get_database_backend();
    let schema = Schema::new(builder);

    create_entity_table(&db, &builder, schema.create_table_from_entity(tenant::Entity)).await;
    create_entity_table(
        &db,
        &builder,
        schema.create_table_from_entity(tenant_module::Entity),
    )
    .await;
    create_entity_table(
        &db,
        &builder,
        schema.create_table_from_entity(outbox_entity::Entity),
    )
    .await;

    db
}

async fn create_entity_table(
    db: &DatabaseConnection,
    builder: &DbBackend,
    mut statement: TableCreateStatement,
) {
    statement.if_not_exists();
    db.execute(builder.build(&statement))
        .await
        .expect("failed to create tenant test table");
}

#[tokio::test]
async fn tenant_crud_flow() {
    let db = setup_db().await;
    let service = TenantService::new(db.clone());

    let created = service
        .create_tenant(CreateTenantInput {
            name: "Acme".to_string(),
            slug: "acme".to_string(),
            domain: Some("acme.example".to_string()),
        })
        .await
        .expect("tenant should be created");

    assert_eq!(created.name, "Acme");
    assert_eq!(created.slug, "acme");
    assert!(created.is_active);

    let fetched = service
        .get_tenant(created.id)
        .await
        .expect("tenant should be fetched by id");
    assert_eq!(fetched.id, created.id);

    let fetched_by_slug = service
        .get_tenant_by_slug("acme")
        .await
        .expect("tenant should be fetched by slug");
    assert_eq!(fetched_by_slug.id, created.id);

    let updated = service
        .update_tenant(
            created.id,
            UpdateTenantInput {
                name: Some("Acme Updated".to_string()),
                domain: Some("shop.acme.example".to_string()),
                is_active: Some(false),
                settings: Some(serde_json::json!({
                    "features": {"checkout": true}
                })),
            },
        )
        .await
        .expect("tenant should be updated");

    assert_eq!(updated.name, "Acme Updated");
    assert_eq!(updated.domain.as_deref(), Some("shop.acme.example"));
    assert!(!updated.is_active);
    assert_eq!(updated.settings["features"]["checkout"], serde_json::json!(true));

    let (items, total) = service
        .list_tenants(1, 10)
        .await
        .expect("tenant list should load");
    assert_eq!(total, 1);
    assert_eq!(items.len(), 1);
}

#[tokio::test]
async fn reject_invalid_tenant_settings_schema() {
    let db = setup_db().await;
    let service = TenantService::new(db);

    let created = service
        .create_tenant(CreateTenantInput {
            name: "Settings Test".to_string(),
            slug: "settings-test".to_string(),
            domain: None,
        })
        .await
        .expect("tenant should be created");

    let err = service
        .update_tenant(
            created.id,
            UpdateTenantInput {
                name: None,
                domain: None,
                is_active: None,
                settings: Some(serde_json::json!(["invalid-root"])),
            },
        )
        .await
        .expect_err("non-object settings root must be rejected");

    assert!(matches!(err, TenantError::InvalidSettingsSchema(_)));
}

#[tokio::test]
#[ignore = "legacy deprecated toggle_module flow; migrate coverage to ModuleLifecycleService-based test"]
async fn module_toggle_flow_legacy() {
    let db = setup_db().await;
    let service = TenantService::new(db);

    let tenant = service
        .create_tenant(CreateTenantInput {
            name: "Toggle Test".to_string(),
            slug: "toggle-test".to_string(),
            domain: None,
        })
        .await
        .expect("tenant should be created");

    let enabled = service
        .toggle_module(
            tenant.id,
            ToggleModuleInput {
                module_slug: "blog".to_string(),
                enabled: true,
            },
        )
        .await
        .expect("module should be enabled");

    assert!(enabled.enabled);

    let disabled = service
        .toggle_module(
            tenant.id,
            ToggleModuleInput {
                module_slug: "blog".to_string(),
                enabled: false,
            },
        )
        .await
        .expect("module should be disabled");

    assert_eq!(disabled.id, enabled.id);
    assert!(!disabled.enabled);

    let modules = service
        .list_tenant_modules(tenant.id)
        .await
        .expect("tenant modules should list");
    assert_eq!(modules.len(), 1);
    assert!(!modules[0].enabled);
}

#[tokio::test]
#[allow(deprecated)]
async fn tenant_mutations_publish_outbox_events() {
    let db = setup_db().await;
    let transport = Arc::new(OutboxTransport::new(db.clone()));
    let event_bus = TransactionalEventBus::new(transport);
    let service = TenantService::with_event_bus(db.clone(), event_bus);

    let tenant = service
        .create_tenant(CreateTenantInput {
            name: "Outbox Tenant".to_string(),
            slug: "outbox-tenant".to_string(),
            domain: None,
        })
        .await
        .expect("tenant should be created");

    service
        .update_tenant(
            tenant.id,
            UpdateTenantInput {
                name: Some("Outbox Tenant Updated".to_string()),
                domain: None,
                is_active: None,
                settings: None,
            },
        )
        .await
        .expect("tenant should be updated");

    service
        .toggle_module(
            tenant.id,
            ToggleModuleInput {
                module_slug: "blog".to_string(),
                enabled: true,
            },
        )
        .await
        .expect("module should be toggled");

    let events = SysEvents::find()
        .order_by_asc(outbox_entity::Column::CreatedAt)
        .all(&db)
        .await
        .expect("outbox events should load");

    assert_eq!(events.len(), 3);
    assert!(events.iter().any(|event| event.event_type == "tenant.created"));
    assert!(events.iter().any(|event| event.event_type == "tenant.updated"));
    assert!(
        events
            .iter()
            .any(|event| event.event_type == "tenant.module.toggled")
    );

    let module_toggle_payload = events
        .iter()
        .find(|event| event.event_type == "tenant.module.toggled")
        .expect("tenant module toggle event must exist");
    assert_eq!(module_toggle_payload.payload["event"]["data"]["module_slug"], "blog");
    assert_eq!(
        module_toggle_payload.payload["event"]["data"]["enabled"],
        serde_json::json!(true)
    );
}
