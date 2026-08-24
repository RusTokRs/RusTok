use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, ForumModule,
    entities::{forum_category, forum_category_translation},
};
use rustok_outbox::OutboxModule;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectOptions, ConnectionTrait, Database,
    DatabaseConnection, EntityTrait, QueryFilter,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test]
async fn forum_category_get_and_list_read_canonical_taxonomy_copy_and_presentation() -> TestResult<()> {
    let db = setup().await?;
    let service = CategoryService::new(db.clone());
    let tenant_id = Uuid::new_v4();

    let root = service
        .create(
            tenant_id,
            admin(),
            create_input("General", "general", None, 0, None, None),
        )
        .await?;
    let support = service
        .create(
            tenant_id,
            admin(),
            create_input(
                "Support",
                "support",
                Some(root.id),
                0,
                Some("life-buoy"),
                Some("#112233"),
            ),
        )
        .await?;

    let legacy_translation = forum_category_translation::Entity::find()
        .filter(forum_category_translation::Column::TenantId.eq(tenant_id))
        .filter(forum_category_translation::Column::CategoryId.eq(support.id))
        .filter(forum_category_translation::Column::Locale.eq("en"))
        .one(&db)
        .await?
        .expect("legacy Forum translation must exist during CAT-5 compatibility");
    let mut legacy_translation: forum_category_translation::ActiveModel = legacy_translation.into();
    legacy_translation.name = Set("STALE LEGACY SUPPORT".to_string());
    legacy_translation.slug = Set("stale-legacy-support".to_string());
    legacy_translation.description = Set(Some("stale legacy description".to_string()));
    legacy_translation.update(&db).await?;

    let legacy_category = forum_category::Entity::find_by_id(support.id)
        .filter(forum_category::Column::TenantId.eq(tenant_id))
        .one(&db)
        .await?
        .expect("legacy Forum policy row must remain during CAT-5 compatibility");
    let mut legacy_category: forum_category::ActiveModel = legacy_category.into();
    legacy_category.parent_id = Set(None);
    legacy_category.position = Set(41);
    legacy_category.icon = Set(Some("legacy-only-icon".to_string()));
    legacy_category.color = Set(Some("#ffffff".to_string()));
    legacy_category.moderated = Set(true);
    legacy_category.topic_count = Set(7);
    legacy_category.reply_count = Set(13);
    legacy_category.update(&db).await?;

    let category = service
        .get_with_locale_fallback(tenant_id, admin(), support.id, "fr-CA", Some("en"))
        .await?;
    assert_eq!(category.id, support.id);
    assert_eq!(category.effective_locale, "en");
    assert_eq!(category.available_locales, vec!["en".to_string()]);
    assert_eq!(category.name, "Support");
    assert_eq!(category.slug, "support");
    assert_eq!(category.description, None);
    assert_eq!(category.icon.as_deref(), Some("life-buoy"));
    assert_eq!(category.color.as_deref(), Some("#112233"));
    assert_eq!(category.parent_id, Some(root.id));
    assert_eq!(category.position, 0);
    assert_eq!(category.topic_count, 7);
    assert_eq!(category.reply_count, 13);
    assert!(category.moderated);

    let (items, total) = service
        .list_paginated_with_locale_fallback(tenant_id, admin(), "fr-CA", 1, 100, Some("en"))
        .await?;
    assert_eq!(total, 2);
    let support_item = items
        .iter()
        .find(|item| item.id == support.id)
        .expect("Support must remain in the Forum-owned category page");
    assert_eq!(support_item.effective_locale, "en");
    assert_eq!(support_item.available_locales, vec!["en".to_string()]);
    assert_eq!(support_item.name, "Support");
    assert_eq!(support_item.slug, "support");
    assert_eq!(support_item.description, None);
    assert_eq!(support_item.icon.as_deref(), Some("life-buoy"));
    assert_eq!(support_item.color.as_deref(), Some("#112233"));
    assert_eq!(support_item.topic_count, 7);
    assert_eq!(support_item.reply_count, 13);

    Ok(())
}

fn create_input(
    name: &str,
    slug: &str,
    parent_id: Option<Uuid>,
    position: i32,
    icon: Option<&str>,
    color: Option<&str>,
) -> CreateCategoryInput {
    CreateCategoryInput {
        locale: "en".to_string(),
        name: name.to_string(),
        slug: slug.to_string(),
        description: None,
        icon: icon.map(ToOwned::to_owned),
        color: color.map(ToOwned::to_owned),
        parent_id,
        position: Some(position),
        moderated: false,
    }
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

async fn setup() -> TestResult<DatabaseConnection> {
    let url = format!(
        "sqlite:file:forum_category_taxonomy_read_cutover_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;
    db.execute_unprepared(
        "CREATE TABLE users (\
            id TEXT NOT NULL PRIMARY KEY, \
            tenant_id TEXT NOT NULL, \
            UNIQUE (tenant_id, id)\
        )",
    )
    .await?;
    let manager = SchemaManager::new(&db);

    for migration in OutboxModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in TaxonomyModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in ForumModule.migrations() {
        migration.up(&manager).await?;
    }
    Ok(db)
}
