use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CategoryTreeQuery, CreateCategoryInput, ForumModule, UpdateCategoryInput,
    entities::{forum_category, forum_category_taxonomy_binding, forum_category_translation},
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
async fn forum_category_tree_and_mutation_responses_use_taxonomy_canonical_data() -> TestResult<()> {
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

    db.execute_unprepared(
        r#"
        CREATE TRIGGER stale_forum_category_create_response
        AFTER INSERT ON sys_events
        BEGIN
            UPDATE forum_category_translations
            SET name = 'STALE LEGACY CREATE',
                slug = 'stale-legacy-create',
                description = 'stale legacy create'
            WHERE slug = 'support';
        END
        "#,
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
    assert_eq!(support.name, "Support");
    assert_eq!(support.slug, "support");
    assert_eq!(support.parent_id, Some(root.id));
    assert_eq!(support.icon.as_deref(), Some("life-buoy"));
    assert_eq!(support.color.as_deref(), Some("#112233"));

    let stale_after_create = forum_category_translation::Entity::find()
        .filter(forum_category_translation::Column::TenantId.eq(tenant_id))
        .filter(forum_category_translation::Column::CategoryId.eq(support.id))
        .filter(forum_category_translation::Column::Locale.eq("en"))
        .one(&db)
        .await?
        .expect("legacy Forum translation remains during CAT-5 compatibility");
    assert_eq!(stale_after_create.name, "STALE LEGACY CREATE");
    assert_eq!(stale_after_create.slug, "stale-legacy-create");

    db.execute_unprepared("DROP TRIGGER stale_forum_category_create_response")
        .await?;

    let lounge = service
        .create(
            tenant_id,
            admin(),
            create_input("Lounge", "lounge", Some(root.id), 1, None, None),
        )
        .await?;

    db.execute_unprepared(&format!(
        r#"
        CREATE TRIGGER stale_forum_category_update_response
        AFTER INSERT ON sys_events
        BEGIN
            UPDATE forum_category_translations
            SET name = 'STALE LEGACY UPDATE',
                slug = 'stale-legacy-update',
                description = 'stale legacy update'
            WHERE category_id = '{}' AND locale = 'en';
        END
        "#,
        support.id
    ))
    .await?;

    let updated = service
        .update(
            tenant_id,
            support.id,
            admin(),
            UpdateCategoryInput {
                locale: "en".to_string(),
                name: Some("Support Updated".to_string()),
                slug: Some("support-updated".to_string()),
                description: Some("Updated description".to_string()),
                icon: Some("headphones".to_string()),
                color: Some("#445566".to_string()),
                position: None,
                moderated: Some(false),
            },
        )
        .await?;
    assert_eq!(updated.name, "Support Updated");
    assert_eq!(updated.slug, "support-updated");
    assert_eq!(updated.description.as_deref(), Some("Updated description"));
    assert_eq!(updated.icon.as_deref(), Some("headphones"));
    assert_eq!(updated.color.as_deref(), Some("#445566"));

    let stale_after_update = forum_category_translation::Entity::find()
        .filter(forum_category_translation::Column::TenantId.eq(tenant_id))
        .filter(forum_category_translation::Column::CategoryId.eq(support.id))
        .filter(forum_category_translation::Column::Locale.eq("en"))
        .one(&db)
        .await?
        .expect("legacy Forum translation remains during CAT-5 compatibility");
    assert_eq!(stale_after_update.name, "STALE LEGACY UPDATE");
    assert_eq!(stale_after_update.slug, "stale-legacy-update");

    db.execute_unprepared("DROP TRIGGER stale_forum_category_update_response")
        .await?;

    let legacy_category = forum_category::Entity::find_by_id(support.id)
        .filter(forum_category::Column::TenantId.eq(tenant_id))
        .one(&db)
        .await?
        .expect("Forum policy row remains during CAT-5 cutover");
    let mut stale_category: forum_category::ActiveModel = legacy_category.into();
    stale_category.parent_id = Set(None);
    stale_category.position = Set(99);
    stale_category.icon = Set(Some("legacy-only-icon".to_string()));
    stale_category.color = Set(Some("#ffffff".to_string()));
    stale_category.moderated = Set(true);
    stale_category.update(&db).await?;

    forum_category_translation::Entity::delete_many()
        .filter(forum_category_translation::Column::TenantId.eq(tenant_id))
        .exec(&db)
        .await?;

    let tree = service
        .tree(
            tenant_id,
            admin(),
            CategoryTreeQuery {
                locale: Some("fr-CA".to_string()),
                fallback_locale: Some("en".to_string()),
            },
        )
        .await?;
    assert_eq!(tree.total_nodes, 3);
    assert_eq!(tree.max_depth, 1);
    assert_eq!(tree.roots.len(), 1);

    let root_node = &tree.roots[0];
    assert_eq!(root_node.id, root.id);
    assert_eq!(root_node.name, "General");
    assert_eq!(root_node.children.len(), 2);
    assert_eq!(root_node.children[0].id, support.id);
    assert_eq!(root_node.children[0].position, 0);
    assert_eq!(root_node.children[1].id, lounge.id);
    assert_eq!(root_node.children[1].position, 1);

    let support_node = &root_node.children[0];
    assert_eq!(support_node.parent_id, Some(root.id));
    assert_eq!(support_node.requested_locale, "fr-CA");
    assert_eq!(support_node.effective_locale, "en");
    assert_eq!(support_node.name, "Support Updated");
    assert_eq!(support_node.slug, "support-updated");
    assert_eq!(support_node.icon.as_deref(), Some("headphones"));
    assert_eq!(support_node.color.as_deref(), Some("#445566"));
    assert!(support_node.moderated, "moderation remains Forum-owned");
    assert_eq!(support_node.breadcrumbs.len(), 2);
    assert_eq!(support_node.breadcrumbs[0].name, "General");
    assert_eq!(support_node.breadcrumbs[1].name, "Support Updated");
    assert!(
        tree.roots.iter().all(|node| node.id != support.id),
        "stale Forum parent_id must not turn Support into a tree root"
    );

    forum_category_taxonomy_binding::Entity::delete_by_id((tenant_id, support.id))
        .exec(&db)
        .await?;
    let missing_binding = service
        .tree(
            tenant_id,
            admin(),
            CategoryTreeQuery {
                locale: Some("en".to_string()),
                fallback_locale: None,
            },
        )
        .await;
    assert!(
        missing_binding.is_err(),
        "tree cutover must fail closed instead of falling back to Forum hierarchy/copy"
    );

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
        description: Some(format!("{name} description")),
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
        "sqlite:file:forum_category_taxonomy_tree_mutation_cutover_{}?mode=memory&cache=shared",
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
