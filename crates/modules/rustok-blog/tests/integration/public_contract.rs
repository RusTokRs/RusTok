use std::sync::Arc;

use async_trait::async_trait;
use rustok_blog::BlogError;
use rustok_blog::dto::{CreatePostInput, PostListQuery};
use rustok_blog::state_machine::BlogPostStatus;
use rustok_core::{
    DomainEvent, EventTransport, MemoryTransport, ReliabilityLevel, SecurityContext, UserRole,
};
use rustok_events::EventEnvelope;
use rustok_outbox::TransactionalEventBus;
use tokio::sync::broadcast;
use uuid::Uuid;

#[derive(Clone)]
struct FailingTransport {
    sender: broadcast::Sender<EventEnvelope>,
}

impl FailingTransport {
    fn new() -> Self {
        let (sender, _) = broadcast::channel(16);
        Self { sender }
    }

    fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        self.sender.subscribe()
    }
}

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

#[test]
fn test_blog_post_transition_table() {
    assert!(BlogPostStatus::Draft.can_transition_to(BlogPostStatus::Published));
    assert!(BlogPostStatus::Published.can_transition_to(BlogPostStatus::Draft));
    assert!(BlogPostStatus::Published.can_transition_to(BlogPostStatus::Archived));
    assert!(BlogPostStatus::Archived.can_transition_to(BlogPostStatus::Draft));

    assert!(!BlogPostStatus::Draft.can_transition_to(BlogPostStatus::Archived));
    assert!(!BlogPostStatus::Archived.can_transition_to(BlogPostStatus::Published));
}

#[test]
fn test_error_conversions() {
    let id = Uuid::new_v4();
    let err = BlogError::post_not_found(id);
    assert!(matches!(err, BlogError::PostNotFound(_)));

    let err = BlogError::duplicate_slug("test-slug");
    assert!(matches!(err, BlogError::DuplicateSlug { .. }));
}

#[test]
fn test_post_list_query() {
    let query = PostListQuery {
        after: Some("cursor".to_string()),
        per_page: Some(25),
        status: Some(BlogPostStatus::Published),
        tag: Some("rust".to_string()),
        ..Default::default()
    };

    assert_eq!(query.after.as_deref(), Some("cursor"));
    assert_eq!(query.per_page(), 25);
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
        command_id: uuid::Uuid::new_v4(),
        locale: "ru".to_string(),
        title: "Заголовок".to_string(),
        content: rustok_blog::richtext::article_document_from_plain_text("Тело поста"),
        excerpt: Some("Краткое содержание".to_string()),
        slug: Some("zagolovok".to_string()),
        publish: false,
        tags: vec!["rust".to_string(), "cms".to_string()],
        category_id: None,
        featured_image_url: Some("https://example.com/image.jpg".to_string()),
        seo_title: Some("SEO заголовок".to_string()),
        seo_description: Some("SEO описание для поисковиков".to_string()),
        channel_slugs: None,
        metadata: None,
        scheduled_at: None,
    };

    assert_eq!(input.locale, "ru");
    assert_eq!(input.tags.len(), 2);
    assert!(input.featured_image_url.is_some());
    assert!(input.seo_title.is_some());
    assert!(input.seo_description.is_some());
    assert!(!input.publish);
}

#[tokio::test]
async fn test_publish_returns_error_and_does_not_deliver_event_with_failing_transport() {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let failing_transport = FailingTransport::new();
    let mut receiver = failing_transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(failing_transport));

    let result = event_bus
        .publish(
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
    assert!(matches!(
        receiver.try_recv(),
        Err(broadcast::error::TryRecvError::Empty)
    ));
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), receiver.recv())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn test_publish_delivers_blog_event_with_memory_transport() {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let transport = MemoryTransport::new();
    let mut receiver = transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));

    event_bus
        .publish(
            tenant_id,
            Some(actor_id),
            DomainEvent::BlogPostCreated {
                post_id: Uuid::new_v4(),
                author_id: Some(actor_id),
                locale: "en".to_string(),
            },
        )
        .await
        .expect("memory transport should accept publish");

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
