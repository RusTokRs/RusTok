use std::sync::Arc;

use rustok_api::{Action, Permission, Resource, RichTextDocument};
use rustok_blog::{PostService, TagService, dto::ListTagsFilter};
use rustok_content::{
    CanonicalRouteResolver, ContentError, ContentOrchestrationService, DemotePostToTopicInput,
    PromoteTopicToPostInput,
};
use rustok_core::{SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ReplyService,
    TopicService, forum_topic,
};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::{
    CreateTaxonomyTermInput, TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
};
use sea_orm::{EntityTrait, TransactionTrait};
use uuid::Uuid;

use crate::OwnerCanonicalRouteResolver;
use crate::ServerContentOrchestrationBridge;
use crate::tests::helpers::{
    admin_security, ensure_conversion_schema, insert_test_actor, setup_conversion_test_db,
    zero_cooldown_providers,
};

#[tokio::test]
async fn conversion_preserves_blog_tag_permissions_and_usage() {
    let db = setup_conversion_test_db().await;
    ensure_conversion_schema(&db).await;
    let events = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let tenant_id = Uuid::new_v4();
    let admin = admin_security();
    insert_test_actor(&db, tenant_id, &admin).await;
    let category = CategoryService::new(db.clone())
        .create(
            tenant_id,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".into(),
                name: "General".into(),
                slug: "general".into(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await
        .unwrap();
    let topic = TopicService::new(db.clone(), events.clone())
        .with_settings_providers(zero_cooldown_providers())
        .create(
            tenant_id,
            admin.clone(),
            CreateTopicInput {
                locale: "en".into(),
                category_id: category.id,
                title: "Release Notes".into(),
                slug: Some("release-notes".into()),
                body: RichTextDocument::single_paragraph("Release notes body"),
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await
        .unwrap();
    let taxonomy = TaxonomyService::new(db.clone());
    let shared_tag = taxonomy
        .create_term(
            tenant_id,
            admin.clone(),
            CreateTaxonomyTermInput {
                kind: TaxonomyTermKind::Tag,
                scope_type: TaxonomyScopeType::Global,
                scope_value: None,
                locale: "en".into(),
                name: "Shared".into(),
                canonical_key: None,
                slug: None,
                description: None,
                aliases: Vec::new(),
            },
        )
        .await
        .unwrap();
    let txn = db.begin().await.unwrap();
    crate::bridge::tags::sync_forum_tags_for_topic_in_tx(
        &taxonomy,
        &txn,
        tenant_id,
        topic.id,
        &["Shared".into(), "Release".into()],
        "en",
    )
    .await
    .unwrap();
    txn.commit().await.unwrap();

    let service = ContentOrchestrationService::new(
        db.clone(),
        events.clone(),
        Arc::new(ServerContentOrchestrationBridge::new(
            db.clone(),
            events.clone(),
        )),
    );
    let input = PromoteTopicToPostInput {
        topic_id: topic.id,
        locale: "en".into(),
        blog_category_id: None,
        reason: None,
        idempotency_key: "promote-with-tags".into(),
    };
    let restricted = SecurityContext::from_permissions(
        UserRole::Admin,
        admin.user_id,
        [
            Permission::new(Resource::ForumTopics, Action::Moderate),
            Permission::new(Resource::BlogPosts, Action::Create),
        ],
    );
    let error = service
        .promote_topic_to_post(tenant_id, restricted, input.clone())
        .await
        .unwrap_err();
    assert!(matches!(error, ContentError::Forbidden(_)));
    assert!(
        forum_topic::Entity::find_by_id(topic.id)
            .one(&db)
            .await
            .unwrap()
            .is_some()
    );

    let promoted = service
        .promote_topic_to_post(tenant_id, admin.clone(), input.clone())
        .await
        .unwrap();
    let replay = service
        .promote_topic_to_post(tenant_id, admin.clone(), input)
        .await
        .unwrap();
    assert_eq!(replay.target_id, promoted.target_id);
    let routes = OwnerCanonicalRouteResolver::new(db.clone());
    let promoted_route = routes
        .resolve_route(
            tenant_id,
            "en",
            &format!("/modules/forum?topic={}", topic.id),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(promoted_route.redirect_required);
    assert_eq!(promoted_route.target_id, promoted.target_id);
    let blog_taxonomy = crate::in_process_blog_taxonomy_port(db.clone());
    let posts = PostService::new(db.clone(), events).with_taxonomy_port(blog_taxonomy.clone());
    let tags = TagService::new(db.clone()).with_taxonomy_port(blog_taxonomy);
    let post = posts
        .get_post(tenant_id, admin.clone(), promoted.target_id, "en")
        .await
        .unwrap();
    assert_eq!(post.tags.len(), 2);
    assert_eq!(
        tags.get_tag(tenant_id, admin.clone(), shared_tag, "en")
            .await
            .unwrap()
            .use_count,
        1
    );
    let demoted = service
        .demote_post_to_topic(
            tenant_id,
            admin.clone(),
            DemotePostToTopicInput {
                post_id: promoted.target_id,
                locale: "en".into(),
                forum_category_id: category.id,
                reason: None,
                idempotency_key: "demote-with-tags".into(),
            },
        )
        .await
        .unwrap();
    let remaining_global_usage = tags
        .get_tag(tenant_id, admin.clone(), shared_tag, "en")
        .await
        .unwrap();
    assert_eq!(remaining_global_usage.use_count, 0);
    let (remaining, total) = tags
        .list_tags(tenant_id, admin, ListTagsFilter::default())
        .await
        .unwrap();
    assert_eq!(total, 1);
    assert_eq!(remaining[0].slug, "release");
    assert_eq!(remaining[0].use_count, 0);
    let demoted_route = routes
        .resolve_route(tenant_id, "en", "/modules/blog?slug=release-notes")
        .await
        .unwrap()
        .unwrap();
    assert!(demoted_route.redirect_required);
    assert_eq!(demoted_route.target_id, demoted.target_id);
    let txn = db.begin().await.unwrap();
    let mut names = crate::bridge::tags::load_forum_tag_names_for_topic_in_tx(
        &txn,
        tenant_id,
        demoted.target_id,
        "en",
        None,
    )
    .await
    .unwrap();
    names
        .iter_mut()
        .for_each(|name| name.make_ascii_lowercase());
    names.sort();
    assert_eq!(names, ["release", "shared"]);
}

#[tokio::test]
async fn promotion_rejects_topics_with_replies_and_preserves_the_source() {
    let db = setup_conversion_test_db().await;
    ensure_conversion_schema(&db).await;

    let events = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let security = admin_security();
    let tenant_id = Uuid::new_v4();
    insert_test_actor(&db, tenant_id, &security).await;

    let category = CategoryService::new(db.clone())
        .create(
            tenant_id,
            security.clone(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "General".to_string(),
                slug: "general".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await
        .expect("forum category should be created");

    let topic = TopicService::new(db.clone(), events.clone())
        .with_settings_providers(zero_cooldown_providers())
        .create(
            tenant_id,
            security.clone(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category.id,
                title: "Forum thread".to_string(),
                slug: Some("forum-thread".to_string()),
                body: RichTextDocument::single_paragraph("Original forum body"),
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await
        .expect("forum topic should be created");

    let reply_service = ReplyService::new(db.clone(), events.clone())
        .with_settings_providers(zero_cooldown_providers());
    reply_service
        .create(
            tenant_id,
            security.clone(),
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: RichTextDocument::single_paragraph("First reply"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("first reply should be created");
    reply_service
        .create(
            tenant_id,
            security.clone(),
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: RichTextDocument::single_paragraph("Second reply"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("second reply should be created");

    let orchestration = ContentOrchestrationService::new(
        db.clone(),
        events.clone(),
        Arc::new(ServerContentOrchestrationBridge::new(
            db.clone(),
            events.clone(),
        )),
    );
    let error = orchestration
        .promote_topic_to_post(
            tenant_id,
            security,
            PromoteTopicToPostInput {
                topic_id: topic.id,
                locale: "en".into(),
                blog_category_id: None,
                reason: None,
                idempotency_key: "topic-with-replies".into(),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        ContentError::Validation(message)
            if message.contains("until Forum uses the canonical richtext contract")
    ));
    let source = forum_topic::Entity::find_by_id(topic.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(source.reply_count, 2);
}
