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
        );
        CREATE TABLE channels (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            slug TEXT NOT NULL,
            name TEXT NOT NULL,
            is_active BOOLEAN NOT NULL DEFAULT 1,
            is_default BOOLEAN NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'experimental',
            settings TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE (tenant_id, slug),
            FOREIGN KEY (tenant_id) REFERENCES tenants (id) ON DELETE CASCADE
        )
        "#,
    ))
    .await
    .expect("tenant and channel tables should exist for Pages migrations");

    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
    let other_channel_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id, "tenant-menu").await;
    seed_tenant(&db, other_tenant_id, "tenant-other").await;
    seed_channel(&db, channel_id, tenant_id, "web").await;
    seed_channel(&db, other_channel_id, other_tenant_id, "other-web").await;

    let manager = SchemaManager::new(&db);
    for migration in PagesModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("pages migration should apply");
    }

    (db, tenant_id, channel_id, other_channel_id)
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

async fn seed_channel(
    db: &DatabaseConnection,
    channel_id: Uuid,
    tenant_id: Uuid,
    slug: &str,
) {
    db.execute(Statement::from_sql_and_values(
        db.get_database_backend(),
        "INSERT INTO channels (id, tenant_id, slug, name, is_active, is_default, status, settings, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        [
            channel_id.into(),
            tenant_id.into(),
            slug.to_string().into(),
            format!("{slug} channel").into(),
            true.into(),
            false.into(),
            "experimental".to_string().into(),
            "{}".to_string().into(),
        ],
    ))
    .await
    .expect("channel should be inserted");
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
async fn active_binding_rejects_cross_tenant_channel_and_menu_rows() {
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

    let error = MenuBindingService::new(db.clone(), event_bus)
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

    let raw_cross_tenant_insert = db
        .execute(Statement::from_sql_and_values(
            db.get_database_backend(),
            "INSERT INTO menu_bindings (id, tenant_id, channel_id, location, menu_id) VALUES (?, ?, ?, ?, ?)",
            [
                Uuid::new_v4().into(),
                Uuid::new_v4().into(),
                other_channel_id.into(),
                "header".to_string().into(),
                created.id.into(),
            ],
        ))
        .await;
    assert!(
        raw_cross_tenant_insert.is_err(),
        "composite tenant/menu foreign key must reject raw cross-tenant rows"
    );
}
