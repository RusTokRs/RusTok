use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxAdminBootstrap {
    pub tenant_slug: Option<String>,
    pub health: String,
    pub counters: Vec<OutboxCounterSnapshot>,
    pub relay_notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxCounterSnapshot {
    pub key: String,
    pub label: String,
    pub value: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxAdminShellText {
    pub badge: String,
    pub title: String,
    pub subtitle: String,
    pub health_label: String,
    pub tenant_context_label: String,
    pub global_tenant_label: String,
    pub relay_notes_title: String,
    pub load_error_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxInfoCardViewModel {
    pub label: String,
    pub value: String,
}

pub fn outbox_info_cards(
    bootstrap: &OutboxAdminBootstrap,
    text: &OutboxAdminShellText,
) -> Vec<OutboxInfoCardViewModel> {
    let mut cards = Vec::with_capacity(bootstrap.counters.len() + 2);
    cards.push(OutboxInfoCardViewModel {
        label: text.health_label.clone(),
        value: bootstrap.health.clone(),
    });
    cards.push(OutboxInfoCardViewModel {
        label: text.tenant_context_label.clone(),
        value: bootstrap
            .tenant_slug
            .clone()
            .unwrap_or_else(|| text.global_tenant_label.clone()),
    });
    cards.extend(
        bootstrap
            .counters
            .iter()
            .map(|counter| OutboxInfoCardViewModel {
                label: counter.label.clone(),
                value: counter.value.to_string(),
            }),
    );
    cards
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_shell_text() -> OutboxAdminShellText {
        OutboxAdminShellText {
            badge: "outbox".to_string(),
            title: "Outbox Relay".to_string(),
            subtitle: "subtitle".to_string(),
            health_label: "Health".to_string(),
            tenant_context_label: "Tenant context".to_string(),
            global_tenant_label: "global".to_string(),
            relay_notes_title: "Relay Notes".to_string(),
            load_error_prefix: "Failed".to_string(),
        }
    }

    #[test]
    fn outbox_info_cards_keep_core_owned_fallbacks() {
        let bootstrap = OutboxAdminBootstrap {
            tenant_slug: None,
            health: "healthy".to_string(),
            counters: vec![OutboxCounterSnapshot {
                key: "pending".to_string(),
                label: "Pending events".to_string(),
                value: 7,
            }],
            relay_notes: vec![],
        };
        let text = test_shell_text();

        let cards = outbox_info_cards(&bootstrap, &text);

        assert_eq!(cards.len(), 3);
        assert_eq!(cards[0].value, "healthy");
        assert_eq!(cards[1].value, "global");
        assert_eq!(cards[2].value, "7");
    }

    #[test]
    fn outbox_info_cards_uses_explicit_tenant_slug() {
        let bootstrap = OutboxAdminBootstrap {
            tenant_slug: Some("acme-corp".to_string()),
            health: "degraded".to_string(),
            counters: vec![],
            relay_notes: vec!["note1".to_string()],
        };
        let text = test_shell_text();

        let cards = outbox_info_cards(&bootstrap, &text);

        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].label, "Health");
        assert_eq!(cards[0].value, "degraded");
        assert_eq!(cards[1].label, "Tenant context");
        assert_eq!(cards[1].value, "acme-corp");
    }

    #[test]
    fn outbox_bootstrap_serde_roundtrip() {
        let bootstrap = OutboxAdminBootstrap {
            tenant_slug: Some("my-tenant".to_string()),
            health: "healthy".to_string(),
            counters: vec![
                OutboxCounterSnapshot {
                    key: "pending".to_string(),
                    label: "Pending".to_string(),
                    value: 12,
                },
                OutboxCounterSnapshot {
                    key: "dispatched".to_string(),
                    label: "Dispatched".to_string(),
                    value: 34,
                },
            ],
            relay_notes: vec!["note-a".to_string()],
        };

        let json = serde_json::to_string(&bootstrap).expect("serialize bootstrap");
        let deserialized: OutboxAdminBootstrap =
            serde_json::from_str(&json).expect("deserialize bootstrap");

        assert_eq!(deserialized.tenant_slug, bootstrap.tenant_slug);
        assert_eq!(deserialized.health, bootstrap.health);
        assert_eq!(deserialized.counters.len(), 2);
        assert_eq!(deserialized.counters[0].key, "pending");
        assert_eq!(deserialized.counters[0].value, 12);
        assert_eq!(deserialized.relay_notes, bootstrap.relay_notes);
    }
}
