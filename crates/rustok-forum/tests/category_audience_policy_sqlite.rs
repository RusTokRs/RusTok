use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, ForumAudienceConstraints,
    ForumCategoryAudiencePolicyService, ForumError, ForumModule,
    SetForumCategoryAudiencePolicyInput,
};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:forum_category_audience_policy_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(db_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("forum category audience sqlite database should connect");
    db.execute_unprepared(
        r#"
        CREATE TABLE users (
            id TEXT NOT NULL PRIMARY KEY,
            tenant_id TEXT NOT NULL
        )
        "#,
    )
    .await
    .expect("SQLite platform user fixture should be created");
    let schema = SchemaManager::new(&db);
    for migration in rustok_outbox::OutboxModule.migrations() {
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
async fn category_audience_layers_inherit_conjunctively_and_remain_bounded() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let foreign_tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let categories = CategoryService::new(db.clone());
    let policies = ForumCategoryAudiencePolicyService::new(db.clone());

    let root = create_category(&categories, tenant_id, admin.clone(), "root", None).await;
    let child = create_category(&categories, tenant_id, admin.clone(), "child", Some(root)).await;
    let leaf = create_category(&categories, tenant_id, admin.clone(), "leaf", Some(child)).await;
    create_category(
        &categories,
        foreign_tenant_id,
        admin.clone(),
        "foreign-root",
        None,
    )
    .await;

    let root_channels = (0..32)
        .map(|index| format!("channel-{index:02}"))
        .collect::<Vec<_>>();
    let root_policy = policies
        .set(
            tenant_id,
            root,
            admin.clone(),
            SetForumCategoryAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    roles_any: vec![UserRole::Admin, UserRole::Manager],
                    channel_members_any: root_channels.clone(),
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("root local audience layer should persist");
    assert_eq!(root_policy.effective_layers.len(), 1);
    assert_eq!(
        root_policy.effective_layers[0].constraints.roles_any,
        vec![UserRole::Manager, UserRole::Admin]
    );
    assert_eq!(
        root_policy.effective_layers[0]
            .constraints
            .channel_members_any,
        root_channels
    );
    assert!(matches!(
        policies
            .get(tenant_id, root, SecurityContext::public_read())
            .await,
        Err(ForumError::Forbidden(_))
    ));

    let group_id = Uuid::new_v4();
    let allowed_user_id = Uuid::new_v4();
    let denied_user_id = Uuid::new_v4();
    policies
        .set(
            tenant_id,
            child,
            admin.clone(),
            SetForumCategoryAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    minimum_trust_level: Some(7),
                    group_members_any: vec![group_id],
                    allow_user_ids: vec![allowed_user_id],
                    deny_user_ids: vec![denied_user_id],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("child local audience layer should persist");

    let inherited = policies
        .get(tenant_id, leaf, admin.clone())
        .await
        .expect("leaf effective audience should resolve");
    assert!(inherited.configured_constraints.is_none());
    assert_eq!(
        inherited
            .effective_layers
            .iter()
            .map(|layer| layer.category_id)
            .collect::<Vec<_>>(),
        vec![root, child]
    );
    assert_eq!(
        inherited.effective_layers[1]
            .constraints
            .minimum_trust_level,
        Some(7)
    );
    assert_eq!(
        inherited.effective_layers[1].constraints.group_members_any,
        vec![group_id]
    );
    assert_eq!(
        inherited.effective_layers[1].constraints.allow_user_ids,
        vec![allowed_user_id]
    );
    assert_eq!(
        inherited.effective_layers[1].constraints.deny_user_ids,
        vec![denied_user_id]
    );

    let cleared = policies
        .set(
            tenant_id,
            child,
            admin.clone(),
            SetForumCategoryAudiencePolicyInput {
                constraints: ForumAudienceConstraints::default(),
            },
        )
        .await
        .expect("empty constraints should clear the local layer");
    assert!(cleared.configured_constraints.is_none());
    assert_eq!(
        cleared
            .effective_layers
            .iter()
            .map(|layer| layer.category_id)
            .collect::<Vec<_>>(),
        vec![root]
    );
    assert_eq!(
        policies
            .get(tenant_id, leaf, admin.clone())
            .await
            .expect("leaf should inherit only the root after clear")
            .effective_layers
            .len(),
        1
    );

    assert!(matches!(
        policies.get(foreign_tenant_id, root, admin.clone()).await,
        Err(ForumError::CategoryNotFound(id)) if id == root
    ));

    let direct_overflow = db
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO forum_category_audience_channels (tenant_id, category_id, channel_slug) VALUES (?, ?, ?)",
            [
                tenant_id.into(),
                root.into(),
                "channel-32".into(),
            ],
        ))
        .await;
    assert!(
        direct_overflow.is_err(),
        "database must reject a thirty-third channel relation"
    );

    let direct_relation_update = db
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "UPDATE forum_category_audience_channels SET channel_slug = ? WHERE tenant_id = ? AND category_id = ? AND channel_slug = ?",
            [
                "changed".into(),
                tenant_id.into(),
                root.into(),
                "channel-00".into(),
            ],
        ))
        .await;
    assert!(
        direct_relation_update.is_err(),
        "database must reject mutable relation-row updates"
    );

    let direct_policy_update = db
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "UPDATE forum_category_audience_policies SET minimum_trust_level = ? WHERE tenant_id = ? AND category_id = ?",
            [
                9.into(),
                tenant_id.into(),
                root.into(),
            ],
        ))
        .await;
    assert!(
        direct_policy_update.is_err(),
        "database must reject mutable policy-row updates"
    );

    let cross_tenant_policy = db
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO forum_category_audience_policies (tenant_id, category_id, minimum_trust_level, updated_at) VALUES (?, ?, ?, CURRENT_TIMESTAMP)",
            [
                foreign_tenant_id.to_string().into(),
                root.to_string().into(),
                1.into(),
            ],
        ))
        .await;
    assert!(
        cross_tenant_policy.is_err(),
        "database must reject a cross-tenant category policy relation"
    );
}
