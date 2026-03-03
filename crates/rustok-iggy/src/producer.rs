use rustok_core::events::EventEnvelope;
use rustok_core::Result;
use rustok_iggy_connector::PublishRequest;

use crate::config::IggyConfig;
use crate::partitioning::partition_key;
use crate::serialization::EventSerializer;

pub fn build_publish_request(
    config: &IggyConfig,
    serializer: &dyn EventSerializer,
    envelope: EventEnvelope,
) -> Result<PublishRequest> {
    let topic = determine_topic(&envelope);
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

fn determine_topic(envelope: &EventEnvelope) -> String {
    if is_system_event(&envelope.event_type) {
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
    use rustok_core::events::{DomainEvent, EventEnvelope};
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
        assert_eq!(determine_topic(&envelope), "domain");
    }

    #[test]
    fn determine_topic_routes_system_events() {
        let envelope = create_test_envelope("index.reindex_requested");
        assert_eq!(determine_topic(&envelope), "system");
    }

    #[test]
    fn determine_topic_routes_build_events_as_system() {
        let event = DomainEvent::BuildRequested {
            build_id: Uuid::new_v4(),
            requested_by: "manual".to_string(),
        };
        let envelope = EventEnvelope::new(Uuid::new_v4(), Some(Uuid::new_v4()), event);

        assert_eq!(determine_topic(&envelope), "system");
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
}
