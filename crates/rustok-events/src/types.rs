use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ulid::Ulid;
use uuid::Uuid;

use super::validation::{validators, EventValidationError, ValidateEvent};

#[derive(Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub id: Uuid,
    /// Event type string for fast filtering and routing
    pub event_type: String,
    /// Schema version for this event type (for evolution tracking)
    pub schema_version: u16,
    pub correlation_id: Uuid,
    pub causation_id: Option<Uuid>,
    pub tenant_id: Uuid,
    pub trace_id: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub actor_id: Option<Uuid>,
    pub event: DomainEvent,
    pub retry_count: u32,
}

impl EventEnvelope {
    pub fn new(tenant_id: Uuid, actor_id: Option<Uuid>, event: DomainEvent) -> Self {
        let id = Uuid::from_bytes(Ulid::new().to_bytes());
        let event_type = event.event_type().to_string();
        let schema_version = event.schema_version();
        Self {
            id,
            event_type,
            schema_version,
            correlation_id: id,
            causation_id: None,
            tenant_id,
            trace_id: rustok_telemetry::current_trace_id(),
            timestamp: Utc::now(),
            actor_id,
            event,
            retry_count: 0,
        }
    }
}

impl std::fmt::Debug for EventEnvelope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventEnvelope")
            .field("id", &self.id)
            .field("type", &self.event_type)
            .field("tenant_id", &self.tenant_id)
            .field("actor_id", &self.actor_id)
            .field("timestamp", &self.timestamp)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum DomainEvent {
    // ════════════════════════════════════════════════════════════════
    // CONTENT EVENTS (nodes, bodies)
    // ════════════════════════════════════════════════════════════════
    NodeCreated {
        node_id: Uuid,
        kind: String,
        author_id: Option<Uuid>,
    },
    NodeUpdated {
        node_id: Uuid,
        kind: String,
    },
    NodeTranslationUpdated {
        node_id: Uuid,
        locale: String,
    },
    NodePublished {
        node_id: Uuid,
        kind: String,
    },
    NodeUnpublished {
        node_id: Uuid,
        kind: String,
    },
    NodeDeleted {
        node_id: Uuid,
        kind: String,
    },
    BodyUpdated {
        node_id: Uuid,
        locale: String,
    },

    // ════════════════════════════════════════════════════════════════
    // CATEGORY EVENTS
    // ════════════════════════════════════════════════════════════════
    CategoryCreated {
        category_id: Uuid,
    },
    CategoryUpdated {
        category_id: Uuid,
    },
    CategoryDeleted {
        category_id: Uuid,
    },

    // ════════════════════════════════════════════════════════════════
    // TAG EVENTS
    // ════════════════════════════════════════════════════════════════
    TagCreated {
        tag_id: Uuid,
    },
    TagAttached {
        tag_id: Uuid,
        target_type: String,
        target_id: Uuid,
    },
    TagDetached {
        tag_id: Uuid,
        target_type: String,
        target_id: Uuid,
    },

    // ════════════════════════════════════════════════════════════════
    // MEDIA EVENTS
    // ════════════════════════════════════════════════════════════════
    MediaUploaded {
        media_id: Uuid,
        mime_type: String,
        size: i64,
    },
    MediaDeleted {
        media_id: Uuid,
    },

    // ════════════════════════════════════════════════════════════════
    // USER EVENTS
    // ════════════════════════════════════════════════════════════════
    UserRegistered {
        user_id: Uuid,
        email: String,
    },
    UserLoggedIn {
        user_id: Uuid,
    },
    UserUpdated {
        user_id: Uuid,
    },
    ProfileUpdated {
        user_id: Uuid,
        handle: String,
        locale: Option<String>,
    },
    UserDeleted {
        user_id: Uuid,
    },

    // ════════════════════════════════════════════════════════════════
    // COMMERCE EVENTS (для будущего модуля)
    // ════════════════════════════════════════════════════════════════
    ProductCreated {
        product_id: Uuid,
    },
    ProductUpdated {
        product_id: Uuid,
    },
    ProductPublished {
        product_id: Uuid,
    },
    ProductDeleted {
        product_id: Uuid,
    },
    VariantCreated {
        variant_id: Uuid,
        product_id: Uuid,
    },
    VariantUpdated {
        variant_id: Uuid,
        product_id: Uuid,
    },
    VariantDeleted {
        variant_id: Uuid,
        product_id: Uuid,
    },
    InventoryUpdated {
        variant_id: Uuid,
        product_id: Uuid,
        location_id: Uuid,
        old_quantity: i32,
        new_quantity: i32,
    },
    InventoryLow {
        variant_id: Uuid,
        product_id: Uuid,
        remaining: i32,
        threshold: i32,
    },
    PriceUpdated {
        variant_id: Uuid,
        product_id: Uuid,
        currency: String,
        old_amount: Option<i64>,
        new_amount: i64,
    },
    OrderPlaced {
        order_id: Uuid,
        customer_id: Option<Uuid>,
        total: i64,
        currency: String,
    },
    OrderStatusChanged {
        order_id: Uuid,
        old_status: String,
        new_status: String,
    },
    OrderCompleted {
        order_id: Uuid,
    },
    OrderCancelled {
        order_id: Uuid,
        reason: Option<String>,
    },

    // ════════════════════════════════════════════════════════════════
    // INDEX EVENTS (CQRS)
    // ════════════════════════════════════════════════════════════════
    ReindexRequested {
        target_type: String,
        target_id: Option<Uuid>,
    },
    IndexUpdated {
        index_name: String,
        target_id: Uuid,
    },

    // ════════════════════════════════════════════════════════════════
    // BUILD EVENTS
    // ════════════════════════════════════════════════════════════════
    BuildRequested {
        build_id: Uuid,
        requested_by: String,
    },

    // ════════════════════════════════════════════════════════════════
    // BLOG EVENTS
    // ════════════════════════════════════════════════════════════════
    BlogPostCreated {
        post_id: Uuid,
        author_id: Option<Uuid>,
        locale: String,
    },
    BlogPostPublished {
        post_id: Uuid,
        author_id: Option<Uuid>,
    },
    BlogPostUnpublished {
        post_id: Uuid,
    },
    BlogPostUpdated {
        post_id: Uuid,
        locale: String,
    },
    BlogPostArchived {
        post_id: Uuid,
        reason: Option<String>,
    },
    BlogPostDeleted {
        post_id: Uuid,
    },

    // ════════════════════════════════════════════════════════════════
    // FORUM EVENTS
    // ════════════════════════════════════════════════════════════════
    ForumTopicCreated {
        topic_id: Uuid,
        category_id: Uuid,
        author_id: Option<Uuid>,
        locale: String,
    },
    ForumTopicReplied {
        topic_id: Uuid,
        reply_id: Uuid,
        author_id: Option<Uuid>,
    },
    ForumTopicStatusChanged {
        topic_id: Uuid,
        old_status: String,
        new_status: String,
        moderator_id: Option<Uuid>,
    },
    ForumTopicPinned {
        topic_id: Uuid,
        is_pinned: bool,
        moderator_id: Option<Uuid>,
    },
    ForumReplyStatusChanged {
        reply_id: Uuid,
        topic_id: Uuid,
        new_status: String,
        moderator_id: Option<Uuid>,
    },

    // Content orchestration events
    TopicPromotedToPost {
        topic_id: Uuid,
        post_id: Uuid,
        moved_comments: u64,
        locale: String,
        reason: Option<String>,
    },
    PostDemotedToTopic {
        post_id: Uuid,
        topic_id: Uuid,
        moved_comments: u64,
        locale: String,
        reason: Option<String>,
    },
    TopicSplit {
        source_topic_id: Uuid,
        target_topic_id: Uuid,
        moved_comment_ids: Vec<Uuid>,
        moved_comments: u64,
        reason: Option<String>,
    },
    TopicsMerged {
        target_topic_id: Uuid,
        moved_comments: u64,
        reason: Option<String>,
    },

    // ════════════════════════════════════════════════════════════════
    // TENANT EVENTS
    // ════════════════════════════════════════════════════════════════
    TenantCreated {
        tenant_id: Uuid,
    },
    TenantUpdated {
        tenant_id: Uuid,
    },
    LocaleEnabled {
        tenant_id: Uuid,
        locale: String,
    },
    LocaleDisabled {
        tenant_id: Uuid,
        locale: String,
    },
    PlatformSettingsChanged {
        category: String,
        changed_by: Uuid,
    },
    SearchSettingsChanged {
        active_engine: String,
        fallback_engine: String,
        changed_by: Uuid,
    },
    SearchRebuildQueued {
        target_type: String,
        target_id: Option<Uuid>,
        queued_by: Uuid,
    },

    // ════════════════════════════════════════════════════════════════
    // FLEX — FIELD DEFINITION EVENTS
    // ════════════════════════════════════════════════════════════════
    FieldDefinitionCreated {
        tenant_id: Uuid,
        /// Entity type key, e.g. "user", "product", "node".
        entity_type: String,
        field_key: String,
        field_type: String,
    },
    FieldDefinitionUpdated {
        tenant_id: Uuid,
        entity_type: String,
        field_key: String,
    },
    FieldDefinitionDeleted {
        tenant_id: Uuid,
        entity_type: String,
        field_key: String,
    },
    FlexSchemaCreated {
        tenant_id: Uuid,
        schema_id: Uuid,
        slug: String,
    },
    FlexSchemaUpdated {
        tenant_id: Uuid,
        schema_id: Uuid,
        slug: String,
    },
    FlexSchemaDeleted {
        tenant_id: Uuid,
        schema_id: Uuid,
    },
    FlexEntryCreated {
        tenant_id: Uuid,
        schema_id: Uuid,
        entry_id: Uuid,
        entity_type: Option<String>,
        entity_id: Option<Uuid>,
    },
    FlexEntryUpdated {
        tenant_id: Uuid,
        schema_id: Uuid,
        entry_id: Uuid,
    },
    FlexEntryDeleted {
        tenant_id: Uuid,
        schema_id: Uuid,
        entry_id: Uuid,
    },
}

