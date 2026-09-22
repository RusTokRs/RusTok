use std::time::Duration;

use rustok_outbox::SysEvents;
use rustok_outbox::entity as outbox_entity;
use rustok_tenant::{
    CreateTenantInput, PortActor, PortContext, PortErrorKind, ReplaceTenantLocalePolicyRequest,
    TenantError, TenantLocale, TenantLocalePolicyEntry, TenantLocalePolicyPort, TenantReadPort,
    TenantReadRequest, TenantReadSelector, TenantService, UpdateTenantInput,
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
        "CREATE TABLE IF NOT EXISTS tenant_locales (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            name TEXT NOT NULL,
            native_name TEXT NOT NULL,
            is_default INTEGER NOT NULL DEFAULT 0,
            is_enabled INTEGER NOT NULL DEFAULT 1,
            fallback_locale TEXT,
            policy_revision INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE (tenant_id, locale)
        )",
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_tenant_locales_one_default
            ON tenant_locales (tenant_id) WHERE is_default = 1",
        "CREATE TABLE IF NOT EXISTS tenant_locale_policy_receipts (
            tenant_id TEXT NOT NULL,
            idempotency_key TEXT NOT NULL,
            request_hash TEXT NOT NULL,
            response TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (tenant_id, idempotency_key)
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
        db.execute_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            sql.to_string(),
        ))
        .await
        .expect("failed to create tenant test table");
    }
}

fn locale_port_context(tenant_id: uuid::Uuid, idempotency_key: Option<&str>) -> PortContext {
    let mut context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user("locale-admin"),
        "en",
        format!("tenant-locale-policy-{tenant_id}"),
    )
    .with_deadline(Duration::from_secs(5));
    if let Some(idempotency_key) = idempotency_key {
        context = context.with_idempotency_key(idempotency_key);
    }
    context
}

fn locale_entry(
    locale: &str,
    is_default: bool,
    is_enabled: bool,
    fallback_locale: Option<&str>,
) -> TenantLocalePolicyEntry {
    TenantLocalePolicyEntry {
        locale: TenantLocale::new(locale).expect("test locale must be valid"),
        name: locale.to_string(),
        native_name: locale.to_string(),
        is_default,
        is_enabled,
        fallback_locale: fallback_locale
            .map(TenantLocale::new)
            .transpose()
            .expect("test fallback locale must be valid"),
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
async fn tenant_locale_policy_port_replaces_with_cas_and_replays_idempotently() {
    let db = setup_db().await;
    let service = TenantService::new(db);
    let tenant = service
        .create_tenant(CreateTenantInput {
            name: "Locale Policy".to_string(),
            slug: "locale-policy".to_string(),
            domain: None,
        })
        .await
        .expect("tenant should be created");

    let initial = service
        .read_locale_policy(locale_port_context(tenant.id, None))
        .await
        .expect("initial locale policy should load");
    assert_eq!(initial.revision, 1);
    assert_eq!(initial.default_locale.as_str(), "en");
    assert_eq!(initial.locales.len(), 1);

    let request = ReplaceTenantLocalePolicyRequest {
        expected_revision: initial.revision,
        locales: vec![
            locale_entry("en", true, true, None),
            locale_entry("pt_br", false, true, Some("en")),
        ],
    };
    let applied = service
        .replace_locale_policy(
            locale_port_context(tenant.id, Some("locale-policy-1")),
            request.clone(),
        )
        .await
        .expect("locale policy should apply");
    assert_eq!(applied.revision, 2);
    assert_eq!(
        applied
            .locales
            .iter()
            .map(|entry| entry.locale.as_str())
            .collect::<Vec<_>>(),
        vec!["en", "pt-BR"]
    );

    let replay = service
        .replace_locale_policy(
            locale_port_context(tenant.id, Some("locale-policy-1")),
            request,
        )
        .await
        .expect("same idempotency request should replay");
    assert_eq!(replay, applied);

    let stale = service
        .replace_locale_policy(
            locale_port_context(tenant.id, Some("locale-policy-stale")),
            ReplaceTenantLocalePolicyRequest {
                expected_revision: 1,
                locales: applied.locales.clone(),
            },
        )
        .await
        .expect_err("stale revision must conflict");
    assert_eq!(stale.kind, PortErrorKind::Conflict);
}

#[tokio::test]
async fn tenant_locale_policy_rejects_und_invalid_fallback_and_key_reuse() {
    assert!(TenantLocale::new("und").is_err());

    let db = setup_db().await;
    let service = TenantService::new(db);
    let tenant = service
        .create_tenant(CreateTenantInput {
            name: "Locale Validation".to_string(),
            slug: "locale-validation".to_string(),
            domain: None,
        })
        .await
        .expect("tenant should be created");

    let invalid_fallback = service
        .replace_locale_policy(
            locale_port_context(tenant.id, Some("invalid-fallback")),
            ReplaceTenantLocalePolicyRequest {
                expected_revision: 1,
                locales: vec![
                    locale_entry("en", true, true, None),
                    locale_entry("ru", false, false, Some("en")),
                    locale_entry("de", false, true, Some("ru")),
                ],
            },
        )
        .await
        .expect_err("fallback target must be enabled");
    assert_eq!(invalid_fallback.kind, PortErrorKind::Validation);

    let first = ReplaceTenantLocalePolicyRequest {
        expected_revision: 1,
        locales: vec![
            locale_entry("en", true, true, None),
            locale_entry("ru", false, true, Some("en")),
        ],
    };
    service
        .replace_locale_policy(locale_port_context(tenant.id, Some("key-reuse")), first)
        .await
        .expect("first request should apply");

    let conflict = service
        .replace_locale_policy(
            locale_port_context(tenant.id, Some("key-reuse")),
            ReplaceTenantLocalePolicyRequest {
                expected_revision: 2,
                locales: vec![
                    locale_entry("en", true, true, None),
                    locale_entry("de", false, true, Some("en")),
                ],
            },
        )
        .await
        .expect_err("same key with a different request must conflict");
    assert_eq!(conflict.kind, PortErrorKind::Conflict);
}

#[tokio::test]
async fn tenant_mutations_always_publish_outbox_events() {
    let db = setup_db().await;
    let service = TenantService::new(db.clone());

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

    let events = SysEvents::find()
        .order_by_asc(outbox_entity::Column::CreatedAt)
        .all(&db)
        .await
        .expect("outbox events should load");

    assert_eq!(events.len(), 2);
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
}
