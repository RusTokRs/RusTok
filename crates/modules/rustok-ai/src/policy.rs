use std::collections::HashSet;

use crate::model::ToolDefinition;

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

    pub fn apply(&self, tools: Vec<ToolDefinition>) -> Vec<ToolDefinition> {
        tools
            .into_iter()
            .filter(|tool| self.is_tool_allowed(&tool.name))
            .map(|mut tool| {
                tool.sensitive = self.is_tool_sensitive(&tool.name);
                tool
            })
            .collect()
    }
}
