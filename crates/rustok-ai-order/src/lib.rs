#![cfg_attr(not(feature = "server"), allow(dead_code))]

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const ORDER_ANALYTICS_TASK_SLUG: &str = "order_analytics";
pub const ORDER_OPS_ASSISTANT_TASK_SLUG: &str = "order_ops_assistant";
pub const ORDER_ANALYTICS_TOOL_NAME: &str = "direct.orders.analytics";
pub const ORDER_OPS_ASSISTANT_TOOL_NAME: &str = "direct.orders.ops_assistant";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderAiExecutionPolicy {
    pub review_required: bool,
    pub persistence: &'static str,
}

const ADVISORY_ORDER_POLICY: OrderAiExecutionPolicy = OrderAiExecutionPolicy {
    review_required: true,
    persistence: "none",
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderAiVerticalDescriptor {
    pub task_slug: &'static str,
    pub tool_name: &'static str,
    pub sensitive: bool,
    pub execution_policy: OrderAiExecutionPolicy,
}

pub const ORDER_AI_VERTICALS: &[OrderAiVerticalDescriptor] = &[
    OrderAiVerticalDescriptor {
        task_slug: ORDER_ANALYTICS_TASK_SLUG,
        tool_name: ORDER_ANALYTICS_TOOL_NAME,
        sensitive: false,
        execution_policy: ADVISORY_ORDER_POLICY,
    },
    OrderAiVerticalDescriptor {
        task_slug: ORDER_OPS_ASSISTANT_TASK_SLUG,
        tool_name: ORDER_OPS_ASSISTANT_TOOL_NAME,
        sensitive: true,
        execution_policy: ADVISORY_ORDER_POLICY,
    },
];

/// Domain-owned registration entrypoint for order AI vertical metadata.
pub fn order_ai_verticals() -> &'static [OrderAiVerticalDescriptor] {
    ORDER_AI_VERTICALS
}

/// Returns the domain-owned execution policy for an order AI vertical.
/// Generated order output is always advisory; an owner-owned operator flow
/// must apply any order mutation separately.
pub fn order_ai_execution_policy(task_slug: &str) -> Option<OrderAiExecutionPolicy> {
    order_ai_verticals()
        .iter()
        .find(|vertical| vertical.task_slug == task_slug)
        .map(|vertical| vertical.execution_policy)
}

/// Backward-compatible entrypoint kept for composition callers. Runtime
/// registration consumes [`order_ai_verticals`] so task identity remains owned
/// by this crate.
pub fn register_order_ai_verticals() -> &'static [OrderAiVerticalDescriptor] {
    order_ai_verticals()
}

/// Domain-owned adapter API for runtime composition layers that need to bind
/// concrete handlers to the vertical descriptors without owning task identity.
pub fn register_order_ai_vertical_handlers(
    mut register: impl FnMut(&'static OrderAiVerticalDescriptor),
) {
    for vertical in order_ai_verticals() {
        register(vertical);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GeneratedOrderAnalytics {
    pub summary: String,
    #[serde(default)]
    pub key_findings: Vec<String>,
    #[serde(default)]
    pub risk_flags: Vec<String>,
    #[serde(default)]
    pub recommended_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
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

fn ensure_non_blank_values(task: &str, field: &str, values: &[String]) -> Result<(), String> {
    if values.iter().any(|value| value.trim().is_empty()) {
        return Err(format!(
            "{task} {field} must contain only non-empty strings"
        ));
    }
    Ok(())
}

pub fn validate_order_analytics_payload(payload: &GeneratedOrderAnalytics) -> Result<(), String> {
    if payload.summary.trim().is_empty() {
        return Err("order_analytics summary must not be empty".to_string());
    }
    ensure_non_blank_values("order_analytics", "key_findings", &payload.key_findings)?;
    ensure_non_blank_values("order_analytics", "risk_flags", &payload.risk_flags)?;
    ensure_non_blank_values(
        "order_analytics",
        "recommended_actions",
        &payload.recommended_actions,
    )?;
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
    if payload.prefill.is_null() {
        return Err(
            "order_ops_assistant prefill must be an object or structured value".to_string(),
        );
    }
    validate_order_ops_assistant_confidence(payload.confidence)
}

#[cfg(test)]
mod tests {
    use super::{
        GeneratedOrderAnalytics, GeneratedOrderOpsAssistant, ORDER_ANALYTICS_TASK_SLUG,
        ORDER_OPS_ASSISTANT_TASK_SLUG, order_ai_execution_policy, order_ai_verticals,
        validate_order_analytics_payload, validate_order_ops_assistant_confidence,
        validate_order_ops_assistant_payload,
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
        for vertical in order_ai_verticals() {
            assert!(vertical.execution_policy.review_required);
            assert_eq!(vertical.execution_policy.persistence, "none");
            assert_eq!(
                order_ai_execution_policy(vertical.task_slug),
                Some(vertical.execution_policy)
            );
        }
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
    fn rejects_blank_order_analytics_array_items() {
        let payload = GeneratedOrderAnalytics {
            summary: "Ready".to_string(),
            key_findings: vec![" ".to_string()],
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

    #[test]
    fn rejects_null_ops_prefill() {
        let payload = GeneratedOrderOpsAssistant {
            recommended_action: "contact_customer".to_string(),
            rationale: "Address mismatch".to_string(),
            prefill: serde_json::Value::Null,
            requires_human: true,
            confidence: 80,
        };
        assert!(validate_order_ops_assistant_payload(&payload).is_err());
    }
}
