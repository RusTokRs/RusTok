use rustok_events::{
    ContractEventEnvelope, DomainEvent, EVENT_SCHEMAS, EventEnvelope, RootDomainEvent,
    RootEventEnvelope, ValidateEvent, domain_event_json_schema, event_envelope_json_schema,
    event_schema,
};
use uuid::Uuid;

fn id(value: u128) -> Uuid {
    Uuid::from_u128(value)
}

fn digest(nibble: char) -> String {
    format!("sha256:{}", nibble.to_string().repeat(64))
}

fn sample_events() -> Vec<DomainEvent> {
    vec![
        DomainEvent::NodeCreated {
            node_id: id(1),
            kind: "post".to_string(),
            author_id: Some(id(2)),
        },
        DomainEvent::NodeUpdated {
            node_id: id(3),
            kind: "page".to_string(),
        },
        DomainEvent::NodeTranslationUpdated {
            node_id: id(4),
            locale: "en".to_string(),
        },
        DomainEvent::NodePublished {
            node_id: id(5),
            kind: "article".to_string(),
        },
        DomainEvent::NodeUnpublished {
            node_id: id(6),
            kind: "article".to_string(),
        },
        DomainEvent::NodeDeleted {
            node_id: id(7),
            kind: "article".to_string(),
        },
        DomainEvent::BodyUpdated {
            node_id: id(8),
            locale: "en".to_string(),
        },
        DomainEvent::CategoryCreated { category_id: id(9) },
        DomainEvent::CategoryUpdated {
            category_id: id(10),
        },
        DomainEvent::CategoryDeleted {
            category_id: id(11),
        },
        DomainEvent::TagCreated { tag_id: id(12) },
        DomainEvent::TagAttached {
            tag_id: id(13),
            target_type: "node".to_string(),
            target_id: id(14),
        },
        DomainEvent::TagDetached {
            tag_id: id(15),
            target_type: "node".to_string(),
            target_id: id(16),
        },
        DomainEvent::MediaUploaded {
            media_id: id(17),
            mime_type: "image/png".to_string(),
            size: 4096,
        },
        DomainEvent::MediaDeleted { media_id: id(18) },
        DomainEvent::UserRegistered {
            user_id: id(19),
            email: "user@example.com".to_string(),
        },
        DomainEvent::UserLoggedIn { user_id: id(20) },
        DomainEvent::UserUpdated { user_id: id(21) },
        DomainEvent::ProfileUpdated {
            user_id: id(22),
            handle: "creator-one".to_string(),
            locale: Some("en".to_string()),
        },
        DomainEvent::UserDeleted { user_id: id(23) },
        DomainEvent::ProductCreated { product_id: id(24) },
        DomainEvent::ProductUpdated { product_id: id(25) },
        DomainEvent::ProductPublished { product_id: id(26) },
        DomainEvent::ProductDeleted { product_id: id(27) },
        DomainEvent::VariantCreated {
            variant_id: id(28),
            product_id: id(29),
        },
        DomainEvent::VariantUpdated {
            variant_id: id(30),
            product_id: id(31),
        },
        DomainEvent::VariantDeleted {
            variant_id: id(32),
            product_id: id(33),
        },
        DomainEvent::InventoryUpdated {
            variant_id: id(34),
            product_id: id(35),
            location_id: id(36),
            old_quantity: 12,
            new_quantity: 8,
        },
        DomainEvent::InventoryLow {
            variant_id: id(37),
            product_id: id(38),
            remaining: 2,
            threshold: 5,
        },
        DomainEvent::PriceUpdated {
            variant_id: id(39),
            product_id: id(40),
            currency: "USD".to_string(),
            old_amount: Some(1200),
            new_amount: 1500,
        },
        DomainEvent::OrderPlaced {
            order_id: id(41),
            customer_id: Some(id(42)),
            total: 1500,
            currency: "USD".to_string(),
        },
        DomainEvent::OrderStatusChanged {
            order_id: id(43),
            old_status: "pending".to_string(),
            new_status: "paid".to_string(),
        },
        DomainEvent::OrderCompleted { order_id: id(44) },
        DomainEvent::OrderCancelled {
            order_id: id(45),
            reason: Some("customer_request".to_string()),
        },
        DomainEvent::ReindexRequested {
            target_type: "product".to_string(),
            target_id: Some(id(46)),
        },
        DomainEvent::IndexUpdated {
            index_name: "products".to_string(),
            target_id: id(47),
        },
        DomainEvent::BuildRequested {
            build_id: id(48),
            requested_by: "release-bot".to_string(),
        },
        DomainEvent::BuildRolledBack {
            requested_build_id: id(149),
            restored_build_id: id(150),
            from_release_id: "release-current".to_string(),
            to_release_id: "release-previous".to_string(),
        },
        DomainEvent::BlogPostCreated {
            post_id: id(49),
            author_id: Some(id(50)),
            locale: "en".to_string(),
        },
        DomainEvent::BlogPostPublished {
            post_id: id(51),
            author_id: Some(id(52)),
        },
        DomainEvent::BlogPostUnpublished { post_id: id(53) },
        DomainEvent::BlogPostUpdated {
            post_id: id(54),
            locale: "en".to_string(),
        },
        DomainEvent::BlogPostArchived {
            post_id: id(55),
            reason: Some("scheduled_cleanup".to_string()),
        },
        DomainEvent::BlogPostDeleted { post_id: id(56) },
        DomainEvent::CommentCreated {
            comment_id: id(57),
            target_type: "blog_post".to_string(),
            target_id: id(58),
            author_id: id(59),
        },
        DomainEvent::CommentDeleted {
            comment_id: id(60),
            target_type: "blog_post".to_string(),
            target_id: id(61),
            author_id: id(62),
        },
        DomainEvent::ForumTopicCreated {
            topic_id: id(63),
            category_id: id(64),
            author_id: Some(id(65)),
            locale: "en".to_string(),
        },
        DomainEvent::ForumTopicReplied {
            topic_id: id(66),
            reply_id: id(67),
            author_id: Some(id(68)),
        },
        DomainEvent::ForumTopicStatusChanged {
            topic_id: id(63),
            old_status: "open".to_string(),
            new_status: "closed".to_string(),
            moderator_id: Some(id(64)),
        },
        DomainEvent::ForumTopicPinned {
            topic_id: id(65),
            is_pinned: true,
            moderator_id: Some(id(66)),
        },
        DomainEvent::ForumReplyStatusChanged {
            reply_id: id(67),
            topic_id: id(68),
            old_status: "pending".to_string(),
            new_status: "approved".to_string(),
            moderator_id: Some(id(69)),
        },
        DomainEvent::TopicPromotedToPost {
            topic_id: id(70),
            post_id: id(71),
            moved_comments: 3,
            locale: "en".to_string(),
            reason: Some("editorial_promotion".to_string()),
        },
        DomainEvent::PostDemotedToTopic {
            post_id: id(72),
            topic_id: id(73),
            moved_comments: 2,
            locale: "en".to_string(),
            reason: Some("discussion_moved".to_string()),
        },
        DomainEvent::TopicSplit {
            source_topic_id: id(74),
            target_topic_id: id(75),
            moved_comment_ids: vec![id(76), id(77)],
            moved_comments: 2,
            reason: Some("scope_split".to_string()),
        },
        DomainEvent::TopicsMerged {
            target_topic_id: id(78),
            moved_comments: 5,
            reason: Some("duplicate_threads".to_string()),
        },
        DomainEvent::CanonicalUrlChanged {
            target_id: id(79),
            target_kind: "blog_post".to_string(),
            locale: "en".to_string(),
            new_canonical_url: "/modules/blog?slug=release-notes".to_string(),
            old_urls: vec!["/modules/forum?topic=79".to_string()],
        },
        DomainEvent::UrlAliasPurged {
            target_id: id(80),
            target_kind: "forum_topic".to_string(),
            locale: "en".to_string(),
            urls: vec!["/modules/blog?slug=old-thread".to_string()],
        },
        DomainEvent::SeoMetaUpserted {
            target_kind: "product".to_string(),
            target_id: id(801),
            locale: "en".to_string(),
            source: "explicit".to_string(),
            idempotency_key: "seo.meta.upserted:801".to_string(),
        },
        DomainEvent::SeoRevisionPublished {
            target_kind: "product".to_string(),
            target_id: id(802),
            revision: 1,
            idempotency_key: "seo.revision.published:802".to_string(),
        },
        DomainEvent::SeoRevisionRolledBack {
            target_kind: "product".to_string(),
            target_id: id(803),
            revision: 1,
            idempotency_key: "seo.revision.rolled_back:803".to_string(),
        },
        DomainEvent::SeoRedirectUpserted {
            redirect_id: id(804),
            source_pattern: "/old".to_string(),
            target_url: "/new".to_string(),
            status_code: 308,
            is_active: true,
            idempotency_key: "seo.redirect.upserted:804".to_string(),
        },
        DomainEvent::SeoRedirectDisabled {
            redirect_id: id(805),
            source_pattern: "/old".to_string(),
            idempotency_key: "seo.redirect.disabled:805".to_string(),
        },
        DomainEvent::SeoSitemapGenerated {
            job_id: id(806),
            file_count: 2,
            idempotency_key: "seo.sitemap.generated:806".to_string(),
        },
        DomainEvent::SeoSitemapSubmitted {
            job_id: id(807),
            endpoint_count: 1,
            success: true,
            error: None,
            idempotency_key: "seo.sitemap.submitted:807".to_string(),
        },
        DomainEvent::SeoBulkCompleted {
            job_id: id(808),
            target_kind: "product".to_string(),
            locale: "en".to_string(),
            status: "completed".to_string(),
            processed_count: 3,
            succeeded_count: 3,
            failed_count: 0,
            idempotency_key: "seo.bulk.completed:808".to_string(),
        },
        DomainEvent::SeoBulkPartial {
            job_id: id(809),
            target_kind: "product".to_string(),
            locale: "en".to_string(),
            status: "partial".to_string(),
            processed_count: 3,
            succeeded_count: 2,
            failed_count: 1,
            idempotency_key: "seo.bulk.partial:809".to_string(),
        },
        DomainEvent::SeoBulkFailed {
            job_id: id(810),
            target_kind: "product".to_string(),
            locale: "en".to_string(),
            status: "failed".to_string(),
            processed_count: 3,
            succeeded_count: 0,
            failed_count: 3,
            idempotency_key: "seo.bulk.failed:810".to_string(),
        },
        DomainEvent::TenantCreated { tenant_id: id(81) },
        DomainEvent::TenantUpdated { tenant_id: id(82) },
        DomainEvent::TenantModuleToggled {
            tenant_id: id(83),
            module_slug: "blog".to_string(),
            enabled: true,
        },
        DomainEvent::ModuleArtifactAdmitted {
            installation_id: id(1001),
            artifact_digest: digest('a'),
            media_type: "application/vnd.rustok.module.wasm-component.v1+wasm".to_string(),
            size_bytes: 1024,
        },
        DomainEvent::ModuleArtifactReverified {
            installation_id: id(1001),
            status: "verified".to_string(),
            revision: 2,
        },
        DomainEvent::ModuleStaticPromotionRequested {
            promotion_id: id(1002),
            release_id: "release-platform".to_string(),
            module_slug: "search".to_string(),
            module_version: "1.2.3".to_string(),
            source_digest: digest('b'),
        },
        DomainEvent::ModuleStaticPromotionApproved {
            promotion_id: id(1002),
            release_id: "release-platform".to_string(),
            module_slug: "search".to_string(),
            module_version: "1.2.3".to_string(),
            revision: 1,
            policy_revision: "policy-static-1".to_string(),
        },
        DomainEvent::ModuleStaticDistributionBuildQueued {
            distribution_build_id: id(1003),
            predecessor_build_id: Some(id(1004)),
            composition_revision: 7,
            composition_digest: digest('c'),
            selected_promotions: 1,
        },
        DomainEvent::ModuleStaticDistributionBuildClaimed {
            distribution_build_id: id(1003),
            claim_id: id(1005),
            attempt_number: 1,
            runner_id: "distribution-runner-1".to_string(),
            reclaimed_expired_lease: false,
        },
        DomainEvent::ModuleStaticDistributionBuildCompleted {
            distribution_build_id: id(1003),
            claim_id: id(1005),
            composition_revision: 7,
            composition_digest: digest('c'),
            outcome: "succeeded".to_string(),
            result_digest: Some(digest('d')),
            completion_digest: digest('e'),
        },
        DomainEvent::ModuleStaticDistributionReleaseActivated {
            distribution_release_id: id(1006),
            predecessor_release_id: Some(id(1007)),
            distribution_build_id: id(1003),
            release_revision: 2,
            composition_revision: 7,
            composition_digest: digest('c'),
            artifact_digest: digest('d'),
            policy_revision: "policy-static-1".to_string(),
        },
        DomainEvent::ModuleStaticDistributionRollbackBuildQueued {
            rollback_id: id(1008),
            from_release_id: id(1006),
            target_release_id: id(1007),
            distribution_build_id: id(1009),
            composition_revision: 8,
            composition_digest: digest('f'),
            policy_revision: "policy-static-1".to_string(),
        },
        DomainEvent::ModuleStaticDistributionReleaseRevoked {
            distribution_release_id: id(1006),
            distribution_build_id: id(1003),
            release_state_revision: 3,
            was_active: true,
            policy_revision: "policy-static-1".to_string(),
        },
        DomainEvent::ModuleStaticDistributionRolloutRequested {
            rollout_id: id(1010),
            predecessor_rollout_id: Some(id(1011)),
            distribution_release_id: id(1006),
            rollout_revision: 1,
            rollout_state_revision: 1,
            composition_revision: 7,
            composition_digest: digest('c'),
            artifact_digest: digest('d'),
            topology_digest: digest('1'),
            policy_revision: "policy-static-1".to_string(),
            target_nodes: 2,
            executor_mode: "static_native".to_string(),
        },
        DomainEvent::ModuleStaticDistributionNodeObserved {
            rollout_id: id(1010),
            node_id: "node-1".to_string(),
            reporter_id: "deployment-controller".to_string(),
            observation_revision: 1,
            phase: "healthy".to_string(),
            report_digest: digest('2'),
        },
        DomainEvent::ModuleStaticDistributionRolloutStatusChanged {
            rollout_id: id(1010),
            distribution_release_id: id(1006),
            rollout_revision: 1,
            rollout_state_revision: 2,
            status: "converged".to_string(),
            observed_rollout_id: Some(id(1010)),
            failure_code: None,
        },
        DomainEvent::ModuleArtifactSecurityStateChanged {
            module_slug: "search".to_string(),
            module_version: "1.2.3".to_string(),
            payload_digest: digest('d'),
            security_revision: 1,
            status: "clear".to_string(),
            policy_revision: "policy-security-1".to_string(),
            reason_code: "verification_passed".to_string(),
        },
        DomainEvent::LocaleEnabled {
            tenant_id: id(84),
            locale: "en".to_string(),
        },
        DomainEvent::LocaleDisabled {
            tenant_id: id(84),
            locale: "fr".to_string(),
        },
        DomainEvent::FieldDefinitionCreated {
            tenant_id: id(85),
            entity_type: "user".to_string(),
            field_key: "nickname".to_string(),
            field_type: "text".to_string(),
        },
        DomainEvent::FieldDefinitionUpdated {
            tenant_id: id(86),
            entity_type: "product".to_string(),
            field_key: "sku_extra".to_string(),
        },
        DomainEvent::FieldDefinitionDeleted {
            tenant_id: id(87),
            entity_type: "order".to_string(),
            field_key: "legacy_note".to_string(),
        },
        DomainEvent::FlexSchemaCreated {
            tenant_id: id(88),
            schema_id: id(89),
            slug: "faq".to_string(),
        },
        DomainEvent::FlexSchemaUpdated {
            tenant_id: id(90),
            schema_id: id(91),
            slug: "faq".to_string(),
        },
        DomainEvent::FlexSchemaDeleted {
            tenant_id: id(92),
            schema_id: id(93),
        },
        DomainEvent::FlexEntryCreated {
            tenant_id: id(94),
            schema_id: id(95),
            entry_id: id(96),
            entity_type: Some("product".to_string()),
            entity_id: Some(id(97)),
        },
        DomainEvent::FlexEntryUpdated {
            tenant_id: id(98),
            schema_id: id(99),
            entry_id: id(100),
        },
        DomainEvent::FlexEntryDeleted {
            tenant_id: id(101),
            schema_id: id(102),
            entry_id: id(103),
        },
    ]
}

