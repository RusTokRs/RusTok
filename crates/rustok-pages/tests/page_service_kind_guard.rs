use rustok_content::entities::node::ContentStatus;
use rustok_core::{MigrationSource, SecurityContext};
use rustok_pages::dto::{
    BlockType, CreateBlockInput, CreatePageInput, PageBodyInput, PageTranslationInput,
    UpdatePageInput,
};
use rustok_pages::error::{
    PagesError, FEATURE_BUILDER_ENABLED, FEATURE_BUILDER_PREVIEW_ENABLED,
    FEATURE_BUILDER_PROPERTIES_ENABLED, FEATURE_BUILDER_PUBLISH_ENABLED,
};
use rustok_pages::services::{BlockService, PageService};
use rustok_pages::PagesModule;
use rustok_tenant::entities::tenant_module;
use rustok_test_utils::{
    db::setup_test_db,
    helpers::{admin_context, customer_context},
    mock_transactional_event_bus,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection,
    EntityTrait, QueryFilter, Statement,
};
use sea_orm_migration::SchemaManager;
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

async fn setup() -> (
    DatabaseConnection,
    PageService,
    BlockService,
    Uuid,
    SecurityContext,
) {
    let db = setup_test_db().await;
    let module = PagesModule;
    let schema = SchemaManager::new(&db);
    for migration in module.migrations() {
        migration
            .up(&schema)
            .await
            .expect("failed to apply pages migrations");
    }
    ensure_tenant_modules_table(&db).await;

    let event_bus = mock_transactional_event_bus();
    let page_service = PageService::new(db.clone(), event_bus.clone());
    let block_service = BlockService::new(db.clone(), event_bus);

    (
        db,
        page_service,
        block_service,
        Uuid::new_v4(),
        admin_context(),
    )
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
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Page".to_string(),
                    slug: Some("page".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: None,
                blocks: None,
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
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: title.to_string(),
                    slug: Some(slug.to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("builder".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: String::new(),
                    format: Some("grapesjs_v1".to_string()),
                    content_json: Some(serde_json::json!({
                        "pages": [{ "name": title, "frames": [] }],
                        "assets": [],
                        "styles": []
                    })),
                }),
                blocks: None,
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect("failed to create grapesjs page")
}

async fn create_block(
    block_service: &BlockService,
    tenant_id: Uuid,
    security: SecurityContext,
    page_id: Uuid,
) -> rustok_pages::dto::BlockResponse {
    block_service
        .create(
            tenant_id,
            security,
            page_id,
            CreateBlockInput {
                block_type: BlockType::Text,
                position: 0,
                data: serde_json::json!({ "text": "hello" }),
                translations: None,
            },
        )
        .await
        .expect("failed to create block")
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
async fn publish_returns_page_not_found_for_block_id_and_keeps_page_status() {
    let (_db, page_service, block_service, tenant_id, security) = setup().await;
    let page = create_page(&page_service, tenant_id, security.clone()).await;
    let block = create_block(&block_service, tenant_id, security.clone(), page.id).await;

    let result = page_service
        .publish(tenant_id, security.clone(), block.id)
        .await;

    assert!(matches!(result, Err(PagesError::PageNotFound(id)) if id == block.id));

    let unchanged = page_service
        .get(tenant_id, security, page.id)
        .await
        .expect("page should remain accessible");
    assert_eq!(unchanged.status, page.status);
}

#[tokio::test]
async fn unpublish_returns_page_not_found_for_block_id_and_keeps_page_status() {
    let (_db, page_service, block_service, tenant_id, security) = setup().await;
    let page = create_page(&page_service, tenant_id, security.clone()).await;
    let block = create_block(&block_service, tenant_id, security.clone(), page.id).await;

    let result = page_service
        .unpublish(tenant_id, security.clone(), block.id)
        .await;

    assert!(matches!(result, Err(PagesError::PageNotFound(id)) if id == block.id));

    let unchanged = page_service
        .get(tenant_id, security, page.id)
        .await
        .expect("page should remain accessible");
    assert_eq!(unchanged.status, page.status);
}

#[tokio::test]
async fn delete_returns_page_not_found_for_block_id_and_keeps_page_record() {
    let (_db, page_service, block_service, tenant_id, security) = setup().await;
    let page = create_page(&page_service, tenant_id, security.clone()).await;
    let block = create_block(&block_service, tenant_id, security.clone(), page.id).await;

    let result = page_service
        .delete(tenant_id, security.clone(), block.id)
        .await;

    assert!(matches!(result, Err(PagesError::PageNotFound(id)) if id == block.id));

    let unchanged = page_service
        .get(tenant_id, security, page.id)
        .await
        .expect("page should remain accessible");
    assert_eq!(unchanged.status, page.status);
}

#[tokio::test]
async fn publish_returns_feature_disabled_when_builder_publish_toggle_is_false() {
    let (db, page_service, _block_service, tenant_id, security) = setup().await;
    seed_pages_module_settings(
        &db,
        tenant_id,
        "{\"builder\":{\"publish\":{\"enabled\":false}}}",
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
    let result = page_service.publish(tenant_id, security, page.id).await;
    assert!(matches!(
        result,
        Err(PagesError::FeatureDisabled { feature }) if feature == FEATURE_BUILDER_PUBLISH_ENABLED
    ));
}

#[tokio::test]
async fn create_grapesjs_body_returns_feature_disabled_when_builder_toggle_is_false() {
    let (db, page_service, _block_service, tenant_id, security) = setup().await;
    seed_pages_module_settings(&db, tenant_id, "{\"builder\":{\"enabled\":false}}").await;

    let result = page_service
        .create(
            tenant_id,
            security,
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Landing".to_string(),
                    slug: Some("landing".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: "".to_string(),
                    format: Some("grapesjs_v1".to_string()),
                    content_json: Some(serde_json::json!({
                        "components": []
                    })),
                }),
                blocks: None,
                channel_slugs: None,
                publish: false,
            },
        )
        .await;

    assert!(matches!(
        result,
        Err(PagesError::FeatureDisabled { feature }) if feature == FEATURE_BUILDER_ENABLED
    ));
}

#[tokio::test]
async fn update_grapesjs_body_returns_feature_disabled_when_builder_toggle_is_false() {
    let (db, page_service, _block_service, tenant_id, security) = setup().await;
    let page = create_page(&page_service, tenant_id, security.clone()).await;
    seed_pages_module_settings(&db, tenant_id, "{\"builder\":{\"enabled\":false}}").await;

    let result = page_service
        .update(
            tenant_id,
            security,
            page.id,
            UpdatePageInput {
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: "".to_string(),
                    format: Some("grapesjs_v1".to_string()),
                    content_json: Some(serde_json::json!({
                        "components": []
                    })),
                }),
                ..Default::default()
            },
        )
        .await;

    assert!(matches!(
        result,
        Err(PagesError::FeatureDisabled { feature }) if feature == FEATURE_BUILDER_ENABLED
    ));
}