impl DomainEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::NodeCreated { .. } => "node.created",
            Self::NodeUpdated { .. } => "node.updated",
            Self::NodeTranslationUpdated { .. } => "node.translation.updated",
            Self::NodePublished { .. } => "node.published",
            Self::NodeUnpublished { .. } => "node.unpublished",
            Self::NodeDeleted { .. } => "node.deleted",
            Self::BodyUpdated { .. } => "body.updated",

            Self::CategoryCreated { .. } => "category.created",
            Self::CategoryUpdated { .. } => "category.updated",
            Self::CategoryDeleted { .. } => "category.deleted",

            Self::TagCreated { .. } => "tag.created",
            Self::TagAttached { .. } => "tag.attached",
            Self::TagDetached { .. } => "tag.detached",

            Self::MediaUploaded { .. } => "media.uploaded",
            Self::MediaDeleted { .. } => "media.deleted",

            Self::UserRegistered { .. } => "user.registered",
            Self::UserLoggedIn { .. } => "user.logged_in",
            Self::UserUpdated { .. } => "user.updated",
            Self::ProfileUpdated { .. } => "profile.updated",
            Self::UserDeleted { .. } => "user.deleted",

            Self::ProductCreated { .. } => "product.created",
            Self::ProductUpdated { .. } => "product.updated",
            Self::ProductPublished { .. } => "product.published",
            Self::ProductDeleted { .. } => "product.deleted",
            Self::VariantCreated { .. } => "variant.created",
            Self::VariantUpdated { .. } => "variant.updated",
            Self::VariantDeleted { .. } => "variant.deleted",
            Self::InventoryUpdated { .. } => "inventory.updated",
            Self::InventoryLow { .. } => "inventory.low",
            Self::PriceUpdated { .. } => "price.updated",
            Self::OrderPlaced { .. } => "order.placed",
            Self::OrderStatusChanged { .. } => "order.status_changed",
            Self::OrderCompleted { .. } => "order.completed",
            Self::OrderCancelled { .. } => "order.cancelled",

            Self::ReindexRequested { .. } => "index.reindex_requested",
            Self::IndexUpdated { .. } => "index.updated",

            Self::BuildRequested { .. } => "build.requested",

            Self::BlogPostCreated { .. } => "blog.post.created",
            Self::BlogPostPublished { .. } => "blog.post.published",
            Self::BlogPostUnpublished { .. } => "blog.post.unpublished",
            Self::BlogPostUpdated { .. } => "blog.post.updated",
            Self::BlogPostArchived { .. } => "blog.post.archived",
            Self::BlogPostDeleted { .. } => "blog.post.deleted",

            Self::ForumTopicCreated { .. } => "forum.topic.created",
            Self::ForumTopicReplied { .. } => "forum.topic.replied",
            Self::ForumTopicStatusChanged { .. } => "forum.topic.status_changed",
            Self::ForumTopicPinned { .. } => "forum.topic.pinned",
            Self::ForumReplyStatusChanged { .. } => "forum.reply.status_changed",
            Self::TopicPromotedToPost { .. } => "content.topic.promoted_to_post",
            Self::PostDemotedToTopic { .. } => "content.post.demoted_to_topic",
            Self::TopicSplit { .. } => "content.topic.split",
            Self::TopicsMerged { .. } => "content.topics.merged",

            Self::TenantCreated { .. } => "tenant.created",
            Self::TenantUpdated { .. } => "tenant.updated",
            Self::LocaleEnabled { .. } => "locale.enabled",
            Self::LocaleDisabled { .. } => "locale.disabled",
            Self::PlatformSettingsChanged { .. } => "platform_settings.changed",
            Self::SearchSettingsChanged { .. } => "search.settings_changed",
            Self::SearchRebuildQueued { .. } => "search.rebuild_queued",

            // Flex field definition events
            Self::FieldDefinitionCreated { .. } => "field_definition.created",
            Self::FieldDefinitionUpdated { .. } => "field_definition.updated",
            Self::FieldDefinitionDeleted { .. } => "field_definition.deleted",
            Self::FlexSchemaCreated { .. } => "flex.schema.created",
            Self::FlexSchemaUpdated { .. } => "flex.schema.updated",
            Self::FlexSchemaDeleted { .. } => "flex.schema.deleted",
            Self::FlexEntryCreated { .. } => "flex.entry.created",
            Self::FlexEntryUpdated { .. } => "flex.entry.updated",
            Self::FlexEntryDeleted { .. } => "flex.entry.deleted",
        }
    }

    /// Returns the schema version for this event type.
    /// Increment this version when making breaking changes to the event structure.
    ///
    /// Version History:
    /// - v1: Initial schema for all events
    pub fn schema_version(&self) -> u16 {
        match self {
            // Content events (v1)
            Self::NodeCreated { .. } => 1,
            Self::NodeUpdated { .. } => 1,
            Self::NodeTranslationUpdated { .. } => 1,
            Self::NodePublished { .. } => 1,
            Self::NodeUnpublished { .. } => 1,
            Self::NodeDeleted { .. } => 1,
            Self::BodyUpdated { .. } => 1,

            // Category events (v1)
            Self::CategoryCreated { .. } => 1,
            Self::CategoryUpdated { .. } => 1,
            Self::CategoryDeleted { .. } => 1,

            // Tag events (v1)
            Self::TagCreated { .. } => 1,
            Self::TagAttached { .. } => 1,
            Self::TagDetached { .. } => 1,

            // Media events (v1)
            Self::MediaUploaded { .. } => 1,
            Self::MediaDeleted { .. } => 1,

            // User events (v1)
            Self::UserRegistered { .. } => 1,
            Self::UserLoggedIn { .. } => 1,
            Self::UserUpdated { .. } => 1,
            Self::ProfileUpdated { .. } => 1,
            Self::UserDeleted { .. } => 1,

            // Commerce events (v1)
            Self::ProductCreated { .. } => 1,
            Self::ProductUpdated { .. } => 1,
            Self::ProductPublished { .. } => 1,
            Self::ProductDeleted { .. } => 1,
            Self::VariantCreated { .. } => 1,
            Self::VariantUpdated { .. } => 1,
            Self::VariantDeleted { .. } => 1,
            Self::InventoryUpdated { .. } => 1,
            Self::InventoryLow { .. } => 1,
            Self::PriceUpdated { .. } => 1,
            Self::OrderPlaced { .. } => 1,
            Self::OrderStatusChanged { .. } => 1,
            Self::OrderCompleted { .. } => 1,
            Self::OrderCancelled { .. } => 1,

            // Index events (v1)
            Self::ReindexRequested { .. } => 1,
            Self::IndexUpdated { .. } => 1,

            // Build events (v1)
            Self::BuildRequested { .. } => 1,

            // Blog events (v1)
            Self::BlogPostCreated { .. } => 1,
            Self::BlogPostPublished { .. } => 1,
            Self::BlogPostUnpublished { .. } => 1,
            Self::BlogPostUpdated { .. } => 1,
            Self::BlogPostArchived { .. } => 1,
            Self::BlogPostDeleted { .. } => 1,

            // Forum events (v1)
            Self::ForumTopicCreated { .. } => 1,
            Self::ForumTopicReplied { .. } => 1,
            Self::ForumTopicStatusChanged { .. } => 1,
            Self::ForumTopicPinned { .. } => 1,
            Self::ForumReplyStatusChanged { .. } => 1,
            Self::TopicPromotedToPost { .. } => 1,
            Self::PostDemotedToTopic { .. } => 1,
            Self::TopicSplit { .. } => 1,
            Self::TopicsMerged { .. } => 1,

            // Tenant events (v1)
            Self::TenantCreated { .. } => 1,
            Self::TenantUpdated { .. } => 1,
            Self::LocaleEnabled { .. } => 1,
            Self::LocaleDisabled { .. } => 1,
            Self::PlatformSettingsChanged { .. } => 1,
            Self::SearchSettingsChanged { .. } => 1,
            Self::SearchRebuildQueued { .. } => 1,

            // Flex field definition events (v1)
            Self::FieldDefinitionCreated { .. } => 1,
            Self::FieldDefinitionUpdated { .. } => 1,
            Self::FieldDefinitionDeleted { .. } => 1,
            Self::FlexSchemaCreated { .. } => 1,
            Self::FlexSchemaUpdated { .. } => 1,
            Self::FlexSchemaDeleted { .. } => 1,
            Self::FlexEntryCreated { .. } => 1,
            Self::FlexEntryUpdated { .. } => 1,
            Self::FlexEntryDeleted { .. } => 1,
        }
    }

    pub fn affects_index(&self) -> bool {
        matches!(
            self,
            Self::NodeCreated { .. }
                | Self::NodeUpdated { .. }
                | Self::NodeTranslationUpdated { .. }
                | Self::NodePublished { .. }
                | Self::NodeUnpublished { .. }
                | Self::NodeDeleted { .. }
                | Self::BodyUpdated { .. }
                | Self::ProductCreated { .. }
                | Self::ProductUpdated { .. }
                | Self::ProductPublished { .. }
                | Self::ProductDeleted { .. }
                | Self::VariantUpdated { .. }
                | Self::InventoryUpdated { .. }
                | Self::PriceUpdated { .. }
                | Self::TagAttached { .. }
                | Self::TagDetached { .. }
                | Self::ProfileUpdated { .. }
                | Self::BlogPostCreated { .. }
                | Self::BlogPostPublished { .. }
                | Self::BlogPostUnpublished { .. }
                | Self::BlogPostUpdated { .. }
                | Self::BlogPostArchived { .. }
                | Self::BlogPostDeleted { .. }
                | Self::ForumTopicCreated { .. }
                | Self::ForumTopicReplied { .. }
                | Self::ForumTopicStatusChanged { .. }
        )
    }
}

