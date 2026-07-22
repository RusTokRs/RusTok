/// Pure unit tests — no database required.
///
/// These tests cover module-level invariants: constants, DTO defaults, and error
/// message formatting.  They run in CI without any external dependencies.
use rustok_forum::constants::{KIND_CATEGORY, KIND_REPLY, KIND_TOPIC, reply_status, topic_status};
use rustok_forum::dto::{
    CreateCategoryInput, CreateReplyInput, CreateTopicInput, ListTopicsFilter,
};
use rustok_forum::error::ForumError;
use rustok_forum::{ReplyStatus, TopicStatus};
use uuid::Uuid;

// ── Constants ────────────────────────────────────────────────────────────────

#[test]
fn kind_constants_are_namespaced() {
    assert!(KIND_CATEGORY.starts_with("forum_"));
    assert!(KIND_TOPIC.starts_with("forum_"));
    assert!(KIND_REPLY.starts_with("forum_"));
}

#[test]
fn kind_constants_are_distinct() {
    assert_ne!(KIND_CATEGORY, KIND_TOPIC);
    assert_ne!(KIND_TOPIC, KIND_REPLY);
    assert_ne!(KIND_CATEGORY, KIND_REPLY);
}

#[test]
fn topic_status_values() {
    assert_eq!(topic_status::OPEN, "open");
    assert_eq!(topic_status::CLOSED, "closed");
    assert_eq!(topic_status::ARCHIVED, "archived");
}

#[test]
fn reply_status_values() {
    assert_eq!(reply_status::PENDING, "pending");
    assert_eq!(reply_status::APPROVED, "approved");
    assert_eq!(reply_status::REJECTED, "rejected");
    assert_eq!(reply_status::HIDDEN, "hidden");
}

#[test]
fn typed_statuses_preserve_existing_wire_values() {
    assert_eq!(
        serde_json::to_value(TopicStatus::Archived).expect("serialize topic status"),
        "archived"
    );
    assert_eq!(
        serde_json::from_value::<TopicStatus>(serde_json::json!("closed"))
            .expect("deserialize topic status"),
        TopicStatus::Closed
    );
    assert_eq!(
        serde_json::to_value(ReplyStatus::Approved).expect("serialize reply status"),
        "approved"
    );
    assert_eq!(
        serde_json::from_value::<ReplyStatus>(serde_json::json!("flagged"))
            .expect("deserialize reply status"),
        ReplyStatus::Flagged
    );
}

// ── DTO defaults ─────────────────────────────────────────────────────────────

#[test]
fn list_topics_filter_serde_defaults() {
    // serde defaults (page=1, per_page=20) apply during JSON deserialization.
    let filter: ListTopicsFilter = serde_json::from_str("{}").expect("deserialise empty object");
    assert_eq!(filter.page, 1);
    assert_eq!(filter.per_page, 20);
    assert!(filter.category_id.is_none());
    assert!(filter.status.is_none());
    assert!(filter.locale.is_none());
}

#[test]
fn list_topics_filter_optional_fields_absent_by_default() {
    let filter = ListTopicsFilter::default();
    assert!(filter.category_id.is_none());
    assert!(filter.status.is_none());
    assert!(filter.locale.is_none());
}

// ── Error display messages ────────────────────────────────────────────────────

#[test]
fn forum_error_display_category_not_found() {
    let id = Uuid::nil();
    let err = ForumError::CategoryNotFound(id);
    let msg = err.to_string();
    assert!(msg.contains("Category not found"));
    assert!(msg.contains(&id.to_string()));
}

#[test]
fn forum_error_display_topic_not_found() {
    let id = Uuid::nil();
    let err = ForumError::TopicNotFound(id);
    let msg = err.to_string();
    assert!(msg.contains("Topic not found"));
    assert!(msg.contains(&id.to_string()));
}

#[test]
fn forum_error_display_reply_not_found() {
    let id = Uuid::nil();
    let err = ForumError::ReplyNotFound(id);
    let msg = err.to_string();
    assert!(msg.contains("Reply not found"));
    assert!(msg.contains(&id.to_string()));
}

#[test]
fn forum_error_display_topic_closed() {
    let err = ForumError::TopicClosed;
    assert_eq!(err.to_string(), "Topic is closed");
}

#[test]
fn forum_error_display_topic_archived() {
    let err = ForumError::TopicArchived;
    assert_eq!(err.to_string(), "Topic is archived");
}

#[test]
fn forum_error_display_validation() {
    let err = ForumError::Validation("title is required".to_string());
    let msg = err.to_string();
    assert!(msg.contains("Validation error"));
    assert!(msg.contains("title is required"));
}

// ── DTO field structure ───────────────────────────────────────────────────────

#[test]
fn create_topic_input_fields() {
    let input = CreateTopicInput {
        locale: "en".to_string(),
        category_id: Uuid::nil(),
        title: "Hello".to_string(),
        slug: None,
        body: "World".to_string(),
        body_format: "markdown".to_string(),
        content_json: None,
        metadata: serde_json::json!({}),
        tags: vec!["tag1".to_string()],
        channel_slugs: None,
    };
    assert_eq!(input.locale, "en");
    assert_eq!(input.title, "Hello");
    assert_eq!(input.tags.len(), 1);
    assert!(input.slug.is_none());
}

#[test]
fn create_category_input_fields() {
    let input = CreateCategoryInput {
        locale: "en".to_string(),
        name: "General".to_string(),
        slug: "general".to_string(),
        description: Some("For general discussion".to_string()),
        icon: None,
        color: None,
        parent_id: None,
        position: Some(0),
        moderated: false,
    };
    assert_eq!(input.name, "General");
    assert_eq!(input.slug, "general");
    assert!(!input.moderated);
}

#[test]
fn create_reply_input_fields() {
    let input = CreateReplyInput {
        locale: "en".to_string(),
        content: "Nice post!".to_string(),
        content_format: "markdown".to_string(),
        content_json: None,
        parent_reply_id: None,
    };
    assert_eq!(input.content, "Nice post!");
    assert!(input.parent_reply_id.is_none());
}
