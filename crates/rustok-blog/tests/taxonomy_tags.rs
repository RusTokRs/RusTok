use std::sync::Arc;

use rustok_blog::{
    BlogModule, CreatePostInput, ListTagsFilter, PostService, TagService, UpdateTagInput,
    entities::blog_post_tag,
};
use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_outbox::{SysEvents, SysEventsMigration, TransactionalEventBus};
use rustok_taxonomy::{
    CreateTaxonomyTermInput, TaxonomyModule, TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
    entities::{taxonomy_term, taxonomy_term_translation},
};
use sea_orm::{
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, EntityTrait,
    QueryFilter, QueryOrder,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

async fn setup_blog_test_db() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:blog_taxonomy_tags_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut opts = ConnectOptions::new(db_url);
    opts.max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);

    Database::connect(opts)
        .await
        .expect("failed to connect blog sqlite database")
}

async fn setup() -> (
    DatabaseConnection,
    TransactionalEventBus,
    tokio::sync::broadcast::Receiver<rustok_events::EventEnvelope>,
    Uuid,
) {
    let db = setup_blog_test_db().await;
    let schema = SchemaManager::new(&db);
    SysEventsMigration
        .up(&schema)
        .await
        .expect("outbox migration should apply");
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("taxonomy migration should apply");
    }
    for migration in BlogModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("blog migration should apply");
    }

    let transport = MemoryTransport::new();
    let receiver = transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    (db, event_bus, receiver, Uuid::new_v4())
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

#[tokio::test]
async fn post_tags_create_blog_scoped_taxonomy_terms_and_usage_counts() {
    let (db, event_bus, _events, tenant_id) = setup().await;
    let post_service = PostService::new(db.clone(), event_bus);
    let tag_service = TagService::new(db.clone());
    let security = admin();

    let post_id = post_service
        .create_post(
            tenant_id,
            security.clone(),
            CreatePostInput {
                locale: "en".to_string(),
                title: "Tagged post".to_string(),
                content: rustok_blog::richtext::article_document_from_plain_text(
                    &"Body".to_string(),
                ),
                excerpt: None,
                slug: Some("tagged-post".to_string()),
                publish: true,
                tags: vec![
                    "rust".to_string(),
                    "backend".to_string(),
                    "rust".to_string(),
                ],
                category_id: None,
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
                channel_slugs: None,
                metadata: None,
            },
        )
        .await
        .expect("post should be created");

    let post = post_service
        .get_post(tenant_id, security.clone(), post_id, "en")
        .await
        .expect("post should load");
    assert_eq!(post.tags.len(), 2);
    assert!(post.tags.contains(&"rust".to_string()));
    assert!(post.tags.contains(&"backend".to_string()));

    let post_tags = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::PostId.eq(post_id))
        .all(&db)
        .await
        .expect("post tag relations should load");
    assert_eq!(post_tags.len(), 2);

    let terms = taxonomy_term::Entity::find()
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .all(&db)
        .await
        .expect("taxonomy terms should load");
    assert_eq!(terms.len(), 2);
    assert!(
        terms
            .iter()
            .all(|term| term.scope_type == TaxonomyScopeType::Module)
    );
    assert!(terms.iter().all(|term| term.scope_value == "blog"));

    let (tags, total) = tag_service
        .list_tags(
            tenant_id,
            security,
            ListTagsFilter {
                locale: Some("en".to_string()),
                page: 1,
                per_page: 10,
            },
        )
        .await
        .expect("blog tags should list");
    assert_eq!(total, 2);
    assert!(tags.iter().all(|item| item.use_count == 1));
}

