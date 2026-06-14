#![cfg_attr(not(feature = "server"), allow(dead_code))]

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const ORDER_ANALYTICS_TASK_SLUG: &str = "order_analytics";
pub const ORDER_OPS_ASSISTANT_TASK_SLUG: &str = "order_ops_assistant";
pub const ORDER_ANALYTICS_TOOL_NAME: &str = "direct.orders.analytics";
pub const ORDER_OPS_ASSISTANT_TOOL_NAME: &str = "direct.orders.ops_assistant";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderAiVerticalDescriptor {
    pub task_slug: &'static str,
    pub tool_name: &'static str,
    pub sensitive: bool,
}

pub const ORDER_AI_VERTICALS: &[OrderAiVerticalDescriptor] = &[
    OrderAiVerticalDescriptor {
        task_slug: ORDER_ANALYTICS_TASK_SLUG,
        tool_name: ORDER_ANALYTICS_TOOL_NAME,
        sensitive: false,
    },
    OrderAiVerticalDescriptor {
        task_slug: ORDER_OPS_ASSISTANT_TASK_SLUG,
        tool_name: ORDER_OPS_ASSISTANT_TOOL_NAME,
        sensitive: true,
    },
];

/// Domain-owned registration entrypoint for order AI vertical metadata.
pub fn order_ai_verticals() -> &'static [OrderAiVerticalDescriptor] {
    ORDER_AI_VERTICALS
}

/// Backward-compatible entrypoint kept for composition callers. Runtime
/// registration consumes [`order_ai_verticals`] so task identity remains owned
/// by this crate.
pub fn register_order_ai_verticals() -> &'static [OrderAiVerticalDescriptor] {
    order_ai_verticals()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedOrderAnalytics {
    pub summary: String,
    #[serde(default)]
    pub key_findings: Vec<String>,
    #[serde(default)]
    pub risk_flags: Vec<String>,
    #[serde(default)]
    pub recommended_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedOrderOpsAssistant {
    pub recommended_action: String,
    pub rationale: String,
    #[serde(default)]
    pub prefill: Value,
    pub requires_human: bool,
    pub confidence: u8,
}

pub fn validate_order_ops_assistant_confidence(confidence: u8) -> Result<(), String> {
    if confidence > 100 {
        return Err("order_ops_assistant confidence must be between 0 and 100".to_string());
    }
    Ok(())
}

pub fn validate_order_analytics_payload(payload: &GeneratedOrderAnalytics) -> Result<(), String> {
    if payload.summary.trim().is_empty() {
        return Err("order_analytics summary must not be empty".to_string());
    }
    Ok(())
}

pub fn validate_order_ops_assistant_payload(
    payload: &GeneratedOrderOpsAssistant,
) -> Result<(), String> {
    if payload.recommended_action.trim().is_empty() {
        return Err("order_ops_assistant recommended_action must not be empty".to_string());
    }
    if payload.rationale.trim().is_empty() {
        return Err("order_ops_assistant rationale must not be empty".to_string());
    }
    validate_order_ops_assistant_confidence(payload.confidence)
}

#[cfg(test)]
mod tests {
    use super::{
        order_ai_verticals, validate_order_analytics_payload,
        validate_order_ops_assistant_confidence, validate_order_ops_assistant_payload,
        GeneratedOrderAnalytics, GeneratedOrderOpsAssistant, ORDER_ANALYTICS_TASK_SLUG,
        ORDER_OPS_ASSISTANT_TASK_SLUG,
    };

    #[test]
    fn exposes_order_vertical_descriptors() {
        let slugs = order_ai_verticals()
            .iter()
            .map(|vertical| vertical.task_slug)
            .collect::<Vec<_>>();
        assert_eq!(
            slugs,
            vec![ORDER_ANALYTICS_TASK_SLUG, ORDER_OPS_ASSISTANT_TASK_SLUG]
        );
        assert!(order_ai_verticals()[1].sensitive);
    }

    #[test]
    fn accepts_confidence_bounds() {
        assert!(validate_order_ops_assistant_confidence(0).is_ok());
        assert!(validate_order_ops_assistant_confidence(100).is_ok());
    }

    #[test]
    fn rejects_confidence_over_100() {
        let err = validate_order_ops_assistant_confidence(101).unwrap_err();
        assert!(err.contains("between 0 and 100"));
    }

    #[test]
    fn validates_order_analytics_payload() {
        let payload = GeneratedOrderAnalytics {
            summary: "Ready".to_string(),
            key_findings: vec![],
            risk_flags: vec![],
            recommended_actions: vec![],
        };
        assert!(validate_order_analytics_payload(&payload).is_ok());
    }

    #[test]
    fn rejects_empty_order_analytics_summary() {
        let payload = GeneratedOrderAnalytics {
            summary: "  ".to_string(),
            key_findings: vec![],
            risk_flags: vec![],
            recommended_actions: vec![],
        };
        assert!(validate_order_analytics_payload(&payload).is_err());
    }

    #[test]
    fn validates_order_ops_assistant_payload() {
        let payload = GeneratedOrderOpsAssistant {
            recommended_action: "contact_customer".to_string(),
            rationale: "Address mismatch".to_string(),
            prefill: serde_json::json!({}),
            requires_human: true,
            confidence: 80,
        };
        assert!(validate_order_ops_assistant_payload(&payload).is_ok());
    }

    #[test]
    fn rejects_empty_ops_fields() {
        let payload = GeneratedOrderOpsAssistant {
            recommended_action: " ".to_string(),
            rationale: " ".to_string(),
            prefill: serde_json::json!({}),
            requires_human: false,
            confidence: 50,
        };
        assert!(validate_order_ops_assistant_payload(&payload).is_err());
    }
}
