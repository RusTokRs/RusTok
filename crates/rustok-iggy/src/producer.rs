use rustok_core::Result;
use rustok_events::{ContractEventEnvelope, EventEnvelope};
use rustok_iggy_connector::PublishRequest;

use crate::MODULE_BUILD_TOPIC;
use crate::config::IggyConfig;
use crate::partitioning::partition_key;
use crate::serialization::EventSerializer;

pub fn build_publish_request(
    config: &IggyConfig,
    serializer: &dyn EventSerializer,
    envelope: EventEnvelope,
) -> Result<PublishRequest> {
    let topic = determine_topic(&envelope.event_type);
    let partition_key = partition_key(envelope.tenant_id);
    let payload = serializer.serialize(&envelope)?;

    Ok(PublishRequest {
        stream: config.topology.stream_name.clone(),
        topic,
        partition_key,
        payload,
        event_id: envelope.id.to_string(),
    })
}

pub fn build_contract_publish_request(
    config: &IggyConfig,
    serializer: &dyn EventSerializer,
    envelope: ContractEventEnvelope,
) -> Result<PublishRequest> {
    envelope
        .validate_registered_schema()
        .map_err(|error| rustok_core::Error::Validation(error.to_string()))?;
    let topic = determine_topic(envelope.event_type());
    let partition_key = partition_key(envelope.tenant_id());
    let payload = serializer.serialize_contract(&envelope)?;

    Ok(PublishRequest {
        stream: config.topology.stream_name.clone(),
        topic,
        partition_key,
        payload,
        event_id: envelope.id().to_string(),
    })
}

fn determine_topic(event_type: &str) -> String {
    if event_type == "module.build.queued" {
        MODULE_BUILD_TOPIC.to_string()
    } else if is_system_event(event_type) {
        "system".to_string()
    } else {
        "domain".to_string()
    }
}

fn is_system_event(event_type: &str) -> bool {
    ["index.", "build."]
        .iter()
        .any(|prefix| event_type.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialization::JsonSerializer;
    use rustok_events::{DomainEvent, EventEnvelope, MarketplaceListingEvent};
    use uuid::Uuid;

    fn create_test_envelope(event_type: &str) -> EventEnvelope {
        let event = if is_system_event(event_type) {
            DomainEvent::ReindexRequested {
                target_type: "test".to_string(),
                target_id: None,
            }
        } else {
            DomainEvent::NodeCreated {
                node_id: Uuid::new_v4(),
                kind: "post".to_string(),
                author_id: None,
            }
        };

        EventEnvelope::new(Uuid::new_v4(), Some(Uuid::new_v4()), event)
    }

    #[test]
    fn build_publish_request_creates_valid_request() {
        let config = IggyConfig::default();
        let serializer = JsonSerializer;
        let envelope = create_test_envelope("node.created");

        let request = build_publish_request(&config, &serializer, envelope.clone()).unwrap();

        assert_eq!(request.stream, "rustok");
        assert_eq!(request.topic, "domain");
        assert_eq!(request.partition_key, envelope.tenant_id.to_string());
        assert_eq!(request.event_id, envelope.id.to_string());
        assert!(!request.payload.is_empty());
    }

    #[test]
    fn determine_topic_routes_domain_events() {
        let envelope = create_test_envelope("node.created");
        assert_eq!(determine_topic(&envelope.event_type), "domain");
    }

    #[test]
    fn determine_topic_routes_system_events() {
        let envelope = create_test_envelope("index.reindex_requested");
        assert_eq!(determine_topic(&envelope.event_type), "system");
    }

    #[test]
    fn determine_topic_routes_build_events_as_system() {
        let event = DomainEvent::BuildRequested {
            build_id: Uuid::new_v4(),
            requested_by: "manual".to_string(),
        };
        let envelope = EventEnvelope::new(Uuid::new_v4(), Some(Uuid::new_v4()), event);

        assert_eq!(determine_topic(&envelope.event_type), "system");
    }

    #[test]
    fn determine_topic_routes_module_build_queue_events_to_dedicated_topic() {
        let event = DomainEvent::ModuleBuildQueued {
            request_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            project_id: "module-demo".to_string(),
            attempt: 1,
        };
        let envelope = EventEnvelope::new(Uuid::new_v4(), None, event);

        assert_eq!(determine_topic(&envelope.event_type), MODULE_BUILD_TOPIC);
    }

    #[test]
    fn partition_key_uses_tenant_id() {
        let tenant_id = Uuid::new_v4();
        let event = DomainEvent::NodeCreated {
            node_id: Uuid::new_v4(),
            kind: "test".to_string(),
            author_id: None,
        };
        let envelope = EventEnvelope::new(tenant_id, None, event);

        let config = IggyConfig::default();
        let serializer = JsonSerializer;

        let request = build_publish_request(&config, &serializer, envelope).unwrap();
        assert_eq!(request.partition_key, tenant_id.to_string());
    }

    #[test]
    fn contract_event_routes_to_domain_without_root_event_deserialization() {
        let tenant_id = Uuid::new_v4();
        let envelope = ContractEventEnvelope::new(
            tenant_id,
            None,
            MarketplaceListingEvent::MarketplaceListingPublished {
                listing_id: Uuid::new_v4(),
                seller_id: Uuid::new_v4(),
                master_product_id: Uuid::new_v4(),
                master_variant_id: Uuid::new_v4(),
                market_slug: "us-market".to_string(),
                channel_slug: "web-store".to_string(),
                terms_version: 2,
            },
        )
        .unwrap();
        let request =
            build_contract_publish_request(&IggyConfig::default(), &JsonSerializer, envelope)
                .unwrap();
        assert_eq!(request.topic, "domain");
        assert_eq!(request.partition_key, tenant_id.to_string());
    }
}
