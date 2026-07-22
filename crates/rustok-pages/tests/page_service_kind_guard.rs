use rustok_content::entities::node::ContentStatus;
use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_pages::PagesModule;
use rustok_pages::dto::{CreatePageInput, PageBodyInput, PageTranslationInput, UpdatePageInput};
use rustok_pages::error::{
    FEATURE_BUILDER_ENABLED, FEATURE_BUILDER_PREVIEW_ENABLED, FEATURE_BUILDER_PROPERTIES_ENABLED,
    FEATURE_BUILDER_PUBLISH_ENABLED, PagesError,
};
use rustok_pages::services::PageService;
use rustok_tenant::entities::tenant_module;
use rustok_test_utils::{
    db::setup_test_db,
    helpers::{admin_context, customer_context},
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection,
    EntityTrait, QueryFilter, Statement,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use std::sync::Arc;
use uuid::Uuid;

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
    .expect("must create tenant_modules table");
}

async fn setup() -> (DatabaseConnection, PageService, Uuid, SecurityContext) {
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
    ensure_tenant_modules_table(&db).await;

    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    (
        db.clone(),
        PageService::new(db, event_bus),
        Uuid::new_v4(),
        admin_context(),
    )
}

fn translation(title: &str, slug: &str) -> PageTranslationInput {
    PageTranslationInput {
        locale: "en".to_string(),
        title: title.to_string(),
        slug: Some(slug.to_string()),
        meta_title: None,
        meta_description: None,
    }
}

fn current_project(title: &str) -> serde_json::Value {
    serde_json::json!({
        "assets": [],
        "styles": [],
        "pages": [{
            "id": "main",
            "name": title,
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": []
            }
        }]
    })
}

async fn create_page(
    page_service: &PageService,
    tenant_id: Uuid,
    security: SecurityContext,
) -> rustok_pages::dto::PageResponse {
    page_service
        .create(
            tenant_id,
            security,
            CreatePageInput {
                translations: vec![translation("Page", "page")],
                template: Some("default".to_string()),
                body: None,
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect("failed to create page")
}

async fn create_grapesjs_page(
    page_service: &PageService,
    tenant_id: Uuid,
    security: SecurityContext,
    title: &str,
    slug: &str,
) -> rustok_pages::dto::PageResponse {
    page_service
        .create(
            tenant_id,
            security,
            CreatePageInput {
                translations: vec![translation(title, slug)],
                template: Some("builder".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: String::new(),
                    format: Some("grapesjs".to_string()),
                    content_json: Some(current_project(title)),
                }),
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect("failed to create grapesjs page")
}

async fn create_markdown_page(
    page_service: &PageService,
    tenant_id: Uuid,
    security: SecurityContext,
    publish: bool,
) -> rustok_pages::dto::PageResponse {
    page_service
        .create(
            tenant_id,
            security,
            CreatePageInput {
                translations: vec![translation("Markdown page", "markdown-page")],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: "# Hello".to_string(),
                    format: Some("markdown".to_string()),
                    content_json: None,
                }),
                channel_slugs: None,
                publish,
            },
        )
        .await
        .expect("markdown path should remain available")
}

async fn seed_pages_module_settings(db: &DatabaseConnection, tenant_id: Uuid, settings: &str) {
    ensure_tenant_modules_table(db).await;
    tenant_module::Entity::delete_many()
        .filter(tenant_module::Column::TenantId.eq(tenant_id))
        .filter(tenant_module::Column::ModuleSlug.eq("pages"))
        .exec(db)
        .await
        .expect("must remove previous pages module settings");

    let settings_json: serde_json::Value =
        serde_json::from_str(settings).expect("settings must be valid JSON");
    let now = chrono::Utc::now().into();
    tenant_module::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        module_slug: Set("pages".to_string()),
        enabled: Set(true),
        settings: Set(settings_json),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(db)
    .await
    .expect("must seed pages module settings");
}

#[tokio::test]
async fn lifecycle_operations_reject_unknown_page_ids_without_mutating_current_page() {
    let (_db, page_service, tenant_id, security) = setup().await;
    let page = create_page(&page_service, tenant_id, security.clone()).await;
    let unknown = Uuid::new_v4();

    assert!(matches!(
        page_service.publish(tenant_id, security.clone(), unknown).await,
        Err(PagesError::PageNotFound(id)) if id == unknown
    ));
    assert!(matches!(
        page_service.unpublish(tenant_id, security.clone(), unknown).await,
        Err(PagesError::PageNotFound(id)) if id == unknown
    ));
    assert!(matches!(
        page_service.delete(tenant_id, security.clone(), unknown).await,
        Err(PagesError::PageNotFound(id)) if id == unknown
    ));

    let unchanged = page_service
        .get(tenant_id, security, page.id)
        .await
        .expect("current page should remain accessible");
    assert_eq!(unchanged.status, page.status);
}

#[tokio::test]
async fn publish_returns_feature_disabled_when_builder_publish_toggle_is_false() {
    let (db, page_service, tenant_id, security) = setup().await;
    seed_pages_module_settings(
        &db,
        tenant_id,
        r#"{"builder":{"publish":{"enabled":false}}}"#,
    )
    .await;
    let page = create_grapesjs_page(
        &page_service,
        tenant_id,
        security.clone(),
        "Builder publish-off page",
        "builder-publish-off-page",
    )
    .await;

    assert!(matches!(
        page_service.publish(tenant_id, security, page.id).await,
        Err(PagesError::FeatureDisabled { feature }) if feature == FEATURE_BUILDER_PUBLISH_ENABLED
    ));
}

#[tokio::test]
async fn create_and_update_grapesjs_body_require_builder_enabled() {
    let (db, page_service, tenant_id, security) = setup().await;
    seed_pages_module_settings(&db, tenant_id, r#"{"builder":{"enabled":false}}"#).await;

    let create_result = page_service
        .create(
            tenant_id,
            security.clone(),
            CreatePageInput {
                translations: vec![translation("Landing", "landing")],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: String::new(),
                    format: Some("grapesjs".to_string()),
                    content_json: Some(current_project("Landing")),
                }),
                channel_slugs: None,
                publish: false,
            },
        )
        .await;
    assert!(matches!(
        create_result,
        Err(PagesError::FeatureDisabled { feature }) if feature == FEATURE_BUILDER_ENABLED
    ));

    seed_pages_module_settings(&db, tenant_id, r#"{"builder":{"enabled":true}}"#).await;
    let page = create_page(&page_service, tenant_id, security.clone()).await;
    seed_pages_module_settings(&db, tenant_id, r#"{"builder":{"enabled":false}}"#).await;
    let update_result = page_service
        .update(
            tenant_id,
            security,
            page.id,
            UpdatePageInput {
                expected_version: None,
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: String::new(),
                    format: Some("grapesjs".to_string()),
                    content_json: Some(current_project("Updated")),
                }),
                ..Default::default()
            },
        )
        .await;
    assert!(matches!(
        update_result,
        Err(PagesError::FeatureDisabled { feature }) if feature == FEATURE_BUILDER_ENABLED
    ));
}

