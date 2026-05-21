#![cfg_attr(not(feature = "server"), allow(dead_code))]

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Domain-owned registration entrypoint for order AI verticals.
pub fn register_order_ai_verticals() {
    // Placeholder for runtime-side registration wiring.
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
