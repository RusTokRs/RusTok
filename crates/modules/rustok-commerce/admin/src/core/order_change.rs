use super::optional_value;

pub struct OrderChangeActionCommand {
    pub change_id: String,
    pub draft: crate::model::CommerceOrderChangeActionDraft,
}

pub fn prepare_order_change_action_command(
    change_id: &str,
    metadata_json: &str,
    reason: &str,
) -> Option<OrderChangeActionCommand> {
    let change_id = change_id.trim().to_string();

    if change_id.is_empty() {
        return None;
    }

    Some(OrderChangeActionCommand {
        change_id,
        draft: crate::model::CommerceOrderChangeActionDraft {
            metadata_json: metadata_json.trim().to_string(),
            reason: reason.trim().to_string(),
        },
    })
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OrderChangeResolutionSummary {
    pub order_return_id: Option<String>,
    pub return_decision_action: Option<String>,
    pub return_decision_source: Option<String>,
    pub cancellation_reason: Option<String>,
}

impl OrderChangeResolutionSummary {
    pub fn has_any(&self) -> bool {
        self.order_return_id.is_some()
            || self.return_decision_action.is_some()
            || self.return_decision_source.is_some()
            || self.cancellation_reason.is_some()
    }

    pub fn order_return_value(&self) -> String {
        optional_value(self.order_return_id.as_deref())
    }

    pub fn return_decision_action_value(&self) -> String {
        optional_value(self.return_decision_action.as_deref())
    }

    pub fn return_decision_source_value(&self) -> String {
        optional_value(self.return_decision_source.as_deref())
    }

    pub fn cancellation_reason_value(&self) -> String {
        optional_value(self.cancellation_reason.as_deref())
    }
}

pub fn order_change_resolution_summary(
    change: &crate::model::CommerceOrderChange,
) -> OrderChangeResolutionSummary {
    let preview = parse_json_object(change.preview.as_str());
    let metadata = parse_json_object(change.metadata.as_str());

    OrderChangeResolutionSummary {
        order_return_id: json_string(&metadata, "order_return_id")
            .or_else(|| json_string(&preview, "order_return_id")),
        return_decision_action: json_string(&metadata, "return_decision_action")
            .or_else(|| json_string(&preview, "return_decision_action"))
            .or_else(|| {
                Some(change.change_type.clone())
                    .filter(|value| value == "exchange" || value == "claim")
            }),
        return_decision_source: json_string(&metadata, "return_decision_source")
            .or_else(|| json_string(&preview, "return_decision_source")),
        cancellation_reason: json_string(&metadata, "cancellation_reason"),
    }
}

fn parse_json_object(value: &str) -> Option<serde_json::Map<String, serde_json::Value>> {
    serde_json::from_str::<serde_json::Value>(value)
        .ok()
        .and_then(|value| value.as_object().cloned())
}

fn json_string(
    object: &Option<serde_json::Map<String, serde_json::Value>>,
    key: &str,
) -> Option<String> {
    object
        .as_ref()
        .and_then(|object| object.get(key))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::CommerceOrderChange;

    fn order_change_with_payload(preview: &str, metadata: &str) -> CommerceOrderChange {
        CommerceOrderChange {
            id: "change-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            order_id: "order-1".to_string(),
            created_by: "operator-1".to_string(),
            change_type: "exchange".to_string(),
            status: "pending".to_string(),
            description: None,
            preview: preview.to_string(),
            metadata: metadata.to_string(),
            created_at: "2026-06-02T00:00:00Z".to_string(),
            updated_at: "2026-06-02T00:00:00Z".to_string(),
            applied_at: None,
            cancelled_at: None,
        }
    }

    #[test]
    fn order_change_action_command_trims_payload_and_requires_id() {
        assert!(prepare_order_change_action_command(" ", "{}", "reason").is_none());

        let command = prepare_order_change_action_command(
            " change-1 ",
            " {\"operator\":true} ",
            " cancelled ",
        )
        .expect("valid command");

        assert_eq!(command.change_id, "change-1");
        assert_eq!(command.draft.metadata_json, "{\"operator\":true}");
        assert_eq!(command.draft.reason, "cancelled");
    }

    #[test]
    fn order_change_resolution_summary_prefers_metadata_context() {
        let change = order_change_with_payload(
            r#"{"order_return_id":"preview-return","return_decision_action":"claim"}"#,
            r#"{"order_return_id":"metadata-return","return_decision_action":"exchange","return_decision_source":"rustok-commerce","cancellation_reason":"operator rejected"}"#,
        );

        let summary = order_change_resolution_summary(&change);

        assert_eq!(summary.order_return_id.as_deref(), Some("metadata-return"));
        assert_eq!(summary.return_decision_action.as_deref(), Some("exchange"));
        assert_eq!(
            summary.return_decision_source.as_deref(),
            Some("rustok-commerce")
        );
        assert_eq!(
            summary.cancellation_reason.as_deref(),
            Some("operator rejected")
        );
        assert!(summary.has_any());
        assert_eq!(summary.order_return_value(), "metadata-return");
        assert_eq!(summary.return_decision_action_value(), "exchange");
        assert_eq!(summary.return_decision_source_value(), "rustok-commerce");
        assert_eq!(summary.cancellation_reason_value(), "operator rejected");
    }

    #[test]
    fn order_change_resolution_summary_falls_back_to_preview_and_change_type() {
        let change = order_change_with_payload(r#"{"order_return_id":"preview-return"}"#, "{}");

        let summary = order_change_resolution_summary(&change);

        assert_eq!(summary.order_return_id.as_deref(), Some("preview-return"));
        assert_eq!(summary.return_decision_action.as_deref(), Some("exchange"));
        assert!(summary.return_decision_source.is_none());
        assert!(summary.cancellation_reason.is_none());
    }
}
