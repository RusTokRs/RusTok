//! Property-based tests for core validation invariants.

#[cfg(test)]
mod tenant_validation_tests {
    use crate::tenant_validation::{TenantIdentifierValidator, TenantValidationError};
    use proptest::prelude::*;

    fn not_reserved(result: &Result<String, TenantValidationError>) -> bool {
        !matches!(result, Err(TenantValidationError::Reserved(_)))
    }

    proptest! {
        #[test]
        fn valid_slug_pattern_is_accepted(s in "[a-z0-9]([a-z0-9-]{0,62})?") {
            let result = TenantIdentifierValidator::validate_slug(&s);
            if not_reserved(&result) {
                prop_assert!(result.is_ok());
            }
        }

        #[test]
        fn uppercase_slug_is_normalized(s in "[a-zA-Z0-9][a-zA-Z0-9-]{0,62}") {
            let result = TenantIdentifierValidator::validate_slug(&s);
            if let Ok(normalized) = result {
                prop_assert!(normalized.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'));
            }
        }

        #[test]
        fn empty_slug_is_rejected(_ in Just(())) {
            let result = TenantIdentifierValidator::validate_slug("");
            prop_assert!(matches!(result, Err(TenantValidationError::Empty)));
        }

        #[test]
        fn long_slug_is_rejected(s in "[a-z0-9-]{65,}") {
            let result = TenantIdentifierValidator::validate_slug(&s);
            prop_assert!(matches!(result, Err(TenantValidationError::TooLong)));
        }

        #[test]
        fn hyphen_boundaries_are_rejected_start(s in "-[a-z0-9-]{0,62}") {
            prop_assert!(TenantIdentifierValidator::validate_slug(&s).is_err());
        }

        #[test]
        fn spaces_are_rejected(s in "[a-z0-9 ]{1,64}") {
            prop_assume!(s.contains(' '));
            let result = TenantIdentifierValidator::validate_slug(&s);
            if s.trim() == s {
                prop_assert!(result.is_err());
            }
        }

        #[test]
        fn special_characters_are_rejected(s in "[!@#$%^&*()_=+]{1,}") {
            prop_assert!(TenantIdentifierValidator::validate_slug(&s).is_err());
        }

        #[test]
        fn slug_trims_whitespace(s in "([a-z0-9-]{1,10}\\s+){1,3}[a-z0-9-]{1,10}") {
            let trimmed = s.trim();
            let result = TenantIdentifierValidator::validate_slug(&s);
            if !trimmed.is_empty()
                && trimmed.len() <= 64
                && trimmed.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                && not_reserved(&result)
            {
                prop_assert!(result.is_ok());
                prop_assert_eq!(result.unwrap(), trimmed);
            }
        }

        #[test]
        fn valid_uuid_is_accepted(uuid in any::<[u8; 16]>()) {
            let uuid_str = uuid::Uuid::from_bytes(uuid).to_string();
            prop_assert!(TenantIdentifierValidator::validate_uuid(&uuid_str).is_ok());
        }

        #[test]
        fn uppercase_uuid_is_normalized(uuid in any::<[u8; 16]>()) {
            let uuid_str = uuid::Uuid::from_bytes(uuid).to_string().to_uppercase();
            let result = TenantIdentifierValidator::validate_uuid(&uuid_str);
            prop_assert!(result.is_ok());
            prop_assert!(result.unwrap().to_string().chars().all(|c| !c.is_ascii_uppercase()));
        }

        #[test]
        fn invalid_uuid_format_is_rejected(s in "[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{3}-[^0-9a-f][0-9a-f]{3}-[0-9a-f]{12}") {
            prop_assert!(TenantIdentifierValidator::validate_uuid(&s).is_err());
        }

        #[test]
        fn valid_host_is_accepted(host in "[a-z0-9]+(\\.[a-z0-9]+){1,3}") {
            prop_assert!(TenantIdentifierValidator::validate_host(&host).is_ok());
        }

        #[test]
        fn host_with_consecutive_dots_is_rejected(s in "[a-z]+\\.{2,}[a-z]+") {
            prop_assert!(TenantIdentifierValidator::validate_host(&s).is_err());
        }

        #[test]
        fn validate_any_accepts_valid_uuid(uuid in any::<[u8; 16]>()) {
            let uuid_str = uuid::Uuid::from_bytes(uuid).to_string();
            prop_assert!(TenantIdentifierValidator::validate_any(&uuid_str).is_ok());
        }

        #[test]
        fn validate_any_accepts_valid_hostname(host in "[a-z0-9]+(\\.[a-z0-9]+){1,3}") {
            prop_assert!(TenantIdentifierValidator::validate_any(&host).is_ok());
        }
    }
}

#[cfg(test)]
mod event_validation_tests {
    use crate::events::validation::{validators, EventValidationError, ValidateEvent};
    use crate::DomainEvent;
    use proptest::prelude::*;
    use uuid::Uuid;