impl ValidateEvent for DomainEvent {
    /// Validates the event data according to business rules using the validation framework.
    /// Returns Ok(()) if valid, or EventValidationError if invalid.
    fn validate(&self) -> Result<(), EventValidationError> {
        match self {
            // ════════════════════════════════════════════════════════════════
            // CONTENT EVENTS
            // ════════════════════════════════════════════════════════════════
            Self::NodeCreated {
                node_id,
                kind,
                author_id,
            } => {
                validators::validate_not_nil_uuid("node_id", node_id)?;
                validators::validate_not_empty("kind", kind)?;
                validators::validate_max_length("kind", kind, 64)?;
                validators::validate_alphanumeric_with_dash("kind", kind)?;
                validators::validate_optional_uuid("author_id", author_id)?;
                Ok(())
            }
            Self::NodeUpdated { node_id, kind } => {
                validators::validate_not_nil_uuid("node_id", node_id)?;
                validators::validate_not_empty("kind", kind)?;
                validators::validate_max_length("kind", kind, 64)?;
                Ok(())
            }
            Self::NodeTranslationUpdated { node_id, locale } => {
                validators::validate_not_nil_uuid("node_id", node_id)?;
                validators::validate_not_empty("locale", locale)?;
                validators::validate_max_length("locale", locale, 10)?;
                Ok(())
            }
            Self::NodePublished { node_id, kind }
            | Self::NodeUnpublished { node_id, kind }
            | Self::NodeDeleted { node_id, kind } => {
                validators::validate_not_nil_uuid("node_id", node_id)?;
                validators::validate_not_empty("kind", kind)?;
                validators::validate_max_length("kind", kind, 64)?;
                Ok(())
            }
            Self::BodyUpdated { node_id, locale } => {
                validators::validate_not_nil_uuid("node_id", node_id)?;
                validators::validate_not_empty("locale", locale)?;
                validators::validate_max_length("locale", locale, 10)?;
                Ok(())
            }

            // ════════════════════════════════════════════════════════════════
            // CATEGORY EVENTS
            // ════════════════════════════════════════════════════════════════
            Self::CategoryCreated { category_id }
            | Self::CategoryUpdated { category_id }
            | Self::CategoryDeleted { category_id } => {
                validators::validate_not_nil_uuid("category_id", category_id)?;
                Ok(())
            }

            // ════════════════════════════════════════════════════════════════
            // TAG EVENTS
            // ════════════════════════════════════════════════════════════════
            Self::TagCreated { tag_id } => {
                validators::validate_not_nil_uuid("tag_id", tag_id)?;
                Ok(())
            }
            Self::TagAttached {
                tag_id,
                target_type,
                target_id,
            }
            | Self::TagDetached {
                tag_id,
                target_type,
                target_id,
            } => {
                validators::validate_not_nil_uuid("tag_id", tag_id)?;
                validators::validate_not_empty("target_type", target_type)?;
                validators::validate_max_length("target_type", target_type, 64)?;
                validators::validate_not_nil_uuid("target_id", target_id)?;
                Ok(())
            }

            // ════════════════════════════════════════════════════════════════
            // MEDIA EVENTS
            // ════════════════════════════════════════════════════════════════
            Self::MediaUploaded {
                media_id,
                mime_type,
                size,
            } => {
                validators::validate_not_nil_uuid("media_id", media_id)?;
                validators::validate_not_empty("mime_type", mime_type)?;
                validators::validate_max_length("mime_type", mime_type, 255)?;
                if !mime_type.contains('/') {
                    return Err(EventValidationError::InvalidValue(
                        "mime_type",
                        "must be in format 'type/subtype'".to_string(),
                    ));
                }
                validators::validate_range("size", *size, 0, i64::MAX)?;
                Ok(())
            }
            Self::MediaDeleted { media_id } => {
                validators::validate_not_nil_uuid("media_id", media_id)?;
                Ok(())
            }

            // ════════════════════════════════════════════════════════════════
            // USER EVENTS
            // ════════════════════════════════════════════════════════════════
            Self::UserRegistered { user_id, email } => {
                validators::validate_not_nil_uuid("user_id", user_id)?;
                validators::validate_not_empty("email", email)?;
                validators::validate_max_length("email", email, 255)?;
                // Basic email validation
                if !email.contains('@') || !email.contains('.') {
                    return Err(EventValidationError::InvalidValue(
                        "email",
                        "invalid email format".to_string(),
                    ));
                }
                Ok(())
            }
            Self::UserLoggedIn { user_id }
            | Self::UserUpdated { user_id }
            | Self::UserDeleted { user_id } => {
                validators::validate_not_nil_uuid("user_id", user_id)?;
                Ok(())
            }
            Self::ProfileUpdated {
                user_id,
                handle,
                locale,
            } => {
                validators::validate_not_nil_uuid("user_id", user_id)?;
                validators::validate_not_empty("handle", handle)?;
                validators::validate_max_length("handle", handle, 64)?;
                if let Some(locale) = locale {
                    validators::validate_not_empty("locale", locale)?;
                    validators::validate_max_length("locale", locale, 16)?;
                }
                Ok(())
            }

            // ════════════════════════════════════════════════════════════════
            // COMMERCE EVENTS - Products
            // ════════════════════════════════════════════════════════════════
            Self::ProductCreated { product_id }
            | Self::ProductUpdated { product_id }
            | Self::ProductPublished { product_id }
            | Self::ProductDeleted { product_id } => {
                validators::validate_not_nil_uuid("product_id", product_id)?;
                Ok(())
            }

            // ════════════════════════════════════════════════════════════════
            // COMMERCE EVENTS - Variants
            // ════════════════════════════════════════════════════════════════
            Self::VariantCreated {
                variant_id,
                product_id,
            }
            | Self::VariantUpdated {
                variant_id,
                product_id,
            }
            | Self::VariantDeleted {
                variant_id,
                product_id,
            } => {
                validators::validate_not_nil_uuid("variant_id", variant_id)?;
                validators::validate_not_nil_uuid("product_id", product_id)?;
                Ok(())
            }

            // ════════════════════════════════════════════════════════════════
            // COMMERCE EVENTS - Inventory
            // ════════════════════════════════════════════════════════════════
            Self::InventoryUpdated {
                variant_id,
                product_id,
                location_id,
                old_quantity,
                new_quantity,
            } => {
                validators::validate_not_nil_uuid("variant_id", variant_id)?;
                validators::validate_not_nil_uuid("product_id", product_id)?;
                validators::validate_not_nil_uuid("location_id", location_id)?;
                validators::validate_range("old_quantity", *old_quantity as i64, 0, i64::MAX)?;
                validators::validate_range("new_quantity", *new_quantity as i64, 0, i64::MAX)?;
                Ok(())
            }
            Self::InventoryLow {
                variant_id,
                product_id,
                remaining,
                threshold,
            } => {
                validators::validate_not_nil_uuid("variant_id", variant_id)?;
                validators::validate_not_nil_uuid("product_id", product_id)?;
                validators::validate_range("remaining", *remaining as i64, 0, i64::MAX)?;
                validators::validate_range("threshold", *threshold as i64, 0, i64::MAX)?;
                if remaining >= threshold {
                    return Err(EventValidationError::InvalidValue(
                        "remaining",
                        "must be less than threshold for low inventory".to_string(),
                    ));
                }
                Ok(())
            }

            // ════════════════════════════════════════════════════════════════
            // COMMERCE EVENTS - Pricing
            // ════════════════════════════════════════════════════════════════
            Self::PriceUpdated {
                variant_id,
                product_id,
                currency,
                old_amount,
                new_amount,
            } => {
                validators::validate_not_nil_uuid("variant_id", variant_id)?;
                validators::validate_not_nil_uuid("product_id", product_id)?;
                validators::validate_currency_code("currency", currency)?;
                if let Some(old) = old_amount {
                    validators::validate_range("old_amount", *old, 0, i64::MAX)?;
                }
                validators::validate_range("new_amount", *new_amount, 0, i64::MAX)?;
                Ok(())
            }

            // ════════════════════════════════════════════════════════════════
            // COMMERCE EVENTS - Orders
            // ════════════════════════════════════════════════════════════════
            Self::OrderPlaced {
                order_id,
                customer_id,
                total,
                currency,
            } => {
                validators::validate_not_nil_uuid("order_id", order_id)?;
                validators::validate_optional_uuid("customer_id", customer_id)?;
                validators::validate_range("total", *total, 0, i64::MAX)?;
                validators::validate_currency_code("currency", currency)?;
                Ok(())
            }
            Self::OrderStatusChanged {
                order_id,
                old_status,
                new_status,
            } => {
                validators::validate_not_nil_uuid("order_id", order_id)?;
                validators::validate_not_empty("old_status", old_status)?;
                validators::validate_max_length("old_status", old_status, 50)?;
                validators::validate_not_empty("new_status", new_status)?;
                validators::validate_max_length("new_status", new_status, 50)?;
                if old_status == new_status {
                    return Err(EventValidationError::InvalidValue(
                        "new_status",
                        "must be different from old_status".to_string(),
                    ));
                }
                Ok(())
            }
            Self::OrderCompleted { order_id } => {
                validators::validate_not_nil_uuid("order_id", order_id)?;
                Ok(())
            }
            Self::OrderCancelled { order_id, reason } => {
                validators::validate_not_nil_uuid("order_id", order_id)?;
                if let Some(r) = reason {
                    validators::validate_max_length("reason", r, 500)?;
                }
                Ok(())
            }

            // ════════════════════════════════════════════════════════════════
            // INDEX EVENTS
            // ════════════════════════════════════════════════════════════════
            Self::ReindexRequested {
                target_type,
                target_id,
            } => {
                validators::validate_not_empty("target_type", target_type)?;
                validators::validate_max_length("target_type", target_type, 64)?;
                validators::validate_optional_uuid("target_id", target_id)?;
                Ok(())
            }
            Self::IndexUpdated {
                index_name,
                target_id,
            } => {
                validators::validate_not_empty("index_name", index_name)?;
                validators::validate_max_length("index_name", index_name, 64)?;
                validators::validate_not_nil_uuid("target_id", target_id)?;
                Ok(())
            }

            // ════════════════════════════════════════════════════════════════
            // BUILD EVENTS
            // ════════════════════════════════════════════════════════════════
            Self::BuildRequested {
                build_id,
                requested_by,
            } => {
                validators::validate_not_nil_uuid("build_id", build_id)?;
                validators::validate_not_empty("requested_by", requested_by)?;
                validators::validate_max_length("requested_by", requested_by, 255)?;
                Ok(())
            }

            // ════════════════════════════════════════════════════════════════
            // BLOG EVENTS
            // ════════════════════════════════════════════════════════════════
            Self::BlogPostCreated {
                post_id,
                author_id,
                locale,
            } => {
                validators::validate_not_nil_uuid("post_id", post_id)?;
                validators::validate_optional_uuid("author_id", author_id)?;
                validators::validate_not_empty("locale", locale)?;
                validators::validate_max_length("locale", locale, 10)?;
                Ok(())
            }
            Self::BlogPostPublished { post_id, author_id } => {
                validators::validate_not_nil_uuid("post_id", post_id)?;
                validators::validate_optional_uuid("author_id", author_id)?;
                Ok(())
            }
            Self::BlogPostUnpublished { post_id } | Self::BlogPostDeleted { post_id } => {
                validators::validate_not_nil_uuid("post_id", post_id)?;
                Ok(())
            }
            Self::BlogPostUpdated { post_id, locale } => {
                validators::validate_not_nil_uuid("post_id", post_id)?;
                validators::validate_not_empty("locale", locale)?;
                validators::validate_max_length("locale", locale, 10)?;
                Ok(())
            }
            Self::BlogPostArchived { post_id, reason } => {
                validators::validate_not_nil_uuid("post_id", post_id)?;
                if let Some(r) = reason {
                    validators::validate_max_length("reason", r, 500)?;
                }
                Ok(())
            }

            // ════════════════════════════════════════════════════════════════
            // FORUM EVENTS
            // ════════════════════════════════════════════════════════════════
            Self::ForumTopicCreated {
                topic_id,
                category_id,
                author_id,
                locale,
            } => {
                validators::validate_not_nil_uuid("topic_id", topic_id)?;
                validators::validate_not_nil_uuid("category_id", category_id)?;
                validators::validate_optional_uuid("author_id", author_id)?;
                validators::validate_not_empty("locale", locale)?;
                validators::validate_max_length("locale", locale, 10)?;
                Ok(())
            }
            Self::ForumTopicReplied {
                topic_id,
                reply_id,
                author_id,
            } => {
                validators::validate_not_nil_uuid("topic_id", topic_id)?;
                validators::validate_not_nil_uuid("reply_id", reply_id)?;
                validators::validate_optional_uuid("author_id", author_id)?;
                Ok(())
            }
            Self::ForumTopicStatusChanged {
                topic_id,
                old_status,
                new_status,
                moderator_id,
            } => {
                validators::validate_not_nil_uuid("topic_id", topic_id)?;
                validators::validate_not_empty("old_status", old_status)?;
                validators::validate_max_length("old_status", old_status, 50)?;
                validators::validate_not_empty("new_status", new_status)?;
                validators::validate_max_length("new_status", new_status, 50)?;
                if old_status == new_status {
                    return Err(EventValidationError::InvalidValue(
                        "new_status",
                        "must be different from old_status".to_string(),
                    ));
                }
                validators::validate_optional_uuid("moderator_id", moderator_id)?;
                Ok(())
            }
            Self::ForumTopicPinned {
                topic_id,
                moderator_id,
                ..
            } => {
                validators::validate_not_nil_uuid("topic_id", topic_id)?;
                validators::validate_optional_uuid("moderator_id", moderator_id)?;
                Ok(())
            }
            Self::ForumReplyStatusChanged {
                reply_id,
                topic_id,
                new_status,
                moderator_id,
            } => {
                validators::validate_not_nil_uuid("reply_id", reply_id)?;
                validators::validate_not_nil_uuid("topic_id", topic_id)?;
                validators::validate_not_empty("new_status", new_status)?;
                validators::validate_max_length("new_status", new_status, 50)?;
                validators::validate_optional_uuid("moderator_id", moderator_id)?;
                Ok(())
            }
            Self::TopicPromotedToPost {
                topic_id,
                post_id,
                locale,
                reason,
                ..
            } => {
                validators::validate_not_nil_uuid("topic_id", topic_id)?;
                validators::validate_not_nil_uuid("post_id", post_id)?;
                validators::validate_not_empty("locale", locale)?;
                validators::validate_max_length("locale", locale, 10)?;
                if let Some(reason) = reason {
                    validators::validate_max_length("reason", reason, 500)?;
                }
                Ok(())
            }
            Self::PostDemotedToTopic {
                post_id,
                topic_id,
                locale,
                reason,
                ..
            } => {
                validators::validate_not_nil_uuid("post_id", post_id)?;
                validators::validate_not_nil_uuid("topic_id", topic_id)?;
                validators::validate_not_empty("locale", locale)?;
                validators::validate_max_length("locale", locale, 10)?;
                if let Some(reason) = reason {
                    validators::validate_max_length("reason", reason, 500)?;
                }
                Ok(())
            }
            Self::TopicSplit {
                source_topic_id,
                target_topic_id,
                moved_comment_ids,
                reason,
                ..
            } => {
                validators::validate_not_nil_uuid("source_topic_id", source_topic_id)?;
                validators::validate_not_nil_uuid("target_topic_id", target_topic_id)?;
                if moved_comment_ids.is_empty() {
                    return Err(EventValidationError::InvalidValue(
                        "moved_comment_ids",
                        "must not be empty".to_string(),
                    ));
                }
                for id in moved_comment_ids {
                    validators::validate_not_nil_uuid("moved_comment_ids[]", id)?;
                }
                if let Some(reason) = reason {
                    validators::validate_max_length("reason", reason, 500)?;
                }
                Ok(())
            }
            Self::TopicsMerged {
                target_topic_id,
                reason,
                ..
            } => {
                validators::validate_not_nil_uuid("target_topic_id", target_topic_id)?;
                if let Some(reason) = reason {
                    validators::validate_max_length("reason", reason, 500)?;
                }
                Ok(())
            }

            // ════════════════════════════════════════════════════════════════
            // TENANT EVENTS
            // ════════════════════════════════════════════════════════════════
            Self::TenantCreated { tenant_id } | Self::TenantUpdated { tenant_id } => {
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                Ok(())
            }
            Self::LocaleEnabled { tenant_id, locale }
            | Self::LocaleDisabled { tenant_id, locale } => {
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                validators::validate_not_empty("locale", locale)?;
                validators::validate_max_length("locale", locale, 10)?;
                Ok(())
            }
            Self::PlatformSettingsChanged {
                category,
                changed_by,
            } => {
                validators::validate_not_nil_uuid("changed_by", changed_by)?;
                validators::validate_not_empty("category", category)?;
                validators::validate_max_length("category", category, 64)?;
                Ok(())
            }
            Self::SearchSettingsChanged {
                active_engine,
                fallback_engine,
                changed_by,
            } => {
                validators::validate_not_nil_uuid("changed_by", changed_by)?;
                validators::validate_not_empty("active_engine", active_engine)?;
                validators::validate_max_length("active_engine", active_engine, 64)?;
                validators::validate_not_empty("fallback_engine", fallback_engine)?;
                validators::validate_max_length("fallback_engine", fallback_engine, 64)?;
                Ok(())
            }
            Self::SearchRebuildQueued {
                target_type,
                target_id,
                queued_by,
            } => {
                validators::validate_not_nil_uuid("queued_by", queued_by)?;
                validators::validate_not_empty("target_type", target_type)?;
                validators::validate_max_length("target_type", target_type, 64)?;
                validators::validate_optional_uuid("target_id", target_id)?;
                Ok(())
            }

            // ════════════════════════════════════════════════════════════════
            // FLEX FIELD DEFINITION EVENTS
            // ════════════════════════════════════════════════════════════════
            Self::FieldDefinitionCreated {
                tenant_id,
                entity_type,
                field_key,
                field_type,
            } => {
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                validators::validate_not_empty("entity_type", entity_type)?;
                validators::validate_max_length("entity_type", entity_type, 64)?;
                validators::validate_not_empty("field_key", field_key)?;
                validators::validate_max_length("field_key", field_key, 128)?;
                validators::validate_not_empty("field_type", field_type)?;
                Ok(())
            }
            Self::FieldDefinitionUpdated {
                tenant_id,
                entity_type,
                field_key,
            }
            | Self::FieldDefinitionDeleted {
                tenant_id,
                entity_type,
                field_key,
            } => {
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                validators::validate_not_empty("entity_type", entity_type)?;
                validators::validate_max_length("entity_type", entity_type, 64)?;
                validators::validate_not_empty("field_key", field_key)?;
                validators::validate_max_length("field_key", field_key, 128)?;
                Ok(())
            }
            Self::FlexSchemaCreated {
                tenant_id,
                schema_id,
                slug,
            }
            | Self::FlexSchemaUpdated {
                tenant_id,
                schema_id,
                slug,
            } => {
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                validators::validate_not_nil_uuid("schema_id", schema_id)?;
                validators::validate_not_empty("slug", slug)?;
                validators::validate_max_length("slug", slug, 64)?;
                Ok(())
            }
            Self::FlexSchemaDeleted {
                tenant_id,
                schema_id,
            } => {
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                validators::validate_not_nil_uuid("schema_id", schema_id)?;
                Ok(())
            }
            Self::FlexEntryCreated {
                tenant_id,
                schema_id,
                entry_id,
                entity_type,
                entity_id,
            } => {
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                validators::validate_not_nil_uuid("schema_id", schema_id)?;
                validators::validate_not_nil_uuid("entry_id", entry_id)?;

                match (entity_type, entity_id) {
                    (Some(entity_type), Some(entity_id)) => {
                        validators::validate_not_empty("entity_type", entity_type)?;
                        validators::validate_max_length("entity_type", entity_type, 64)?;
                        validators::validate_not_nil_uuid("entity_id", entity_id)?;
                    }
                    (None, None) => {}
                    _ => {
                        return Err(EventValidationError::InvalidValue(
                            "entity_binding",
                            "entity_type and entity_id must be provided together".to_string(),
                        ));
                    }
                }

                Ok(())
            }
            Self::FlexEntryUpdated {
                tenant_id,
                schema_id,
                entry_id,
            }
            | Self::FlexEntryDeleted {
                tenant_id,
                schema_id,
                entry_id,
            } => {
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                validators::validate_not_nil_uuid("schema_id", schema_id)?;
                validators::validate_not_nil_uuid("entry_id", entry_id)?;
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_created_valid() {
        let event = DomainEvent::NodeCreated {
            node_id: Uuid::new_v4(),
            kind: "post".to_string(),
            author_id: Some(Uuid::new_v4()),
        };
        assert!(event.validate().is_ok());
    }

    #[test]
    fn test_node_created_nil_id() {
        let event = DomainEvent::NodeCreated {
            node_id: Uuid::nil(),
            kind: "post".to_string(),
            author_id: None,
        };
        assert!(event.validate().is_err());
    }

    #[test]
    fn test_node_created_empty_kind() {
        let event = DomainEvent::NodeCreated {
            node_id: Uuid::new_v4(),
            kind: "".to_string(),
            author_id: None,
        };
        assert!(event.validate().is_err());
    }

    #[test]
    fn test_node_created_invalid_kind_characters() {
        let event = DomainEvent::NodeCreated {
            node_id: Uuid::new_v4(),
            kind: "invalid@kind".to_string(),
            author_id: None,
        };
        assert!(event.validate().is_err());
    }

    #[test]
    fn test_order_placed_valid() {
        let event = DomainEvent::OrderPlaced {
            order_id: Uuid::new_v4(),
            customer_id: Some(Uuid::new_v4()),
            total: 10000,
            currency: "USD".to_string(),
        };
        assert!(event.validate().is_ok());
    }

    #[test]
    fn test_order_placed_negative_total() {
        let event = DomainEvent::OrderPlaced {
            order_id: Uuid::new_v4(),
            customer_id: None,
            total: -100,
            currency: "USD".to_string(),
        };
        assert!(event.validate().is_err());
    }

    #[test]
    fn test_order_placed_invalid_currency() {
        let event = DomainEvent::OrderPlaced {
            order_id: Uuid::new_v4(),
            customer_id: None,
            total: 10000,
            currency: "US".to_string(), // too short
        };
        assert!(event.validate().is_err());
    }

    #[test]
    fn test_user_registered_valid() {
        let event = DomainEvent::UserRegistered {
            user_id: Uuid::new_v4(),
            email: "user@example.com".to_string(),
        };
        assert!(event.validate().is_ok());
    }

    #[test]
    fn test_user_registered_invalid_email() {
        let event = DomainEvent::UserRegistered {
            user_id: Uuid::new_v4(),
            email: "invalid-email".to_string(),
        };
        assert!(event.validate().is_err());
    }

    #[test]
    fn test_inventory_updated_valid() {
        let event = DomainEvent::InventoryUpdated {
            variant_id: Uuid::new_v4(),
            product_id: Uuid::new_v4(),
            location_id: Uuid::new_v4(),
            old_quantity: 10,
            new_quantity: 5,
        };
        assert!(event.validate().is_ok());
    }

    #[test]
    fn test_inventory_updated_negative_quantity() {
        let event = DomainEvent::InventoryUpdated {
            variant_id: Uuid::new_v4(),
            product_id: Uuid::new_v4(),
            location_id: Uuid::new_v4(),
            old_quantity: -5,
            new_quantity: 10,
        };
        assert!(event.validate().is_err());
    }

    #[test]
    fn test_inventory_low_valid() {
        let event = DomainEvent::InventoryLow {
            variant_id: Uuid::new_v4(),
            product_id: Uuid::new_v4(),
            remaining: 5,
            threshold: 10,
        };
        assert!(event.validate().is_ok());
    }

    #[test]
    fn test_inventory_low_invalid_remaining_above_threshold() {
        let event = DomainEvent::InventoryLow {
            variant_id: Uuid::new_v4(),
            product_id: Uuid::new_v4(),
            remaining: 15,
            threshold: 10,
        };
        assert!(event.validate().is_err());
    }

    #[test]
    fn test_order_status_changed_valid() {
        let event = DomainEvent::OrderStatusChanged {
            order_id: Uuid::new_v4(),
            old_status: "pending".to_string(),
            new_status: "processing".to_string(),
        };
        assert!(event.validate().is_ok());
    }

    #[test]
    fn test_order_status_changed_same_status() {
        let event = DomainEvent::OrderStatusChanged {
            order_id: Uuid::new_v4(),
            old_status: "pending".to_string(),
            new_status: "pending".to_string(),
        };
        assert!(event.validate().is_err());
    }

    #[test]
    fn test_media_uploaded_valid() {
        let event = DomainEvent::MediaUploaded {
            media_id: Uuid::new_v4(),
            mime_type: "image/jpeg".to_string(),
            size: 102400,
        };
        assert!(event.validate().is_ok());
    }

    #[test]
    fn test_media_uploaded_invalid_mime_type() {
        let event = DomainEvent::MediaUploaded {
            media_id: Uuid::new_v4(),
            mime_type: "invalid".to_string(), // no slash
            size: 102400,
        };
        assert!(event.validate().is_err());
    }

    #[test]
    fn test_build_requested_valid_and_metadata() {
        let event = DomainEvent::BuildRequested {
            build_id: Uuid::new_v4(),
            requested_by: "admin@rustok.local".to_string(),
        };

        assert_eq!(event.event_type(), "build.requested");
        assert_eq!(event.schema_version(), 1);
        assert!(event.validate().is_ok());
    }

    #[test]
    fn test_build_requested_invalid_requested_by() {
        let event = DomainEvent::BuildRequested {
            build_id: Uuid::new_v4(),
            requested_by: "".to_string(),
        };

        assert!(event.validate().is_err());
    }

    #[test]
    fn test_flex_entry_created_valid_standalone_binding() {
        let event = DomainEvent::FlexEntryCreated {
            tenant_id: Uuid::new_v4(),
            schema_id: Uuid::new_v4(),
            entry_id: Uuid::new_v4(),
            entity_type: None,
            entity_id: None,
        };

        assert!(event.validate().is_ok());
    }

    #[test]
    fn test_flex_entry_created_invalid_partial_binding() {
        let event = DomainEvent::FlexEntryCreated {
            tenant_id: Uuid::new_v4(),
            schema_id: Uuid::new_v4(),
            entry_id: Uuid::new_v4(),
            entity_type: Some("product".to_string()),
            entity_id: None,
        };

        assert!(event.validate().is_err());
    }
}