#[tokio::test]
async fn create_markdown_body_is_allowed_when_builder_toggle_is_false() {
    let (db, page_service, _block_service, tenant_id, security) = setup().await;
    seed_pages_module_settings(&db, tenant_id, "{\"builder\":{\"enabled\":false}}").await;

    let created = page_service
        .create(
            tenant_id,
            security,
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Markdown page".to_string(),
                    slug: Some("markdown-page".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: "# Hello".to_string(),
                    format: Some("markdown".to_string()),
                    content_json: None,
                }),
                blocks: None,
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect("markdown path should remain available when builder is disabled");

    let body = created.body.expect("body should be present");
    assert_eq!(body.format, "markdown");
}

#[tokio::test]
async fn update_markdown_body_is_allowed_when_builder_toggle_is_false() {
    let (db, page_service, _block_service, tenant_id, security) = setup().await;
    let page = create_page(&page_service, tenant_id, security.clone()).await;
    seed_pages_module_settings(&db, tenant_id, "{\"builder\":{\"enabled\":false}}").await;

    let updated = page_service
        .update(
            tenant_id,
            security,
            page.id,
            UpdatePageInput {
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: "Updated markdown body".to_string(),
                    format: Some("markdown".to_string()),
                    content_json: None,
                }),
                ..Default::default()
            },
        )
        .await
        .expect("markdown update path should remain available when builder is disabled");

    let body = updated.body.expect("body should be present");
    assert_eq!(body.format, "markdown");
}