    proptest! {
        #[test]
        fn validate_not_empty_accepts_non_empty(s in "[a-zA-Z0-9]{1,100}") {
            prop_assert!(validators::validate_not_empty("field", &s).is_ok());
        }

        #[test]
        fn validate_not_empty_rejects_whitespace(s in "[ \\t\\n\\r]{1,10}") {
            prop_assert!(matches!(
                validators::validate_not_empty("field", &s),
                Err(EventValidationError::EmptyField(_))
            ));
        }

        #[test]
        fn validate_max_length_boundary_case(max in 10usize..100usize) {
            let exact = "a".repeat(max);
            prop_assert!(validators::validate_max_length("field", &exact, max).is_ok());

            let over = "a".repeat(max + 1);
            prop_assert!(validators::validate_max_length("field", &over, max).is_err());
        }

        #[test]
        fn validate_not_nil_uuid_accepts_non_nil(uuid in any::<[u8; 16]>()) {
            prop_assume!(uuid != [0u8; 16]);
            let uuid_obj = Uuid::from_bytes(uuid);
            prop_assert!(validators::validate_not_nil_uuid("field", &uuid_obj).is_ok());
        }

        #[test]
        fn validate_not_nil_uuid_rejects_nil(_ in Just(())) {
            let nil = Uuid::nil();
            prop_assert!(matches!(
                validators::validate_not_nil_uuid("field", &nil),
                Err(EventValidationError::NilUuid(_))
            ));
        }

        #[test]
        fn validate_range_boundary_cases(min in -100i64..100i64) {
            let max = min + 10;
            prop_assert!(validators::validate_range("field", min, min, max).is_ok());
            prop_assert!(validators::validate_range("field", max, min, max).is_ok());
            prop_assert!(validators::validate_range("field", min - 1, min, max).is_err());
            prop_assert!(validators::validate_range("field", max + 1, min, max).is_err());
        }

        #[test]
        fn node_created_kind_len_respected(kind in "[a-z]{1,100}") {
            let event = DomainEvent::NodeCreated {
                node_id: Uuid::new_v4(),
                kind: kind.clone(),
                author_id: None,
            };

            if kind.len() <= 64 {
                prop_assert!(event.validate().is_ok());
            } else {
                prop_assert!(event.validate().is_err());
            }
        }

        #[test]
        fn nil_node_id_is_rejected(_ in Just(())) {
            let event = DomainEvent::NodeCreated {
                node_id: Uuid::nil(),
                kind: "article".to_string(),
                author_id: None,
            };
            prop_assert!(matches!(event.validate(), Err(EventValidationError::NilUuid(_))));
        }

        #[test]
        fn valid_node_id_is_accepted(uuid in any::<[u8; 16]>()) {
            prop_assume!(uuid != [0u8; 16]);
            let event = DomainEvent::NodeCreated {
                node_id: Uuid::from_bytes(uuid),
                kind: "article".to_string(),
                author_id: None,
            };
            prop_assert!(event.validate().is_ok());
        }
    }
}

#[cfg(test)]
mod event_serialization_tests {
    use crate::{DomainEvent, EventEnvelope};
    use proptest::prelude::*;
    use uuid::Uuid;

    proptest! {
        #[test]
        fn event_roundtrip(kind in "[a-z]{1,50}") {
            let original = DomainEvent::NodeCreated {
                node_id: Uuid::new_v4(),
                kind,
                author_id: Some(Uuid::new_v4()),
            };

            let json = serde_json::to_string(&original).unwrap();
            let decoded: DomainEvent = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(decoded, original);
        }

        #[test]
        fn envelope_roundtrip(tenant_id in any::<[u8; 16]>()) {
            let original = EventEnvelope::new(
                Uuid::from_bytes(tenant_id),
                Some(Uuid::new_v4()),
                DomainEvent::NodeCreated {
                    node_id: Uuid::new_v4(),
                    kind: "article".to_string(),
                    author_id: None,
                },
            );

            let json = serde_json::to_string(&original).unwrap();
            let decoded: EventEnvelope = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(original.id, decoded.id);
            prop_assert_eq!(original.tenant_id, decoded.tenant_id);
            prop_assert_eq!(original.event, decoded.event);
        }

        #[test]
        fn event_serialization_produces_valid_json(kind in "[a-z]{1,50}") {
            let event = DomainEvent::NodeCreated {
                node_id: Uuid::new_v4(),
                kind,
                author_id: Some(Uuid::new_v4()),
            };

            let json = serde_json::to_string(&event).unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            prop_assert!(parsed.is_object());
        }

        #[test]
        fn envelope_has_required_fields(_ in Just(())) {
            let envelope = EventEnvelope::new(
                Uuid::new_v4(),
                Some(Uuid::new_v4()),
                DomainEvent::NodeCreated {
                    node_id: Uuid::new_v4(),
                    kind: "article".to_string(),
                    author_id: None,
                },
            );

            let json = serde_json::to_string(&envelope).unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            prop_assert!(parsed.get("id").is_some());
            prop_assert!(parsed.get("event_type").is_some());
            prop_assert!(parsed.get("tenant_id").is_some());
            prop_assert!(parsed.get("event").is_some());
        }

        #[test]
        fn json_contains_type_and_data(kind in "[a-z]{1,20}") {
            let event = DomainEvent::NodeCreated {
                node_id: Uuid::new_v4(),
                kind,
                author_id: None,
            };

            let json = serde_json::to_string(&event).unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            prop_assert!(parsed.get("type").is_some());
            prop_assert!(parsed.get("data").is_some());
            prop_assert!(parsed.get("data").unwrap().is_object());
        }
    }
}
