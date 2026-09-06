use chrono::Utc;
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, ForumCategoryVisibility,
    ForumCategoryVisibilityPolicyService, ForumModule, SetForumCategoryVisibilityPolicyInput,
    UpdateCategoryTopicPolicyInput, entities::forum_category_policy,
};
use rustok_outbox::OutboxModule;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectOptions, ConnectionTrait, Database,
    DatabaseConnection,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:forum_category_visibility_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(db_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("forum category visibility sqlite database should connect");
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
    db: &DatabaseConnection,
    tenant_id: Uuid,
    security: SecurityContext,
    slug: &str,
    parent_id: Option<Uuid>,
) -> Uuid {
    CategoryService::new(db.clone())
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
async fn authenticated_visibility_inherits_and_cannot_be_broadened() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let foreign_tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let reader = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));

    let root = create_category(&db, tenant_id, admin.clone(), "root", None).await;
    let child = create_category(&db, tenant_id, admin.clone(), "child", Some(root)).await;
    let grandchild =
        create_category(&db, tenant_id, admin.clone(), "grandchild", Some(child)).await;
    let sibling = create_category(&db, tenant_id, admin.clone(), "sibling", Some(root)).await;
    let foreign = create_category(&db, foreign_tenant_id, admin.clone(), "foreign", None).await;

    let service = ForumCategoryVisibilityPolicyService::new(db.clone());
    let default_policy = service
        .get(tenant_id, grandchild, reader.clone())
        .await
        .expect("missing overrides should resolve to public");
    assert_eq!(
        default_policy.effective_visibility,
        ForumCategoryVisibility::Public
    );
    assert_eq!(default_policy.configured_visibility, None);
    assert_eq!(default_policy.effective_from_category_id, None);

    let child_policy = service
        .set(
            tenant_id,
            child,
            admin.clone(),
            SetForumCategoryVisibilityPolicyInput {
                visibility: ForumCategoryVisibility::Authenticated,
            },
        )
        .await
        .expect("child should narrow to authenticated");
    assert_eq!(
        child_policy.configured_visibility,
        Some(ForumCategoryVisibility::Authenticated)
    );
    assert_eq!(child_policy.effective_from_category_id, Some(child));

    let inherited = service
        .get(tenant_id, grandchild, reader.clone())
        .await
        .expect("grandchild should inherit the authenticated floor");
    assert_eq!(
        inherited.effective_visibility,
        ForumCategoryVisibility::Authenticated
    );
    assert_eq!(inherited.configured_visibility, None);
    assert_eq!(inherited.effective_from_category_id, Some(child));

    let sibling_policy = service
        .get(tenant_id, sibling, reader.clone())
        .await
        .expect("unrelated sibling should remain public");
    assert_eq!(
        sibling_policy.effective_visibility,
        ForumCategoryVisibility::Public
    );

    let broaden_error = service
        .set(
            tenant_id,
            grandchild,
            admin.clone(),
            SetForumCategoryVisibilityPolicyInput {
                visibility: ForumCategoryVisibility::Public,
            },
        )
        .await
        .expect_err("child must not broaden an authenticated ancestor");
    assert!(broaden_error.to_string().contains("cannot broaden"));

    CategoryService::new(db.clone())
        .set_topic_policy(
            tenant_id,
            child,
            admin.clone(),
            UpdateCategoryTopicPolicyInput {
                allows_topics: false,
            },
        )
        .await
        .expect("topic placement policy should update independently");
    let preserved = service
        .get(tenant_id, child, reader.clone())
        .await
        .expect("topic policy write must preserve visibility");
    assert_eq!(
        preserved.configured_visibility,
        Some(ForumCategoryVisibility::Authenticated)
    );

    service
        .set(
            tenant_id,
            child,
            admin.clone(),
            SetForumCategoryVisibilityPolicyInput {
                visibility: ForumCategoryVisibility::Public,
            },
        )
        .await
        .expect("local override may clear beneath a public parent");
    assert_eq!(
        service
            .get(tenant_id, grandchild, reader.clone())
            .await
            .expect("cleared subtree should become public")
            .effective_visibility,
        ForumCategoryVisibility::Public
    );

    service
        .set(
            tenant_id,
            root,
            admin.clone(),
            SetForumCategoryVisibilityPolicyInput {
                visibility: ForumCategoryVisibility::Authenticated,
            },
        )
        .await
        .expect("root should narrow the entire hierarchy");
    let root_inherited = service
        .get(tenant_id, grandchild, reader.clone())
        .await
        .expect("root floor should propagate");
    assert_eq!(root_inherited.effective_from_category_id, Some(root));
    assert_eq!(
        root_inherited.effective_visibility,
        ForumCategoryVisibility::Authenticated
    );

    assert!(
        service
            .set(
                tenant_id,
                child,
                admin.clone(),
                SetForumCategoryVisibilityPolicyInput {
                    visibility: ForumCategoryVisibility::Public,
                },
            )
            .await
            .is_err()
    );
    assert!(service.get(tenant_id, foreign, reader).await.is_err());

    let direct_public_override = forum_category_policy::ActiveModel {
        category_id: Set(sibling),
        tenant_id: Set(tenant_id),
        allows_topics: Set(true),
        visibility_override: Set(Some(ForumCategoryVisibility::Public)),
        updated_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await
    .expect_err("database must reject a broadening public override");
    assert!(direct_public_override.to_string().contains("must narrow"));
}