#[tokio::test]
async fn markdown_paths_are_independent_from_builder_toggles() {
    let (db, page_service, tenant_id, security) = setup().await;
    seed_pages_module_settings(
        &db,
        tenant_id,
        r#"{"builder":{"enabled":false,"publish":{"enabled":false}}}"#,
    )
    .await;

    let published = create_markdown_page(&page_service, tenant_id, security.clone(), true).await;
    assert_eq!(published.status, ContentStatus::Published);

    let draft = page_service
        .create(
            tenant_id,
            security.clone(),
            CreatePageInput {
                translations: vec![translation("Draft markdown", "draft-markdown")],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: "draft".to_string(),
                    format: Some("markdown".to_string()),
                    content_json: None,
                }),
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect("markdown draft should be created");
    let updated = page_service
        .update(
            tenant_id,
            security,
            draft.id,
            UpdatePageInput {
                expected_version: None,
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: "updated".to_string(),
                    format: Some("markdown".to_string()),
                    content_json: None,
                }),
                status: Some(ContentStatus::Published),
                ..Default::default()
            },
        )
        .await
        .expect("markdown update/publish should remain available");
    assert_eq!(updated.status, ContentStatus::Published);
    assert_eq!(updated.body.expect("body").format, "markdown");
}

#[tokio::test]
async fn foreign_tenant_page_is_not_found_before_builder_toggle_checks() {
    let (db, page_service, tenant_a, security) = setup().await;
    let page = create_grapesjs_page(
        &page_service,
        tenant_a,
        security.clone(),
        "Tenant A page",
        "tenant-a-page",
    )
    .await;
    let tenant_b = Uuid::new_v4();
    seed_pages_module_settings(
        &db,
        tenant_b,
        r#"{"builder":{"enabled":false,"publish":{"enabled":false}}}"#,
    )
    .await;

    assert!(matches!(
        page_service.publish(tenant_b, security, page.id).await,
        Err(PagesError::PageNotFound(id)) if id == page.id
    ));
}

