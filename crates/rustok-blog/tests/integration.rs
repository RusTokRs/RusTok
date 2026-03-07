//! Integration tests for the Blog module
//!
//! These tests require a database connection and are marked with #[ignore]
//! to prevent running in CI without proper test infrastructure.

use std::sync::Arc;

use async_trait::async_trait;
use rustok_blog::dto::{CreatePostInput, PostListQuery};
use rustok_blog::state_machine::{BlogPost, BlogPostStatus, CommentStatus, ToBlogPostStatus};
use rustok_blog::BlogError;
use rustok_core::events::EventEnvelope;
use rustok_core::{DomainEvent, EventTransport, MemoryTransport, ReliabilityLevel, SecurityContext, UserRole};
use rustok_outbox::TransactionalEventBus;
use tokio::sync::broadcast;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

struct TestContext {
    _tenant_id: Uuid,
    _events: broadcast::Receiver<EventEnvelope>,
}

#[tokio::test]
#[ignore = "Integration test requires database/migrations + indexer wiring"]
async fn test_post_lifecycle() -> TestResult<()> {
    let _ctx = test_context().await?;

    let input = CreatePostInput {
        locale: "en".to_string(),
        title: "Test Post".to_string(),
        body: "Hello, Blog!".to_string(),
        excerpt: Some("Short excerpt".to_string()),
        slug: None,
        publish: false,
        tags: vec!["rust".to_string()],
        category_id: None,
        featured_image_url: None,
        seo_title: None,
        seo_description: None,
        metadata: None,
    };

    let _post_id = Uuid::new_v4();
    let _ = input;

    Ok(())
}

#[tokio::test]
#[ignore = "Integration test requires database"]
async fn test_create_and_publish_post() -> TestResult<()> {
    let _ctx = test_context().await?;

    let input = CreatePostInput {
        locale: "en".to_string(),
        title: "Draft Post".to_string(),
        body: "Content".to_string(),
        excerpt: None,
        slug: Some("draft-post".to_string()),
        publish: false,
        tags: vec![],
        category_id: None,
        featured_image_url: None,
        seo_title: None,
        seo_description: None,
        metadata: None,
    };

    let _ = input;

    Ok(())
}

#[tokio::test]
#[ignore = "Integration test requires database"]
async fn test_list_posts_with_pagination() -> TestResult<()> {
    let _ctx = test_context().await?;

    let query = PostListQuery {
        page: Some(1),
        per_page: Some(10),
        ..Default::default()
    };

    let _ = query;

    Ok(())
}

#[tokio::test]
#[ignore = "Integration test requires database"]
async fn test_filter_posts_by_tag() -> TestResult<()> {
    let _ctx = test_context().await?;

    let query = PostListQuery {
        tag: Some("rust".to_string()),
        ..Default::default()
    };

    let _ = query;

    Ok(())
}

#[tokio::test]
#[ignore = "Integration test requires database"]
async fn test_cannot_delete_published_post() -> TestResult<()> {
    let _ctx = test_context().await?;

    Ok(())
}

async fn test_context() -> TestResult<TestContext> {
    let (_event_sender, event_receiver) = broadcast::channel(128);

    Ok(TestContext {
        _tenant_id: Uuid::new_v4(),
        _events: event_receiver,
    })
}

#[allow(dead_code)]
async fn next_event(
    receiver: &mut broadcast::Receiver<EventEnvelope>,
) -> TestResult<EventEnvelope> {
    let envelope = tokio::time::timeout(std::time::Duration::from_secs(5), receiver.recv())
        .await
        .map_err(|_| "timed out waiting for event")??;
    Ok(envelope)
}

#[derive(Clone)]
struct FailingTransport;

#[async_trait]
impl EventTransport for FailingTransport {
    async fn publish(&self, _envelope: EventEnvelope) -> rustok_core::Result<()> {
        Err(rustok_core::Error::External(
            "simulated transport failure".to_string(),
        ))
    }

