use rustok_events::{
    ContractEventEnvelope, ContractEventPayload, MARKETPLACE_LISTING_EVENT_SCHEMAS,
    MarketplaceListingEvent, ValidateEvent, event_schema, event_schemas,
};
use uuid::Uuid;

fn event() -> MarketplaceListingEvent {
    MarketplaceListingEvent::MarketplaceListingCreated {
        listing_id: Uuid::from_u128(1),
        seller_id: Uuid::from_u128(2),
        master_product_id: Uuid::from_u128(3),
        master_variant_id: Uuid::from_u128(4),
        market_slug: "us-market".to_string(),
        channel_slug: "web-store".to_string(),
        terms_version: 1,
    }
}

#[test]
fn listing_event_family_has_nine_registered_versioned_contracts() {
    assert_eq!(MARKETPLACE_LISTING_EVENT_SCHEMAS.len(), 9);
    let registered = event_schemas()
        .filter(|schema| schema.event_type.starts_with("marketplace.listing."))
        .count();
    assert_eq!(registered, 9);
    for schema in MARKETPLACE_LISTING_EVENT_SCHEMAS {
        assert_eq!(schema.version, 1);
        let found = event_schema(schema.event_type).expect("registered event schema");
        assert_eq!(found.event_type, schema.event_type);
        assert_eq!(found.version, schema.version);
    }
}

#[test]
fn listing_event_contract_is_typed_validated_and_enveloped() {
    let event = event();
    assert_eq!(event.event_type(), "marketplace.listing.created");
    assert_eq!(event.schema_version(), 1);
    event.validate().expect("valid listing event");

    let envelope =
        ContractEventEnvelope::new(Uuid::from_u128(10), Some(Uuid::from_u128(11)), event)
            .expect("valid contract envelope");
    assert_eq!(envelope.event_type(), "marketplace.listing.created");
    assert_eq!(envelope.schema_version(), 1);
    assert!(matches!(
        envelope.payload().expect("validated payload"),
        ContractEventPayload::MarketplaceListing(
            MarketplaceListingEvent::MarketplaceListingCreated { .. }
        )
    ));
    assert!(matches!(
        envelope.into_payload().expect("validated payload"),
        ContractEventPayload::MarketplaceListing(
            MarketplaceListingEvent::MarketplaceListingCreated { .. }
        )
    ));
}

#[test]
fn listing_event_rejects_noncanonical_scope_and_invalid_version() {
    let invalid = MarketplaceListingEvent::MarketplaceListingTermsUpdated {
        listing_id: Uuid::from_u128(1),
        seller_id: Uuid::from_u128(2),
        master_product_id: Uuid::from_u128(3),
        master_variant_id: Uuid::from_u128(4),
        market_slug: "US_market".to_string(),
        channel_slug: "-web".to_string(),
        terms_version: 0,
    };
    assert!(invalid.validate().is_err());
}

#[test]
fn decoded_listing_envelope_revalidates_payload_fields() {
    let envelope = ContractEventEnvelope::new(Uuid::from_u128(10), None, event())
        .expect("valid contract envelope");
    let mut value = serde_json::to_value(envelope).expect("serialize envelope");
    value["event"]["event"]["data"]["terms_version"] = serde_json::json!(0);
    let decoded: ContractEventEnvelope =
        serde_json::from_value(value).expect("decode structurally valid envelope");
    assert!(decoded.validate_registered_schema().is_err());
    assert!(decoded.payload().is_err());
}

#[test]
fn listing_external_payload_excludes_owner_private_prose_and_metadata() {
    let value = serde_json::to_value(event()).expect("serialize listing event");
    let encoded = value.to_string();
    for forbidden in [
        "note",
        "reason",
        "metadata",
        "approval_note",
        "suspension_reason",
    ] {
        assert!(!encoded.contains(forbidden), "payload leaked {forbidden}");
    }
}
