use std::sync::Arc;

use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CategoryTreeQuery, CreateCategoryInput, CreateReplyInput, CreateTopicInput,
    ForumModule, ReplyService, TopicService,
    entities::{forum_category, forum_category_taxonomy_binding, forum_category_translation},
};
use rustok_outbox::{OutboxModule, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectOptions, ConnectionTrait, Database,
    DatabaseConnection, EntityTrait, QueryFilter,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test]
async fn forum_category_reads_use_taxonomy_copy_hierarchy_and_presentation() -> TestResult<()> {
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
    let lounge = service
        .create(
            tenant_id,
            admin(),
            create_input("Lounge", "lounge", Some(root.id), 1, None, None),
        )
        .await?;

    let author_id = Uuid::new_v4();
    db.execute_unprepared(&format!(
        "INSERT INTO users (id, tenant_id) VALUES ('{author_id}', '{tenant_id}')"
    ))
    .await?;
    let author = SecurityContext::new(UserRole::Admin, Some(author_id));
    let event_bus = TransactionalEventBus::new(Arc::new(MemoryTransport::new()));
    let topic_service = TopicService::new(db.clone(), event_bus.clone());
    let reply_service = ReplyService::new(db.clone(), event_bus);
    let topic = topic_service
        .create(
            tenant_id,
            author.clone(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: support.id,
                title: "Read cutover counter proof".to_string(),
                slug: Some("read-cutover-counter-proof".to_string()),
                body: rustok_api::RichTextDocument::single_paragraph("Counter proof body"),
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await?;
    reply_service
        .create(
            tenant_id,
            author,
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph("Counter proof reply"),
                parent_reply_id: None,
            },
        )
        .await?;

    forum_category_translation::Entity::delete_many()
        .filter(forum_category_translation::Column::TenantId.eq(tenant_id))
        .exec(&db)
        .await?;

    let forum_support = forum_category::Entity::find_by_id(support.id)
        .filter(forum_category::Column::TenantId.eq(tenant_id))
        .one(&db)
        .await?
        .expect("Forum policy row remains during CAT-5 read cutover");
    let mut stale_forum_support: forum_category::ActiveModel = forum_support.into();
    stale_forum_support.parent_id = Set(None);
    stale_forum_support.position = Set(99);
    stale_forum_support.icon = Set(Some("legacy-only".to_string()));
    stale_forum_support.color = Set(Some("#abcdef".to_string()));
    stale_forum_support.moderated = Set(true);
    stale_forum_support.update(&db).await?;

    let exact = service.get(tenant_id, admin(), support.id, "en").await?;
    assert_eq!(exact.name, "Support");
    assert_eq!(exact.slug, "support");
    assert_eq!(exact.parent_id, Some(root.id));
    assert_eq!(exact.position, 0);
    assert_eq!(exact.icon.as_deref(), Some("life-buoy"));
    assert_eq!(exact.color.as_deref(), Some("#112233"));
    assert!(exact.moderated, "moderation remains Forum-owned");
    assert_eq!(exact.topic_count, 1, "counters remain Forum-owned");
    assert_eq!(exact.reply_count, 1, "counters remain Forum-owned");

    let fallback = service
        .get_with_locale_fallback(tenant_id, admin(), support.id, "fr", Some("en"))
        .await?;
    assert_eq!(fallback.requested_locale, "fr");
    assert_eq!(fallback.effective_locale, "en");
    assert_eq!(fallback.name, "Support");

    let listed = service.list(tenant_id, admin(), "en").await?;
    let listed_support = listed
        .iter()
        .find(|category| category.id == support.id)
        .expect("Support remains in Forum category list");
    assert_eq!(listed_support.name, "Support");
    assert_eq!(listed_support.icon.as_deref(), Some("life-buoy"));
    assert_eq!(listed_support.color.as_deref(), Some("#112233"));
    assert_eq!(listed_support.topic_count, 1);
    assert_eq!(listed_support.reply_count, 1);

    let tree = service
        .tree(
            tenant_id,
            admin(),
            CategoryTreeQuery {
                locale: Some("en".to_string()),
                fallback_locale: None,
            },
        )
        .await?;
    let root_node = tree
        .roots
        .iter()
        .find(|category| category.id == root.id)
        .expect("canonical root remains a tree root");
    assert_eq!(root_node.children.len(), 2);
    assert_eq!(root_node.children[0].id, support.id);
    assert_eq!(root_node.children[0].position, 0);
    assert_eq!(root_node.children[0].name, "Support");
    assert!(root_node.children[0].moderated);
    assert_eq!(root_node.children[0].topic_count, 1);
    assert_eq!(root_node.children[0].reply_count, 1);
    assert_eq!(root_node.children[1].id, lounge.id);
    assert_eq!(root_node.children[1].position, 1);
    assert!(
        tree.roots.iter().all(|category| category.id != support.id),
        "stale Forum parent_id must not turn Support into a root"
    );

    forum_category_taxonomy_binding::Entity::delete_by_id((tenant_id, support.id))
        .exec(&db)
        .await?;
    let missing_binding = service.get(tenant_id, admin(), support.id, "en").await;
    assert!(
        missing_binding.is_err(),
        "read cutover must fail closed instead of falling back to legacy Forum copy"
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
