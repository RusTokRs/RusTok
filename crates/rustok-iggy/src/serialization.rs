use rustok_core::Result;
use rustok_events::{ContractEventEnvelope, EventEnvelope};

use crate::config::SerializationFormat;

pub trait EventSerializer: Send + Sync {
    fn format(&self) -> SerializationFormat;
    fn serialize(&self, envelope: &EventEnvelope) -> Result<Vec<u8>>;
    fn deserialize(&self, payload: &[u8]) -> Result<EventEnvelope>;
    fn serialize_contract(&self, envelope: &ContractEventEnvelope) -> Result<Vec<u8>>;
    fn deserialize_contract(&self, payload: &[u8]) -> Result<ContractEventEnvelope>;
}

#[derive(Debug, Default)]
pub struct JsonSerializer;

impl EventSerializer for JsonSerializer {
    fn format(&self) -> SerializationFormat {
        SerializationFormat::Json
    }

    fn serialize(&self, envelope: &EventEnvelope) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(envelope)?)
    }

    fn deserialize(&self, payload: &[u8]) -> Result<EventEnvelope> {
        let envelope: EventEnvelope = serde_json::from_slice(payload)?;
        envelope
            .validate_registered_schema()
            .map_err(|error| rustok_core::Error::Validation(error.to_string()))?;
        Ok(envelope)
    }

    fn serialize_contract(&self, envelope: &ContractEventEnvelope) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(envelope)?)
    }

    fn deserialize_contract(&self, payload: &[u8]) -> Result<ContractEventEnvelope> {
        Ok(serde_json::from_slice(payload)?)
    }
}

#[derive(Debug, Default)]
pub struct PostcardSerializer;

impl EventSerializer for PostcardSerializer {
    fn format(&self) -> SerializationFormat {
        SerializationFormat::Postcard
    }

    fn serialize(&self, envelope: &EventEnvelope) -> Result<Vec<u8>> {
        postcard::to_stdvec(envelope).map_err(|err| rustok_core::Error::External(err.to_string()))
    }

    fn deserialize(&self, payload: &[u8]) -> Result<EventEnvelope> {
        let envelope: EventEnvelope = postcard::from_bytes(payload)
            .map_err(|err| rustok_core::Error::External(err.to_string()))?;
        envelope
            .validate_registered_schema()
            .map_err(|error| rustok_core::Error::Validation(error.to_string()))?;
        Ok(envelope)
    }

    fn serialize_contract(&self, envelope: &ContractEventEnvelope) -> Result<Vec<u8>> {
        postcard::to_stdvec(envelope).map_err(|err| rustok_core::Error::External(err.to_string()))
    }

    fn deserialize_contract(&self, payload: &[u8]) -> Result<ContractEventEnvelope> {
        postcard::from_bytes(payload).map_err(|err| rustok_core::Error::External(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_core::events::{DomainEvent, EventEnvelope};
    use rustok_events::MarketplaceListingEvent;
    use uuid::Uuid;

    fn create_test_envelope() -> EventEnvelope {
        EventEnvelope::new(
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            DomainEvent::NodeCreated {
                node_id: Uuid::new_v4(),
                kind: "test".to_string(),
                author_id: None,
            },
        )
    }

    fn create_contract_envelope() -> ContractEventEnvelope {
        ContractEventEnvelope::new(
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            MarketplaceListingEvent::MarketplaceListingCreated {
                listing_id: Uuid::new_v4(),
                seller_id: Uuid::new_v4(),
                master_product_id: Uuid::new_v4(),
                master_variant_id: Uuid::new_v4(),
                market_slug: "us-market".to_string(),
                channel_slug: "web-store".to_string(),
                terms_version: 1,
            },
        )
        .expect("valid contract envelope")
    }

    #[test]
    fn json_serializer_format() {
        let serializer = JsonSerializer;
        assert_eq!(serializer.format(), SerializationFormat::Json);
    }

    #[test]
    fn postcard_serializer_format() {
        let serializer = PostcardSerializer;
        assert_eq!(serializer.format(), SerializationFormat::Postcard);
    }

    #[test]
    fn json_serialize_event() {
        let serializer = JsonSerializer;
        let envelope = create_test_envelope();

        let result = serializer.serialize(&envelope);
        assert!(result.is_ok());

        let bytes = result.unwrap();
        assert!(!bytes.is_empty());

        let json_str = String::from_utf8(bytes).unwrap();
        assert!(json_str.contains("node.created"));
    }

    #[test]
    fn postcard_serialize_event() {
        let serializer = PostcardSerializer;
        let envelope = create_test_envelope();

        let result = serializer.serialize(&envelope);
        assert!(result.is_ok());

        let bytes = result.unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn json_roundtrip() {
        let serializer = JsonSerializer;
        let envelope = create_test_envelope();

        let bytes = serializer.serialize(&envelope).unwrap();
        let deserialized = serializer.deserialize(&bytes).unwrap();

        assert_eq!(envelope.id, deserialized.id);
        assert_eq!(envelope.tenant_id, deserialized.tenant_id);
    }

    #[test]
    fn postcard_roundtrip() {
        let serializer = PostcardSerializer;
        let envelope = create_test_envelope();

        let bytes = serializer.serialize(&envelope).unwrap();
        let deserialized = serializer.deserialize(&bytes).unwrap();

        assert_eq!(envelope.id, deserialized.id);
        assert_eq!(envelope.tenant_id, deserialized.tenant_id);
    }

    #[test]
    fn contract_envelope_roundtrips_in_both_formats() {
        let envelope = create_contract_envelope();
        let json = JsonSerializer;
        let json_bytes = json.serialize_contract(&envelope).unwrap();
        let json_roundtrip = json.deserialize_contract(&json_bytes).unwrap();
        assert_eq!(json_roundtrip.id(), envelope.id());
        assert_eq!(json_roundtrip.event_type(), envelope.event_type());

        let postcard = PostcardSerializer;
        let postcard_bytes = postcard.serialize_contract(&envelope).unwrap();
        let postcard_roundtrip = postcard.deserialize_contract(&postcard_bytes).unwrap();
        assert_eq!(postcard_roundtrip.id(), envelope.id());
        assert_eq!(postcard_roundtrip.event_type(), envelope.event_type());
    }
}