#[tokio::test]
async fn create_and_publish_markdown_is_allowed_when_builder_disabled_but_publish_enabled() {
    let (db, page_service, _block_service, tenant_id, security) = setup().await;
    seed_pages_module_settings(
        &db,
        tenant_id,
        "{\"builder\":{\"enabled\":false,\"publish\":{\"enabled\":true}}}",
    )
    .await;

    let created = page_service
        .create(
            tenant_id,
            security,
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Published markdown page".to_string(),
                    slug: Some("published-markdown-page".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: "publish markdown path".to_string(),
                    format: Some("markdown".to_string()),
                    content_json: None,
                }),
                blocks: None,
                channel_slugs: None,
                publish: true,
            },
        )
        .await
        .expect("markdown publish should remain available when builder is disabled");

    assert_eq!(
        created.status,
        rustok_content::entities::node::ContentStatus::Published
    );
}

#[tokio::test]
async fn create_and_publish_markdown_is_allowed_when_builder_publish_toggle_is_false() {
    let (db, page_service, _block_service, tenant_id, security) = setup().await;
    seed_pages_module_settings(
        &db,
        tenant_id,
        "{\"builder\":{\"enabled\":true,\"publish\":{\"enabled\":false}}}",
    )
    .await;

    let created = page_service
        .create(
            tenant_id,
            security,
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Published markdown with publish-off".to_string(),
                    slug: Some("published-markdown-publish-off".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: "markdown publish path when builder publish off".to_string(),
                    format: Some("markdown".to_string()),
                    content_json: None,
                }),
                blocks: None,
                channel_slugs: None,
                publish: true,
            },
        )
        .await
        .expect("markdown publish should remain available when builder.publish is disabled");

    assert_eq!(
        created.status,
        rustok_content::entities::node::ContentStatus::Published
    );
}

#[tokio::test]
async fn publish_grapesjs_page_is_blocked_when_builder_disabled_even_if_publish_enabled() {
    let (db, page_service, _block_service, tenant_id, security) = setup().await;
    let draft = page_service
        .create(
            tenant_id,
            security.clone(),
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Grapes page".to_string(),
                    slug: Some("grapes-page".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: "".to_string(),
                    format: Some("grapesjs_v1".to_string()),
                    content_json: Some(serde_json::json!({
                        "components": []
                    })),
                }),
                blocks: None,
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect("draft grapesjs page should be created while builder is enabled");

    seed_pages_module_settings(
        &db,
        tenant_id,
        "{\"builder\":{\"enabled\":false,\"publish\":{\"enabled\":true}}}",
    )
    .await;

    let result = page_service.publish(tenant_id, security, draft.id).await;
    assert!(matches!(
        result,
        Err(PagesError::FeatureDisabled { feature }) if feature == FEATURE_BUILDER_ENABLED
    ));
}

#[tokio::test]
async fn publish_with_foreign_page_id_returns_page_not_found_before_builder_toggle_checks() {
    let (db, page_service, _block_service, tenant_a, security) = setup().await;
    let page = page_service
        .create(
            tenant_a,
            security.clone(),
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Tenant A grapes page".to_string(),
                    slug: Some("tenant-a-grapes".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: "".to_string(),
                    format: Some("grapesjs_v1".to_string()),
                    content_json: Some(serde_json::json!({
                        "components": []
                    })),
                }),
                blocks: None,
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect("must create page in tenant A");

    let tenant_b = Uuid::new_v4();
    seed_pages_module_settings(
        &db,
        tenant_b,
        "{\"builder\":{\"enabled\":false,\"publish\":{\"enabled\":true}}}",
    )
    .await;

    let result = page_service.publish(tenant_b, security, page.id).await;
    assert!(matches!(result, Err(PagesError::PageNotFound(id)) if id == page.id));
}

