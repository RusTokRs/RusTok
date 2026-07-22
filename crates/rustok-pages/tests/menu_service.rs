use rustok_core::{MigrationSource, error::RichError};
use rustok_pages::PagesModule;
use rustok_pages::dto::{
    CreateMenuInput, MenuItemInput, MenuItemTranslationInput, MenuLocation, MenuTranslationInput,
};
use rustok_pages::services::{MENU_LOCALE_NOT_FOUND_ERROR_CODE, MenuService};
use rustok_test_utils::{db::setup_test_db, helpers::admin_context, mock_transactional_event_bus};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> (DatabaseConnection, MenuService, Uuid) {
    let db = setup_test_db().await;
    let schema = SchemaManager::new(&db);
    for migration in PagesModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("failed to apply pages migrations");
    }
    let service = MenuService::new(db.clone(), mock_transactional_event_bus());
    (db, service, Uuid::new_v4())
}

fn menu_translation(locale: &str, name: &str) -> MenuTranslationInput {
    MenuTranslationInput {
        locale: locale.to_string(),
        name: name.to_string(),
    }
}

fn item_translation(locale: &str, title: &str) -> MenuItemTranslationInput {
    MenuItemTranslationInput {
        locale: locale.to_string(),
        title: title.to_string(),
    }
}

fn localized_item(en: &str, ru: &str, url: &str, position: i32) -> MenuItemInput {
    MenuItemInput {
        translations: vec![item_translation("en", en), item_translation("ru", ru)],
        url: Some(url.to_string()),
        page_id: None,
        icon: None,
        position,
        children: None,
    }
}

fn localized_menu() -> CreateMenuInput {
    CreateMenuInput {
        translations: vec![
            menu_translation("en", "Main"),
            menu_translation("ru", "Главное"),
        ],
        location: MenuLocation::Header,
        items: vec![
            localized_item("Home", "Главная", "/", 0),
            MenuItemInput {
                translations: vec![
                    item_translation("en", "Catalog"),
                    item_translation("ru", "Каталог"),
                ],
                url: Some("/catalog".to_string()),
                page_id: None,
                icon: Some("grid".to_string()),
                position: 1,
                children: Some(vec![localized_item(
                    "Sale",
                    "Распродажа",
                    "/catalog/sale",
                    0,
                )]),
            },
        ],
    }
}

#[tokio::test]
async fn menu_round_trip_uses_exact_host_selected_locale() {
    let (_db, service, tenant_id) = setup().await;
    let russian = service
        .create(tenant_id, admin_context(), "ru", localized_menu())
        .await
        .expect("localized menu should be created");

    assert_eq!(russian.effective_locale, "ru");
    assert_eq!(russian.available_locales, vec!["en", "ru"]);
    assert_eq!(russian.name, "Главное");
    assert_eq!(russian.items[0].title, "Главная");
    assert_eq!(russian.items[1].title, "Каталог");
    assert_eq!(russian.items[1].children[0].title, "Распродажа");

    let english = service
        .get(tenant_id, admin_context(), russian.id, "en")
        .await
        .expect("English locale should resolve exactly");
    assert_eq!(english.effective_locale, "en");
    assert_eq!(english.name, "Main");
    assert_eq!(english.items[0].title, "Home");
    assert_eq!(english.items[1].title, "Catalog");
}

#[tokio::test]
async fn missing_effective_locale_never_falls_back_to_english() {
    let (_db, service, tenant_id) = setup().await;
    let menu = service
        .create(tenant_id, admin_context(), "ru", localized_menu())
        .await
        .expect("localized menu should be created");

    let error = service
        .get(tenant_id, admin_context(), menu.id, "de")
        .await
        .expect_err("missing effective locale must fail closed");
    let rich: RichError = error.into();
    assert_eq!(
        rich.error_code.as_deref(),
        Some(MENU_LOCALE_NOT_FOUND_ERROR_CODE)
    );
}

#[tokio::test]
async fn menu_item_locale_set_must_match_menu_locale_set() {
    let (_db, service, tenant_id) = setup().await;
    let error = service
        .create(
            tenant_id,
            admin_context(),
            "en",
            CreateMenuInput {
                translations: vec![
                    menu_translation("en", "Main"),
                    menu_translation("ru", "Главное"),
                ],
                location: MenuLocation::Header,
                items: vec![MenuItemInput {
                    translations: vec![item_translation("en", "Home")],
                    url: Some("/".to_string()),
                    page_id: None,
                    icon: None,
                    position: 0,
                    children: None,
                }],
            },
        )
        .await
        .expect_err("partial item translation set must fail");
    assert!(error.to_string().contains("must exactly match"));
}

#[tokio::test]
async fn sqlite_rejects_cross_tenant_menu_item_translation() {
    let (db, service, tenant_id) = setup().await;
    let menu = service
        .create(tenant_id, admin_context(), "en", localized_menu())
        .await
        .expect("localized menu should be created");
    let item_id = menu.items[0].id;

    let result = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO menu_item_translations (id, menu_item_id, locale, title, tenant_id, menu_id) VALUES (?, ?, ?, ?, ?, ?)",
            [
                Uuid::new_v4().into(),
                item_id.into(),
                "en".into(),
                "Start".into(),
                Uuid::new_v4().into(),
                menu.id.into(),
            ],
        ))
        .await;
    assert!(
        result.is_err(),
        "DB trigger must reject cross-tenant menu item copy"
    );
}
