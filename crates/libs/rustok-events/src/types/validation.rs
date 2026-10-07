use super::domain_event::DomainEvent;
use crate::validation::{EventValidationError, ValidateEvent, validators};

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
            Self::TranslationTargetChanged {
                owner_slug,
                resource_kind,
                resource_id,
                changed_locale,
                resource_revision,
                target_revision,
                operation,
                correlation_id,
            } => {
                for (field, value, max) in [
                    ("owner_slug", owner_slug, 191),
                    ("resource_kind", resource_kind, 191),
                    ("resource_id", resource_id, 191),
                    ("changed_locale", changed_locale, 35),
                    ("resource_revision", resource_revision, 256),
                    ("target_revision", target_revision, 256),
                    ("operation", operation, 64),
                    ("correlation_id", correlation_id, 191),
                ] {
                    validators::validate_not_empty(field, value)?;
                    validators::validate_max_length(field, value, max)?;
                }
                Ok(())
            }

            // ════════════════════════════════════════════════════════════════
            // USER EVENTS
            // ════════════════════════════════════════════════════════════════
            Self::UserAccountRegistered { user_id } => {
                validators::validate_not_nil_uuid("user_id", user_id)?;
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
            | Self::ProductUnpublished { product_id }
            | Self::ProductArchived { product_id }
            | Self::ProductDeleted { product_id } => {
                validators::validate_not_nil_uuid("product_id", product_id)?;
                Ok(())
            }
            Self::ProductAttributeCreated { attribute_id }
            | Self::ProductAttributeUpdated { attribute_id }
            | Self::ProductAttributeDeleted { attribute_id } => {
                validators::validate_not_nil_uuid("attribute_id", attribute_id)?;
                Ok(())
            }
            Self::ProductAttributeOptionCreated {
                option_id,
                attribute_id,
            }
            | Self::ProductAttributeOptionUpdated {
                option_id,
                attribute_id,
            }
            | Self::ProductAttributeOptionDeleted {
                option_id,
                attribute_id,
            } => {
                validators::validate_not_nil_uuid("option_id", option_id)?;
                validators::validate_not_nil_uuid("attribute_id", attribute_id)?;
                Ok(())
            }
            Self::ProductAttributeSchemaCreated { schema_id }
            | Self::ProductAttributeSchemaUpdated { schema_id }
            | Self::ProductAttributeSchemaDeleted { schema_id }
            | Self::ProductAttributeSchemaBindingsChanged { schema_id } => {
                validators::validate_not_nil_uuid("schema_id", schema_id)?;
                Ok(())
            }
            Self::CatalogCategoryCreated { category_id }
            | Self::CatalogCategoryUpdated { category_id }
            | Self::CatalogCategoryDeleted { category_id }
            | Self::CatalogCategorySchemaModeChanged { category_id }
            | Self::CatalogCategoryAttributesChanged { category_id } => {
                validators::validate_not_nil_uuid("category_id", category_id)?;
                Ok(())
            }
            Self::ProductPrimaryCategoryChanged {
                product_id,
                old_category_id,
                new_category_id,
            } => {
                validators::validate_not_nil_uuid("product_id", product_id)?;
                validators::validate_optional_uuid("old_category_id", old_category_id)?;
                validators::validate_optional_uuid("new_category_id", new_category_id)?;
                Ok(())
            }
            Self::ProductCategoryAssignmentsChanged { product_id }
            | Self::ProductAttributeValuesChanged { product_id } => {
                validators::validate_not_nil_uuid("product_id", product_id)?;
                Ok(())
            }

            // ════════════���════════════════��══════════════════════════════════
            // COMMERCE EVENTS - Variants
            // ═══════���════════════════════════════════════════════════════════
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
            Self::TargetDeleted {
                target_type,
                target_id,
            } => {
                validators::validate_not_empty("target_type", target_type)?;
                validators::validate_max_length("target_type", target_type, 64)?;
                validators::validate_not_nil_uuid("target_id", target_id)?;
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
            Self::BuildRolledBack {
                requested_build_id,
                restored_build_id,
                from_release_id,
                to_release_id,
            } => {
                validators::validate_not_nil_uuid("requested_build_id", requested_build_id)?;
                validators::validate_not_nil_uuid("restored_build_id", restored_build_id)?;
                validators::validate_not_empty("from_release_id", from_release_id)?;
                validators::validate_max_length("from_release_id", from_release_id, 255)?;
                validators::validate_not_empty("to_release_id", to_release_id)?;
                validators::validate_max_length("to_release_id", to_release_id, 255)?;
                if from_release_id == to_release_id {
                    return Err(EventValidationError::InvalidValue(
                        "to_release_id",
                        "must differ from from_release_id".to_string(),
                    ));
                }
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
            // COMMENT EVENTS
            // ════════════════════════════════════════════════════════════════
            Self::CommentCreated {
                comment_id,
                target_type,
                target_id,
                author_id,
            }
            | Self::CommentUpdated {
                comment_id,
                target_type,
                target_id,
                author_id,
            }
            | Self::CommentDeleted {
                comment_id,
                target_type,
                target_id,
                author_id,
            } => {
                validators::validate_not_nil_uuid("comment_id", comment_id)?;
                validators::validate_not_empty("target_type", target_type)?;
                validators::validate_max_length("target_type", target_type, 64)?;
                validators::validate_not_nil_uuid("target_id", target_id)?;
                validators::validate_not_nil_uuid("author_id", author_id)?;
                Ok(())
            }
            Self::CommentStatusChanged {
                comment_id,
                target_type,
                target_id,
                author_id,
                old_status,
                new_status,
            } => {
                validators::validate_not_nil_uuid("comment_id", comment_id)?;
                validators::validate_not_empty("target_type", target_type)?;
                validators::validate_max_length("target_type", target_type, 64)?;
                validators::validate_not_nil_uuid("target_id", target_id)?;
                validators::validate_not_nil_uuid("author_id", author_id)?;
                validators::validate_not_empty("old_status", old_status)?;
                validators::validate_max_length("old_status", old_status, 32)?;
                validators::validate_not_empty("new_status", new_status)?;
                validators::validate_max_length("new_status", new_status, 32)?;
                if old_status == new_status {
                    return Err(EventValidationError::InvalidValue(
                        "status_transition",
                        "old_status and new_status must differ".to_string(),
                    ));
                }
                Ok(())
            }

            // FORUM EVENTS
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
                old_status,
                new_status,
                moderator_id,
            } => {
                validators::validate_not_nil_uuid("reply_id", reply_id)?;
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
            Self::CanonicalUrlChanged {
                target_id,
                target_kind,
                locale,
                new_canonical_url,
                old_urls,
            } => {
                validators::validate_not_nil_uuid("target_id", target_id)?;
                validators::validate_not_empty("target_kind", target_kind)?;
                validators::validate_max_length("target_kind", target_kind, 64)?;
                validators::validate_not_empty("locale", locale)?;
                validators::validate_max_length("locale", locale, 16)?;
                validators::validate_not_empty("new_canonical_url", new_canonical_url)?;
                validators::validate_max_length("new_canonical_url", new_canonical_url, 512)?;
                if !new_canonical_url.starts_with('/') {
                    return Err(EventValidationError::InvalidValue(
                        "new_canonical_url",
                        "must start with `/`".to_string(),
                    ));
                }
                for url in old_urls {
                    validators::validate_not_empty("old_urls[]", url)?;
                    validators::validate_max_length("old_urls[]", url, 512)?;
                    if !url.starts_with('/') {
                        return Err(EventValidationError::InvalidValue(
                            "old_urls[]",
                            "must start with `/`".to_string(),
                        ));
                    }
                }
                Ok(())
            }
            Self::UrlAliasPurged {
                target_id,
                target_kind,
                locale,
                urls,
            } => {
                validators::validate_not_nil_uuid("target_id", target_id)?;
                validators::validate_not_empty("target_kind", target_kind)?;
                validators::validate_max_length("target_kind", target_kind, 64)?;
                validators::validate_not_empty("locale", locale)?;
                validators::validate_max_length("locale", locale, 16)?;
                if urls.is_empty() {
                    return Err(EventValidationError::InvalidValue(
                        "urls",
                        "must not be empty".to_string(),
                    ));
                }
                for url in urls {
                    validators::validate_not_empty("urls[]", url)?;
                    validators::validate_max_length("urls[]", url, 512)?;
                    if !url.starts_with('/') {
                        return Err(EventValidationError::InvalidValue(
                            "urls[]",
                            "must start with `/`".to_string(),
                        ));
                    }
                }
                Ok(())
            }

            // ════════════════════════════════════════════════════════════════
            // SEO EVENTS
            // ════════════════════════════════════════════════════════════════
            Self::SeoMetaUpserted {
                target_kind,
                target_id,
                locale,
                source,
                idempotency_key,
            } => {
                validators::validate_not_empty("target_kind", target_kind)?;
                validators::validate_max_length("target_kind", target_kind, 64)?;
                validators::validate_not_nil_uuid("target_id", target_id)?;
                validators::validate_not_empty("locale", locale)?;
                validators::validate_max_length("locale", locale, 32)?;
                validators::validate_not_empty("source", source)?;
                validators::validate_max_length("source", source, 64)?;
                validators::validate_not_empty("idempotency_key", idempotency_key)?;
                validators::validate_max_length("idempotency_key", idempotency_key, 255)?;
                Ok(())
            }
            Self::SeoRevisionPublished {
                target_kind,
                target_id,
                revision,
                idempotency_key,
            }
            | Self::SeoRevisionRolledBack {
                target_kind,
                target_id,
                revision,
                idempotency_key,
            } => {
                validators::validate_not_empty("target_kind", target_kind)?;
                validators::validate_max_length("target_kind", target_kind, 64)?;
                validators::validate_not_nil_uuid("target_id", target_id)?;
                validators::validate_range("revision", *revision as i64, 1, i32::MAX as i64)?;
                validators::validate_not_empty("idempotency_key", idempotency_key)?;
                validators::validate_max_length("idempotency_key", idempotency_key, 255)?;
                Ok(())
            }
            Self::SeoRedirectUpserted {
                redirect_id,
                source_pattern,
                target_url,
                status_code,
                idempotency_key,
                ..
            } => {
                validators::validate_not_nil_uuid("redirect_id", redirect_id)?;
                validators::validate_not_empty("source_pattern", source_pattern)?;
                validators::validate_max_length("source_pattern", source_pattern, 512)?;
                validators::validate_not_empty("target_url", target_url)?;
                validators::validate_max_length("target_url", target_url, 2048)?;
                validators::validate_range("status_code", *status_code as i64, 100, 599)?;
                validators::validate_not_empty("idempotency_key", idempotency_key)?;
                validators::validate_max_length("idempotency_key", idempotency_key, 255)?;
                Ok(())
            }
            Self::SeoRedirectDisabled {
                redirect_id,
                source_pattern,
                idempotency_key,
            } => {
                validators::validate_not_nil_uuid("redirect_id", redirect_id)?;
                validators::validate_not_empty("source_pattern", source_pattern)?;
                validators::validate_max_length("source_pattern", source_pattern, 512)?;
                validators::validate_not_empty("idempotency_key", idempotency_key)?;
                validators::validate_max_length("idempotency_key", idempotency_key, 255)?;
                Ok(())
            }
            Self::SeoSitemapGenerated {
                job_id,
                file_count,
                idempotency_key,
            } => {
                validators::validate_not_nil_uuid("job_id", job_id)?;
                validators::validate_range("file_count", *file_count as i64, 0, i32::MAX as i64)?;
                validators::validate_not_empty("idempotency_key", idempotency_key)?;
                validators::validate_max_length("idempotency_key", idempotency_key, 255)?;
                Ok(())
            }
            Self::SeoSitemapSubmitted {
                job_id,
                endpoint_count,
                error,
                idempotency_key,
                ..
            } => {
                validators::validate_not_nil_uuid("job_id", job_id)?;
                validators::validate_range(
                    "endpoint_count",
                    *endpoint_count as i64,
                    0,
                    i32::MAX as i64,
                )?;
                if let Some(error) = error {
                    validators::validate_max_length("error", error, 2048)?;
                }
                validators::validate_not_empty("idempotency_key", idempotency_key)?;
                validators::validate_max_length("idempotency_key", idempotency_key, 255)?;
                Ok(())
            }
            Self::SeoBulkCompleted {
                job_id,
                target_kind,
                locale,
                status,
                processed_count,
                succeeded_count,
                failed_count,
                idempotency_key,
            }
            | Self::SeoBulkPartial {
                job_id,
                target_kind,
                locale,
                status,
                processed_count,
                succeeded_count,
                failed_count,
                idempotency_key,
            }
            | Self::SeoBulkFailed {
                job_id,
                target_kind,
                locale,
                status,
                processed_count,
                succeeded_count,
                failed_count,
                idempotency_key,
            } => {
                validators::validate_not_nil_uuid("job_id", job_id)?;
                validators::validate_not_empty("target_kind", target_kind)?;
                validators::validate_max_length("target_kind", target_kind, 64)?;
                validators::validate_not_empty("locale", locale)?;
                validators::validate_max_length("locale", locale, 32)?;
                validators::validate_not_empty("status", status)?;
                validators::validate_max_length("status", status, 32)?;
                validators::validate_range(
                    "processed_count",
                    *processed_count as i64,
                    0,
                    i32::MAX as i64,
                )?;
                validators::validate_range(
                    "succeeded_count",
                    *succeeded_count as i64,
                    0,
                    i32::MAX as i64,
                )?;
                validators::validate_range(
                    "failed_count",
                    *failed_count as i64,
                    0,
                    i32::MAX as i64,
                )?;
                validators::validate_not_empty("idempotency_key", idempotency_key)?;
                validators::validate_max_length("idempotency_key", idempotency_key, 255)?;
                Ok(())
            }

            // ════════════════════════════════════════════════════════════════
            // TENANT EVENTS
            // ════════════════════════════════════════════════════════════════
            Self::TenantCreated { tenant_id } | Self::TenantUpdated { tenant_id } => {
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                Ok(())
            }
            Self::TenantModuleToggled {
                tenant_id,
                module_slug,
                ..
            } => {
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                validators::validate_not_empty("module_slug", module_slug)?;
                validators::validate_max_length("module_slug", module_slug, 128)?;
                Ok(())
            }
            Self::ModuleArtifactAdmitted {
                installation_id,
                artifact_digest,
                media_type,
                size_bytes: _,
            } => {
                validators::validate_not_nil_uuid("installation_id", installation_id)?;
                validators::validate_not_empty("artifact_digest", artifact_digest)?;
                validators::validate_max_length("artifact_digest", artifact_digest, 128)?;
                validators::validate_not_empty("media_type", media_type)?;
                validators::validate_max_length("media_type", media_type, 255)?;
                Ok(())
            }
            Self::ModuleArtifactReverified {
                installation_id,
                status,
                revision,
            } => {
                validators::validate_not_nil_uuid("installation_id", installation_id)?;
                validators::validate_not_empty("status", status)?;
                validators::validate_max_length("status", status, 32)?;
                if *revision == 0 {
                    return Err(EventValidationError::InvalidValue(
                        "revision",
                        "must be positive".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleArtifactActivated {
                installation_id,
                predecessor_installation_id,
                revision,
            } => {
                validators::validate_not_nil_uuid("installation_id", installation_id)?;
                if let Some(predecessor_installation_id) = predecessor_installation_id {
                    validators::validate_not_nil_uuid(
                        "predecessor_installation_id",
                        predecessor_installation_id,
                    )?;
                }
                if *revision == 0 {
                    return Err(EventValidationError::InvalidValue(
                        "revision",
                        "must be positive".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleArtifactRolledBack {
                installation_id,
                target_installation_id,
            } => {
                validators::validate_not_nil_uuid("installation_id", installation_id)?;
                validators::validate_not_nil_uuid("target_installation_id", target_installation_id)
            }
            Self::ModuleTransitionFinalized {
                operation_id,
                module_slug,
                revision,
                released_holds: _,
            } => {
                validators::validate_not_nil_uuid("operation_id", operation_id)?;
                validators::validate_not_empty("module_slug", module_slug)?;
                validators::validate_max_length("module_slug", module_slug, 128)?;
                if *revision == 0 {
                    return Err(EventValidationError::InvalidValue(
                        "revision",
                        "must be positive".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleTransitionFailedClosed {
                operation_id,
                module_slug,
                revision,
                failure_reason,
            } => {
                validators::validate_not_nil_uuid("operation_id", operation_id)?;
                validators::validate_not_empty("module_slug", module_slug)?;
                validators::validate_max_length("module_slug", module_slug, 128)?;
                validators::validate_not_empty("failure_reason", failure_reason)?;
                validators::validate_max_length("failure_reason", failure_reason, 2_000)?;
                if *revision == 0 {
                    return Err(EventValidationError::InvalidValue(
                        "revision",
                        "must be positive".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleArtifactUninstalled {
                installation_id,
                revision,
            } => {
                validators::validate_not_nil_uuid("installation_id", installation_id)?;
                if *revision == 0 {
                    return Err(EventValidationError::InvalidValue(
                        "revision",
                        "must be positive".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleArtifactMigrationCheckpointed {
                installation_id,
                revision,
                has_irreversible_migration: _,
            } => {
                validators::validate_not_nil_uuid("installation_id", installation_id)?;
                if *revision == 0 {
                    return Err(EventValidationError::InvalidValue(
                        "revision",
                        "must be positive".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleArtifactDeactivated {
                installation_id,
                revision,
            } => {
                validators::validate_not_nil_uuid("installation_id", installation_id)?;
                if *revision == 0 {
                    return Err(EventValidationError::InvalidValue(
                        "revision",
                        "must be positive".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleArtifactTenantDisabled {
                installation_id,
                tenant_id,
                revision,
            } => {
                validators::validate_not_nil_uuid("installation_id", installation_id)?;
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                if *revision == 0 {
                    return Err(EventValidationError::InvalidValue(
                        "revision",
                        "must be positive".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleArtifactTenantEnabled {
                installation_id,
                tenant_id,
                revision,
            } => {
                validators::validate_not_nil_uuid("installation_id", installation_id)?;
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                if *revision == 0 {
                    return Err(EventValidationError::InvalidValue(
                        "revision",
                        "must be positive".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleArtifactDataPurged {
                tenant_id,
                module_slug,
                data_contract_revision,
                namespace_revision,
                purged_records: _,
            } => {
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                validators::validate_not_empty("module_slug", module_slug)?;
                validators::validate_max_length("module_slug", module_slug, 48)?;
                if *data_contract_revision == 0 || *namespace_revision == 0 {
                    return Err(EventValidationError::InvalidValue(
                        "data or namespace revision",
                        "must be positive".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleArtifactSettingsRecoveryPointCreated {
                recovery_point_id,
                tenant_id,
                installation_id,
                settings_instance_id,
                settings_revision,
            } => {
                validators::validate_not_nil_uuid("recovery_point_id", recovery_point_id)?;
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                validators::validate_not_nil_uuid("installation_id", installation_id)?;
                validators::validate_not_nil_uuid("settings_instance_id", settings_instance_id)?;
                if *settings_revision == 0 {
                    return Err(EventValidationError::InvalidValue(
                        "settings_revision",
                        "must be positive".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleArtifactSettingsPurged {
                recovery_point_id,
                tenant_id,
                installation_id,
                settings_instance_id,
                tombstone_revision,
            } => {
                validators::validate_not_nil_uuid("recovery_point_id", recovery_point_id)?;
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                validators::validate_not_nil_uuid("installation_id", installation_id)?;
                validators::validate_not_nil_uuid("settings_instance_id", settings_instance_id)?;
                if *tombstone_revision == 0 {
                    return Err(EventValidationError::InvalidValue(
                        "tombstone_revision",
                        "must be positive".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleArtifactSettingsRestored {
                recovery_point_id,
                tenant_id,
                target_installation_id,
                settings_instance_id,
            } => {
                validators::validate_not_nil_uuid("recovery_point_id", recovery_point_id)?;
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                if target_installation_id.is_some_and(|installation_id| installation_id.is_nil()) {
                    return Err(EventValidationError::NilUuid("target_installation_id"));
                }
                validators::validate_not_nil_uuid("settings_instance_id", settings_instance_id)
            }
            Self::ModuleArtifactSettingsRecoveryRetentionUpdated {
                recovery_point_id,
                tenant_id,
                retention_revision,
                retain_until: _,
                legal_hold: _,
                audit_hold: _,
                incident_hold: _,
            } => {
                validators::validate_not_nil_uuid("recovery_point_id", recovery_point_id)?;
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                if *retention_revision == 0 {
                    return Err(EventValidationError::InvalidValue(
                        "retention_revision",
                        "must be positive".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleArtifactSettingsRecoveryRewrapped {
                recovery_point_id,
                tenant_id,
                previous_key_version,
                key_version,
            } => {
                validators::validate_not_nil_uuid("recovery_point_id", recovery_point_id)?;
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                validators::validate_not_empty("previous_key_version", previous_key_version)?;
                validators::validate_not_empty("key_version", key_version)?;
                validators::validate_max_length("previous_key_version", previous_key_version, 256)?;
                validators::validate_max_length("key_version", key_version, 256)
            }
            Self::ModuleArtifactSettingsRecoveryCollected {
                collection_id,
                recovery_point_id,
                tenant_id,
            } => {
                validators::validate_not_nil_uuid("collection_id", collection_id)?;
                validators::validate_not_nil_uuid("recovery_point_id", recovery_point_id)?;
                validators::validate_not_nil_uuid("tenant_id", tenant_id)
            }
            Self::ModuleArtifactSettingsRecoveryBound {
                recovery_point_id,
                tenant_id,
                target_installation_id,
                settings_instance_id,
            } => {
                validators::validate_not_nil_uuid("recovery_point_id", recovery_point_id)?;
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                validators::validate_not_nil_uuid(
                    "target_installation_id",
                    target_installation_id,
                )?;
                validators::validate_not_nil_uuid("settings_instance_id", settings_instance_id)
            }
            Self::ModuleArtifactDataExported {
                export_id,
                tenant_id,
                module_slug,
                data_contract_revision,
                namespace_revision,
                exported_records: _,
            } => {
                validators::validate_not_nil_uuid("export_id", export_id)?;
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                validators::validate_not_empty("module_slug", module_slug)?;
                validators::validate_max_length("module_slug", module_slug, 48)?;
                if *data_contract_revision == 0 || *namespace_revision == 0 {
                    return Err(EventValidationError::InvalidValue(
                        "data or namespace revision",
                        "must be positive".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleArtifactDataSnapshotCreated {
                snapshot_id,
                tenant_id,
                module_slug,
                data_contract_revision,
                namespace_revision,
                manifest_digest,
                structured_records: _,
                objects: _,
            } => {
                validators::validate_not_nil_uuid("snapshot_id", snapshot_id)?;
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                validators::validate_not_empty("module_slug", module_slug)?;
                validators::validate_max_length("module_slug", module_slug, 48)?;
                validators::validate_not_empty("manifest_digest", manifest_digest)?;
                if *data_contract_revision == 0
                    || *namespace_revision == 0
                    || manifest_digest.len() != 71
                    || !manifest_digest.starts_with("sha256:")
                    || !manifest_digest[7..]
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
                {
                    return Err(EventValidationError::InvalidValue(
                        "data snapshot identity",
                        "must contain positive revisions and a sha256 digest".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleArtifactDataSnapshotRestored {
                snapshot_id,
                tenant_id,
                module_slug,
                data_contract_revision,
                namespace_revision,
                restored_records: _,
                restored_objects: _,
            } => {
                validators::validate_not_nil_uuid("snapshot_id", snapshot_id)?;
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                validators::validate_not_empty("module_slug", module_slug)?;
                validators::validate_max_length("module_slug", module_slug, 48)?;
                if *data_contract_revision == 0 || *namespace_revision == 0 {
                    return Err(EventValidationError::InvalidValue(
                        "data or namespace revision",
                        "must be positive".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleArtifactDataSnapshotRetentionUpdated {
                snapshot_id,
                tenant_id,
                retention_revision,
                retain_until: _,
                legal_hold: _,
            } => {
                validators::validate_not_nil_uuid("snapshot_id", snapshot_id)?;
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                if *retention_revision == 0 {
                    return Err(EventValidationError::InvalidValue(
                        "retention_revision",
                        "must be positive".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleArtifactDataSnapshotCollected {
                collection_id,
                snapshot_id,
                tenant_id,
                module_slug,
                data_contract_revision,
                policy_snapshot_id,
                deleted_objects: _,
            } => {
                validators::validate_not_nil_uuid("collection_id", collection_id)?;
                validators::validate_not_nil_uuid("snapshot_id", snapshot_id)?;
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                validators::validate_not_empty("module_slug", module_slug)?;
                validators::validate_max_length("module_slug", module_slug, 48)?;
                validators::validate_not_empty("policy_snapshot_id", policy_snapshot_id)?;
                validators::validate_max_length("policy_snapshot_id", policy_snapshot_id, 128)?;
                if *data_contract_revision == 0 {
                    return Err(EventValidationError::InvalidValue(
                        "data_contract_revision",
                        "must be positive".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleArtifactSecretBound {
                tenant_id,
                module_slug,
                installation_id,
                data_owner_id,
                secret_instance_id,
                revision,
            } => {
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                validators::validate_not_nil_uuid("installation_id", installation_id)?;
                validators::validate_not_nil_uuid("data_owner_id", data_owner_id)?;
                validators::validate_not_nil_uuid("secret_instance_id", secret_instance_id)?;
                validators::validate_not_empty("module_slug", module_slug)?;
                validators::validate_max_length("module_slug", module_slug, 48)?;
                if *revision == 0 {
                    return Err(EventValidationError::InvalidValue(
                        "binding revision",
                        "must be positive".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleBuildQueued {
                request_id,
                tenant_id,
                project_id,
                attempt,
            } => {
                validators::validate_not_nil_uuid("request_id", request_id)?;
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                validators::validate_not_empty("project_id", project_id)?;
                validators::validate_max_length("project_id", project_id, 256)?;
                if *attempt == 0 {
                    return Err(EventValidationError::InvalidValue(
                        "attempt",
                        "must be positive".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleBuildCompleted {
                request_id,
                tenant_id,
                outcome,
                retryable: _,
            } => {
                validators::validate_not_nil_uuid("request_id", request_id)?;
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                if !matches!(
                    outcome.as_str(),
                    "succeeded" | "failed" | "cancelled" | "nondeterministic"
                ) {
                    return Err(EventValidationError::InvalidValue(
                        "outcome",
                        "must be a canonical module build outcome".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleStaticPromotionRequested {
                promotion_id,
                release_id,
                module_slug,
                module_version,
                source_digest,
            } => {
                validators::validate_not_nil_uuid("promotion_id", promotion_id)?;
                validators::validate_not_empty("release_id", release_id)?;
                validators::validate_max_length("release_id", release_id, 256)?;
                validators::validate_not_empty("module_slug", module_slug)?;
                validators::validate_max_length("module_slug", module_slug, 128)?;
                validators::validate_not_empty("module_version", module_version)?;
                validators::validate_max_length("module_version", module_version, 128)?;
                validate_sha256_digest("source_digest", source_digest)
            }
            Self::ModuleStaticPromotionApproved {
                promotion_id,
                release_id,
                module_slug,
                module_version,
                revision,
                policy_revision,
            } => {
                validators::validate_not_nil_uuid("promotion_id", promotion_id)?;
                validators::validate_not_empty("release_id", release_id)?;
                validators::validate_max_length("release_id", release_id, 256)?;
                validators::validate_not_empty("module_slug", module_slug)?;
                validators::validate_max_length("module_slug", module_slug, 128)?;
                validators::validate_not_empty("module_version", module_version)?;
                validators::validate_max_length("module_version", module_version, 128)?;
                validators::validate_not_empty("policy_revision", policy_revision)?;
                validators::validate_max_length("policy_revision", policy_revision, 128)?;
                if *revision == 0 {
                    return Err(EventValidationError::InvalidValue(
                        "revision",
                        "must be positive".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleStaticDistributionBuildQueued {
                distribution_build_id,
                predecessor_build_id,
                composition_revision,
                composition_digest,
                selected_promotions,
            } => {
                validators::validate_not_nil_uuid("distribution_build_id", distribution_build_id)?;
                if predecessor_build_id
                    .is_some_and(|value| value.is_nil() || value == *distribution_build_id)
                {
                    return Err(EventValidationError::InvalidValue(
                        "predecessor_build_id",
                        "must be absent or a distinct non-nil UUID".to_string(),
                    ));
                }
                if *composition_revision == 0 || *selected_promotions > 256 {
                    return Err(EventValidationError::InvalidValue(
                        "static distribution build identity",
                        "must contain a positive revision and at most 256 promotions".to_string(),
                    ));
                }
                validate_sha256_digest("composition_digest", composition_digest)
            }
            Self::ModuleStaticDistributionBuildClaimed {
                distribution_build_id,
                claim_id,
                attempt_number,
                runner_id,
                reclaimed_expired_lease: _,
            } => {
                validators::validate_not_nil_uuid("distribution_build_id", distribution_build_id)?;
                validators::validate_not_nil_uuid("claim_id", claim_id)?;
                validators::validate_not_empty("runner_id", runner_id)?;
                validators::validate_max_length("runner_id", runner_id, 128)?;
                if *attempt_number == 0 || runner_id.chars().any(char::is_control) {
                    return Err(EventValidationError::InvalidValue(
                        "distribution claim",
                        "must contain a positive attempt and a safe runner identity".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleStaticDistributionBuildCompleted {
                distribution_build_id,
                claim_id,
                composition_revision,
                composition_digest,
                outcome,
                bundle_root_digest,
                role_set_digest,
                completion_digest,
            } => {
                validators::validate_not_nil_uuid("distribution_build_id", distribution_build_id)?;
                validators::validate_not_nil_uuid("claim_id", claim_id)?;
                if *composition_revision == 0
                    || !matches!(outcome.as_str(), "succeeded" | "failed" | "cancelled")
                    || (outcome == "succeeded") != bundle_root_digest.is_some()
                    || bundle_root_digest.is_some() != role_set_digest.is_some()
                {
                    return Err(EventValidationError::InvalidValue(
                        "static distribution completion",
                        "must contain a positive revision and canonical terminal evidence"
                            .to_string(),
                    ));
                }
                validate_sha256_digest("composition_digest", composition_digest)?;
                validate_sha256_digest("completion_digest", completion_digest)?;
                if let Some(bundle_root_digest) = bundle_root_digest {
                    validate_sha256_digest("bundle_root_digest", bundle_root_digest)?;
                }
                if let Some(role_set_digest) = role_set_digest {
                    validate_sha256_digest("role_set_digest", role_set_digest)?;
                }
                Ok(())
            }
            Self::ModuleStaticDistributionReleaseAdmitted {
                distribution_release_id,
                predecessor_release_id,
                distribution_build_id,
                release_revision,
                composition_revision,
                composition_digest,
                bundle_root_digest,
                role_set_digest,
                policy_revision,
            } => {
                validators::validate_not_nil_uuid(
                    "distribution_release_id",
                    distribution_release_id,
                )?;
                validators::validate_not_nil_uuid("distribution_build_id", distribution_build_id)?;
                if predecessor_release_id
                    .is_some_and(|value| value.is_nil() || value == *distribution_release_id)
                {
                    return Err(EventValidationError::InvalidValue(
                        "predecessor_release_id",
                        "must be absent or a distinct non-nil UUID".to_string(),
                    ));
                }
                if *release_revision == 0 || *composition_revision == 0 {
                    return Err(EventValidationError::InvalidValue(
                        "static distribution release identity",
                        "must contain positive release and composition revisions".to_string(),
                    ));
                }
                validators::validate_not_empty("policy_revision", policy_revision)?;
                validators::validate_max_length("policy_revision", policy_revision, 128)?;
                if policy_revision.trim() != policy_revision
                    || policy_revision.chars().any(char::is_control)
                {
                    return Err(EventValidationError::InvalidValue(
                        "policy_revision",
                        "must be a canonical printable value".to_string(),
                    ));
                }
                validate_sha256_digest("composition_digest", composition_digest)?;
                validate_sha256_digest("bundle_root_digest", bundle_root_digest)?;
                validate_sha256_digest("role_set_digest", role_set_digest)
            }
            Self::ModuleStaticDistributionReleaseActivated {
                distribution_release_id,
                predecessor_release_id,
                rollout_id,
                release_state_revision,
            } => {
                validators::validate_not_nil_uuid(
                    "distribution_release_id",
                    distribution_release_id,
                )?;
                validators::validate_not_nil_uuid("rollout_id", rollout_id)?;
                if predecessor_release_id
                    .is_some_and(|value| value.is_nil() || value == *distribution_release_id)
                    || *release_state_revision == 0
                {
                    return Err(EventValidationError::InvalidValue(
                        "static distribution serving identity",
                        "must contain a valid predecessor and positive state revision".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleStaticDistributionReleaseRevoked {
                distribution_release_id,
                distribution_build_id,
                release_state_revision,
                was_active: _,
                policy_revision,
            } => {
                validators::validate_not_nil_uuid(
                    "distribution_release_id",
                    distribution_release_id,
                )?;
                validators::validate_not_nil_uuid("distribution_build_id", distribution_build_id)?;
                if *release_state_revision == 0 {
                    return Err(EventValidationError::InvalidValue(
                        "release_state_revision",
                        "must be positive".to_string(),
                    ));
                }
                validate_policy_revision(policy_revision)
            }
            Self::ModuleStaticDistributionRolloutRequested {
                rollout_id,
                predecessor_rollout_id,
                distribution_release_id,
                rollout_revision,
                rollout_state_revision,
                composition_revision,
                composition_digest,
                bundle_root_digest,
                role_set_digest,
                topology_digest,
                policy_revision,
                target_assignments,
                executor_mode,
            } => {
                validators::validate_not_nil_uuid("rollout_id", rollout_id)?;
                validators::validate_not_nil_uuid(
                    "distribution_release_id",
                    distribution_release_id,
                )?;
                if predecessor_rollout_id
                    .is_some_and(|value| value.is_nil() || value == *rollout_id)
                    || *rollout_revision == 0
                    || *rollout_state_revision == 0
                    || *composition_revision == 0
                    || *target_assignments == 0
                    || *target_assignments > 1024
                    || executor_mode != "static_native"
                {
                    return Err(EventValidationError::InvalidValue(
                        "static distribution rollout identity",
                        "must contain canonical positive revisions, topology, and static/native executor identity"
                            .to_string(),
                    ));
                }
                validate_policy_revision(policy_revision)?;
                validate_sha256_digest("composition_digest", composition_digest)?;
                validate_sha256_digest("bundle_root_digest", bundle_root_digest)?;
                validate_sha256_digest("role_set_digest", role_set_digest)?;
                validate_sha256_digest("topology_digest", topology_digest)
            }
            Self::ModuleStaticDistributionRecoveryRequested {
                rollout_id,
                predecessor_rollout_id,
                from_release_id,
                target_release_id,
                rollout_revision,
                rollout_state_revision,
                topology_digest,
                policy_revision,
                reason,
            } => {
                validators::validate_not_nil_uuid("rollout_id", rollout_id)?;
                validators::validate_not_nil_uuid(
                    "predecessor_rollout_id",
                    predecessor_rollout_id,
                )?;
                validators::validate_not_nil_uuid("from_release_id", from_release_id)?;
                validators::validate_not_nil_uuid("target_release_id", target_release_id)?;
                if rollout_id == predecessor_rollout_id
                    || from_release_id == target_release_id
                    || *rollout_revision == 0
                    || *rollout_state_revision == 0
                {
                    return Err(EventValidationError::InvalidValue(
                        "static distribution recovery identity",
                        "must bind distinct predecessor/current identities and positive revisions"
                            .to_string(),
                    ));
                }
                validate_policy_revision(policy_revision)?;
                validators::validate_not_empty("reason", reason)?;
                validators::validate_max_length("reason", reason, 2_000)?;
                validate_sha256_digest("topology_digest", topology_digest)
            }
            Self::ModuleStaticDistributionRecoveryConverged {
                rollout_id,
                from_release_id,
                target_release_id,
                release_state_revision,
                rollout_state_revision,
            } => {
                validators::validate_not_nil_uuid("rollout_id", rollout_id)?;
                validators::validate_not_nil_uuid("from_release_id", from_release_id)?;
                validators::validate_not_nil_uuid("target_release_id", target_release_id)?;
                if from_release_id == target_release_id
                    || *release_state_revision == 0
                    || *rollout_state_revision == 0
                {
                    return Err(EventValidationError::InvalidValue(
                        "static distribution recovery convergence",
                        "must bind distinct releases and positive revisions".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleStaticDistributionAssignmentObserved {
                rollout_id,
                node_id,
                role,
                candidate_artifact_digest,
                reporter_id,
                observation_revision,
                phase,
                report_digest,
            } => {
                validators::validate_not_nil_uuid("rollout_id", rollout_id)?;
                validators::validate_not_empty("node_id", node_id)?;
                validators::validate_max_length("node_id", node_id, 128)?;
                validators::validate_not_empty("role", role)?;
                validators::validate_not_empty("reporter_id", reporter_id)?;
                validators::validate_max_length("reporter_id", reporter_id, 128)?;
                if *observation_revision == 0
                    || node_id.trim() != node_id
                    || node_id.chars().any(char::is_control)
                    || !matches!(
                        role.as_str(),
                        "monolith" | "api" | "admin_ssr" | "storefront_ssr" | "worker" | "registry"
                    )
                    || reporter_id.trim() != reporter_id
                    || reporter_id.chars().any(char::is_control)
                    || !matches!(phase.as_str(), "prepared" | "healthy" | "active" | "failed")
                {
                    return Err(EventValidationError::InvalidValue(
                        "static distribution assignment observation",
                        "must contain a positive revision, canonical node and role, and supported phase"
                            .to_string(),
                    ));
                }
                validate_sha256_digest("candidate_artifact_digest", candidate_artifact_digest)?;
                validate_sha256_digest("report_digest", report_digest)
            }
            Self::ModuleStaticDistributionRolloutStatusChanged {
                rollout_id,
                distribution_release_id,
                rollout_revision,
                rollout_state_revision,
                status,
                observed_rollout_id,
                failure_code,
            } => {
                validators::validate_not_nil_uuid("rollout_id", rollout_id)?;
                validators::validate_not_nil_uuid(
                    "distribution_release_id",
                    distribution_release_id,
                )?;
                if *rollout_revision == 0
                    || *rollout_state_revision == 0
                    || !matches!(
                        status.as_str(),
                        "activating" | "converged" | "failed" | "degraded"
                    )
                    || observed_rollout_id.is_some_and(|value| value.is_nil())
                    || (status == "converged" && *observed_rollout_id != Some(*rollout_id))
                    || matches!(status.as_str(), "failed" | "degraded") != failure_code.is_some()
                {
                    return Err(EventValidationError::InvalidValue(
                        "static distribution rollout status",
                        "must contain canonical revision, observation, and failure state"
                            .to_string(),
                    ));
                }
                if let Some(failure_code) = failure_code {
                    validators::validate_not_empty("failure_code", failure_code)?;
                    validators::validate_max_length("failure_code", failure_code, 128)?;
                }
                Ok(())
            }
            Self::ModuleArtifactNodeReconciliationRequested {
                reconciliation_id,
                predecessor_reconciliation_id,
                reconciliation_revision,
                reconciliation_state_revision,
                topology_digest,
                policy_revision,
                target_assignments,
            } => {
                validators::validate_not_nil_uuid("reconciliation_id", reconciliation_id)?;
                if predecessor_reconciliation_id
                    .is_some_and(|value| value.is_nil() || value == *reconciliation_id)
                    || *reconciliation_revision == 0
                    || *reconciliation_state_revision == 0
                    || *target_assignments == 0
                    || *target_assignments > 1024
                {
                    return Err(EventValidationError::InvalidValue(
                        "artifact node reconciliation identity",
                        "must contain distinct non-nil identities and positive bounded revisions"
                            .to_string(),
                    ));
                }
                validate_sha256_digest("topology_digest", topology_digest)?;
                validate_sha256_digest("policy_revision", policy_revision)
            }
            Self::ModuleArtifactNodeAssignmentObserved {
                reconciliation_id,
                node_id,
                installation_id,
                release_digest,
                reporter_id,
                observation_revision,
                phase,
                report_digest,
            } => {
                validators::validate_not_nil_uuid("reconciliation_id", reconciliation_id)?;
                validators::validate_not_nil_uuid("node_id", node_id)?;
                validators::validate_not_nil_uuid("installation_id", installation_id)?;
                validators::validate_not_empty("reporter_id", reporter_id)?;
                validators::validate_max_length("reporter_id", reporter_id, 128)?;
                if *observation_revision == 0
                    || reporter_id.trim() != reporter_id
                    || reporter_id.chars().any(char::is_control)
                    || !matches!(phase.as_str(), "prepared" | "healthy" | "failed")
                {
                    return Err(EventValidationError::InvalidValue(
                        "artifact node assignment observation",
                        "must contain an exact identity, positive revision, canonical reporter, and agent-reportable phase"
                            .to_string(),
                    ));
                }
                validate_sha256_digest("release_digest", release_digest)?;
                validate_sha256_digest("report_digest", report_digest)
            }
            Self::ModuleArtifactNodeReconciliationStatusChanged {
                reconciliation_id,
                reconciliation_revision,
                reconciliation_state_revision,
                status,
                observed_reconciliation_id,
                failure_code,
            } => {
                validators::validate_not_nil_uuid("reconciliation_id", reconciliation_id)?;
                if *reconciliation_revision == 0
                    || *reconciliation_state_revision == 0
                    || !matches!(
                        status.as_str(),
                        "activating" | "converged" | "failed" | "degraded"
                    )
                    || observed_reconciliation_id.is_some_and(|value| value.is_nil())
                    || (status == "converged"
                        && *observed_reconciliation_id != Some(*reconciliation_id))
                    || matches!(status.as_str(), "failed" | "degraded") != failure_code.is_some()
                {
                    return Err(EventValidationError::InvalidValue(
                        "artifact node reconciliation status",
                        "must contain canonical revision, observed head, and failure state"
                            .to_string(),
                    ));
                }
                if let Some(failure_code) = failure_code {
                    validators::validate_not_empty("failure_code", failure_code)?;
                    validators::validate_max_length("failure_code", failure_code, 128)?;
                }
                Ok(())
            }
            Self::ModuleArtifactSecurityStateChanged {
                module_slug,
                module_version,
                payload_digest,
                security_revision,
                status,
                policy_revision,
                reason_code,
            } => {
                validators::validate_not_empty("module_slug", module_slug)?;
                validators::validate_max_length("module_slug", module_slug, 128)?;
                validators::validate_not_empty("module_version", module_version)?;
                validators::validate_max_length("module_version", module_version, 128)?;
                validate_sha256_digest("payload_digest", payload_digest)?;
                validate_policy_revision(policy_revision)?;
                validators::validate_not_empty("reason_code", reason_code)?;
                validators::validate_max_length("reason_code", reason_code, 128)?;
                if *security_revision == 0
                    || !matches!(status.as_str(), "clear" | "quarantined" | "revoked")
                {
                    return Err(EventValidationError::InvalidValue(
                        "artifact security state",
                        "must contain a positive revision and supported status".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleEffectivePolicyRevisionChanged {
                consumer_key,
                previous_revision,
                next_revision,
            } => {
                validators::validate_not_empty("consumer_key", consumer_key)?;
                validators::validate_max_length("consumer_key", consumer_key, 128)?;
                if consumer_key.trim() != consumer_key || consumer_key.chars().any(char::is_control)
                {
                    return Err(EventValidationError::InvalidValue(
                        "consumer_key",
                        "must be a canonical printable value".to_string(),
                    ));
                }
                if let Some(previous_revision) = previous_revision {
                    validate_sha256_digest("previous_revision", previous_revision)?;
                }
                validate_sha256_digest("next_revision", next_revision)?;
                if previous_revision.as_deref() == Some(next_revision.as_str()) {
                    return Err(EventValidationError::InvalidValue(
                        "policy revision transition",
                        "predecessor and successor must differ".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ModuleGuestEventEmitted {
                module_slug, topic, ..
            } => {
                validators::validate_not_empty("module_slug", module_slug)?;
                validators::validate_not_empty("topic", topic)?;
                Ok(())
            }
            Self::LocaleEnabled { tenant_id, locale }
            | Self::LocaleDisabled { tenant_id, locale } => {
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                validators::validate_not_empty("locale", locale)?;
                validators::validate_max_length("locale", locale, 32)?;
                let normalized =
                    rustok_api::TenantLocale::new(locale.as_str()).map_err(|error| {
                        EventValidationError::InvalidValue("locale", error.to_string())
                    })?;
                if normalized.as_str() != locale {
                    return Err(EventValidationError::InvalidValue(
                        "locale",
                        "tenant locale event tags must already be canonical".to_string(),
                    ));
                }
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

fn validate_sha256_digest(field: &'static str, value: &str) -> Result<(), EventValidationError> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(EventValidationError::InvalidValue(
            field,
            "must be a canonical lowercase sha256 digest".to_string(),
        ));
    }
    Ok(())
}

fn validate_policy_revision(value: &str) -> Result<(), EventValidationError> {
    validators::validate_not_empty("policy_revision", value)?;
    validators::validate_max_length("policy_revision", value, 128)?;
    if value.trim() != value || value.chars().any(char::is_control) {
        return Err(EventValidationError::InvalidValue(
            "policy_revision",
            "must be a canonical printable value".to_string(),
        ));
    }
    Ok(())
}
