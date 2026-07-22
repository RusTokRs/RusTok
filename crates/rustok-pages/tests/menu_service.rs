use rustok_core::MigrationSource;
use rustok_pages::PagesModule;
use rustok_pages::dto::{CreateMenuInput, MenuItemInput, MenuLocation};
use rustok_pages::services::MenuService;
use rustok_test_utils::{db::setup_test_db, helpers::admin_context, mock_transactional_event_bus};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> (MenuService, Uuid) {
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
    (MenuService::new(db, event_bus), Uuid::new_v4())
}

#[tokio::test]
async fn menu_round_trip_uses_module_owned_storage() {
    let (service, tenant_id) = setup().await;
    let menu = service
        .create(
            tenant_id,
            admin_context(),
            CreateMenuInput {
                name: "Main".to_string(),
                location: MenuLocation::Header,
                items: vec![
                    MenuItemInput {
                        title: "Home".to_string(),
                        url: Some("/".to_string()),
                        page_id: None,
                        icon: None,
                        position: 0,
                        children: None,
                    },
                    MenuItemInput {
                        title: "Catalog".to_string(),
                        url: Some("/catalog".to_string()),
                        page_id: None,
                        icon: Some("grid".to_string()),
                        position: 1,
                        children: Some(vec![MenuItemInput {
                            title: "Sale".to_string(),
                            url: Some("/catalog/sale".to_string()),
                            page_id: None,
                            icon: None,
                            position: 0,
                            children: None,
                        }]),
                    },
                ],
            },
        )
        .await
        .expect("menu should be created");

    let fetched = service
        .get(tenant_id, admin_context(), menu.id)
        .await
        .expect("menu should be readable");

    assert_eq!(fetched.name, "Main");
    assert!(matches!(fetched.location, MenuLocation::Header));
    assert_eq!(fetched.items.len(), 2);
    assert_eq!(fetched.items[0].title.as_deref(), Some("Home"));
    assert_eq!(fetched.items[1].title.as_deref(), Some("Catalog"));
    assert_eq!(fetched.items[1].children.len(), 1);
    assert_eq!(fetched.items[1].children[0].title.as_deref(), Some("Sale"));
}