#[test]
fn every_domain_event_variant_is_valid_and_has_matching_schema_metadata() {
    for event in sample_events() {
        event.validate().expect("sample event should be valid");
        let schema = event_schema(event.event_type()).expect("schema must exist");
        assert_eq!(schema.event_type, event.event_type());
        assert_eq!(schema.version, event.schema_version());
    }
}

#[test]
fn every_domain_event_variant_roundtrips_through_envelope_json() {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    for event in sample_events() {
        let envelope = EventEnvelope::new(tenant_id, Some(actor_id), event.clone());
        let json = serde_json::to_value(&envelope).expect("envelope should serialize");
        let restored: EventEnvelope =
            serde_json::from_value(json.clone()).expect("envelope should deserialize");

        assert_eq!(json["event_type"], event.event_type());
        assert_eq!(json["schema_version"], event.schema_version());
        assert_eq!(restored.event_type, envelope.event_type);
        assert_eq!(restored.schema_version, envelope.schema_version);
        assert_eq!(restored.tenant_id, envelope.tenant_id);
        assert_eq!(restored.actor_id, envelope.actor_id);
        assert_eq!(restored.event, envelope.event);
    }
}

#[test]
fn root_aliases_still_build_compatibility_envelopes() {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let event = RootDomainEvent::NodePublished {
        node_id: Uuid::new_v4(),
        kind: "article".to_string(),
    };

    let envelope = RootEventEnvelope::new(tenant_id, Some(actor_id), event);
    let restored: EventEnvelope =
        serde_json::from_value(serde_json::to_value(&envelope).expect("serialize"))
            .expect("deserialize");

    assert_eq!(restored.event_type, "node.published");
    assert_eq!(restored.schema_version, 1);
    assert_eq!(restored.tenant_id, tenant_id);
    assert_eq!(restored.actor_id, Some(actor_id));
}

