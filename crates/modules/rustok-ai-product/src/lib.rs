#![cfg_attr(not(feature = "server"), allow(dead_code))]

use serde::{Deserialize, Serialize};

pub const PRODUCT_COPY_TASK_SLUG: &str = "product_copy";
pub const PRODUCT_ATTRIBUTES_TASK_SLUG: &str = "product_attributes";
pub const PRODUCT_COPY_TOOL_NAME: &str = "direct.commerce.product_copy";
pub const PRODUCT_ATTRIBUTES_TOOL_NAME: &str = "direct.commerce.product_attributes";

/// Product-owned declarations for non-human principals. The generic AI runtime
/// consumes these declarations but does not invent product task identities or
/// input contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductAiAgentDescriptor {
    pub slug: &'static str,
    pub display_name: &'static str,
    pub responsibility: &'static str,
    pub task_slug: &'static str,
    pub required_permissions: &'static [&'static str],
    pub required_capabilities: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductAiWorkflowStageDescriptor {
    pub id: &'static str,
    pub agent_slug: &'static str,
    pub depends_on: &'static [&'static str],
    pub requires_approval: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductAiWorkflowDescriptor {
    pub slug: &'static str,
    pub display_name: &'static str,
    pub stages: &'static [ProductAiWorkflowStageDescriptor],
}

const PRODUCT_AGENT_PERMISSIONS: &[&str] = &["ai:tasks:text:run", "products:update"];

pub const PRODUCT_AI_AGENTS: &[ProductAiAgentDescriptor] = &[
    ProductAiAgentDescriptor {
        slug: "product_copywriter",
        display_name: "Product copywriter",
        responsibility: "Generate localized product copy through the product-owned task.",
        task_slug: PRODUCT_COPY_TASK_SLUG,
        required_permissions: PRODUCT_AGENT_PERMISSIONS,
        required_capabilities: &["text_generation"],
    },
    ProductAiAgentDescriptor {
        slug: "product_attribute_enricher",
        display_name: "Product attribute enricher",
        responsibility: "Generate bounded product attributes through the product-owned task.",
        task_slug: PRODUCT_ATTRIBUTES_TASK_SLUG,
        required_permissions: PRODUCT_AGENT_PERMISSIONS,
        required_capabilities: &["structured_generation"],
    },
];

const PRODUCT_ENRICHMENT_STAGES: &[ProductAiWorkflowStageDescriptor] = &[
    ProductAiWorkflowStageDescriptor {
        id: "copy",
        agent_slug: "product_copywriter",
        depends_on: &[],
        requires_approval: true,
    },
    ProductAiWorkflowStageDescriptor {
        id: "attributes",
        agent_slug: "product_attribute_enricher",
        depends_on: &["copy"],
        requires_approval: true,
    },
];

pub const PRODUCT_AI_WORKFLOWS: &[ProductAiWorkflowDescriptor] = &[ProductAiWorkflowDescriptor {
    slug: "product_enrichment",
    display_name: "Product enrichment",
    stages: PRODUCT_ENRICHMENT_STAGES,
}];

pub fn product_ai_agents() -> &'static [ProductAiAgentDescriptor] {
    PRODUCT_AI_AGENTS
}

pub fn product_ai_workflows() -> &'static [ProductAiWorkflowDescriptor] {
    PRODUCT_AI_WORKFLOWS
}

pub fn product_agent_task_slug(agent_slug: &str) -> Option<&'static str> {
    product_ai_agents()
        .iter()
        .find(|agent| agent.slug == agent_slug)
        .map(|agent| agent.task_slug)
}

/// Performs the owner-level admission check before the generic scheduler
/// creates a canonical task run. Full payload validation remains in the
/// product direct handler, where tenant and locale context are available.
pub fn validate_product_agent_stage_input(
    agent_slug: &str,
    payload: &serde_json::Value,
) -> Result<&'static str, String> {
    let task_slug = product_agent_task_slug(agent_slug)
        .ok_or_else(|| format!("unknown product agent `{agent_slug}`"))?;
    let product_id = payload
        .get("product_id")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{agent_slug} requires a non-empty product_id"))?;
    uuid::Uuid::parse_str(product_id)
        .map_err(|_| format!("{agent_slug} requires a valid product_id UUID"))?;
    Ok(task_slug)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductAiVerticalDescriptor {
    pub task_slug: &'static str,
    pub tool_name: &'static str,
    pub sensitive: bool,
}

pub const PRODUCT_AI_VERTICALS: &[ProductAiVerticalDescriptor] = &[
    ProductAiVerticalDescriptor {
        task_slug: PRODUCT_COPY_TASK_SLUG,
        tool_name: PRODUCT_COPY_TOOL_NAME,
        sensitive: false,
    },
    ProductAiVerticalDescriptor {
        task_slug: PRODUCT_ATTRIBUTES_TASK_SLUG,
        tool_name: PRODUCT_ATTRIBUTES_TOOL_NAME,
        sensitive: false,
    },
];