#[tokio::test]
async fn update_to_published_is_blocked_for_existing_grapesjs_page_when_builder_is_disabled() {
    let (db, page_service, _block_service, tenant_id, security) = setup().await;
    let draft = page_service
        .create(
            tenant_id,
            security.clone(),
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Draft grapes page".to_string(),
                    slug: Some("draft-grapes-page".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: "".to_string(),
                    format: Some("grapesjs_v1".to_string()),
                    content_json: Some(serde_json::json!({
                        "components": []
                    })),
                }),
                blocks: None,
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect("must create draft grapesjs page");

    seed_pages_module_settings(
        &db,
        tenant_id,
        "{\"builder\":{\"enabled\":false,\"publish\":{\"enabled\":true}}}",
    )
    .await;

    let result = page_service
        .update(
            tenant_id,
            security,
            draft.id,
            UpdatePageInput {
                status: Some(rustok_content::entities::node::ContentStatus::Published),
                ..Default::default()
            },
        )
        .await;

    assert!(matches!(
        result,
        Err(PagesError::FeatureDisabled { feature }) if feature == FEATURE_BUILDER_ENABLED
    ));
}

#[tokio::test]
async fn update_to_published_markdown_is_allowed_when_builder_disabled_but_publish_enabled() {
    let (db, page_service, _block_service, tenant_id, security) = setup().await;
    let draft = page_service
        .create(
            tenant_id,
            security.clone(),
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Draft markdown page".to_string(),
                    slug: Some("draft-markdown-page".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: "markdown draft".to_string(),
                    format: Some("markdown".to_string()),
                    content_json: None,
                }),
                blocks: None,
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect("must create draft markdown page");

    seed_pages_module_settings(
        &db,
        tenant_id,
        "{\"builder\":{\"enabled\":false,\"publish\":{\"enabled\":true}}}",
    )
    .await;

    let updated = page_service
        .update(
            tenant_id,
            security,
            draft.id,
            UpdatePageInput {
                status: Some(rustok_content::entities::node::ContentStatus::Published),
                ..Default::default()
            },
        )
        .await
        .expect("markdown publish transition should remain available");

    assert_eq!(
        updated.status,
        rustok_content::entities::node::ContentStatus::Published
    );
}

#[tokio::test]
async fn update_to_published_markdown_is_allowed_when_builder_publish_toggle_is_false() {
    let (db, page_service, _block_service, tenant_id, security) = setup().await;
    let draft = page_service
        .create(
            tenant_id,
            security.clone(),
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Draft markdown publish-off page".to_string(),
                    slug: Some("draft-markdown-publish-off".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: "markdown draft publish-off".to_string(),
                    format: Some("markdown".to_string()),
                    content_json: None,
                }),
                blocks: None,
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect("must create draft markdown page");

    seed_pages_module_settings(
        &db,
        tenant_id,
        "{\"builder\":{\"enabled\":true,\"publish\":{\"enabled\":false}}}",
    )
    .await;

    let updated = page_service
        .update(
            tenant_id,
            security,
            draft.id,
            UpdatePageInput {
                status: Some(rustok_content::entities::node::ContentStatus::Published),
                ..Default::default()
            },
        )
        .await
        .expect(
            "markdown publish transition should remain available when builder.publish is disabled",
        );

    assert_eq!(
        updated.status,
        rustok_content::entities::node::ContentStatus::Published
    );
}

#[tokio::test]
async fn publish_forbidden_user_gets_forbidden_before_builder_toggle_errors() {
    let (db, page_service, _block_service, tenant_id, admin) = setup().await;
    let draft = page_service
        .create(
            tenant_id,
            admin,
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Protected grapes page".to_string(),
                    slug: Some("protected-grapes-page".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: "".to_string(),
                    format: Some("grapesjs_v1".to_string()),
                    content_json: Some(serde_json::json!({
                        "components": []
                    })),
                }),
                blocks: None,
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect("must create protected draft page");

    seed_pages_module_settings(
        &db,
        tenant_id,
        "{\"builder\":{\"enabled\":false,\"publish\":{\"enabled\":false}}}",
    )
    .await;

    let result = page_service
        .publish(tenant_id, customer_context(), draft.id)
        .await;

    assert!(matches!(result, Err(PagesError::Forbidden(_))));
}

