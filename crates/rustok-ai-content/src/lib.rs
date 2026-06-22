#![cfg_attr(not(feature = "server"), allow(dead_code))]

use serde::{Deserialize, Serialize};

pub const CONTENT_MODERATION_TASK_SLUG: &str = "content_moderation";
pub const BLOG_DRAFT_TASK_SLUG: &str = "blog_draft";
pub const CONTENT_MODERATION_TOOL_NAME: &str = "direct.content.moderation";
pub const BLOG_DRAFT_TOOL_NAME: &str = "direct.blog.generate_draft";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentAiApprovalMode {
    Auto,
    OperatorApproval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentAiPolicyRule {
    pub task_slug: &'static str,
    pub tool_name: &'static str,
    pub approval_mode: ContentAiApprovalMode,
    pub rationale: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentAiVerticalDescriptor {
    pub task_slug: &'static str,
    pub tool_name: &'static str,
    pub sensitive: bool,
}

pub const CONTENT_AI_VERTICALS: &[ContentAiVerticalDescriptor] = &[
    ContentAiVerticalDescriptor {
        task_slug: CONTENT_MODERATION_TASK_SLUG,
        tool_name: CONTENT_MODERATION_TOOL_NAME,
        sensitive: true,
    },
    ContentAiVerticalDescriptor {
        task_slug: BLOG_DRAFT_TASK_SLUG,
        tool_name: BLOG_DRAFT_TOOL_NAME,
        sensitive: false,
    },
];

pub const CONTENT_AI_POLICY_MATRIX: &[ContentAiPolicyRule] = &[
    ContentAiPolicyRule {
        task_slug: CONTENT_MODERATION_TASK_SLUG,
        tool_name: CONTENT_MODERATION_TOOL_NAME,
        approval_mode: ContentAiApprovalMode::OperatorApproval,
        rationale: "moderation decisions can hide or block user-generated content",
    },
    ContentAiPolicyRule {
        task_slug: BLOG_DRAFT_TASK_SLUG,
        tool_name: BLOG_DRAFT_TOOL_NAME,
        approval_mode: ContentAiApprovalMode::Auto,
        rationale: "blog drafts create unpublished editorial artifacts",
    },
];

pub fn content_ai_policy_matrix() -> &'static [ContentAiPolicyRule] {
    CONTENT_AI_POLICY_MATRIX
}

pub fn content_ai_sensitive_tools() -> Vec<String> {
    CONTENT_AI_POLICY_MATRIX
        .iter()
        .filter(|rule| matches!(rule.approval_mode, ContentAiApprovalMode::OperatorApproval))
        .map(|rule| rule.tool_name.to_string())
        .collect()
}

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

/// Domain-owned adapter API for runtime composition layers that need to bind
/// concrete handlers to the vertical descriptors without owning task identity.
pub fn register_content_ai_vertical_handlers(
    mut register: impl FnMut(&'static ContentAiVerticalDescriptor),
) {
    for vertical in content_ai_verticals() {
        register(vertical);
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GeneratedBlogDraft {
    pub title: Option<String>,
    pub slug: Option<String>,
    pub body: Option<String>,
    pub excerpt: Option<String>,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
}

pub fn validate_blog_draft_payload(payload: &GeneratedBlogDraft) -> Result<(), String> {
    let text_fields = [
        ("title", payload.title.as_deref()),
        ("slug", payload.slug.as_deref()),
        ("body", payload.body.as_deref()),
        ("excerpt", payload.excerpt.as_deref()),
        ("seo_title", payload.seo_title.as_deref()),
        ("seo_description", payload.seo_description.as_deref()),
    ];

    for (field, value) in text_fields
        .into_iter()
        .filter_map(|(field, value)| value.map(|value| (field, value)))
    {
        if value.trim().is_empty() {
            return Err(format!(
                "blog_draft {field} must not be blank when provided"
            ));
        }
    }
    Ok(())
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
        content_ai_policy_matrix, content_ai_sensitive_tools, content_ai_verticals,
        normalize_moderation_decision, validate_moderation_decision, ContentAiApprovalMode,
        GeneratedBlogDraft, GeneratedModerationDecision, BLOG_DRAFT_TASK_SLUG,
        CONTENT_MODERATION_TASK_SLUG, CONTENT_MODERATION_TOOL_NAME,
    };

    #[test]
    fn exposes_content_moderation_descriptor() {
        assert_eq!(
            content_ai_verticals()[0].task_slug,
            CONTENT_MODERATION_TASK_SLUG
        );
        assert!(content_ai_verticals()[0].sensitive);
        assert_eq!(content_ai_verticals()[1].task_slug, BLOG_DRAFT_TASK_SLUG);
        assert!(!content_ai_verticals()[1].sensitive);
    }

    #[test]
    fn exposes_policy_matrix_for_approval_routing() {
        let matrix = content_ai_policy_matrix();
        assert_eq!(matrix[0].task_slug, CONTENT_MODERATION_TASK_SLUG);
        assert_eq!(
            matrix[0].approval_mode,
            ContentAiApprovalMode::OperatorApproval
        );
        assert_eq!(matrix[1].approval_mode, ContentAiApprovalMode::Auto);
        assert_eq!(
            content_ai_sensitive_tools(),
            vec![CONTENT_MODERATION_TOOL_NAME]
        );
    }

    #[test]
    fn accepts_partial_blog_draft_payload() {
        let payload = GeneratedBlogDraft {
            title: Some("Title".to_string()),
            body: Some("Body".to_string()),
            ..GeneratedBlogDraft::default()
        };
        assert!(super::validate_blog_draft_payload(&payload).is_ok());
    }

    #[test]
    fn rejects_blank_blog_draft_body_when_provided() {
        let payload = GeneratedBlogDraft {
            title: Some("Title".to_string()),
            body: Some(" ".to_string()),
            ..GeneratedBlogDraft::default()
        };
        assert!(super::validate_blog_draft_payload(&payload).is_err());
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