#[test]
fn schema_registry_covers_curated_root_event_contracts() {
    let schema_event_types: std::collections::BTreeSet<_> = EVENT_SCHEMAS
        .iter()
        .map(|schema| schema.event_type)
        .collect();
    let domain_event_types: std::collections::BTreeSet<_> = sample_events()
        .into_iter()
        .map(|event| event.event_type())
        .collect();

    assert!(
        domain_event_types.is_subset(&schema_event_types),
        "every curated root event contract must have a schema"
    );
}

#[test]
fn schema_registry_covers_all_previously_unregistered_root_event_types() {
    for event_type in [
        "catalog.category.attributes_changed",
        "catalog.category.created",
        "catalog.category.deleted",
        "catalog.category.schema_mode_changed",
        "catalog.category.updated",
        "module.artifact.data_exported",
        "module.artifact.data_purged",
        "module.artifact.data_snapshot_collected",
        "module.artifact.data_snapshot_created",
        "module.artifact.data_snapshot_restored",
        "module.artifact.data_snapshot_retention_updated",
        "module.artifact.deactivated",
        "module.artifact.migration_checkpointed",
        "module.artifact.rolled_back",
        "module.artifact.secret_bound",
        "module.artifact.tenant_disabled",
        "module.artifact.tenant_enabled",
        "module.artifact.uninstalled",
        "module.build.completed",
        "module.build.queued",
        "module.effective_policy_revision_changed",
        "platform_settings.changed",
        "product.attribute.created",
        "product.attribute.deleted",
        "product.attribute.updated",
        "product.attribute_option.created",
        "product.attribute_option.deleted",
        "product.attribute_option.updated",
        "product.attribute_schema.bindings_changed",
        "product.attribute_schema.created",
        "product.attribute_schema.deleted",
        "product.attribute_schema.updated",
        "product.attribute_values.changed",
        "product.category_assignments.changed",
        "product.primary_category.changed",
        "search.rebuild_queued",
        "search.settings_changed",
    ] {
        assert!(
            event_schema(event_type).is_some(),
            "root event type {event_type} must be registered"
        );
    }
}

