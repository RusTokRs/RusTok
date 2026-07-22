use std::sync::Arc;

use rustok_outbox::entity as outbox_entity;
use rustok_outbox::{OutboxTransport, SysEvents, TransactionalEventBus};
use rustok_tenant::{
    CreateTenantInput, PortActor, PortContext, PortErrorKind, TenantError, TenantReadPort,
    TenantReadRequest, TenantReadSelector, TenantService, ToggleModuleInput, UpdateTenantInput,
};
use sea_orm::{
    ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, EntityTrait, QueryOrder,
    Statement,
};

async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("failed to connect in-memory sqlite");

    create_sqlite_test_tables(&db).await;

    db
}

async fn create_sqlite_test_tables(db: &DatabaseConnection) {
    for sql in [
        "CREATE TABLE IF NOT EXISTS tenants (
            id TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            slug TEXT NOT NULL UNIQUE,
            domain TEXT UNIQUE,
            settings TEXT NOT NULL DEFAULT '{}',
            default_locale TEXT NOT NULL,
            is_active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        "CREATE TABLE IF NOT EXISTS tenant_modules (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            module_slug TEXT NOT NULL,
            enabled INTEGER NOT NULL DEFAULT 1,
            settings TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        "CREATE TABLE IF NOT EXISTS sys_events (
            id TEXT PRIMARY KEY NOT NULL,
            event_type TEXT NOT NULL,
            schema_version INTEGER NOT NULL,
            payload TEXT NOT NULL,
            status TEXT NOT NULL,
            retry_count INTEGER NOT NULL DEFAULT 0,
            next_attempt_at TEXT NULL,
            last_error TEXT NULL,
            claimed_by TEXT NULL,
            claimed_at TEXT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            dispatched_at TEXT NULL
        )",
    ] {
        db.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            sql.to_string(),
        ))
        .await
        .expect("failed to create tenant test table");
    }
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
    assert_eq!(
        updated.settings["features"]["checkout"],
        serde_json::json!(true)
    );

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
async fn tenant_read_port_requires_deadline_and_valid_slug() {
    let db = setup_db().await;
    let service = TenantService::new(db);

    let missing_deadline = service
        .read_tenant(
            PortContext::new(
                "tenant-read-port".to_string(),
                PortActor::service("tenant-test"),
                "en",
                "corr-missing-deadline".to_string(),
            ),
            TenantReadRequest {
                selector: TenantReadSelector::Slug("read-port".to_string()),
                include_inactive: false,
            },
        )
        .await
        .expect_err("read port calls without a deadline must fail before storage access");

    assert_eq!(missing_deadline.kind, PortErrorKind::Timeout);
    assert_eq!(missing_deadline.code, "port.deadline_required");
    assert!(missing_deadline.retryable);

    let empty_slug = service
        .read_tenant(
            PortContext::new(
                "tenant-read-port".to_string(),
                PortActor::service("tenant-test"),
                "en",
                "corr-empty-slug".to_string(),
            )
            .with_deadline(std::time::Duration::from_millis(250)),
            TenantReadRequest {
                selector: TenantReadSelector::Slug("   ".to_string()),
                include_inactive: false,
            },
        )
        .await
        .expect_err("blank slug selectors must map to typed validation errors");

    assert_eq!(empty_slug.kind, PortErrorKind::Validation);
    assert_eq!(empty_slug.code, "tenant.slug_empty");
    assert!(!empty_slug.retryable);
}

