#![cfg_attr(not(feature = "server"), allow(dead_code))]

use serde::{Deserialize, Serialize};

pub const CONTENT_MODERATION_TASK_SLUG: &str = "content_moderation";
pub const CONTENT_MODERATION_TOOL_NAME: &str = "direct.content.moderation";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentAiVerticalDescriptor {
    pub task_slug: &'static str,
    pub tool_name: &'static str,
    pub sensitive: bool,
}

pub const CONTENT_AI_VERTICALS: &[ContentAiVerticalDescriptor] = &[ContentAiVerticalDescriptor {
    task_slug: CONTENT_MODERATION_TASK_SLUG,
    tool_name: CONTENT_MODERATION_TOOL_NAME,
    sensitive: true,
}];

/// Domain-owned registration entrypoint for content AI vertical metadata.
pub fn content_ai_verticals() -> &'static [ContentAiVerticalDescriptor] {
    CONTENT_AI_VERTICALS
}

/// Backward-compatible entrypoint kept for composition callers. Runtime
/// registration consumes [`content_ai_verticals`] so task identity remains owned
/// by this crate.
pub fn register_content_ai_verticals() -> &'static [ContentAiVerticalDescriptor] {
    content_ai_verticals()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedModerationDecision {
    pub decision: String,
    #[serde(default)]
    pub labels: Vec<String>,
    pub severity: u8,
    pub explanation: String,
    pub requires_human: bool,
    pub recommended_action: Option<String>,
}

pub fn normalize_moderation_decision(decision: &str) -> Result<String, String> {
    let decision_slug = decision.trim().to_ascii_lowercase();
    if !matches!(decision_slug.as_str(), "allow" | "review" | "block") {
        return Err("content_moderation decision must be one of: allow, review, block".to_string());
    }
    Ok(decision_slug)
}

pub fn validate_moderation_severity(severity: u8) -> Result<(), String> {
    if severity > 100 {
        return Err("content_moderation severity must be between 0 and 100".to_string());
    }
    Ok(())
}

pub fn validate_moderation_decision(
    payload: &GeneratedModerationDecision,
) -> Result<GeneratedModerationDecision, String> {
    let decision = normalize_moderation_decision(&payload.decision)?;
    validate_moderation_severity(payload.severity)?;
    if payload.explanation.trim().is_empty() {
        return Err("content_moderation explanation must not be empty".to_string());
    }
    Ok(GeneratedModerationDecision {
        decision,
        labels: payload.labels.clone(),
        severity: payload.severity,
        explanation: payload.explanation.clone(),
        requires_human: payload.requires_human,
        recommended_action: payload.recommended_action.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        content_ai_verticals, normalize_moderation_decision, validate_moderation_decision,
        GeneratedModerationDecision, CONTENT_MODERATION_TASK_SLUG,
    };

    #[test]
    fn exposes_content_moderation_descriptor() {
        assert_eq!(
            content_ai_verticals()[0].task_slug,
            CONTENT_MODERATION_TASK_SLUG
        );
        assert!(content_ai_verticals()[0].sensitive);
    }

    #[test]
    fn normalizes_known_decisions() {
        assert_eq!(normalize_moderation_decision(" Review ").unwrap(), "review");
    }

    #[test]
    fn rejects_unknown_decisions() {
        assert!(normalize_moderation_decision("maybe").is_err());
    }

    #[test]
    fn validates_and_normalizes_payload() {
        let payload = GeneratedModerationDecision {
            decision: "BLOCK".to_string(),
            labels: vec!["spam".to_string()],
            severity: 99,
            explanation: "Spam pattern".to_string(),
            requires_human: true,
            recommended_action: Some("hide".to_string()),
        };
        let normalized = validate_moderation_decision(&payload).unwrap();
        assert_eq!(normalized.decision, "block");
    }

    #[test]
    fn rejects_empty_explanation() {
        let payload = GeneratedModerationDecision {
            decision: "allow".to_string(),
            labels: vec![],
            severity: 0,
            explanation: " ".to_string(),
            requires_human: false,
            recommended_action: None,
        };
        assert!(validate_moderation_decision(&payload).is_err());
    }
}
