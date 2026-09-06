use rustok_events::SocialGraphRelationEvent;

use crate::entities::relation;

pub(crate) fn event_for_relation(relation: &relation::Model) -> SocialGraphRelationEvent {
    SocialGraphRelationEvent::RelationStateChanged {
        relation_id: relation.id,
        source_user_id: relation.source_user_id,
        target_user_id: relation.target_user_id,
        relation_kind: relation.relation_kind.as_str().to_string(),
        active: relation.active,
        revision: relation.revision,
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rustok_events::ValidateEvent;
    use uuid::Uuid;

    use super::*;
    use crate::model::SocialRelationKind;

    #[test]
    fn maps_relation_without_command_metadata() {
        let now = Utc::now().fixed_offset();
        let relation = relation::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            source_user_id: Uuid::new_v4(),
            target_user_id: Uuid::new_v4(),
            relation_kind: SocialRelationKind::Follow,
            active: true,
            revision: 1,
            created_at: now,
            updated_at: now,
        };
        let event = event_for_relation(&relation);
        event.validate().expect("relation event should validate");
        let encoded = serde_json::to_string(&event).expect("event should serialize");
        for forbidden in [
            "tenant_id",
            "idempotency_key",
            "expected_revision",
            "request_json",
            "response_json",
        ] {
            assert!(!encoded.contains(forbidden), "event leaked {forbidden}");
        }
    }
}
