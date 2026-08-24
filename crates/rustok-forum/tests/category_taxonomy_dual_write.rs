use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, ForumModule, MoveCategoryInput,
    ReorderCategorySiblingsInput, UpdateCategoryInput, entities::forum_category_taxonomy_binding,
};
use rustok_taxonomy::{
    TaxonomyModule, TaxonomyOwnerCategoryReader, TaxonomyScopeType, entities::taxonomy_term_alias,
};
use sea_orm::{
    ColumnTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait, QueryFilter,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test]
async fn forum_category_writes_keep_taxonomy_category_mirror_current() -> TestResult<()> {
    let db = setup().await?;
    let service = CategoryService::new(db.clone());
    let reader = TaxonomyOwnerCategoryReader::new(db.clone());
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
    let lounge = service
        .create(
            tenant_id,
            admin(),
            create_input("Lounge", "lounge", Some(root.id), 0, None, None),
        )
        .await?;

    let binding = forum_category_taxonomy_binding::Entity::find_by_id((tenant_id, support.id))
        .one(&db)
        .await?
        .expect("dual-write must create the transitional same-ID binding");
    assert_eq!(binding.taxonomy_category_id, support.id);

    let projected = load_category(&reader, tenant_id, support.id).await?;
    assert_eq!(projected.name, "Support");
    assert_eq!(projected.slug, "support");
    assert_eq!(projected.parent_id, Some(root.id));
    assert_eq!(
        projected.position, 1,
        "second create shifts Support after Lounge"
    );
    assert_eq!(projected.icon_key.as_deref(), Some("life-buoy"));
    assert_eq!(projected.color.as_deref(), Some("#112233"));

    service
        .update(
            tenant_id,
            support.id,
            admin(),
            UpdateCategoryInput {
                locale: "en".to_string(),
                name: Some("Help & Support".to_string()),
                slug: Some("help-support".to_string()),
                description: Some("Get help from the community".to_string()),
                icon: Some("headphones".to_string()),
                color: Some("#445566".to_string()),
                position: None,
                moderated: Some(true),
            },
        )
        .await?;

    let projected = load_category(&reader, tenant_id, support.id).await?;
    assert_eq!(projected.name, "Help & Support");
    assert_eq!(projected.slug, "help-support");
    assert_eq!(
        projected.description.as_deref(),
        Some("Get help from the community")
    );
    assert_eq!(projected.icon_key.as_deref(), Some("headphones"));
    assert_eq!(projected.color.as_deref(), Some("#445566"));

    let aliases = taxonomy_term_alias::Entity::find()
        .filter(taxonomy_term_alias::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term_alias::Column::TermId.eq(support.id))
        .all(&db)
        .await?;
    assert!(aliases.iter().any(|alias| alias.slug == "support"));

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

    Ok(())
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
        .expect("mirrored Taxonomy Category must exist"))
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
        "sqlite:file:forum_category_taxonomy_dual_write_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;
    let manager = SchemaManager::new(&db);

    for migration in TaxonomyModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in ForumModule.migrations() {
        migration.up(&manager).await?;
    }
    Ok(db)
}
