//! Framework-neutral host rendering contract for admitted artifact UI.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Host surface on which one declarative artifact contribution may render.
/// Values name platform presentation slots, never a host package, URL, iframe,
/// or executable component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactUiSurface {
    AdminSettings,
    AdminActions,
    AdminStatus,
    AdminHelp,
    AdminNavigation,
    AdminTable,
    AdminForm,
    StorefrontSlot,
}

/// User confirmation required before an admitted action or form dispatches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactUiActionConfirmation {
    None,
    Acknowledge,
    Destructive,
}

/// Host-safe localized projection of one admitted artifact UI contribution.
/// It never exposes raw locale catalogs, localization keys, binding IDs,
/// permissions, or executable UI material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactUiContributionView {
    pub id: String,
    pub surface: ArtifactUiSurface,
    pub content: ArtifactUiContributionViewContent,
}

/// Localized declarative content rendered exclusively with host-owned shared
/// primitives. Schema values are immutable admitted metadata, never guest UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ArtifactUiContributionViewContent {
    Settings {
        title: String,
        schema: Value,
    },
    Action {
        title: String,
        confirmation: ArtifactUiActionConfirmation,
        destructive: bool,
    },
    Status {
        title: String,
        status: String,
    },
    Help {
        title: String,
        body: String,
    },
    Navigation {
        title: String,
        route: String,
    },
    Table {
        title: String,
        schema: Value,
    },
    Form {
        title: String,
        schema: Value,
        confirmation: ArtifactUiActionConfirmation,
        destructive: bool,
    },
    StorefrontSlot {
        title: String,
        slot: String,
    },
}

/// Redacted execution evidence for the admitted binding selected by one
/// host-rendered action or form. It deliberately contains no binding identity,
/// input, output, actor, trace, credential, capability, or grant data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactBindingExecutionAuditEntry {
    pub execution_id: Uuid,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_ms: Option<u64>,
    pub error_code: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_round_trips_without_hidden_descriptor_identity() {
        let view = ArtifactUiContributionView {
            id: "profile_form".to_string(),
            surface: ArtifactUiSurface::AdminForm,
            content: ArtifactUiContributionViewContent::Form {
                title: "Profile".to_string(),
                schema: serde_json::json!({"type": "object"}),
                confirmation: ArtifactUiActionConfirmation::Acknowledge,
                destructive: false,
            },
        };

        let encoded = serde_json::to_value(&view).expect("view serializes");
        assert_eq!(encoded["content"]["kind"], "form");
        assert!(encoded.get("binding_id").is_none());
        assert!(encoded.get("permission").is_none());
        assert_eq!(
            serde_json::from_value::<ArtifactUiContributionView>(encoded)
                .expect("view deserializes"),
            view
        );
    }

    #[test]
    fn projection_rejects_unadmitted_descriptor_fields() {
        assert!(
            serde_json::from_value::<ArtifactUiContributionView>(serde_json::json!({
                "id": "profile_form",
                "surface": "admin_form",
                "content": {
                    "kind": "form",
                    "title": "Profile",
                    "schema": {"type": "object"},
                    "confirmation": "none",
                    "destructive": false,
                    "binding_id": "untrusted"
                }
            }))
            .is_err()
        );
    }

    #[test]
    fn audit_entry_round_trips_without_hidden_binding_identity() {
        let entry = ArtifactBindingExecutionAuditEntry {
            execution_id: uuid::uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
            status: "succeeded".to_string(),
            started_at: "2026-08-22T12:00:00Z".to_string(),
            finished_at: Some("2026-08-22T12:00:01Z".to_string()),
            duration_ms: Some(1_000),
            error_code: None,
        };

        let encoded = serde_json::to_value(&entry).expect("entry serializes");
        assert!(encoded.get("binding_id").is_none());
        assert!(encoded.get("actor_id").is_none());
        assert_eq!(
            serde_json::from_value::<ArtifactBindingExecutionAuditEntry>(encoded)
                .expect("entry deserializes"),
            entry
        );
    }
}
