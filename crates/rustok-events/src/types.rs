use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ulid::Ulid;
use uuid::Uuid;

use super::validation::{EventValidationError, ValidateEvent, validators};

/// Keeps the JSON event contract human-readable while encoding timestamps as
/// UTC microseconds for non-human-readable formats such as MessagePack.
pub(crate) mod timestamp_serde {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

    pub fn serialize<S>(timestamp: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            timestamp.to_rfc3339().serialize(serializer)
        } else {
            timestamp.timestamp_micros().serialize(serializer)
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let value = String::deserialize(deserializer)?;
            DateTime::parse_from_rfc3339(&value)
                .map(|timestamp| timestamp.with_timezone(&Utc))
                .map_err(D::Error::custom)
        } else {
            let micros = i64::deserialize(deserializer)?;
            DateTime::from_timestamp_micros(micros)
                .ok_or_else(|| D::Error::custom("timestamp microseconds are out of range"))
        }
    }
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
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
    #[serde(with = "timestamp_serde")]
    #[schemars(with = "DateTime<Utc>")]
    pub timestamp: DateTime<Utc>,
    pub actor_id: Option<Uuid>,
    pub event: DomainEvent,
    pub retry_count: u32,
}

impl EventEnvelope {
    pub fn new(tenant_id: Uuid, actor_id: Option<Uuid>, event: DomainEvent) -> Self {
        let id = Uuid::from_bytes(Ulid::r#gen().to_bytes());
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

    /// Validates envelope metadata, the typed payload, and its registered
    /// schema. Every durable and remote ingress path must call this method
    /// before accepting a root event.
    pub fn validate_registered_schema(&self) -> Result<(), EventEnvelopeError> {
        if self.id.is_nil() {
            return Err(EventValidationError::NilUuid("id").into());
        }
        if self.correlation_id.is_nil() {
            return Err(EventValidationError::NilUuid("correlation_id").into());
        }
        if self.tenant_id.is_nil() {
            return Err(EventValidationError::NilUuid("tenant_id").into());
        }
        validators::validate_optional_uuid("causation_id", &self.causation_id)?;
        validators::validate_optional_uuid("actor_id", &self.actor_id)?;
        if let Some(trace_id) = &self.trace_id {
            validators::validate_not_empty("trace_id", trace_id)?;
            validators::validate_max_length("trace_id", trace_id, 512)?;
        }
        self.event.validate()?;

        let schema = crate::event_schema(&self.event_type)
            .ok_or_else(|| EventEnvelopeError::UnregisteredEventType(self.event_type.clone()))?;
        if self.schema_version != schema.version {
            return Err(EventEnvelopeError::SchemaVersionMismatch {
                event_type: self.event_type.clone(),
                envelope_version: self.schema_version,
                registered_version: schema.version,
            });
        }
        if self.event_type != self.event.event_type()
            || self.schema_version != self.event.schema_version()
        {
            return Err(EventEnvelopeError::PayloadMetadataMismatch {
                envelope_type: self.event_type.clone(),
                envelope_version: self.schema_version,
                payload_type: self.event.event_type().to_string(),
                payload_version: self.event.schema_version(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum EventEnvelopeError {
    #[error("event envelope validation failed: {0}")]
    Validation(#[from] EventValidationError),
    #[error("event type `{0}` is not registered")]
    UnregisteredEventType(String),
    #[error(
        "event schema version mismatch for `{event_type}`: envelope={envelope_version}, registered={registered_version}"
    )]
    SchemaVersionMismatch {
        event_type: String,
        envelope_version: u16,
        registered_version: u16,
    },
    #[error(
        "event payload metadata mismatch: envelope=`{envelope_type}`/{envelope_version}, payload=`{payload_type}`/{payload_version}"
    )]
    PayloadMetadataMismatch {
        envelope_type: String,
        envelope_version: u16,
        payload_type: String,
        payload_version: u16,
    },
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
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
    UserAccountRegistered {
        user_id: Uuid,
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
    ProductAttributeCreated {
        attribute_id: Uuid,
    },
    ProductAttributeUpdated {
        attribute_id: Uuid,
    },
    ProductAttributeDeleted {
        attribute_id: Uuid,
    },
    ProductAttributeOptionCreated {
        option_id: Uuid,
        attribute_id: Uuid,
    },
    ProductAttributeOptionUpdated {
        option_id: Uuid,
        attribute_id: Uuid,
    },
    ProductAttributeOptionDeleted {
        option_id: Uuid,
        attribute_id: Uuid,
    },
    ProductAttributeSchemaCreated {
        schema_id: Uuid,
    },
    ProductAttributeSchemaUpdated {
        schema_id: Uuid,
    },
    ProductAttributeSchemaDeleted {
        schema_id: Uuid,
    },
    ProductAttributeSchemaBindingsChanged {
        schema_id: Uuid,
    },
    CatalogCategoryCreated {
        category_id: Uuid,
    },
    CatalogCategoryUpdated {
        category_id: Uuid,
    },
    CatalogCategoryDeleted {
        category_id: Uuid,
    },
    CatalogCategorySchemaModeChanged {
        category_id: Uuid,
    },
    CatalogCategoryAttributesChanged {
        category_id: Uuid,
    },
    ProductPrimaryCategoryChanged {
        product_id: Uuid,
        old_category_id: Option<Uuid>,
        new_category_id: Option<Uuid>,
    },
    ProductCategoryAssignmentsChanged {
        product_id: Uuid,
    },
    ProductAttributeValuesChanged {
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
    BuildRolledBack {
        requested_build_id: Uuid,
        restored_build_id: Uuid,
        from_release_id: String,
        to_release_id: String,
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

    // COMMENT EVENTS
    CommentCreated {
        comment_id: Uuid,
        target_type: String,
        target_id: Uuid,
        author_id: Uuid,
    },
    CommentDeleted {
        comment_id: Uuid,
        target_type: String,
        target_id: Uuid,
        author_id: Uuid,
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
        old_status: String,
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
    CanonicalUrlChanged {
        target_id: Uuid,
        target_kind: String,
        locale: String,
        new_canonical_url: String,
        old_urls: Vec<String>,
    },
    UrlAliasPurged {
        target_id: Uuid,
        target_kind: String,
        locale: String,
        urls: Vec<String>,
    },

    // ════════════════════════════════════════════════════════════════
    // SEO EVENTS
    // ════════════════════════════════════════════════════════════════
    SeoMetaUpserted {
        target_kind: String,
        target_id: Uuid,
        locale: String,
        source: String,
        idempotency_key: String,
    },
    SeoRevisionPublished {
        target_kind: String,
        target_id: Uuid,
        revision: i32,
        idempotency_key: String,
    },
    SeoRevisionRolledBack {
        target_kind: String,
        target_id: Uuid,
        revision: i32,
        idempotency_key: String,
    },
    SeoRedirectUpserted {
        redirect_id: Uuid,
        source_pattern: String,
        target_url: String,
        status_code: i32,
        is_active: bool,
        idempotency_key: String,
    },
    SeoRedirectDisabled {
        redirect_id: Uuid,
        source_pattern: String,
        idempotency_key: String,
    },
    SeoSitemapGenerated {
        job_id: Uuid,
        file_count: i32,
        idempotency_key: String,
    },
    SeoSitemapSubmitted {
        job_id: Uuid,
        endpoint_count: i32,
        success: bool,
        error: Option<String>,
        idempotency_key: String,
    },
    SeoBulkCompleted {
        job_id: Uuid,
        target_kind: String,
        locale: String,
        status: String,
        processed_count: i32,
        succeeded_count: i32,
        failed_count: i32,
        idempotency_key: String,
    },
    SeoBulkPartial {
        job_id: Uuid,
        target_kind: String,
        locale: String,
        status: String,
        processed_count: i32,
        succeeded_count: i32,
        failed_count: i32,
        idempotency_key: String,
    },
    SeoBulkFailed {
        job_id: Uuid,
        target_kind: String,
        locale: String,
        status: String,
        processed_count: i32,
        succeeded_count: i32,
        failed_count: i32,
        idempotency_key: String,
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
    TenantModuleToggled {
        tenant_id: Uuid,
        module_slug: String,
        enabled: bool,
    },
    ModuleArtifactAdmitted {
        installation_id: Uuid,
        artifact_digest: String,
        media_type: String,
        size_bytes: u64,
    },
    ModuleArtifactReverified {
        installation_id: Uuid,
        status: String,
        revision: u64,
    },
    ModuleArtifactRolledBack {
        installation_id: Uuid,
        target_installation_id: Uuid,
    },
    ModuleArtifactUninstalled {
        installation_id: Uuid,
        revision: u64,
    },
    ModuleArtifactMigrationCheckpointed {
        installation_id: Uuid,
        revision: u64,
        has_irreversible_migration: bool,
    },
    ModuleArtifactDeactivated {
        installation_id: Uuid,
        revision: u64,
    },
    ModuleArtifactTenantDisabled {
        installation_id: Uuid,
        tenant_id: Uuid,
        revision: u64,
    },
    ModuleArtifactTenantEnabled {
        installation_id: Uuid,
        tenant_id: Uuid,
        revision: u64,
    },
    ModuleArtifactDataPurged {
        tenant_id: Uuid,
        module_slug: String,
        data_contract_revision: u64,
        namespace_revision: u64,
        purged_records: u64,
    },
    ModuleArtifactDataExported {
        export_id: Uuid,
        tenant_id: Uuid,
        module_slug: String,
        data_contract_revision: u64,
        namespace_revision: u64,
        exported_records: u64,
    },
    ModuleArtifactDataSnapshotCreated {
        snapshot_id: Uuid,
        tenant_id: Uuid,
        module_slug: String,
        data_contract_revision: u64,
        namespace_revision: u64,
        manifest_digest: String,
        structured_records: u64,
        objects: u64,
    },
    ModuleArtifactDataSnapshotRestored {
        snapshot_id: Uuid,
        tenant_id: Uuid,
        module_slug: String,
        data_contract_revision: u64,
        namespace_revision: u64,
        restored_records: u64,
        restored_objects: u64,
    },
    ModuleArtifactDataSnapshotRetentionUpdated {
        snapshot_id: Uuid,
        tenant_id: Uuid,
        retention_revision: u64,
        retain_until: DateTime<Utc>,
        legal_hold: bool,
    },
    ModuleArtifactDataSnapshotCollected {
        collection_id: Uuid,
        snapshot_id: Uuid,
        tenant_id: Uuid,
        module_slug: String,
        data_contract_revision: u64,
        policy_snapshot_id: String,
        deleted_objects: u64,
    },
    ModuleArtifactSecretBound {
        tenant_id: Uuid,
        module_slug: String,
        data_contract_revision: u64,
        revision: u64,
    },
    ModuleBuildQueued {
        request_id: Uuid,
        tenant_id: Uuid,
        project_id: String,
        attempt: u32,
    },
    ModuleBuildCompleted {
        request_id: Uuid,
        tenant_id: Uuid,
        outcome: String,
        retryable: bool,
    },
    ModuleStaticPromotionRequested {
        promotion_id: Uuid,
        release_id: String,
        module_slug: String,
        module_version: String,
        source_digest: String,
    },
    ModuleStaticPromotionApproved {
        promotion_id: Uuid,
        release_id: String,
        module_slug: String,
        module_version: String,
        revision: u64,
        policy_revision: String,
    },
    ModuleStaticDistributionBuildQueued {
        distribution_build_id: Uuid,
        predecessor_build_id: Option<Uuid>,
        composition_revision: u64,
        composition_digest: String,
        selected_promotions: u32,
    },
    ModuleStaticDistributionBuildClaimed {
        distribution_build_id: Uuid,
        claim_id: Uuid,
        attempt_number: u32,
        runner_id: String,
        reclaimed_expired_lease: bool,
    },
    ModuleStaticDistributionBuildCompleted {
        distribution_build_id: Uuid,
        claim_id: Uuid,
        composition_revision: u64,
        composition_digest: String,
        outcome: String,
        result_digest: Option<String>,
        completion_digest: String,
    },
    ModuleStaticDistributionReleaseActivated {
        distribution_release_id: Uuid,
        predecessor_release_id: Option<Uuid>,
        distribution_build_id: Uuid,
        release_revision: u64,
        composition_revision: u64,
        composition_digest: String,
        artifact_digest: String,
        policy_revision: String,
    },
    ModuleStaticDistributionRollbackBuildQueued {
        rollback_id: Uuid,
        from_release_id: Uuid,
        target_release_id: Uuid,
        distribution_build_id: Uuid,
        composition_revision: u64,
        composition_digest: String,
        policy_revision: String,
    },
    ModuleStaticDistributionReleaseRevoked {
        distribution_release_id: Uuid,
        distribution_build_id: Uuid,
        release_state_revision: u64,
        was_active: bool,
        policy_revision: String,
    },
    ModuleStaticDistributionRolloutRequested {
        rollout_id: Uuid,
        predecessor_rollout_id: Option<Uuid>,
        distribution_release_id: Uuid,
        rollout_revision: u64,
        rollout_state_revision: u64,
        composition_revision: u64,
        composition_digest: String,
        artifact_digest: String,
        topology_digest: String,
        policy_revision: String,
        target_nodes: u32,
        executor_mode: String,
    },
    ModuleStaticDistributionNodeObserved {
        rollout_id: Uuid,
        node_id: String,
        reporter_id: String,
        observation_revision: u64,
        phase: String,
        report_digest: String,
    },
    ModuleStaticDistributionRolloutStatusChanged {
        rollout_id: Uuid,
        distribution_release_id: Uuid,
        rollout_revision: u64,
        rollout_state_revision: u64,
        status: String,
        observed_rollout_id: Option<Uuid>,
        failure_code: Option<String>,
    },
    ModuleArtifactSecurityStateChanged {
        module_slug: String,
        module_version: String,
        payload_digest: String,
        security_revision: u64,
        status: String,
        policy_revision: String,
        reason_code: String,
    },
    /// Explicit predecessor-bound effective-policy transition. The envelope
    /// supplies the tenant; this payload identifies the consumer projection
    /// whose cursor must apply the transition exactly once.
    ModuleEffectivePolicyRevisionChanged {
        consumer_key: String,
        previous_revision: Option<String>,
        next_revision: String,
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

            Self::UserAccountRegistered { .. } => "user.account_registered",
            Self::UserLoggedIn { .. } => "user.logged_in",
            Self::UserUpdated { .. } => "user.updated",
            Self::ProfileUpdated { .. } => "profile.updated",
            Self::UserDeleted { .. } => "user.deleted",

            Self::ProductCreated { .. } => "product.created",
            Self::ProductUpdated { .. } => "product.updated",
            Self::ProductPublished { .. } => "product.published",
            Self::ProductDeleted { .. } => "product.deleted",
            Self::ProductAttributeCreated { .. } => "product.attribute.created",
            Self::ProductAttributeUpdated { .. } => "product.attribute.updated",
            Self::ProductAttributeDeleted { .. } => "product.attribute.deleted",
            Self::ProductAttributeOptionCreated { .. } => "product.attribute_option.created",
            Self::ProductAttributeOptionUpdated { .. } => "product.attribute_option.updated",
            Self::ProductAttributeOptionDeleted { .. } => "product.attribute_option.deleted",
            Self::ProductAttributeSchemaCreated { .. } => "product.attribute_schema.created",
            Self::ProductAttributeSchemaUpdated { .. } => "product.attribute_schema.updated",
            Self::ProductAttributeSchemaDeleted { .. } => "product.attribute_schema.deleted",
            Self::ProductAttributeSchemaBindingsChanged { .. } => {
                "product.attribute_schema.bindings_changed"
            }
            Self::CatalogCategoryCreated { .. } => "catalog.category.created",
            Self::CatalogCategoryUpdated { .. } => "catalog.category.updated",
            Self::CatalogCategoryDeleted { .. } => "catalog.category.deleted",
            Self::CatalogCategorySchemaModeChanged { .. } => "catalog.category.schema_mode_changed",
            Self::CatalogCategoryAttributesChanged { .. } => "catalog.category.attributes_changed",
            Self::ProductPrimaryCategoryChanged { .. } => "product.primary_category.changed",
            Self::ProductCategoryAssignmentsChanged { .. } => {
                "product.category_assignments.changed"
            }
            Self::ProductAttributeValuesChanged { .. } => "product.attribute_values.changed",
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
            Self::BuildRolledBack { .. } => "build.rolled_back",

            Self::BlogPostCreated { .. } => "blog.post.created",
            Self::BlogPostPublished { .. } => "blog.post.published",
            Self::BlogPostUnpublished { .. } => "blog.post.unpublished",
            Self::BlogPostUpdated { .. } => "blog.post.updated",
            Self::BlogPostArchived { .. } => "blog.post.archived",
            Self::BlogPostDeleted { .. } => "blog.post.deleted",

            Self::CommentCreated { .. } => "comment.created",
            Self::CommentDeleted { .. } => "comment.deleted",

            Self::ForumTopicCreated { .. } => "forum.topic.created",
            Self::ForumTopicReplied { .. } => "forum.topic.replied",
            Self::ForumTopicStatusChanged { .. } => "forum.topic.status_changed",
            Self::ForumTopicPinned { .. } => "forum.topic.pinned",
            Self::ForumReplyStatusChanged { .. } => "forum.reply.status_changed",
            Self::TopicPromotedToPost { .. } => "content.topic.promoted_to_post",
            Self::PostDemotedToTopic { .. } => "content.post.demoted_to_topic",
            Self::TopicSplit { .. } => "content.topic.split",
            Self::TopicsMerged { .. } => "content.topics.merged",
            Self::CanonicalUrlChanged { .. } => "content.canonical_url.changed",
            Self::UrlAliasPurged { .. } => "content.url_alias.purged",

            Self::SeoMetaUpserted { .. } => "seo.meta.upserted",
            Self::SeoRevisionPublished { .. } => "seo.revision.published",
            Self::SeoRevisionRolledBack { .. } => "seo.revision.rolled_back",
            Self::SeoRedirectUpserted { .. } => "seo.redirect.upserted",
            Self::SeoRedirectDisabled { .. } => "seo.redirect.disabled",
            Self::SeoSitemapGenerated { .. } => "seo.sitemap.generated",
            Self::SeoSitemapSubmitted { .. } => "seo.sitemap.submitted",
            Self::SeoBulkCompleted { .. } => "seo.bulk.completed",
            Self::SeoBulkPartial { .. } => "seo.bulk.partial",
            Self::SeoBulkFailed { .. } => "seo.bulk.failed",

            Self::TenantCreated { .. } => "tenant.created",
            Self::TenantUpdated { .. } => "tenant.updated",
            Self::TenantModuleToggled { .. } => "tenant.module.toggled",
            Self::ModuleArtifactAdmitted { .. } => "module.artifact.admitted",
            Self::ModuleArtifactReverified { .. } => "module.artifact.reverified",
            Self::ModuleArtifactRolledBack { .. } => "module.artifact.rolled_back",
            Self::ModuleArtifactUninstalled { .. } => "module.artifact.uninstalled",
            Self::ModuleArtifactMigrationCheckpointed { .. } => {
                "module.artifact.migration_checkpointed"
            }
            Self::ModuleArtifactDeactivated { .. } => "module.artifact.deactivated",
            Self::ModuleArtifactTenantDisabled { .. } => "module.artifact.tenant_disabled",
            Self::ModuleArtifactTenantEnabled { .. } => "module.artifact.tenant_enabled",
            Self::ModuleArtifactDataPurged { .. } => "module.artifact.data_purged",
            Self::ModuleArtifactDataExported { .. } => "module.artifact.data_exported",
            Self::ModuleArtifactDataSnapshotCreated { .. } => {
                "module.artifact.data_snapshot_created"
            }
            Self::ModuleArtifactDataSnapshotRestored { .. } => {
                "module.artifact.data_snapshot_restored"
            }
            Self::ModuleArtifactDataSnapshotRetentionUpdated { .. } => {
                "module.artifact.data_snapshot_retention_updated"
            }
            Self::ModuleArtifactDataSnapshotCollected { .. } => {
                "module.artifact.data_snapshot_collected"
            }
            Self::ModuleArtifactSecretBound { .. } => "module.artifact.secret_bound",
            Self::ModuleBuildQueued { .. } => "module.build.queued",
            Self::ModuleBuildCompleted { .. } => "module.build.completed",
            Self::ModuleStaticPromotionRequested { .. } => "module.static_promotion.requested",
            Self::ModuleStaticPromotionApproved { .. } => "module.static_promotion.approved",
            Self::ModuleStaticDistributionBuildQueued { .. } => {
                "module.static_distribution.build_queued"
            }
            Self::ModuleStaticDistributionBuildClaimed { .. } => {
                "module.static_distribution.build_claimed"
            }
            Self::ModuleStaticDistributionBuildCompleted { .. } => {
                "module.static_distribution.build_completed"
            }
            Self::ModuleStaticDistributionReleaseActivated { .. } => {
                "module.static_distribution.release_activated"
            }
            Self::ModuleStaticDistributionRollbackBuildQueued { .. } => {
                "module.static_distribution.rollback_build_queued"
            }
            Self::ModuleStaticDistributionReleaseRevoked { .. } => {
                "module.static_distribution.release_revoked"
            }
            Self::ModuleStaticDistributionRolloutRequested { .. } => {
                "module.static_distribution.rollout_requested"
            }
            Self::ModuleStaticDistributionNodeObserved { .. } => {
                "module.static_distribution.node_observed"
            }
            Self::ModuleStaticDistributionRolloutStatusChanged { .. } => {
                "module.static_distribution.rollout_status_changed"
            }
            Self::ModuleArtifactSecurityStateChanged { .. } => {
                "module.artifact.security_state_changed"
            }
            Self::ModuleEffectivePolicyRevisionChanged { .. } => {
                "module.effective_policy_revision_changed"
            }
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
            Self::UserAccountRegistered { .. } => 1,
            Self::UserLoggedIn { .. } => 1,
            Self::UserUpdated { .. } => 1,
            Self::ProfileUpdated { .. } => 1,
            Self::UserDeleted { .. } => 1,

            // Commerce events (v1)
            Self::ProductCreated { .. } => 1,
            Self::ProductUpdated { .. } => 1,
            Self::ProductPublished { .. } => 1,
            Self::ProductDeleted { .. } => 1,
            Self::ProductAttributeCreated { .. } => 1,
            Self::ProductAttributeUpdated { .. } => 1,
            Self::ProductAttributeDeleted { .. } => 1,
            Self::ProductAttributeOptionCreated { .. } => 1,
            Self::ProductAttributeOptionUpdated { .. } => 1,
            Self::ProductAttributeOptionDeleted { .. } => 1,
            Self::ProductAttributeSchemaCreated { .. } => 1,
            Self::ProductAttributeSchemaUpdated { .. } => 1,
            Self::ProductAttributeSchemaDeleted { .. } => 1,
            Self::ProductAttributeSchemaBindingsChanged { .. } => 1,
            Self::CatalogCategoryCreated { .. } => 1,
            Self::CatalogCategoryUpdated { .. } => 1,
            Self::CatalogCategoryDeleted { .. } => 1,
            Self::CatalogCategorySchemaModeChanged { .. } => 1,
            Self::CatalogCategoryAttributesChanged { .. } => 1,
            Self::ProductPrimaryCategoryChanged { .. } => 1,
            Self::ProductCategoryAssignmentsChanged { .. } => 1,
            Self::ProductAttributeValuesChanged { .. } => 1,
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
            Self::BuildRolledBack { .. } => 1,

            // Blog events (v1)
            Self::BlogPostCreated { .. } => 1,
            Self::BlogPostPublished { .. } => 1,
            Self::BlogPostUnpublished { .. } => 1,
            Self::BlogPostUpdated { .. } => 1,
            Self::BlogPostArchived { .. } => 1,
            Self::BlogPostDeleted { .. } => 1,

            Self::CommentCreated { .. } => 1,
            Self::CommentDeleted { .. } => 1,

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
            Self::CanonicalUrlChanged { .. } => 1,
            Self::UrlAliasPurged { .. } => 1,

            // SEO events (v1)
            Self::SeoMetaUpserted { .. } => 1,
            Self::SeoRevisionPublished { .. } => 1,
            Self::SeoRevisionRolledBack { .. } => 1,
            Self::SeoRedirectUpserted { .. } => 1,
            Self::SeoRedirectDisabled { .. } => 1,
            Self::SeoSitemapGenerated { .. } => 1,
            Self::SeoSitemapSubmitted { .. } => 1,
            Self::SeoBulkCompleted { .. } => 1,
            Self::SeoBulkPartial { .. } => 1,
            Self::SeoBulkFailed { .. } => 1,

            // Tenant events (v1)
            Self::TenantCreated { .. } => 1,
            Self::TenantUpdated { .. } => 1,
            Self::TenantModuleToggled { .. } => 1,
            Self::ModuleArtifactAdmitted { .. } => 1,
            Self::ModuleArtifactReverified { .. } => 1,
            Self::ModuleArtifactRolledBack { .. } => 1,
            Self::ModuleArtifactUninstalled { .. } => 1,
            Self::ModuleArtifactMigrationCheckpointed { .. } => 1,
            Self::ModuleArtifactDeactivated { .. } => 1,
            Self::ModuleArtifactTenantDisabled { .. } => 1,
            Self::ModuleArtifactTenantEnabled { .. } => 1,
            Self::ModuleArtifactDataPurged { .. } => 1,
            Self::ModuleArtifactDataExported { .. } => 1,
            Self::ModuleArtifactDataSnapshotCreated { .. } => 1,
            Self::ModuleArtifactDataSnapshotRestored { .. } => 1,
            Self::ModuleArtifactDataSnapshotRetentionUpdated { .. } => 1,
            Self::ModuleArtifactDataSnapshotCollected { .. } => 1,
            Self::ModuleArtifactSecretBound { .. } => 1,
            Self::ModuleBuildQueued { .. } => 1,
            Self::ModuleBuildCompleted { .. } => 1,
            Self::ModuleStaticPromotionRequested { .. } => 1,
            Self::ModuleStaticPromotionApproved { .. } => 1,
            Self::ModuleStaticDistributionBuildQueued { .. } => 1,
            Self::ModuleStaticDistributionBuildClaimed { .. } => 1,
            Self::ModuleStaticDistributionBuildCompleted { .. } => 1,
            Self::ModuleStaticDistributionReleaseActivated { .. } => 1,
            Self::ModuleStaticDistributionRollbackBuildQueued { .. } => 1,
            Self::ModuleStaticDistributionReleaseRevoked { .. } => 1,
            Self::ModuleStaticDistributionRolloutRequested { .. } => 1,
            Self::ModuleStaticDistributionNodeObserved { .. } => 1,
            Self::ModuleStaticDistributionRolloutStatusChanged { .. } => 1,
            Self::ModuleArtifactSecurityStateChanged { .. } => 1,
            Self::ModuleEffectivePolicyRevisionChanged { .. } => 1,
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
                | Self::ProductAttributeCreated { .. }
                | Self::ProductAttributeUpdated { .. }
                | Self::ProductAttributeDeleted { .. }
                | Self::ProductAttributeOptionCreated { .. }
                | Self::ProductAttributeOptionUpdated { .. }
                | Self::ProductAttributeOptionDeleted { .. }
                | Self::ProductAttributeSchemaCreated { .. }
                | Self::ProductAttributeSchemaUpdated { .. }
                | Self::ProductAttributeSchemaDeleted { .. }
                | Self::ProductAttributeSchemaBindingsChanged { .. }
                | Self::CatalogCategoryCreated { .. }
                | Self::CatalogCategoryUpdated { .. }
                | Self::CatalogCategoryDeleted { .. }
                | Self::CatalogCategorySchemaModeChanged { .. }
                | Self::CatalogCategoryAttributesChanged { .. }
                | Self::ProductPrimaryCategoryChanged { .. }
                | Self::ProductCategoryAssignmentsChanged { .. }
                | Self::ProductAttributeValuesChanged { .. }
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
                | Self::CanonicalUrlChanged { .. }
                | Self::UrlAliasPurged { .. }
                | Self::SeoMetaUpserted { .. }
                | Self::SeoRevisionPublished { .. }
                | Self::SeoRevisionRolledBack { .. }
                | Self::SeoRedirectUpserted { .. }
                | Self::SeoRedirectDisabled { .. }
                | Self::SeoSitemapGenerated { .. }
                | Self::SeoSitemapSubmitted { .. }
                | Self::SeoBulkCompleted { .. }
                | Self::SeoBulkPartial { .. }
                | Self::SeoBulkFailed { .. }
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
            Self::ModuleArtifactRolledBack {
                installation_id,
                target_installation_id,
            } => {
                validators::validate_not_nil_uuid("installation_id", installation_id)?;
                validators::validate_not_nil_uuid("target_installation_id", target_installation_id)
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
                data_contract_revision,
                revision,
            } => {
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                validators::validate_not_empty("module_slug", module_slug)?;
                validators::validate_max_length("module_slug", module_slug, 48)?;
                if *data_contract_revision == 0 || *revision == 0 {
                    return Err(EventValidationError::InvalidValue(
                        "data contract or binding revision",
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
                result_digest,
                completion_digest,
            } => {
                validators::validate_not_nil_uuid("distribution_build_id", distribution_build_id)?;
                validators::validate_not_nil_uuid("claim_id", claim_id)?;
                if *composition_revision == 0
                    || !matches!(outcome.as_str(), "succeeded" | "failed" | "cancelled")
                    || (outcome == "succeeded") != result_digest.is_some()
                {
                    return Err(EventValidationError::InvalidValue(
                        "static distribution completion",
                        "must contain a positive revision and canonical terminal evidence"
                            .to_string(),
                    ));
                }
                validate_sha256_digest("composition_digest", composition_digest)?;
                validate_sha256_digest("completion_digest", completion_digest)?;
                if let Some(result_digest) = result_digest {
                    validate_sha256_digest("result_digest", result_digest)?;
                }
                Ok(())
            }
            Self::ModuleStaticDistributionReleaseActivated {
                distribution_release_id,
                predecessor_release_id,
                distribution_build_id,
                release_revision,
                composition_revision,
                composition_digest,
                artifact_digest,
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
                validate_sha256_digest("artifact_digest", artifact_digest)
            }
            Self::ModuleStaticDistributionRollbackBuildQueued {
                rollback_id,
                from_release_id,
                target_release_id,
                distribution_build_id,
                composition_revision,
                composition_digest,
                policy_revision,
            } => {
                validators::validate_not_nil_uuid("rollback_id", rollback_id)?;
                validators::validate_not_nil_uuid("from_release_id", from_release_id)?;
                validators::validate_not_nil_uuid("target_release_id", target_release_id)?;
                validators::validate_not_nil_uuid("distribution_build_id", distribution_build_id)?;
                if from_release_id == target_release_id || *composition_revision == 0 {
                    return Err(EventValidationError::InvalidValue(
                        "static distribution rollback identity",
                        "must contain distinct releases and a positive composition revision"
                            .to_string(),
                    ));
                }
                validate_policy_revision(policy_revision)?;
                validate_sha256_digest("composition_digest", composition_digest)
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
                artifact_digest,
                topology_digest,
                policy_revision,
                target_nodes,
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
                    || *target_nodes == 0
                    || *target_nodes > 1024
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
                validate_sha256_digest("artifact_digest", artifact_digest)?;
                validate_sha256_digest("topology_digest", topology_digest)
            }
            Self::ModuleStaticDistributionNodeObserved {
                rollout_id,
                node_id,
                reporter_id,
                observation_revision,
                phase,
                report_digest,
            } => {
                validators::validate_not_nil_uuid("rollout_id", rollout_id)?;
                validators::validate_not_empty("node_id", node_id)?;
                validators::validate_max_length("node_id", node_id, 128)?;
                validators::validate_not_empty("reporter_id", reporter_id)?;
                validators::validate_max_length("reporter_id", reporter_id, 128)?;
                if *observation_revision == 0
                    || node_id.trim() != node_id
                    || node_id.chars().any(char::is_control)
                    || reporter_id.trim() != reporter_id
                    || reporter_id.chars().any(char::is_control)
                    || !matches!(phase.as_str(), "prepared" | "healthy" | "active" | "failed")
                {
                    return Err(EventValidationError::InvalidValue(
                        "static distribution node observation",
                        "must contain a positive revision, canonical node, and supported phase"
                            .to_string(),
                    ));
                }
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
    fn user_account_registered_requires_only_a_non_nil_identity() {
        let event = DomainEvent::UserAccountRegistered {
            user_id: Uuid::new_v4(),
        };
        assert!(event.validate().is_ok());
    }

    #[test]
    fn user_account_registered_rejects_a_nil_identity() {
        let event = DomainEvent::UserAccountRegistered {
            user_id: Uuid::nil(),
        };
        assert!(event.validate().is_err());
    }

    #[test]
    fn user_account_registered_serialization_contains_no_contact_data() {
        let event = DomainEvent::UserAccountRegistered {
            user_id: Uuid::from_u128(42),
        };

        let serialized = serde_json::to_string(&event).expect("serialize event");
        assert_eq!(event.event_type(), "user.account_registered");
        assert_eq!(event.schema_version(), 1);
        assert!(!serialized.contains("email"));
        assert!(!serialized.contains('@'));
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
    fn test_build_rolled_back_valid_and_metadata() {
        let event = DomainEvent::BuildRolledBack {
            requested_build_id: Uuid::new_v4(),
            restored_build_id: Uuid::new_v4(),
            from_release_id: "release-current".to_string(),
            to_release_id: "release-previous".to_string(),
        };

        assert_eq!(event.event_type(), "build.rolled_back");
        assert_eq!(event.schema_version(), 1);
        assert!(event.validate().is_ok());
    }

    #[test]
    fn test_build_rolled_back_rejects_same_release() {
        let event = DomainEvent::BuildRolledBack {
            requested_build_id: Uuid::new_v4(),
            restored_build_id: Uuid::new_v4(),
            from_release_id: "release-current".to_string(),
            to_release_id: "release-current".to_string(),
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

    #[test]
    fn test_tenant_module_toggled_event_contract() {
        let event = DomainEvent::TenantModuleToggled {
            tenant_id: Uuid::new_v4(),
            module_slug: "blog".to_string(),
            enabled: true,
        };

        assert_eq!(event.event_type(), "tenant.module.toggled");
        assert_eq!(event.schema_version(), 1);
        assert!(event.validate().is_ok());
    }

    #[test]
    fn test_tenant_module_toggled_rejects_empty_module_slug() {
        let event = DomainEvent::TenantModuleToggled {
            tenant_id: Uuid::new_v4(),
            module_slug: "".to_string(),
            enabled: true,
        };

        assert!(event.validate().is_err());
    }

    #[test]
    fn test_effective_policy_revision_transition_is_predecessor_bound() {
        let event = DomainEvent::ModuleEffectivePolicyRevisionChanged {
            consumer_key: "runtime.node-1".to_string(),
            previous_revision: Some(format!("sha256:{}", "a".repeat(64))),
            next_revision: format!("sha256:{}", "b".repeat(64)),
        };

        assert_eq!(
            event.event_type(),
            "module.effective_policy_revision_changed"
        );
        assert_eq!(event.schema_version(), 1);
        assert!(event.validate().is_ok());
    }

    #[test]
    fn test_effective_policy_revision_transition_rejects_noop() {
        let revision = format!("sha256:{}", "a".repeat(64));
        let event = DomainEvent::ModuleEffectivePolicyRevisionChanged {
            consumer_key: "runtime.node-1".to_string(),
            previous_revision: Some(revision.clone()),
            next_revision: revision,
        };

        assert!(event.validate().is_err());
    }
}
