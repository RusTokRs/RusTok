use rustok_channel::{ChannelService, CreateChannelInput, migrations as channel_migrations};
use rustok_core::MigrationSource;
use rustok_pages::PagesModule;
use rustok_pages::dto::{
    CreateMenuInput, MenuItemInput, MenuItemTranslationInput, MenuLocation, MenuTranslationInput,
};
use rustok_pages::entities::menu_binding;
use rustok_pages::services::{MenuBindingService, MenuService};
use rustok_test_utils::{db::setup_test_db, helpers::admin_context, mock_transactional_event_bus};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> (DatabaseConnection, Uuid, Uuid, Uuid) {
    let db = setup_test_db().await;
    db.execute(Statement::from_string(
        db.get_database_backend(),
        r#"
        CREATE TABLE tenants (
            id TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            slug TEXT NOT NULL UNIQUE,
            domain TEXT NULL UNIQUE,
            settings TEXT NOT NULL DEFAULT '{}',
            default_locale TEXT NOT NULL DEFAULT 'en',
            is_active BOOLEAN NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    ))
    .await
    .expect("tenants table should exist for channel foreign keys");

    let manager = SchemaManager::new(&db);
    for migration in channel_migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("channel migration should apply");
    }
    for migration in PagesModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("pages migration should apply");
    }

    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id, "tenant-menu").await;
    seed_tenant(&db, other_tenant_id, "tenant-other").await;

    let channel_service = ChannelService::new(db.clone());
    let channel = channel_service
        .create_channel(CreateChannelInput {
            tenant_id,
            slug: "web".to_string(),
            name: "Web".to_string(),
            settings: None,
        })
        .await
        .expect("tenant channel should be created");
    let other_channel = channel_service
        .create_channel(CreateChannelInput {
            tenant_id: other_tenant_id,
            slug: "other-web".to_string(),
            name: "Other Web".to_string(),
            settings: None,
        })
        .await
        .expect("other tenant channel should be created");

    (db, tenant_id, channel.id, other_channel.id)
}

async fn seed_tenant(db: &DatabaseConnection, tenant_id: Uuid, slug: &str) {
    db.execute(Statement::from_sql_and_values(
        db.get_database_backend(),
        "INSERT INTO tenants (id, name, slug, settings, default_locale, is_active, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        [
            tenant_id.into(),
            format!("{slug} tenant").into(),
            slug.to_string().into(),
            "{}".to_string().into(),
            "en".to_string().into(),
            true.into(),
        ],
    ))
    .await
    .expect("tenant should be inserted");
}

fn menu(name: &str, title: &str, location: MenuLocation) -> CreateMenuInput {
    CreateMenuInput {
        translations: vec![MenuTranslationInput {
            locale: "en".to_string(),
            name: name.to_string(),
        }],
        location,
        items: vec![MenuItemInput {
            translations: vec![MenuItemTranslationInput {
                locale: "en".to_string(),
                title: title.to_string(),
            }],
            url: Some("/".to_string()),
            page_id: None,
            icon: None,
            position: 0,
            children: None,
        }],
    }
}

#[tokio::test]
async fn active_header_binding_is_unique_and_replaced_atomically() {
    let (db, tenant_id, channel_id, _) = setup().await;
    let event_bus = mock_transactional_event_bus();
    let menu_service = MenuService::new(db.clone(), event_bus.clone());
    let first = menu_service
        .create(
            tenant_id,
            admin_context(),
            "en",
            menu("Primary", "Home", MenuLocation::Header),
        )
        .await
        .expect("first menu should be created");
    let second = menu_service
        .create(
            tenant_id,
            admin_context(),
            "en",
            menu("Replacement", "Start", MenuLocation::Header),
        )
        .await
        .expect("second menu should be created");

    let bindings = MenuBindingService::new(db.clone(), event_bus);
    bindings
        .bind(
            tenant_id,
            admin_context(),
            channel_id,
            MenuLocation::Header,
            first.id,
        )
        .await
        .expect("first menu should become active");
    bindings
        .bind(
            tenant_id,
            admin_context(),
            channel_id,
            MenuLocation::Header,
            second.id,
        )
        .await
        .expect("replacement should update the same active identity");

    let active = bindings
        .get_active(
            tenant_id,
            admin_context(),
            channel_id,
            MenuLocation::Header,
            "en",
        )
        .await
        .expect("active menu lookup should succeed")
        .expect("header binding should exist");
    assert_eq!(active.id, second.id);
    assert_eq!(active.name, "Replacement");
    assert_eq!(active.items[0].title, "Start");

    let stored = menu_binding::Entity::find()
        .filter(menu_binding::Column::TenantId.eq(tenant_id))
        .filter(menu_binding::Column::ChannelId.eq(channel_id))
        .filter(menu_binding::Column::Location.eq("header"))
        .all(&db)
        .await
        .expect("bindings should be readable");
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].menu_id, second.id);
}

#[tokio::test]
async fn active_binding_rejects_cross_tenant_channel() {
    let (db, tenant_id, _, other_channel_id) = setup().await;
    let event_bus = mock_transactional_event_bus();
    let created = MenuService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            admin_context(),
            "en",
            menu("Primary", "Home", MenuLocation::Header),
        )
        .await
        .expect("menu should be created");

    let error = MenuBindingService::new(db, event_bus)
        .bind(
            tenant_id,
            admin_context(),
            other_channel_id,
            MenuLocation::Header,
            created.id,
        )
        .await
        .expect_err("cross-tenant channel binding must fail closed");
    assert!(error.to_string().contains("does not belong to tenant"));
}