#[test]
fn contract_envelope_accepts_a_formerly_unregistered_root_event() {
    let envelope = ContractEventEnvelope::new(
        Uuid::new_v4(),
        None,
        DomainEvent::ProductAttributeCreated {
            attribute_id: Uuid::new_v4(),
        },
    )
    .expect("registered root event should build a contract envelope");

    assert_eq!(envelope.event_type(), "product.attribute.created");
}

#[test]
fn generated_json_schemas_are_valid_and_describe_root_wire_contracts() {
    let domain_schema = domain_event_json_schema();
    let envelope_schema = event_envelope_json_schema();

    jsonschema::meta::validate(&domain_schema).expect("domain event schema must be valid");
    jsonschema::meta::validate(&envelope_schema).expect("envelope schema must be valid");
    assert!(domain_schema.is_object());
    assert_eq!(envelope_schema["type"], "object");
}

#[test]
fn root_envelope_rejects_tampered_metadata_and_nil_causation_id() {
    let event = DomainEvent::NodeCreated {
        node_id: Uuid::new_v4(),
        kind: "post".to_string(),
        author_id: None,
    };
    let mut envelope = EventEnvelope::new(Uuid::new_v4(), None, event);
    envelope.event_type = "node.deleted".to_string();
    assert!(envelope.validate_registered_schema().is_err());

    envelope.event_type = envelope.event.event_type().to_string();
    envelope.causation_id = Some(Uuid::nil());
    assert!(envelope.validate_registered_schema().is_err());
}

