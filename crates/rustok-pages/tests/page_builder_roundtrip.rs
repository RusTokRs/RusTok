use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_pages::dto::{
    BlockType, CreateBlockInput, CreatePageInput, PageBodyInput, PageTranslationInput,
    UpdatePageInput,
};
use rustok_pages::services::PageService;
use rustok_pages::PagesModule;
use rustok_test_utils::{db::setup_test_db, helpers::admin_context};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use std::sync::Arc;
use uuid::Uuid;

async fn setup() -> (PageService, Uuid, SecurityContext) {
    let db = setup_test_db().await;
    let module = PagesModule;
    let schema = SchemaManager::new(&db);
    SysEventsMigration
        .up(&schema)
        .await
        .expect("failed to apply outbox migrations");
    for migration in module.migrations() {
        migration
            .up(&schema)
            .await
            .expect("failed to apply pages migrations");
    }

    let tenant_id = Uuid::new_v4();
    ensure_tenant_modules_table(&db).await;
    enable_pages_builder_module(&db, tenant_id).await;

    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    (PageService::new(db, event_bus), tenant_id, admin_context())
}

async fn ensure_tenant_modules_table(db: &DatabaseConnection) {
    db.execute(Statement::from_string(
        db.get_database_backend(),
        "CREATE TABLE IF NOT EXISTS tenant_modules (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            module_slug TEXT NOT NULL,
            enabled BOOLEAN NOT NULL DEFAULT TRUE,
            settings TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );"
        .to_string(),
    ))
    .await
    .expect("failed to create tenant_modules table");
}

async fn enable_pages_builder_module(db: &DatabaseConnection, tenant_id: Uuid) {
    db.execute(Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
        INSERT INTO tenant_modules (id, tenant_id, module_slug, enabled, settings, created_at, updated_at)
        VALUES (?, ?, 'pages', TRUE, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#,
        [
            Uuid::new_v4().into(),
            tenant_id.into(),
            serde_json::json!({
                "builder_enabled": true,
                "features": {
                    "builder": {
                        "enabled": true,
                        "preview": true,
                        "properties": true,
                        "publish": true
                    }
                }
            })
            .into(),
        ],
    ))
    .await
    .expect("failed to seed pages builder module settings");
}

fn grapes_project(locale: &str, label: &str) -> serde_json::Value {
    serde_json::json!({
        "pages": [
            {
                "name": label,
                "frames": [
                    {
                        "component": {
                            "type": "wrapper",
                            "components": [
                                {
                                    "type": "text",
                                    "content": format!("Hello from {label}")
                                }
                            ]
                        }
                    }
                ]
            }
        ],
        "assets": [],
        "styles": [],
        "locale": locale
    })
}

fn legacy_text_block(text: &str, position: i32) -> CreateBlockInput {
    CreateBlockInput {
        block_type: BlockType::Text,
        position,
        data: serde_json::json!({
            "text": text,
        }),
        translations: None,
    }
}

#[tokio::test]
async fn grapesjs_body_round_trips_on_create_and_get() {
    let (service, tenant_id, security) = setup().await;
    let project = grapes_project("en", "landing");

    let created = service
        .create(
            tenant_id,
            security.clone(),
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Landing".to_string(),
                    slug: Some("landing".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("landing".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: String::new(),
                    format: Some("grapesjs".to_string()),
                    content_json: Some(project.clone()),
                }),
                blocks: None,
                channel_slugs: Some(vec!["web".to_string(), "mobile".to_string()]),
                publish: false,
            },
        )
        .await
        .expect("page with grapesjs body should be created");

    let body = created.body.expect("body should be present after create");
    assert_eq!(body.format, "grapesjs");
    assert_eq!(body.content_json, Some(project.clone()));
    assert_eq!(
        created.channel_slugs,
        vec!["mobile".to_string(), "web".to_string()]
    );

    let loaded = service
        .get(tenant_id, security, created.id)
        .await
        .expect("page should be readable after create");
    let loaded_body = loaded.body.expect("body should be present after get");
    assert_eq!(loaded_body.format, "grapesjs");
    assert_eq!(loaded_body.content_json, Some(project));
    assert_eq!(
        loaded.channel_slugs,
        vec!["mobile".to_string(), "web".to_string()]
    );
}

