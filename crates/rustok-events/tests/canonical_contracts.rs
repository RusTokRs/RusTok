use rustok_events::{
    event_schema, DomainEvent, EventEnvelope, RootDomainEvent, RootEventEnvelope, ValidateEvent,
    EVENT_SCHEMAS,
};
use uuid::Uuid;

fn id(value: u128) -> Uuid {
    Uuid::from_u128(value)
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
        DomainEvent::UserDeleted { user_id: id(22) },
        DomainEvent::ProductCreated { product_id: id(23) },
        DomainEvent::ProductUpdated { product_id: id(24) },
        DomainEvent::ProductPublished { product_id: id(25) },
        DomainEvent::ProductDeleted { product_id: id(26) },
        DomainEvent::VariantCreated {
            variant_id: id(27),
            product_id: id(28),
        },
        DomainEvent::VariantUpdated {
            variant_id: id(29),
            product_id: id(30),
        },
        DomainEvent::VariantDeleted {
            variant_id: id(31),
            product_id: id(32),
        },
        DomainEvent::InventoryUpdated {
            variant_id: id(33),
            product_id: id(34),
            location_id: id(35),
            old_quantity: 12,
            new_quantity: 8,
        },
        DomainEvent::InventoryLow {
            variant_id: id(36),
            product_id: id(37),
            remaining: 2,
            threshold: 5,
        },
        DomainEvent::PriceUpdated {
            variant_id: id(38),
            product_id: id(39),
            currency: "USD".to_string(),
            old_amount: Some(1200),
            new_amount: 1500,
        },
        DomainEvent::OrderPlaced {
            order_id: id(40),
            customer_id: Some(id(41)),
            total: 1500,
            currency: "USD".to_string(),
        },
        DomainEvent::OrderStatusChanged {
            order_id: id(42),
            old_status: "pending".to_string(),
            new_status: "paid".to_string(),
        },
        DomainEvent::OrderCompleted { order_id: id(43) },
        DomainEvent::OrderCancelled {
            order_id: id(44),
            reason: Some("customer_request".to_string()),
        },
        DomainEvent::ReindexRequested {
            target_type: "product".to_string(),
            target_id: Some(id(45)),
        },
        DomainEvent::IndexUpdated {
            index_name: "products".to_string(),
            target_id: id(46),
        },
        DomainEvent::BuildRequested {
            build_id: id(47),
            requested_by: "release-bot".to_string(),
        },
        DomainEvent::BlogPostCreated {
            post_id: id(48),
            author_id: Some(id(49)),
            locale: "en".to_string(),
        },
        DomainEvent::BlogPostPublished {
            post_id: id(50),
            author_id: Some(id(51)),
        },
        DomainEvent::BlogPostUnpublished { post_id: id(52) },
        DomainEvent::BlogPostUpdated {
            post_id: id(53),
            locale: "en".to_string(),
        },
        DomainEvent::BlogPostArchived {
            post_id: id(54),
            reason: Some("scheduled_cleanup".to_string()),
        },
        DomainEvent::BlogPostDeleted { post_id: id(55) },
        DomainEvent::ForumTopicCreated {
            topic_id: id(56),
            category_id: id(57),
            author_id: Some(id(58)),
            locale: "en".to_string(),
        },
        DomainEvent::ForumTopicReplied {
            topic_id: id(59),
            reply_id: id(60),
            author_id: Some(id(61)),
        },
        DomainEvent::ForumTopicStatusChanged {
            topic_id: id(62),
            old_status: "open".to_string(),
            new_status: "closed".to_string(),
            moderator_id: Some(id(63)),
        },
        DomainEvent::ForumTopicPinned {
            topic_id: id(64),
            is_pinned: true,
            moderator_id: Some(id(65)),
        },
        DomainEvent::ForumReplyStatusChanged {
            reply_id: id(66),
            topic_id: id(67),
            new_status: "approved".to_string(),
            moderator_id: Some(id(68)),
        },
        DomainEvent::TopicPromotedToPost {
            topic_id: id(69),
            post_id: id(70),
            moved_comments: 3,
            locale: "en".to_string(),
            reason: Some("editorial_promotion".to_string()),
        },
        DomainEvent::PostDemotedToTopic {
            post_id: id(71),
            topic_id: id(72),
            moved_comments: 2,
            locale: "en".to_string(),
            reason: Some("discussion_moved".to_string()),
        },
        DomainEvent::TopicSplit {
            source_topic_id: id(73),
            target_topic_id: id(74),
            moved_comment_ids: vec![id(75), id(76)],
            moved_comments: 2,
            reason: Some("scope_split".to_string()),
        },
        DomainEvent::TopicsMerged {
            target_topic_id: id(77),
            moved_comments: 5,
            reason: Some("duplicate_threads".to_string()),
        },
        DomainEvent::TenantCreated { tenant_id: id(78) },
        DomainEvent::TenantUpdated { tenant_id: id(79) },
        DomainEvent::LocaleEnabled {
            tenant_id: id(80),
            locale: "en".to_string(),
        },
        DomainEvent::LocaleDisabled {
            tenant_id: id(81),
            locale: "fr".to_string(),
        },
        DomainEvent::FieldDefinitionCreated {
            tenant_id: id(82),
            entity_type: "user".to_string(),
            field_key: "nickname".to_string(),
            field_type: "text".to_string(),
        },
        DomainEvent::FieldDefinitionUpdated {
            tenant_id: id(83),
            entity_type: "product".to_string(),
            field_key: "sku_extra".to_string(),
        },
        DomainEvent::FieldDefinitionDeleted {
            tenant_id: id(84),
            entity_type: "order".to_string(),
            field_key: "legacy_note".to_string(),
        },
        DomainEvent::FlexSchemaCreated {
            tenant_id: id(85),
            schema_id: id(86),
            slug: "faq".to_string(),
        },
        DomainEvent::FlexSchemaUpdated {
            tenant_id: id(87),
            schema_id: id(88),
            slug: "faq".to_string(),
        },
        DomainEvent::FlexSchemaDeleted {
            tenant_id: id(89),
            schema_id: id(90),
        },
        DomainEvent::FlexEntryCreated {
            tenant_id: id(91),
            schema_id: id(92),
            entry_id: id(93),
            entity_type: Some("product".to_string()),
            entity_id: Some(id(94)),
        },
        DomainEvent::FlexEntryUpdated {
            tenant_id: id(95),
            schema_id: id(96),
            entry_id: id(97),
        },
        DomainEvent::FlexEntryDeleted {
            tenant_id: id(98),
            schema_id: id(99),
            entry_id: id(100),
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
fn schema_registry_exactly_matches_domain_event_type_set() {
    let schema_event_types: std::collections::BTreeSet<_> = EVENT_SCHEMAS
        .iter()
        .map(|schema| schema.event_type)
        .collect();
    let domain_event_types: std::collections::BTreeSet<_> = sample_events()
        .into_iter()
        .map(|event| event.event_type())
        .collect();

    assert_eq!(schema_event_types, domain_event_types);
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