#[tokio::test]
async fn post_tag_sync_reuses_existing_global_taxonomy_term() {
    let (db, event_bus, _events, tenant_id) = setup().await;
    let post_service = PostService::new(db.clone(), event_bus);
    let taxonomy_service = TaxonomyService::new(db.clone());
    let security = admin();

    let global_rust_term_id = taxonomy_service
        .create_term(
            tenant_id,
            security.clone(),
            CreateTaxonomyTermInput {
                kind: TaxonomyTermKind::Tag,
                scope_type: TaxonomyScopeType::Global,
                scope_value: None,
                locale: "en".to_string(),
                name: "rust".to_string(),
                slug: None,
                canonical_key: None,
                description: None,
                aliases: vec![],
            },
        )
        .await
        .expect("global term should be created");

    let post_id = post_service
        .create_post(
            tenant_id,
            security,
            CreatePostInput {
                locale: "en".to_string(),
                title: "Global tag reuse".to_string(),
                content: rustok_blog::richtext::article_document_from_plain_text(
                    &"Body".to_string(),
                ),
                excerpt: None,
                slug: Some("global-tag-reuse".to_string()),
                publish: true,
                tags: vec!["rust".to_string(), "backend".to_string()],
                category_id: None,
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
                channel_slugs: None,
                metadata: None,
            },
        )
        .await
        .expect("post should be created");

    let attached_term_ids = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::PostId.eq(post_id))
        .all(&db)
        .await
        .expect("blog post tags should load")
        .into_iter()
        .map(|row| row.tag_id)
        .collect::<Vec<_>>();
    assert!(attached_term_ids.contains(&global_rust_term_id));

    let blog_scoped_terms = taxonomy_term::Entity::find()
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
        .filter(taxonomy_term::Column::ScopeValue.eq("blog"))
        .all(&db)
        .await
        .expect("blog-scoped terms should load");
    assert_eq!(blog_scoped_terms.len(), 1);
    assert_eq!(blog_scoped_terms[0].canonical_key, "backend");
}

#[tokio::test]
async fn post_read_does_not_resurrect_metadata_tags_after_relations_are_removed() {
    let (db, event_bus, _events, tenant_id) = setup().await;
    let post_service = PostService::new(db.clone(), event_bus);
    let security = admin();

    let post_id = post_service
        .create_post(
            tenant_id,
            security.clone(),
            CreatePostInput {
                locale: "en".to_string(),
                title: "Canonical tag relation".to_string(),
                content: rustok_blog::richtext::article_document_from_plain_text(
                    &"Body".to_string(),
                ),
                excerpt: None,
                slug: Some("canonical-tag-relation".to_string()),
                publish: true,
                tags: vec!["stale-metadata-tag".to_string()],
                category_id: None,
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
                channel_slugs: None,
                metadata: None,
            },
        )
        .await
        .expect("post should be created");

    blog_post_tag::Entity::delete_many()
        .filter(blog_post_tag::Column::PostId.eq(post_id))
        .exec(&db)
        .await
        .expect("test should remove canonical relations while leaving compatibility metadata intact");

    let post = post_service
        .get_post(tenant_id, security, post_id, "en")
        .await
        .expect("post should load after relation removal");
    assert!(post.tags.is_empty());
}

#[tokio::test]
async fn tag_update_commits_dictionary_change_and_blog_reindex_together() {
    let (db, event_bus, _events, tenant_id) = setup().await;
    let post_service = PostService::new(db.clone(), event_bus);
    let tag_service = TagService::new(db.clone());
    let security = admin();

    let post_id = post_service
        .create_post(
            tenant_id,
            security.clone(),
            CreatePostInput {
                locale: "en".to_string(),
                title: "Atomic tag update".to_string(),
                content: rustok_blog::richtext::article_document_from_plain_text(&"Body".to_string()),
                excerpt: None,
                slug: Some("atomic-tag-update".to_string()),
                publish: true,
                tags: vec!["rust".to_string()],
                category_id: None,
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
                channel_slugs: None,
                metadata: None,
            },
        )
        .await
        .expect("post should be created");
    let tag_id = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::PostId.eq(post_id))
        .one(&db)
        .await
        .expect("relation lookup should work")
        .expect("post should have one tag")
        .tag_id;

    let updated = tag_service
        .update_tag(
            tenant_id,
            tag_id,
            security.clone(),
            UpdateTagInput {
                locale: "en".to_string(),
                name: Some("backend".to_string()),
                slug: None,
            },
        )
        .await
        .expect("tag update should commit");
    assert_eq!(updated.name, "backend");
    assert_eq!(updated.slug, "backend");

    let post = post_service
        .get_post(tenant_id, security, post_id, "en")
        .await
        .expect("post should resolve renamed taxonomy tag");
    assert_eq!(post.tags, vec!["backend".to_string()]);

    let event = SysEvents::find()
        .order_by_desc(rustok_outbox::entity::Column::CreatedAt)
        .one(&db)
        .await
        .expect("outbox lookup should work")
        .expect("tag update should retain one durable reindex event");
    let envelope: EventEnvelope = serde_json::from_value(event.payload)
        .expect("outbox payload should decode as canonical event envelope");
    assert_eq!(envelope.tenant_id, tenant_id);
    assert!(matches!(
        envelope.event,
        DomainEvent::ReindexRequested { ref target_type, target_id: None }
            if target_type == "blog"
    ));
}

