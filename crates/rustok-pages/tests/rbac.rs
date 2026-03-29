use rustok_core::{MigrationSource, SecurityContext};
use rustok_pages::dto::{
    BlockType, CreateBlockInput, CreateMenuInput, CreatePageInput, ListPagesFilter, MenuLocation,
    PageTranslationInput, UpdatePageInput,
};
use rustok_pages::error::PagesError;
use rustok_pages::services::{BlockService, MenuService, PageService};
use rustok_pages::PagesModule;
use rustok_test_utils::{
    db::setup_test_db,
    helpers::{admin_context, customer_context, manager_context},
    mock_transactional_event_bus,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> (PageService, BlockService, MenuService, Uuid) {
    let db = setup_test_db().await;
    let module = PagesModule;
    let schema = SchemaManager::new(&db);
    for migration in module.migrations() {
        migration
            .up(&schema)
            .await
            .expect("failed to apply pages migrations");
    }

    let event_bus = mock_transactional_event_bus();
    (
        PageService::new(db.clone(), event_bus.clone()),
        BlockService::new(db.clone(), event_bus.clone()),
        MenuService::new(db, event_bus),
        Uuid::new_v4(),
    )
}

async fn create_page(
    service: &PageService,
    tenant_id: Uuid,
    security: SecurityContext,
    slug: &str,
    publish: bool,
) -> rustok_pages::dto::PageResponse {
    service
        .create(
            tenant_id,
            security,
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: slug.to_string(),
                    slug: Some(slug.to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: None,
                blocks: None,
                channel_slugs: None,
                publish,
            },
        )
        .await
        .expect("page should be created")
}

#[tokio::test]
async fn manager_cannot_publish_via_create_or_update() {
    let (page_service, _, _, tenant_id) = setup().await;
    let manager = manager_context();

    let denied_create = page_service
        .create(
            tenant_id,
            manager.clone(),
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Published".to_string(),
                    slug: Some("published".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: None,
                blocks: None,
                channel_slugs: None,
                publish: true,
            },
        )
        .await
        .expect_err("manager should not publish during create");
    assert!(matches!(denied_create, PagesError::Forbidden(_)));

    let draft = create_page(&page_service, tenant_id, manager.clone(), "draft", false).await;
    let denied_update = page_service
        .update(
            tenant_id,
            manager,
            draft.id,
            UpdatePageInput {
                status: Some(rustok_content::entities::node::ContentStatus::Published),
                ..Default::default()
            },
        )
        .await
        .expect_err("manager should not publish through update");
    assert!(matches!(denied_update, PagesError::Forbidden(_)));
}

#[tokio::test]
async fn customer_cannot_read_drafts_and_list_only_returns_published_pages() {
    let (page_service, _, _, tenant_id) = setup().await;
    let admin = admin_context();
    let customer = customer_context();

    let draft = create_page(&page_service, tenant_id, admin.clone(), "draft-page", false).await;
    let published = create_page(
        &page_service,
        tenant_id,
        admin.clone(),
        "published-page",
        true,
    )
    .await;

    let denied = page_service
        .get(tenant_id, customer.clone(), draft.id)
        .await
        .expect_err("customer should not read draft page by id");
    assert!(matches!(denied, PagesError::Forbidden(_)));

    let visible = page_service
        .get(tenant_id, customer.clone(), published.id)
        .await
        .expect("customer should read published page");
    assert_eq!(visible.id, published.id);

    let (items, total) = page_service
        .list(
            tenant_id,
            customer,
            ListPagesFilter {
                status: None,
                template: None,
                locale: Some("en".to_string()),
                page: 1,
                per_page: 20,
            },
        )
        .await
        .expect("customer list should succeed");
    assert_eq!(total, 1);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, published.id);
}

#[tokio::test]
async fn customer_cannot_mutate_blocks_or_menus() {
    let (page_service, block_service, menu_service, tenant_id) = setup().await;
    let admin = admin_context();
    let customer = customer_context();

    let page = create_page(&page_service, tenant_id, admin, "page", false).await;

    let denied_block = block_service
        .create(
            tenant_id,
            customer.clone(),
            page.id,
            CreateBlockInput {
                block_type: BlockType::Text,
                position: 0,
                data: serde_json::json!({ "text": "nope" }),
                translations: None,
            },
        )
        .await
        .expect_err("customer should not create blocks");
    assert!(matches!(denied_block, PagesError::Forbidden(_)));

    let denied_menu = menu_service
        .create(
            tenant_id,
            customer,
            CreateMenuInput {
                name: "Main".to_string(),
                location: MenuLocation::Header,
                items: Vec::new(),
            },
        )
        .await
        .expect_err("customer should not create menus");
    assert!(matches!(denied_menu, PagesError::Forbidden(_)));
}