/// Domain-owned registration entrypoint for product AI vertical metadata.
pub fn product_ai_verticals() -> &'static [ProductAiVerticalDescriptor] {
    PRODUCT_AI_VERTICALS
}

/// Backward-compatible entrypoint kept for callers that only need to touch the
/// product vertical package during composition. Runtime registration consumes
/// [`product_ai_verticals`] so task identity remains owned by this crate.
pub fn register_product_ai_verticals() -> &'static [ProductAiVerticalDescriptor] {
    product_ai_verticals()
}

/// Domain-owned adapter API for runtime composition layers that need to bind
/// concrete handlers to the vertical descriptors without owning task identity.
pub fn register_product_ai_vertical_handlers(
    mut register: impl FnMut(&'static ProductAiVerticalDescriptor),
) {
    for vertical in product_ai_verticals() {
        register(vertical);
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GeneratedProductCopy {
    pub title: Option<String>,
    pub handle: Option<String>,
    pub description: Option<String>,
    pub meta_title: Option<String>,
    pub meta_description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GeneratedProductAttributes {
    pub brand: Option<String>,
    pub material: Option<String>,
    pub color: Option<String>,
    pub size: Option<String>,
    pub dimensions: Option<String>,
    pub compatibility: Option<String>,
    pub care_instructions: Option<String>,
    pub hazmat: Option<String>,
    #[serde(default)]
    pub flex_attributes: Vec<GeneratedFlexAttribute>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GeneratedFlexAttribute {
    pub key: String,
    pub value: String,
}

pub fn validate_product_attributes_payload(
    payload: &GeneratedProductAttributes,
) -> Result<(), String> {
    if payload
        .flex_attributes
        .iter()
        .any(|attr| attr.key.trim().is_empty() || attr.value.trim().is_empty())
    {
        return Err(
            "product_attributes flex_attributes must contain non-empty key/value".to_string(),
        );
    }
    Ok(())
}

pub fn validate_product_copy_payload(payload: &GeneratedProductCopy) -> Result<(), String> {
    let has_text = [
        payload.title.as_deref(),
        payload.handle.as_deref(),
        payload.description.as_deref(),
        payload.meta_title.as_deref(),
        payload.meta_description.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|value| !value.trim().is_empty());

    if !has_text {
        return Err("product_copy must contain at least one non-empty localized field".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        GeneratedFlexAttribute, GeneratedProductAttributes, GeneratedProductCopy,
        PRODUCT_ATTRIBUTES_TASK_SLUG, PRODUCT_COPY_TASK_SLUG, product_ai_agents,
        product_ai_verticals, product_ai_workflows, validate_product_agent_stage_input,
        validate_product_attributes_payload, validate_product_copy_payload,
    };

    #[test]
    fn exposes_product_vertical_descriptors() {
        let slugs = product_ai_verticals()
            .iter()
            .map(|vertical| vertical.task_slug)
            .collect::<Vec<_>>();
        assert_eq!(
            slugs,
            vec![PRODUCT_COPY_TASK_SLUG, PRODUCT_ATTRIBUTES_TASK_SLUG]
        );
    }

    #[test]
    fn product_agents_are_owner_bound_and_require_product_input() {
        assert_eq!(product_ai_agents().len(), 2);
        assert_eq!(product_ai_workflows()[0].slug, "product_enrichment");
        assert!(product_ai_workflows()[0].stages[0].requires_approval);
        assert_eq!(
            validate_product_agent_stage_input(
                "product_copywriter",
                &serde_json::json!({"product_id":"00000000-0000-0000-0000-000000000001"}),
            )
            .unwrap(),
            PRODUCT_COPY_TASK_SLUG
        );
        assert!(
            validate_product_agent_stage_input("product_copywriter", &serde_json::json!({}),)
                .is_err()
        );
    }

    #[test]
    fn accepts_product_attributes_without_flex_attributes() {
        let payload = GeneratedProductAttributes {
            brand: Some("Brand".to_string()),
            material: None,
            color: None,
            size: None,
            dimensions: None,
            compatibility: None,
            care_instructions: None,
            hazmat: None,
            flex_attributes: vec![],
        };
        assert!(validate_product_attributes_payload(&payload).is_ok());
    }

    #[test]
    fn rejects_blank_product_attributes_flex_key_or_value() {
        let payload = GeneratedProductAttributes {
            brand: None,
            material: None,
            color: None,
            size: None,
            dimensions: None,
            compatibility: None,
            care_instructions: None,
            hazmat: None,
            flex_attributes: vec![GeneratedFlexAttribute {
                key: " ".to_string(),
                value: "cotton".to_string(),
            }],
        };
        assert!(validate_product_attributes_payload(&payload).is_err());
    }

    #[test]
    fn accepts_product_copy_with_any_non_empty_field() {
        let payload = GeneratedProductCopy {
            title: Some("Localized title".to_string()),
            ..GeneratedProductCopy::default()
        };
        assert!(validate_product_copy_payload(&payload).is_ok());
    }

    #[test]
    fn rejects_empty_product_copy() {
        let payload = GeneratedProductCopy::default();
        assert!(validate_product_copy_payload(&payload).is_err());
    }
}
