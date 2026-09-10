use std::collections::{BTreeSet, HashSet};

use serde_json::json;
use sha2::{Digest, Sha256};

use crate::model::{ToolDefinition, ToolPolicyEvidence};

#[derive(Debug, Clone, Default)]
pub struct ToolExecutionPolicy {
    allowed_tools: Option<HashSet<String>>,
    denied_tools: HashSet<String>,
    sensitive_tools: HashSet<String>,
}

impl ToolExecutionPolicy {
    pub fn new(
        allowed_tools: Option<Vec<String>>,
        denied_tools: Vec<String>,
        sensitive_tools: Vec<String>,
    ) -> Self {
        Self {
            allowed_tools: allowed_tools.map(|items| items.into_iter().collect()),
            denied_tools: denied_tools.into_iter().collect(),
            sensitive_tools: sensitive_tools.into_iter().collect(),
        }
    }

    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        if self.denied_tools.contains(tool_name) {
            return false;
        }

        match &self.allowed_tools {
            Some(allowed) => allowed.contains(tool_name),
            None => true,
        }
    }

    pub fn is_tool_sensitive(&self, tool_name: &str) -> bool {
        self.sensitive_tools.contains(tool_name)
    }

    /// Consequential owner classes always require a human approval. A tenant
    /// profile can only add that boundary for otherwise automatic operations.
    pub fn requires_operator_approval(&self, definition: &ToolDefinition) -> bool {
        definition.operation_class.requires_operator_approval()
            || self.is_tool_sensitive(&definition.name)
    }

    /// Returns the exact non-secret policy facts that an approval authorizes.
    /// Both digests are deterministic, bounded identifiers rather than raw
    /// schemas or profile data.
    pub fn evidence(&self, definition: &ToolDefinition) -> ToolPolicyEvidence {
        ToolPolicyEvidence {
            operation_class: definition.operation_class,
            requires_operator_approval: self.requires_operator_approval(definition),
            definition_digest: digest_value(&json!({
                "name": definition.name,
                "input_schema": definition.input_schema,
                "operation_class": definition.operation_class,
            })),
            policy_digest: self.policy_digest(),
        }
    }

    fn policy_digest(&self) -> String {
        let allowed_tools = self
            .allowed_tools
            .as_ref()
            .map(|values| values.iter().cloned().collect::<BTreeSet<_>>())
            .map(|values| values.into_iter().collect::<Vec<_>>());
        let denied_tools = self
            .denied_tools
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let sensitive_tools = self
            .sensitive_tools
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        digest_value(&json!({
            "allowed_tools": allowed_tools,
            "denied_tools": denied_tools,
            "sensitive_tools": sensitive_tools,
        }))
    }

    pub fn apply(&self, tools: Vec<ToolDefinition>) -> Vec<ToolDefinition> {
        tools
            .into_iter()
            .filter(|tool| self.is_tool_allowed(&tool.name))
            .map(|mut tool| {
                tool.sensitive = self.requires_operator_approval(&tool);
                tool
            })
            .collect()
    }
}

fn digest_value(value: &serde_json::Value) -> String {
    let encoded = serde_json::to_vec(value).expect("tool policy evidence is serializable");
    let digest = Sha256::digest(encoded);
    format!("sha256:{}", hex::encode(digest))
}

#[cfg(test)]
mod tests {
    use super::ToolExecutionPolicy;
    use crate::{ToolDefinition, ToolOperationClass};

    fn definition(operation_class: ToolOperationClass) -> ToolDefinition {
        ToolDefinition {
            name: "module_apply".to_string(),
            description: "Apply a reviewed draft".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
            operation_class,
            sensitive: false,
        }
    }

    #[test]
    fn owner_consequence_cannot_be_downgraded_by_profile() {
        let policy = ToolExecutionPolicy::new(None, Vec::new(), Vec::new());
        assert!(
            policy.requires_operator_approval(&definition(ToolOperationClass::WorkspaceMutation))
        );
        assert!(!policy.requires_operator_approval(&definition(ToolOperationClass::ReadOnly)));
    }

    #[test]
    fn policy_evidence_changes_when_schema_or_profile_changes() {
        let definition = definition(ToolOperationClass::ReadOnly);
        let default_policy = ToolExecutionPolicy::new(None, Vec::new(), Vec::new());
        let sensitive_policy =
            ToolExecutionPolicy::new(None, Vec::new(), vec!["module_apply".to_string()]);
        assert_ne!(
            default_policy.evidence(&definition),
            sensitive_policy.evidence(&definition)
        );
    }
}