#[tokio::test]
async fn forbidden_actor_is_rejected_before_builder_toggle_errors() {
    let (db, page_service, tenant_id, admin) = setup().await;
    let page = create_grapesjs_page(
        &page_service,
        tenant_id,
        admin,
        "Protected page",
        "protected-page",
    )
    .await;
    seed_pages_module_settings(
        &db,
        tenant_id,
        r#"{"builder":{"enabled":false,"publish":{"enabled":false}}}"#,
    )
    .await;

    assert!(matches!(
        page_service
            .publish(tenant_id, customer_context(), page.id)
            .await,
        Err(PagesError::Forbidden(_))
    ));
}

#[tokio::test]
async fn all_on_profile_allows_current_document_publish_and_reads() {
    let (db, page_service, tenant_id, security) = setup().await;
    seed_pages_module_settings(
        &db,
        tenant_id,
        r#"{"builder":{"enabled":true,"preview":{"enabled":true},"properties":{"enabled":true},"publish":{"enabled":true}}}"#,
    )
    .await;
    let page = create_grapesjs_page(
        &page_service,
        tenant_id,
        security.clone(),
        "All on",
        "all-on",
    )
    .await;

    page_service
        .ensure_builder_preview_enabled_for_tenant(tenant_id)
        .await
        .expect("preview enabled");
    page_service
        .ensure_builder_properties_enabled_for_tenant(tenant_id)
        .await
        .expect("properties enabled");
    let published = page_service
        .publish(tenant_id, security.clone(), page.id)
        .await
        .expect("publish enabled");
    assert_eq!(published.status, ContentStatus::Published);

    let loaded = page_service
        .get(tenant_id, security.clone(), page.id)
        .await
        .expect("read path stable");
    assert_eq!(loaded.body.expect("builder body").format, "grapesjs");
    let (items, total) = page_service
        .list(tenant_id, security, Default::default())
        .await
        .expect("list path stable");
    assert_eq!(total, 1);
    assert!(items.iter().any(|item| item.id == page.id));
}

#[tokio::test]
async fn disabled_capabilities_fail_independently_while_reads_remain_available() {
    let (db, page_service, tenant_id, security) = setup().await;
    seed_pages_module_settings(&db, tenant_id, r#"{"builder":{"enabled":true}}"#).await;
    let page = create_grapesjs_page(
        &page_service,
        tenant_id,
        security.clone(),
        "Degraded",
        "degraded",
    )
    .await;
    seed_pages_module_settings(
        &db,
        tenant_id,
        r#"{"builder":{"enabled":true,"preview":{"enabled":false},"properties":{"enabled":false},"publish":{"enabled":false}}}"#,
    )
    .await;

    assert!(matches!(
        page_service
            .ensure_builder_preview_enabled_for_tenant(tenant_id)
            .await,
        Err(PagesError::FeatureDisabled { feature }) if feature == FEATURE_BUILDER_PREVIEW_ENABLED
    ));
    assert!(matches!(
        page_service
            .ensure_builder_properties_enabled_for_tenant(tenant_id)
            .await,
        Err(PagesError::FeatureDisabled { feature }) if feature == FEATURE_BUILDER_PROPERTIES_ENABLED
    ));
    assert!(matches!(
        page_service
            .publish(tenant_id, security.clone(), page.id)
            .await,
        Err(PagesError::FeatureDisabled { feature }) if feature == FEATURE_BUILDER_PUBLISH_ENABLED
    ));

    let loaded = page_service
        .get(tenant_id, security.clone(), page.id)
        .await
        .expect("read remains available");
    assert_eq!(loaded.id, page.id);
    let (items, total) = page_service
        .list(tenant_id, security, Default::default())
        .await
        .expect("list remains available");
    assert_eq!(total, 1);
    assert!(items.iter().any(|item| item.id == page.id));
}

#[tokio::test]
async fn preview_and_properties_default_to_enabled() {
    let (_db, page_service, tenant_id, _security) = setup().await;
    page_service
        .ensure_builder_preview_enabled_for_tenant(tenant_id)
        .await
        .expect("preview defaults enabled");
    page_service
        .ensure_builder_properties_enabled_for_tenant(tenant_id)
        .await
        .expect("properties defaults enabled");
}
