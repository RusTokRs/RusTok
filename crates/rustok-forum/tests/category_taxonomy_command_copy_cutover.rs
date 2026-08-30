use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, ForumModule, MoveCategoryInput,
    ReorderCategorySiblingsInput, UpdateCategoryInput,
};
use rustok_outbox::OutboxModule;
use rustok_taxonomy::{TaxonomyModule, TaxonomyOwnerCategoryReader, TaxonomyScopeType};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test]
async fn canonical_category_commands_do_not_write_legacy_translation_mirror() -> TestResult<()> {
    let db = setup().await?;
    let service = CategoryService::new(db.clone());
    let reader = TaxonomyOwnerCategoryReader::new(db.clone());
    let tenant_id = Uuid::new_v4();

    let root = service
        .create(
            tenant_id,
            admin(),
            create_input("General", "general", None, 0),
        )
        .await?;
    let support = service
        .create(
            tenant_id,
            admin(),
            create_input("Support", "support", Some(root.id), 0),
        )
        .await?;
    let lounge = service
        .create(
            tenant_id,
            admin(),
            create_input("Lounge", "lounge", Some(root.id), 0),
        )
        .await?;

    service
        .update(
            tenant_id,
            support.id,
            admin(),
            UpdateCategoryInput {
                locale: "en".to_string(),
                name: Some("Help & Support".to_string()),
                slug: Some("help-support".to_string()),
                description: Some("Canonical Taxonomy copy".to_string()),
                icon: None,
                color: None,
                position: None,
                moderated: None,
            },
        )
        .await?;

    let projected = load_category(&reader, tenant_id, support.id).await?;
    assert_eq!(projected.name, "Help & Support");
    assert_eq!(projected.slug, "help-support");
    assert_eq!(
        projected.description.as_deref(),
        Some("Canonical Taxonomy copy")
    );

    service
        .move_category(
            tenant_id,
            support.id,
            admin(),
            MoveCategoryInput {
                parent_id: Some(root.id),
                position: 0,
            },
        )
        .await?;

    let support_after_move = load_category(&reader, tenant_id, support.id).await?;
    let lounge_after_move = load_category(&reader, tenant_id, lounge.id).await?;
    assert_eq!(support_after_move.position, 0);
    assert_eq!(lounge_after_move.position, 1);
    assert_eq!(support_after_move.name, "Help & Support");
    assert_eq!(lounge_after_move.name, "Lounge");

    service
        .reorder_siblings(
            tenant_id,
            admin(),
            ReorderCategorySiblingsInput {
                parent_id: Some(root.id),
                ordered_category_ids: vec![lounge.id, support.id],
            },
        )
        .await?;

    let support_after_reorder = load_category(&reader, tenant_id, support.id).await?;
    let lounge_after_reorder = load_category(&reader, tenant_id, lounge.id).await?;
    assert_eq!(lounge_after_reorder.position, 0);
    assert_eq!(support_after_reorder.position, 1);
    assert_eq!(support_after_reorder.slug, "help-support");

    let public = service.get(tenant_id, admin(), support.id, "en").await?;
    assert_eq!(public.name, "Help & Support");
    assert_eq!(public.slug, "help-support");

    Ok(())
}

#[test]
fn category_owner_writes_have_no_legacy_translation_dependency() {
    const ADAPTER: &str = include_str!("../src/services/category_taxonomy_sync.rs");
    const OWNER: &str = include_str!("../src/services/category_projection_owner.rs");
    const IMPORT: &str = include_str!("../src/services/category_import.rs");

    assert!(!ADAPTER.contains("forum_category_translation"));
    assert!(!OWNER.contains("forum_category_translation"));
    assert!(!IMPORT.contains("forum_category_translation"));
    assert!(ADAPTER.contains("load_module_category_locale_copy_in_tx"));
    assert!(ADAPTER.contains("sync_module_category_structure_with_owned_copy_in_tx"));
    assert!(ADAPTER.contains("sync_module_category_with_owned_aliases_in_tx"));
}

async fn load_category(
    reader: &TaxonomyOwnerCategoryReader,
    tenant_id: Uuid,
    category_id: Uuid,
) -> TestResult<rustok_taxonomy::TaxonomyOwnerCategory> {
    Ok(reader
        .load_scoped_categories(
            tenant_id,
            TaxonomyScopeType::Module,
            Some("forum"),
            Some(&[category_id]),
            "en",
            None,
        )
        .await?
        .pop()
        .expect("canonical Taxonomy Category must exist"))
}

fn create_input(
    name: &str,
    slug: &str,
    parent_id: Option<Uuid>,
    position: i32,
) -> CreateCategoryInput {
    CreateCategoryInput {
        locale: "en".to_string(),
        name: name.to_string(),
        slug: slug.to_string(),
        description: None,
        icon: None,
        color: None,
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
        "sqlite:file:forum_category_taxonomy_command_copy_cutover_{}?mode=memory&cache=shared",
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