#[test]
fn event_schema_registry_has_unique_event_types() {
    let mut event_types = std::collections::BTreeSet::new();
    for schema in EVENT_SCHEMAS {
        assert!(
            event_types.insert(schema.event_type),
            "duplicate schema entry for {}",
            schema.event_type
        );
        assert!(schema.version >= 1, "schema versions must start at 1");
    }
}

#[test]
fn field_schema_metadata_generates_valid_json_schema() {
    let schema = rustok_events::EventSchema {
        event_type: "test.schema",
        version: 1,
        description: "Test schema for all supported field primitives.",
        fields: &[
            rustok_events::FieldSchema {
                name: "id",
                data_type: "uuid",
                optional: false,
            },
            rustok_events::FieldSchema {
                name: "signed",
                data_type: "int32",
                optional: false,
            },
            rustok_events::FieldSchema {
                name: "large_signed",
                data_type: "int64",
                optional: false,
            },
            rustok_events::FieldSchema {
                name: "unsigned",
                data_type: "uint64",
                optional: false,
            },
            rustok_events::FieldSchema {
                name: "enabled",
                data_type: "bool",
                optional: false,
            },
            rustok_events::FieldSchema {
                name: "label",
                data_type: "string",
                optional: true,
            },
        ],
    };

    jsonschema::meta::validate(&schema.to_json_schema())
        .expect("field schema metadata must produce valid JSON Schema");
}