#[tokio::test]
async fn pages_builder_fallback_all_on_allows_publish_and_keeps_read_list_paths() {
    let (db, page_service, _block_service, tenant_id, security) = setup().await;
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
        "Fallback all-on",
        "fallback-all-on",
    )
    .await;

    page_service
        .ensure_builder_preview_enabled_for_tenant(tenant_id)
        .await
        .expect("preview capability must be available in all_on profile");
    page_service
        .ensure_builder_properties_enabled_for_tenant(tenant_id)
        .await
        .expect("properties capability must be available in all_on profile");

    let published = page_service
        .publish(tenant_id, security.clone(), page.id)
        .await
        .expect("publish must be available in all_on profile");
    assert_eq!(published.status, ContentStatus::Published);

    let loaded = page_service
        .get(tenant_id, security.clone(), page.id)
        .await
        .expect("read path must remain stable in all_on profile");
    assert_eq!(loaded.id, page.id);
    assert_eq!(
        loaded.body.expect("builder body must stay readable").format,
        "grapesjs_v1"
    );

    let (items, total) = page_service
        .list(tenant_id, security, Default::default())
        .await
        .expect("list path must remain stable in all_on profile");
    assert_eq!(total, 1);
    assert!(items.iter().any(|item| item.id == page.id));
}