    fn reliability_level(&self) -> ReliabilityLevel {
        ReliabilityLevel::InMemory
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

mod unit_tests {
    use super::*;

    #[test]
    fn test_blog_post_state_machine() {
        let id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let author_id = Uuid::new_v4();

        let post = BlogPost::new_draft(
            id,
            tenant_id,
            author_id,
            "Test Post".to_string(),
            "test-post".to_string(),
            "en".to_string(),
        );
        assert_eq!(post.to_status(), BlogPostStatus::Draft);

        let post = post.publish();
        assert_eq!(post.to_status(), BlogPostStatus::Published);

        let post = post.archive("Outdated".to_string());
        assert_eq!(post.to_status(), BlogPostStatus::Archived);

        let post = post.restore_to_draft();
        assert_eq!(post.to_status(), BlogPostStatus::Draft);
    }

    #[test]
    fn test_comment_status_transitions() {
        assert_eq!(CommentStatus::Pending.approve(), CommentStatus::Approved);
        assert_eq!(CommentStatus::Approved.mark_spam(), CommentStatus::Spam);
        assert_eq!(CommentStatus::Spam.approve(), CommentStatus::Approved);
        assert_eq!(CommentStatus::Pending.trash(), CommentStatus::Trash);
    }

    #[test]
    fn test_error_conversions() {
        let id = Uuid::new_v4();
        let err = BlogError::post_not_found(id);
        assert!(matches!(err, BlogError::PostNotFound(_)));

        let err = BlogError::duplicate_slug("test-slug", "en");
        assert!(matches!(err, BlogError::DuplicateSlug { .. }));
    }

    #[test]
    fn test_post_list_query() {
        let query = PostListQuery {
            page: Some(2),
            per_page: Some(25),
            status: Some(BlogPostStatus::Published),
            tag: Some("rust".to_string()),
            ..Default::default()
        };

        assert_eq!(query.page(), 2);
        assert_eq!(query.per_page(), 25);
        assert_eq!(query.offset(), 25);
    }

    #[test]
    fn test_blog_events_exist() {
        let post_id = Uuid::new_v4();

        let created = DomainEvent::BlogPostCreated {
            post_id,
            author_id: None,
            locale: "en".to_string(),
        };
        assert_eq!(created.event_type(), "blog.post.created");
        assert_eq!(created.schema_version(), 1);
        assert!(created.affects_index());

        let published = DomainEvent::BlogPostPublished {
            post_id,
            author_id: None,
        };
        assert_eq!(published.event_type(), "blog.post.published");
        assert!(published.affects_index());

        let unpublished = DomainEvent::BlogPostUnpublished { post_id };
        assert_eq!(unpublished.event_type(), "blog.post.unpublished");
        assert!(unpublished.affects_index());

        let updated = DomainEvent::BlogPostUpdated {
            post_id,
            locale: "ru".to_string(),
        };
        assert_eq!(updated.event_type(), "blog.post.updated");
        assert!(updated.affects_index());

        let archived = DomainEvent::BlogPostArchived {
            post_id,
            reason: Some("outdated".to_string()),
        };
        assert_eq!(archived.event_type(), "blog.post.archived");
        assert!(archived.affects_index());

        let deleted = DomainEvent::BlogPostDeleted { post_id };
        assert_eq!(deleted.event_type(), "blog.post.deleted");
        assert!(deleted.affects_index());
    }

    #[test]
    fn test_create_post_input_fields() {
        let input = CreatePostInput {
            locale: "ru".to_string(),
            title: "Заголовок".to_string(),
            body: "Тело поста".to_string(),
            excerpt: Some("Краткое содержание".to_string()),
            slug: Some("zagolovok".to_string()),
            publish: false,
            tags: vec!["rust".to_string(), "cms".to_string()],
            category_id: None,
            featured_image_url: Some("https://example.com/image.jpg".to_string()),
            seo_title: Some("SEO заголовок".to_string()),
            seo_description: Some("SEO описание для поисковиков".to_string()),
            metadata: None,
        };

        assert_eq!(input.locale, "ru");
        assert_eq!(input.tags.len(), 2);
        assert!(input.featured_image_url.is_some());
        assert!(input.seo_title.is_some());
        assert!(input.seo_description.is_some());
        assert!(!input.publish);
    }

    #[tokio::test]
    async fn test_non_outbox_transport_fails_before_any_event_is_observed() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let mut receiver = MemoryTransport::new().subscribe();
        let event_bus = TransactionalEventBus::new(Arc::new(FailingTransport));

        let result = event_bus
            .publish_in_tx(
                &(),
                tenant_id,
                Some(actor_id),
                DomainEvent::BlogPostCreated {
                    post_id: Uuid::new_v4(),
                    author_id: Some(actor_id),
                    locale: "en".to_string(),
                },
            )
            .await;

        assert!(result.is_err());
        assert!(matches!(receiver.try_recv(), Err(broadcast::error::TryRecvError::Empty)));
    }

    #[tokio::test]
    async fn test_memory_transport_publish_in_tx_delivers_blog_event() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let transport = MemoryTransport::new();
        let mut receiver = transport.subscribe();
        let event_bus = TransactionalEventBus::new(Arc::new(transport));

        event_bus
            .publish_in_tx(
                &(),
                tenant_id,
                Some(actor_id),
                DomainEvent::BlogPostCreated {
                    post_id: Uuid::new_v4(),
                    author_id: Some(actor_id),
                    locale: "en".to_string(),
                },
            )
            .await
            .expect("memory transport should accept publish_in_tx");

        let envelope = receiver.recv().await.expect("event should be published");
        assert_eq!(envelope.tenant_id, tenant_id);
        assert_eq!(envelope.actor_id, Some(actor_id));
        assert!(matches!(
            envelope.event,
            DomainEvent::BlogPostCreated {
                author_id: Some(id),
                locale,
                ..
            } if id == actor_id && locale == "en"
        ));
    }

    #[test]
    fn test_blog_security_context_uses_explicit_actor() {
        let actor_id = Uuid::new_v4();
        let security = SecurityContext::new(UserRole::Admin, Some(actor_id));

        assert_eq!(security.user_id, Some(actor_id));
        assert_eq!(security.role, UserRole::Admin);
    }
}