#[tokio::test]
async fn grapesjs_body_round_trips_on_update() {
    let (service, tenant_id, security) = setup().await;
    let created = service
        .create(
            tenant_id,
            security.clone(),
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Builder page".to_string(),
                    slug: Some("builder-page".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: "legacy".to_string(),
                    format: Some("markdown".to_string()),
                    content_json: None,
                }),
                blocks: None,
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect("seed page should be created");

    let updated_project = grapes_project("en", "builder-v2");
    let updated = service
        .update(
            tenant_id,
            security,
            created.id,
            UpdatePageInput {
                expected_version: None,
                translations: None,
                template: Some("builder".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: String::new(),
                    format: Some("grapesjs".to_string()),
                    content_json: Some(updated_project.clone()),
                }),
                channel_slugs: Some(vec!["app".to_string(), "app".to_string()]),
                status: None,
            },
        )
        .await
        .expect("page should accept grapesjs update");

    let body = updated.body.expect("body should be present after update");
    assert_eq!(body.format, "grapesjs");
    assert_eq!(body.content_json, Some(updated_project));
    assert_eq!(updated.template, "builder");
    assert_eq!(updated.channel_slugs, vec!["app".to_string()]);
}

#[tokio::test]
async fn legacy_block_driven_page_round_trips_without_body() {
    let (service, tenant_id, security) = setup().await;
    let created = service
        .create(
            tenant_id,
            security.clone(),
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Legacy blocks".to_string(),
                    slug: Some("legacy-blocks".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("legacy".to_string()),
                body: None,
                blocks: Some(vec![legacy_text_block("  Legacy body via blocks  ", 0)]),
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect("legacy block-driven page should be created");

    assert!(
        created.body.is_none(),
        "legacy block-driven pages must not synthesize a body"
    );
    assert_eq!(created.blocks.len(), 1);
    assert_eq!(created.blocks[0].data["text"], "Legacy body via blocks");

    let updated = service
        .update(
            tenant_id,
            security.clone(),
            created.id,
            UpdatePageInput {
                expected_version: None,
                translations: None,
                template: Some("legacy-updated".to_string()),
                body: None,
                channel_slugs: None,
                status: None,
            },
        )
        .await
        .expect("metadata-only update should keep legacy block payloads intact");

    assert!(
        updated.body.is_none(),
        "update without body must keep block-driven pages body-less"
    );
    assert_eq!(updated.template, "legacy-updated");
    assert_eq!(updated.blocks.len(), 1);
    assert_eq!(updated.blocks[0].data["text"], "Legacy body via blocks");

    let loaded = service
        .get(tenant_id, security, created.id)
        .await
        .expect("legacy block-driven page should stay readable");
    assert!(loaded.body.is_none());
    assert_eq!(loaded.blocks.len(), 1);
    assert_eq!(loaded.blocks[0].data["text"], "Legacy body via blocks");
}

#[tokio::test]
async fn grapesjs_body_update_preserves_legacy_blocks() {
    let (service, tenant_id, security) = setup().await;
    let created = service
        .create(
            tenant_id,
            security.clone(),
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Hybrid page".to_string(),
                    slug: Some("hybrid-page".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("legacy".to_string()),
                body: None,
                blocks: Some(vec![legacy_text_block("Legacy block payload", 0)]),
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect("legacy page should be created");

    let updated_project = grapes_project("en", "hybrid-builder");
    let updated = service
        .update(
            tenant_id,
            security.clone(),
            created.id,
            UpdatePageInput {
                expected_version: None,
                translations: None,
                template: Some("builder".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: String::new(),
                    format: Some("grapesjs".to_string()),
                    content_json: Some(updated_project.clone()),
                }),
                channel_slugs: None,
                status: None,
            },
        )
        .await
        .expect("page should accept grapesjs body without deleting legacy blocks");

    let body = updated.body.expect("grapesjs body should be present");
    assert_eq!(body.format, "grapesjs");
    assert_eq!(body.content_json, Some(updated_project.clone()));
    assert_eq!(updated.blocks.len(), 1);
    assert_eq!(updated.blocks[0].data["text"], "Legacy block payload");

    let loaded = service
        .get(tenant_id, security, created.id)
        .await
        .expect("hybrid page should stay readable");
    let loaded_body = loaded.body.expect("body should survive reload");
    assert_eq!(loaded_body.format, "grapesjs");
    assert_eq!(loaded_body.content_json, Some(updated_project));
    assert_eq!(loaded.blocks.len(), 1);
    assert_eq!(loaded.blocks[0].data["text"], "Legacy block payload");
}