#[tokio::test]
async fn pages_builder_fallback_publish_off_blocks_grapesjs_publish_but_keeps_read_list_paths() {
    let (db, page_service, _block_service, tenant_id, security) = setup().await;
    seed_pages_module_settings(&db, tenant_id, r#"{"builder":{"enabled":true}}"#).await;
    let page = create_grapesjs_page(
        &page_service,
        tenant_id,
        security.clone(),
        "Fallback publish-off",
        "fallback-publish-off",
    )
    .await;

    seed_pages_module_settings(
        &db,
        tenant_id,
        r#"{"builder":{"enabled":true,"publish":{"enabled":false}}}"#,
    )
    .await;

    let publish_result = page_service
        .publish(tenant_id, security.clone(), page.id)
        .await;
    assert!(matches!(
        publish_result,
        Err(PagesError::FeatureDisabled { feature }) if feature == FEATURE_BUILDER_PUBLISH_ENABLED
    ));

    let loaded = page_service
        .get(tenant_id, security.clone(), page.id)
        .await
        .expect("read path must remain stable when builder publish is disabled");
    assert_eq!(loaded.id, page.id);
    assert_eq!(
        loaded.body.expect("builder body must stay readable").format,
        "grapesjs_v1"
    );

    let (items, total) = page_service
        .list(tenant_id, security, Default::default())
        .await
        .expect("list path must remain stable when builder publish is disabled");
    assert_eq!(total, 1);
    assert!(items.iter().any(|item| item.id == page.id));
}

#[tokio::test]
async fn pages_builder_fallback_preview_off_blocks_preview_publish_but_keeps_read_list_paths() {
    let (db, page_service, _block_service, tenant_id, security) = setup().await;
    seed_pages_module_settings(&db, tenant_id, r#"{"builder":{"enabled":true}}"#).await;
    let page = create_grapesjs_page(
        &page_service,
        tenant_id,
        security.clone(),
        "Fallback preview-off",
        "fallback-preview-off",
    )
    .await;

    seed_pages_module_settings(
        &db,
        tenant_id,
        r#"{"builder":{"enabled":true,"preview":{"enabled":false},"properties":{"enabled":true},"publish":{"enabled":false}}}"#,
    )
    .await;

    let preview_result = page_service
        .ensure_builder_preview_enabled_for_tenant(tenant_id)
        .await;
    assert!(matches!(
        preview_result,
        Err(PagesError::FeatureDisabled { feature }) if feature == FEATURE_BUILDER_PREVIEW_ENABLED
    ));

    page_service
        .ensure_builder_properties_enabled_for_tenant(tenant_id)
        .await
        .expect("properties capability must remain available in preview_off profile");

    let publish_result = page_service
        .publish(tenant_id, security.clone(), page.id)
        .await;
    assert!(matches!(
        publish_result,
        Err(PagesError::FeatureDisabled { feature }) if feature == FEATURE_BUILDER_PUBLISH_ENABLED
    ));

    let loaded = page_service
        .get(tenant_id, security.clone(), page.id)
        .await
        .expect("read path must remain stable when preview is disabled");
    assert_eq!(loaded.id, page.id);
    assert_eq!(
        loaded.body.expect("builder body must stay readable").format,
        "grapesjs_v1"
    );

    let (items, total) = page_service
        .list(tenant_id, security, Default::default())
        .await
        .expect("list path must remain stable when preview is disabled");
    assert_eq!(total, 1);
    assert!(items.iter().any(|item| item.id == page.id));
}

#[tokio::test]
async fn pages_builder_fallback_builder_off_keeps_read_and_list_paths() {
    let (db, page_service, _block_service, tenant_id, security) = setup().await;
    seed_pages_module_settings(&db, tenant_id, r#"{"builder":{"enabled":true}}"#).await;
    let page = create_grapesjs_page(
        &page_service,
        tenant_id,
        security.clone(),
        "Fallback builder-off",
        "fallback-builder-off",
    )
    .await;

    seed_pages_module_settings(
        &db,
        tenant_id,
        r#"{"builder":{"enabled":false,"preview":{"enabled":false},"properties":{"enabled":false},"publish":{"enabled":false}}}"#,
    )
    .await;

    let loaded = page_service
        .get(tenant_id, security.clone(), page.id)
        .await
        .expect("read path must remain stable when builder is disabled");
    assert_eq!(loaded.id, page.id);
    assert_eq!(
        loaded.body.expect("builder body must stay readable").format,
        "grapesjs_v1"
    );

    let (items, total) = page_service
        .list(tenant_id, security.clone(), Default::default())
        .await
        .expect("list path must remain stable when builder is disabled");
    assert_eq!(total, 1);
    assert!(items.iter().any(|item| item.id == page.id));

    let publish_result = page_service.publish(tenant_id, security, page.id).await;
    assert!(matches!(
        publish_result,
        Err(PagesError::FeatureDisabled { feature }) if feature == FEATURE_BUILDER_ENABLED
    ));
}

#[tokio::test]
async fn preview_capability_returns_feature_disabled_when_preview_toggle_is_false() {
    let (db, page_service, _block_service, tenant_id, _security) = setup().await;
    seed_pages_module_settings(
        &db,
        tenant_id,
        "{\"builder\":{\"enabled\":true,\"preview\":{\"enabled\":false}}}",
    )
    .await;

    let result = page_service
        .ensure_builder_preview_enabled_for_tenant(tenant_id)
        .await;
    assert!(matches!(
        result,
        Err(PagesError::FeatureDisabled { feature }) if feature == FEATURE_BUILDER_PREVIEW_ENABLED
    ));
}

#[tokio::test]
async fn properties_capability_returns_feature_disabled_when_properties_toggle_is_false() {
    let (db, page_service, _block_service, tenant_id, _security) = setup().await;
    seed_pages_module_settings(
        &db,
        tenant_id,
        "{\"builder\":{\"enabled\":true,\"properties\":{\"enabled\":false}}}",
    )
    .await;

    let result = page_service
        .ensure_builder_properties_enabled_for_tenant(tenant_id)
        .await;
    assert!(matches!(
        result,
        Err(PagesError::FeatureDisabled { feature }) if feature == FEATURE_BUILDER_PROPERTIES_ENABLED
    ));
}

#[tokio::test]
async fn preview_capability_is_enabled_by_default_when_settings_absent() {
    let (_db, page_service, _block_service, tenant_id, _security) = setup().await;

    let result = page_service
        .ensure_builder_preview_enabled_for_tenant(tenant_id)
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn properties_capability_is_enabled_by_default_when_settings_absent() {
    let (_db, page_service, _block_service, tenant_id, _security) = setup().await;

    let result = page_service
        .ensure_builder_properties_enabled_for_tenant(tenant_id)
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn preview_capability_is_enabled_when_preview_toggle_is_true() {
    let (db, page_service, _block_service, tenant_id, _security) = setup().await;
    seed_pages_module_settings(
        &db,
        tenant_id,
        "{\"builder\":{\"enabled\":true,\"preview\":{\"enabled\":true}}}",
    )
    .await;

    let result = page_service
        .ensure_builder_preview_enabled_for_tenant(tenant_id)
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn properties_capability_is_enabled_when_properties_toggle_is_true() {
    let (db, page_service, _block_service, tenant_id, _security) = setup().await;
    seed_pages_module_settings(
        &db,
        tenant_id,
        "{\"builder\":{\"enabled\":true,\"properties\":{\"enabled\":true}}}",
    )
    .await;

    let result = page_service
        .ensure_builder_properties_enabled_for_tenant(tenant_id)
        .await;
    assert!(result.is_ok());
}
