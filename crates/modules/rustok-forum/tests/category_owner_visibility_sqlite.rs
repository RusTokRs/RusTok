use std::collections::HashSet;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CategoryTreeQuery, CreateCategoryInput, ForumCategoryVisibility,
    ForumCategoryVisibilityPolicyService, ForumError, ForumModule,
    SetForumCategoryVisibilityPolicyInput,
};
use rustok_outbox::OutboxModule;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:forum_category_owner_visibility_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(db_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("forum category owner visibility sqlite database should connect");
    db.execute_unprepared(
        r#"
        CREATE TABLE users (
            id TEXT NOT NULL PRIMARY KEY,
            tenant_id TEXT NOT NULL
        )
        "#,
    )
    .await
    .expect("users table should be created");
    let schema = SchemaManager::new(&db);
    for migration in OutboxModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("outbox migration should apply");
    }
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("taxonomy migration should apply");
    }
    for migration in ForumModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("forum migration should apply");
    }
    db
}

async fn create_category(
    service: &CategoryService,
    tenant_id: Uuid,
    security: SecurityContext,
    slug: &str,
    parent_id: Option<Uuid>,
) -> Uuid {
    service
        .create(
            tenant_id,
            security,
            CreateCategoryInput {
                locale: "en".into(),
                name: slug.replace('-', " "),
                slug: slug.into(),
                description: None,
                icon: None,
                color: None,
                parent_id,
                position: Some(0),
                moderated: false,
            },
        )
        .await
        .expect("category should be created")
        .id
}

#[tokio::test]
async fn inherited_authenticated_floor_guards_category_exact_page_and_tree_reads() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let authenticated = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let public = SecurityContext::public_read();
    let categories = CategoryService::new(db.clone());

    let root = create_category(&categories, tenant_id, admin.clone(), "root", None).await;
    let public_child = create_category(
        &categories,
        tenant_id,
        admin.clone(),
        "public-child",
        Some(root),
    )
    .await;
    let restricted_child = create_category(
        &categories,
        tenant_id,
        admin.clone(),
        "members-child",
        Some(root),
    )
    .await;
    let restricted_grandchild = create_category(
        &categories,
        tenant_id,
        admin.clone(),
        "members-grandchild",
        Some(restricted_child),
    )
    .await;

    ForumCategoryVisibilityPolicyService::new(db)
        .set(
            tenant_id,
            restricted_child,
            admin,
            SetForumCategoryVisibilityPolicyInput {
                visibility: ForumCategoryVisibility::Authenticated,
            },
        )
        .await
        .expect("child category should narrow to authenticated viewers");

    assert!(
        matches!(
            categories
                .get_with_locale_fallback(
                    tenant_id,
                    public.clone(),
                    restricted_child,
                    "en",
                    Some("en"),
                )
                .await,
            Err(ForumError::CategoryNotFound(id)) if id == restricted_child
        ),
        "public exact category read must not expose an inherited authenticated target"
    );
    assert_eq!(
        categories
            .get_with_locale_fallback(
                tenant_id,
                authenticated.clone(),
                restricted_grandchild,
                "en",
                Some("en"),
            )
            .await
            .expect("authenticated exact category read should resolve")
            .id,
        restricted_grandchild
    );

    let (public_page, public_total) = categories
        .list_paginated_with_locale_fallback(tenant_id, public.clone(), "en", 1, 20, Some("en"))
        .await
        .expect("public category page should resolve");
    assert_eq!(public_total, 2);
    assert_eq!(
        public_page
            .iter()
            .map(|category| category.id)
            .collect::<HashSet<_>>(),
        HashSet::from([root, public_child])
    );

    let (authenticated_page, authenticated_total) = categories
        .list_paginated_with_locale_fallback(
            tenant_id,
            authenticated.clone(),
            "en",
            1,
            20,
            Some("en"),
        )
        .await
        .expect("authenticated category page should resolve");
    assert_eq!(authenticated_total, 4);
    assert_eq!(
        authenticated_page
            .iter()
            .map(|category| category.id)
            .collect::<HashSet<_>>(),
        HashSet::from([root, public_child, restricted_child, restricted_grandchild])
    );

    let public_tree = categories
        .tree(
            tenant_id,
            public,
            CategoryTreeQuery {
                locale: Some("en".into()),
                fallback_locale: Some("en".into()),
            },
        )
        .await
        .expect("public category tree should resolve");
    assert_eq!(public_tree.total_nodes, 2);
    assert_eq!(public_tree.max_depth, 1);
    assert_eq!(public_tree.roots.len(), 1);
    assert_eq!(public_tree.roots[0].id, root);
    assert!(public_tree.roots[0].has_children);
    assert_eq!(public_tree.roots[0].children_count, 1);
    assert_eq!(public_tree.roots[0].children[0].id, public_child);

    let authenticated_tree = categories
        .tree(
            tenant_id,
            authenticated,
            CategoryTreeQuery {
                locale: Some("en".into()),
                fallback_locale: Some("en".into()),
            },
        )
        .await
        .expect("authenticated category tree should resolve");
    assert_eq!(authenticated_tree.total_nodes, 4);
    assert_eq!(authenticated_tree.max_depth, 2);
    let root_node = &authenticated_tree.roots[0];
    assert_eq!(root_node.id, root);
    assert_eq!(root_node.children_count, 2);
    let restricted_node = root_node
        .children
        .iter()
        .find(|node| node.id == restricted_child)
        .expect("authenticated tree should contain restricted child");
    assert_eq!(restricted_node.children_count, 1);
    assert_eq!(restricted_node.children[0].id, restricted_grandchild);
}