#[tokio::test]
async fn tenant_read_port_preserves_projection_and_inactive_degraded_mode() {
    let db = setup_db().await;
    let service = TenantService::new(db);

    let tenant = service
        .create_tenant(CreateTenantInput {
            name: "Read Port Tenant".to_string(),
            slug: "read-port-tenant".to_string(),
            domain: Some("read-port.example".to_string()),
        })
        .await
        .expect("tenant should be created");

    let active_projection = service
        .read_tenant(
            PortContext::new(
                tenant.id.to_string(),
                PortActor::service("tenant-test"),
                "en",
                "corr-active-read".to_string(),
            )
            .with_deadline(std::time::Duration::from_millis(500)),
            TenantReadRequest {
                selector: TenantReadSelector::Id(tenant.id),
                include_inactive: false,
            },
        )
        .await
        .expect("active tenant should resolve through read port");

    assert_eq!(active_projection.id, tenant.id);
    assert_eq!(active_projection.slug, "read-port-tenant");
    assert_eq!(
        active_projection.domain.as_deref(),
        Some("read-port.example")
    );
    assert!(active_projection.is_active);

    service
        .update_tenant(
            tenant.id,
            UpdateTenantInput {
                name: None,
                domain: None,
                is_active: Some(false),
                settings: None,
            },
        )
        .await
        .expect("tenant should be deactivated");

    let hidden_inactive = service
        .read_tenant(
            PortContext::new(
                tenant.id.to_string(),
                PortActor::service("tenant-test"),
                "en",
                "corr-hidden-inactive".to_string(),
            )
            .with_deadline(std::time::Duration::from_millis(500)),
            TenantReadRequest {
                selector: TenantReadSelector::Slug("read-port-tenant".to_string()),
                include_inactive: false,
            },
        )
        .await
        .expect_err("inactive tenants must be hidden unless explicitly requested");

    assert_eq!(hidden_inactive.kind, PortErrorKind::NotFound);
    assert_eq!(hidden_inactive.code, "tenant.inactive");
    assert!(!hidden_inactive.retryable);

    let inactive_projection = service
        .read_tenant(
            PortContext::new(
                tenant.id.to_string(),
                PortActor::service("tenant-test"),
                "en",
                "corr-include-inactive".to_string(),
            )
            .with_deadline(std::time::Duration::from_millis(500)),
            TenantReadRequest {
                selector: TenantReadSelector::Slug("read-port-tenant".to_string()),
                include_inactive: true,
            },
        )
        .await
        .expect("include_inactive should expose inactive tenant projection");

    assert_eq!(inactive_projection.id, tenant.id);
    assert!(!inactive_projection.is_active);
}

#[tokio::test]
async fn tenant_read_port_resolves_domain_and_validates_blank_domain() {
    let db = setup_db().await;
    let service = TenantService::new(db);

    let tenant = service
        .create_tenant(CreateTenantInput {
            name: "Domain Read Tenant".to_string(),
            slug: "domain-read-tenant".to_string(),
            domain: Some("domain-read.example".to_string()),
        })
        .await
        .expect("tenant should be created");

    let projection = service
        .read_tenant(
            PortContext::new(
                tenant.id.to_string(),
                PortActor::service("tenant-domain-resolution-test"),
                "en",
                "corr-domain-read".to_string(),
            )
            .with_deadline(std::time::Duration::from_millis(500)),
            TenantReadRequest {
                selector: TenantReadSelector::Domain("domain-read.example".to_string()),
                include_inactive: false,
            },
        )
        .await
        .expect("domain selector should resolve the tenant projection");

    assert_eq!(projection.id, tenant.id);
    assert_eq!(projection.slug, "domain-read-tenant");
    assert_eq!(projection.domain.as_deref(), Some("domain-read.example"));

    let blank_domain = service
        .read_tenant(
            PortContext::new(
                tenant.id.to_string(),
                PortActor::service("tenant-domain-resolution-test"),
                "en",
                "corr-blank-domain".to_string(),
            )
            .with_deadline(std::time::Duration::from_millis(500)),
            TenantReadRequest {
                selector: TenantReadSelector::Domain("   ".to_string()),
                include_inactive: false,
            },
        )
        .await
        .expect_err("blank domain selectors must map to typed validation errors");

    assert_eq!(blank_domain.kind, PortErrorKind::Validation);
    assert_eq!(blank_domain.code, "tenant.domain_empty");
    assert!(!blank_domain.retryable);
}

#[tokio::test]
#[allow(deprecated)]
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
    assert!(
        events
            .iter()
            .any(|event| event.event_type == "tenant.created")
    );
    assert!(
        events
            .iter()
            .any(|event| event.event_type == "tenant.updated")
    );
    assert!(
        events
            .iter()
            .any(|event| event.event_type == "tenant.module.toggled")
    );

    let module_toggle_payload = events
        .iter()
        .find(|event| event.event_type == "tenant.module.toggled")
        .expect("tenant module toggle event must exist");
    assert_eq!(
        module_toggle_payload.payload["event"]["data"]["module_slug"],
        "blog"
    );
    assert_eq!(
        module_toggle_payload.payload["event"]["data"]["enabled"],
        serde_json::json!(true)
    );
}
