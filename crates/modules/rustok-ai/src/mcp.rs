use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

use crate::{
    error::{AiError, AiResult},
    model::ToolDefinition,
};

/// Content-free source provenance issued by the owner that executed an MCP
/// operation. The AI runtime accepts it only when it is bound to the invoked
/// tool and has canonical, opaque identifiers.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ToolSourceLineage {
    pub owner: String,
    pub tool_name: String,
    pub source_id: String,
    pub source_digest: String,
}

pub(crate) fn validated_source_lineage(
    tool_name: &str,
    source_lineage: &[ToolSourceLineage],
) -> AiResult<Vec<ToolSourceLineage>> {
    const MAX_SOURCE_LINEAGE: usize = 8;

    if source_lineage.len() > MAX_SOURCE_LINEAGE {
        return Err(AiError::Validation(
            "MCP tool result exceeds the maximum owner source lineage evidence".to_string(),
        ));
    }

    let mut seen = BTreeSet::new();
    for lineage in source_lineage {
        if lineage.tool_name != tool_name
            || !is_owner_slug(&lineage.owner)
            || Uuid::parse_str(&lineage.source_id).is_err()
            || !is_sha256_digest(&lineage.source_digest)
            || !seen.insert(lineage.clone())
        {
            return Err(AiError::Validation(
                "MCP tool returned invalid owner source lineage evidence".to_string(),
            ));
        }
    }

    Ok(seen.into_iter().collect())
}

fn is_owner_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn is_sha256_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

#[derive(Debug, Clone)]
pub struct ToolExecutionResult {
    pub content: String,
    pub raw_payload: serde_json::Value,
    pub source_lineage: Vec<ToolSourceLineage>,
}

#[async_trait]
pub trait McpClientAdapter: Send + Sync {
    async fn list_tools(&self) -> AiResult<Vec<ToolDefinition>>;
    async fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> AiResult<ToolExecutionResult>;
}

#[cfg(test)]
mod tests {
    use super::{ToolSourceLineage, validated_source_lineage};
    use uuid::Uuid;

    fn lineage(tool_name: &str) -> ToolSourceLineage {
        ToolSourceLineage {
            owner: "rustok-mcp".to_string(),
            tool_name: tool_name.to_string(),
            source_id: Uuid::nil().to_string(),
            source_digest: format!("sha256:{}", "a".repeat(64)),
        }
    }

    #[test]
    fn source_lineage_requires_a_canonical_owner_bound_digest() {
        let evidence =
            validated_source_lineage("alloy_scaffold_module", &[lineage("alloy_scaffold_module")])
                .expect("canonical owner source lineage");
        assert_eq!(evidence.len(), 1);

        let mut malformed = lineage("other_tool");
        malformed.source_digest = "sha256:ABC".to_string();
        assert!(validated_source_lineage("alloy_scaffold_module", &[malformed]).is_err());
    }

    #[test]
    fn source_lineage_rejects_duplicate_owner_receipts() {
        let receipt = lineage("alloy_scaffold_module");
        assert!(
            validated_source_lineage("alloy_scaffold_module", &[receipt.clone(), receipt]).is_err()
        );
    }
}
