use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    TranslationTargetChanged {
        owner_slug: String,
        resource_kind: String,
        resource_id: String,
        changed_locale: String,
        resource_revision: String,
        target_revision: String,
        operation: String,
        correlation_id: String,
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
    ProductUnpublished {
        product_id: Uuid,
    },
    ProductArchived {
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
    TargetDeleted {
        target_type: String,
        target_id: Uuid,
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
    CommentUpdated {
        comment_id: Uuid,
        target_type: String,
        target_id: Uuid,
        author_id: Uuid,
    },
    CommentStatusChanged {
        comment_id: Uuid,
        target_type: String,
        target_id: Uuid,
        author_id: Uuid,
        old_status: String,
        new_status: String,
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
    ModuleArtifactActivated {
        installation_id: Uuid,
        predecessor_installation_id: Option<Uuid>,
        revision: u64,
    },
    ModuleArtifactRolledBack {
        installation_id: Uuid,
        target_installation_id: Uuid,
    },
    ModuleTransitionFinalized {
        operation_id: Uuid,
        module_slug: String,
        revision: u64,
        released_holds: u64,
    },
    ModuleTransitionFailedClosed {
        operation_id: Uuid,
        module_slug: String,
        revision: u64,
        failure_reason: String,
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
    ModuleArtifactSettingsRecoveryPointCreated {
        recovery_point_id: Uuid,
        tenant_id: Uuid,
        installation_id: Uuid,
        settings_instance_id: Uuid,
        settings_revision: u64,
    },
    ModuleArtifactSettingsPurged {
        recovery_point_id: Uuid,
        tenant_id: Uuid,
        installation_id: Uuid,
        settings_instance_id: Uuid,
        tombstone_revision: u64,
    },
    ModuleArtifactSettingsRestored {
        recovery_point_id: Uuid,
        tenant_id: Uuid,
        target_installation_id: Option<Uuid>,
        settings_instance_id: Uuid,
    },
    ModuleArtifactSettingsRecoveryRetentionUpdated {
        recovery_point_id: Uuid,
        tenant_id: Uuid,
        retention_revision: u64,
        retain_until: DateTime<Utc>,
        legal_hold: bool,
        audit_hold: bool,
        incident_hold: bool,
    },
    ModuleArtifactSettingsRecoveryRewrapped {
        recovery_point_id: Uuid,
        tenant_id: Uuid,
        previous_key_version: String,
        key_version: String,
    },
    ModuleArtifactSettingsRecoveryCollected {
        collection_id: Uuid,
        recovery_point_id: Uuid,
        tenant_id: Uuid,
    },
    ModuleArtifactSettingsRecoveryBound {
        recovery_point_id: Uuid,
        tenant_id: Uuid,
        target_installation_id: Uuid,
        settings_instance_id: Uuid,
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
        installation_id: Uuid,
        data_owner_id: Uuid,
        secret_instance_id: Uuid,
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
        bundle_root_digest: Option<String>,
        role_set_digest: Option<String>,
        completion_digest: String,
    },
    ModuleStaticDistributionReleaseAdmitted {
        distribution_release_id: Uuid,
        predecessor_release_id: Option<Uuid>,
        distribution_build_id: Uuid,
        release_revision: u64,
        composition_revision: u64,
        composition_digest: String,
        bundle_root_digest: String,
        role_set_digest: String,
        policy_revision: String,
    },
    ModuleStaticDistributionReleaseActivated {
        distribution_release_id: Uuid,
        predecessor_release_id: Option<Uuid>,
        rollout_id: Uuid,
        release_state_revision: u64,
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
        bundle_root_digest: String,
        role_set_digest: String,
        topology_digest: String,
        policy_revision: String,
        target_assignments: u32,
        executor_mode: String,
    },
    ModuleStaticDistributionRecoveryRequested {
        rollout_id: Uuid,
        predecessor_rollout_id: Uuid,
        from_release_id: Uuid,
        target_release_id: Uuid,
        rollout_revision: u64,
        rollout_state_revision: u64,
        topology_digest: String,
        policy_revision: String,
        reason: String,
    },
    ModuleStaticDistributionRecoveryConverged {
        rollout_id: Uuid,
        from_release_id: Uuid,
        target_release_id: Uuid,
        release_state_revision: u64,
        rollout_state_revision: u64,
    },
    ModuleStaticDistributionAssignmentObserved {
        rollout_id: Uuid,
        node_id: String,
        role: String,
        candidate_artifact_digest: String,
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
    /// Desired dynamic artifact/sandbox assignments have been durably selected
    /// by the module control-plane owner. The payload deliberately contains no
    /// tenant data or assignment list; each node agent claims only its own
    /// exact work item from the owner ledger.
    ModuleArtifactNodeReconciliationRequested {
        reconciliation_id: Uuid,
        predecessor_reconciliation_id: Option<Uuid>,
        reconciliation_revision: u64,
        reconciliation_state_revision: u64,
        topology_digest: String,
        policy_revision: String,
        target_assignments: u32,
    },
    /// One authenticated node agent observed the exact immutable identity it
    /// was assigned. Full evidence remains in the owner ledger.
    ModuleArtifactNodeAssignmentObserved {
        reconciliation_id: Uuid,
        node_id: Uuid,
        installation_id: Uuid,
        release_digest: String,
        reporter_id: String,
        observation_revision: u64,
        phase: String,
        report_digest: String,
    },
    /// The durable dynamic-artifact reconciliation head changed state. A
    /// converged head is the only readiness fact that may serve policy.
    ModuleArtifactNodeReconciliationStatusChanged {
        reconciliation_id: Uuid,
        reconciliation_revision: u64,
        reconciliation_state_revision: u64,
        status: String,
        observed_reconciliation_id: Option<Uuid>,
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
    /// Dynamic guest-emitted event published from an admitted sandbox execution
    /// under the `platform.events` capability.
    ModuleGuestEventEmitted {
        module_slug: String,
        topic: String,
        payload: serde_json::Value,
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
    /// Returns whether this event may be emitted by the platform scope.
    ///
    /// The root envelope represents platform scope with the nil tenant UUID.
    /// Every event not listed here remains tenant-scoped and rejects that
    /// sentinel at the durable event boundary.
    pub fn allows_platform_scope(&self) -> bool {
        matches!(
            self,
            Self::ModuleArtifactAdmitted { .. }
                | Self::ModuleArtifactReverified { .. }
                | Self::ModuleArtifactActivated { .. }
                | Self::ModuleArtifactRolledBack { .. }
                | Self::ModuleTransitionFinalized { .. }
                | Self::ModuleTransitionFailedClosed { .. }
                | Self::ModuleArtifactUninstalled { .. }
                | Self::ModuleArtifactMigrationCheckpointed { .. }
                | Self::ModuleArtifactDeactivated { .. }
                | Self::ModuleStaticPromotionRequested { .. }
                | Self::ModuleStaticPromotionApproved { .. }
                | Self::ModuleStaticDistributionBuildQueued { .. }
                | Self::ModuleStaticDistributionBuildClaimed { .. }
                | Self::ModuleStaticDistributionBuildCompleted { .. }
                | Self::ModuleStaticDistributionReleaseAdmitted { .. }
                | Self::ModuleStaticDistributionReleaseActivated { .. }
                | Self::ModuleStaticDistributionReleaseRevoked { .. }
                | Self::ModuleStaticDistributionRolloutRequested { .. }
                | Self::ModuleStaticDistributionRecoveryRequested { .. }
                | Self::ModuleStaticDistributionRecoveryConverged { .. }
                | Self::ModuleStaticDistributionAssignmentObserved { .. }
                | Self::ModuleStaticDistributionRolloutStatusChanged { .. }
                | Self::ModuleArtifactNodeReconciliationRequested { .. }
                | Self::ModuleArtifactNodeAssignmentObserved { .. }
                | Self::ModuleArtifactNodeReconciliationStatusChanged { .. }
                | Self::ModuleArtifactSecurityStateChanged { .. }
                | Self::BuildRequested { .. }
        )
    }

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
            Self::TranslationTargetChanged { .. } => "translation.target.changed",

            Self::UserAccountRegistered { .. } => "user.account_registered",
            Self::UserLoggedIn { .. } => "user.logged_in",
            Self::UserUpdated { .. } => "user.updated",
            Self::ProfileUpdated { .. } => "profile.updated",
            Self::UserDeleted { .. } => "user.deleted",

            Self::ProductCreated { .. } => "product.created",
            Self::ProductUpdated { .. } => "product.updated",
            Self::ProductPublished { .. } => "product.published",
            Self::ProductUnpublished { .. } => "product.unpublished",
            Self::ProductArchived { .. } => "product.archived",
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
            Self::TargetDeleted { .. } => "target.deleted",
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
            Self::CommentUpdated { .. } => "comment.updated",
            Self::CommentStatusChanged { .. } => "comment.status_changed",
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
            Self::ModuleArtifactActivated { .. } => "module.artifact.activated",
            Self::ModuleArtifactRolledBack { .. } => "module.artifact.rolled_back",
            Self::ModuleTransitionFinalized { .. } => "module.transition.finalized",
            Self::ModuleTransitionFailedClosed { .. } => "module.transition.failed_closed",
            Self::ModuleArtifactUninstalled { .. } => "module.artifact.uninstalled",
            Self::ModuleArtifactMigrationCheckpointed { .. } => {
                "module.artifact.migration_checkpointed"
            }
            Self::ModuleArtifactDeactivated { .. } => "module.artifact.deactivated",
            Self::ModuleArtifactTenantDisabled { .. } => "module.artifact.tenant_disabled",
            Self::ModuleArtifactTenantEnabled { .. } => "module.artifact.tenant_enabled",
            Self::ModuleArtifactDataPurged { .. } => "module.artifact.data_purged",
            Self::ModuleArtifactSettingsRecoveryPointCreated { .. } => {
                "module.artifact.settings_recovery_point_created"
            }
            Self::ModuleArtifactSettingsPurged { .. } => "module.artifact.settings_purged",
            Self::ModuleArtifactSettingsRestored { .. } => "module.artifact.settings_restored",
            Self::ModuleArtifactSettingsRecoveryRetentionUpdated { .. } => {
                "module.artifact.settings_recovery_retention_updated"
            }
            Self::ModuleArtifactSettingsRecoveryRewrapped { .. } => {
                "module.artifact.settings_recovery_rewrapped"
            }
            Self::ModuleArtifactSettingsRecoveryCollected { .. } => {
                "module.artifact.settings_recovery_collected"
            }
            Self::ModuleArtifactSettingsRecoveryBound { .. } => {
                "module.artifact.settings_recovery_bound"
            }
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
            Self::ModuleStaticDistributionReleaseAdmitted { .. } => {
                "module.static_distribution.release_admitted"
            }
            Self::ModuleStaticDistributionReleaseActivated { .. } => {
                "module.static_distribution.release_activated"
            }
            Self::ModuleStaticDistributionReleaseRevoked { .. } => {
                "module.static_distribution.release_revoked"
            }
            Self::ModuleStaticDistributionRolloutRequested { .. } => {
                "module.static_distribution.rollout_requested"
            }
            Self::ModuleStaticDistributionRecoveryRequested { .. } => {
                "module.static_distribution.recovery_requested"
            }
            Self::ModuleStaticDistributionRecoveryConverged { .. } => {
                "module.static_distribution.recovery_converged"
            }
            Self::ModuleStaticDistributionAssignmentObserved { .. } => {
                "module.static_distribution.assignment_observed"
            }
            Self::ModuleStaticDistributionRolloutStatusChanged { .. } => {
                "module.static_distribution.rollout_status_changed"
            }
            Self::ModuleArtifactNodeReconciliationRequested { .. } => {
                "module.artifact_node.reconciliation_requested"
            }
            Self::ModuleArtifactNodeAssignmentObserved { .. } => {
                "module.artifact_node.assignment_observed"
            }
            Self::ModuleArtifactNodeReconciliationStatusChanged { .. } => {
                "module.artifact_node.reconciliation_status_changed"
            }
            Self::ModuleArtifactSecurityStateChanged { .. } => {
                "module.artifact.security_state_changed"
            }
            Self::ModuleEffectivePolicyRevisionChanged { .. } => {
                "module.effective_policy_revision_changed"
            }
            Self::ModuleGuestEventEmitted { .. } => "module.guest.event_emitted",
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
            Self::TranslationTargetChanged { .. } => 1,

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
            Self::ProductUnpublished { .. } => 1,
            Self::ProductArchived { .. } => 1,
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
            Self::TargetDeleted { .. } => 1,
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
            Self::CommentUpdated { .. } => 1,
            Self::CommentStatusChanged { .. } => 1,
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
            Self::ModuleArtifactActivated { .. } => 1,
            Self::ModuleArtifactRolledBack { .. } => 1,
            Self::ModuleTransitionFinalized { .. } => 1,
            Self::ModuleTransitionFailedClosed { .. } => 1,
            Self::ModuleArtifactUninstalled { .. } => 1,
            Self::ModuleArtifactMigrationCheckpointed { .. } => 1,
            Self::ModuleArtifactDeactivated { .. } => 1,
            Self::ModuleArtifactTenantDisabled { .. } => 1,
            Self::ModuleArtifactTenantEnabled { .. } => 1,
            Self::ModuleArtifactDataPurged { .. } => 1,
            Self::ModuleArtifactSettingsRecoveryPointCreated { .. } => 1,
            Self::ModuleArtifactSettingsPurged { .. } => 1,
            Self::ModuleArtifactSettingsRestored { .. } => 1,
            Self::ModuleArtifactSettingsRecoveryRetentionUpdated { .. } => 1,
            Self::ModuleArtifactSettingsRecoveryRewrapped { .. } => 1,
            Self::ModuleArtifactSettingsRecoveryCollected { .. } => 1,
            Self::ModuleArtifactSettingsRecoveryBound { .. } => 1,
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
            Self::ModuleStaticDistributionReleaseAdmitted { .. } => 1,
            Self::ModuleStaticDistributionReleaseActivated { .. } => 1,
            Self::ModuleStaticDistributionReleaseRevoked { .. } => 1,
            Self::ModuleStaticDistributionRolloutRequested { .. } => 1,
            Self::ModuleStaticDistributionRecoveryRequested { .. } => 1,
            Self::ModuleStaticDistributionRecoveryConverged { .. } => 1,
            Self::ModuleStaticDistributionAssignmentObserved { .. } => 1,
            Self::ModuleStaticDistributionRolloutStatusChanged { .. } => 1,
            Self::ModuleArtifactNodeReconciliationRequested { .. } => 1,
            Self::ModuleArtifactNodeAssignmentObserved { .. } => 1,
            Self::ModuleArtifactNodeReconciliationStatusChanged { .. } => 1,
            Self::ModuleArtifactSecurityStateChanged { .. } => 1,
            Self::ModuleEffectivePolicyRevisionChanged { .. } => 1,
            Self::ModuleGuestEventEmitted { .. } => 1,
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
                | Self::ProductUnpublished { .. }
                | Self::ProductArchived { .. }
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
                | Self::UserUpdated { .. }
                | Self::UserDeleted { .. }
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