#[tokio::test]
async fn tag_update_rolls_back_when_blog_reindex_outbox_write_fails() {
    let (db, event_bus, _events, tenant_id) = setup().await;
    let post_service = PostService::new(db.clone(), event_bus);
    let tag_service = TagService::new(db.clone());
    let security = admin();

    let post_id = post_service
        .create_post(
            tenant_id,
            security.clone(),
            CreatePostInput {
                locale: "en".to_string(),
                title: "Rollback tag update".to_string(),
                content: rustok_blog::richtext::article_document_from_plain_text(&"Body".to_string()),
                excerpt: None,
                slug: Some("rollback-tag-update".to_string()),
                publish: true,
                tags: vec!["rust".to_string()],
                category_id: None,
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
                channel_slugs: None,
                metadata: None,
            },
        )
        .await
        .expect("post should be created");
    let tag_id = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::PostId.eq(post_id))
        .one(&db)
        .await
        .expect("relation lookup should work")
        .expect("post should have one tag")
        .tag_id;

    db.execute_unprepared("DROP TABLE sys_events")
        .await
        .expect("test should make canonical outbox persistence unavailable");

    tag_service
        .update_tag(
            tenant_id,
            tag_id,
            security,
            UpdateTagInput {
                locale: "en".to_string(),
                name: Some("backend".to_string()),
                slug: None,
            },
        )
        .await
        .expect_err("outbox failure must abort the owner transaction");

    let translation = taxonomy_term_translation::Entity::find()
        .filter(taxonomy_term_translation::Column::TermId.eq(tag_id))
        .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term_translation::Column::Locale.eq("en"))
        .one(&db)
        .await
        .expect("translation lookup should work")
        .expect("rolled-back translation should remain");
    assert_eq!(translation.name, "rust");
    assert_eq!(translation.slug, "rust");
    assert_eq!(translation.revision, 1);

    let term = taxonomy_term::Entity::find_by_id(tag_id)
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .one(&db)
        .await
        .expect("term lookup should work")
        .expect("rolled-back term should remain");
    assert_eq!(term.revision, 1);
}

#[tokio::test]
async fn tag_delete_relies_on_taxonomy_fk_cascade_and_retains_reindex() {
    let (db, event_bus, _events, tenant_id) = setup().await;
    let post_service = PostService::new(db.clone(), event_bus);
    let tag_service = TagService::new(db.clone());
    let security = admin();

    let post_id = post_service
        .create_post(
            tenant_id,
            security.clone(),
            CreatePostInput {
                locale: "en".to_string(),
                title: "Atomic tag delete".to_string(),
                content: rustok_blog::richtext::article_document_from_plain_text(&"Body".to_string()),
                excerpt: None,
                slug: Some("atomic-tag-delete".to_string()),
                publish: true,
                tags: vec!["rust".to_string()],
                category_id: None,
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
                channel_slugs: None,
                metadata: None,
            },
        )
        .await
        .expect("post should be created");
    let tag_id = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::PostId.eq(post_id))
        .one(&db)
        .await
        .expect("relation lookup should work")
        .expect("post should have one tag")
        .tag_id;

    tag_service
        .delete_tag(tenant_id, tag_id, security)
        .await
        .expect("tag delete should commit");

    assert!(
        taxonomy_term::Entity::find_by_id(tag_id)
            .one(&db)
            .await
            .expect("term lookup should work")
            .is_none()
    );
    assert!(
        blog_post_tag::Entity::find()
            .filter(blog_post_tag::Column::TagId.eq(tag_id))
            .one(&db)
            .await
            .expect("relation lookup should work")
            .is_none()
    );
    assert_eq!(
        SysEvents::find()
            .all(&db)
            .await
            .expect("outbox lookup should work")
            .len(),
        1
    );
}
