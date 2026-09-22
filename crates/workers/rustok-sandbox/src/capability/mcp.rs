use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{CapabilityCall, CapabilityGrant};
use crate::{SandboxError, SandboxResult};

/// One admitted MCP server/tool target. The transport endpoint, credentials,
/// and tool implementation remain deployment-owned and never appear here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpToolGrant {
    pub server: String,
    pub tool: String,
}

/// Typed policy for the `platform.mcp` capability.
///
/// Sandbox calls use a preconfigured server alias and an exact tool name. A
/// guest never chooses an MCP endpoint, transport, credential, or arbitrary
/// tool discovery target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpCapabilityConstraints {
    pub tools: Vec<McpToolGrant>,
    pub operations: Vec<String>,
}

impl McpCapabilityConstraints {
    pub(crate) fn from_grant(grant: &CapabilityGrant) -> SandboxResult<Self> {
        let constraints =
            serde_json::from_value::<Self>(grant.constraints.clone()).map_err(|error| {
                SandboxError::CapabilityConstraintDenied {
                    capability: grant.name.clone(),
                    reason: format!("invalid MCP constraints: {error}"),
                }
            })?;
        if constraints.tools.is_empty() || constraints.operations.is_empty() {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason: "MCP constraints require non-empty tools and operations".to_string(),
            });
        }
        if constraints.tools.iter().enumerate().any(|(index, target)| {
            !valid_mcp_name(&target.server)
                || !valid_mcp_name(&target.tool)
                || constraints.tools[..index]
                    .iter()
                    .any(|previous| previous == target)
        }) {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason: "MCP server/tool targets must be unique logical identifiers".to_string(),
            });
        }
        let mut operations = std::collections::BTreeSet::new();
        if constraints
            .operations
            .iter()
            .any(|operation| operation != "call" || !operations.insert(operation))
        {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason: "MCP operations must contain only the unique call operation".to_string(),
            });
        }
        Ok(constraints)
    }

    pub(crate) fn validate(&self, call: &CapabilityCall) -> SandboxResult<()> {
        if call.operation != "call" || !self.operations.iter().any(|operation| operation == "call")
        {
            return Err(mcp_constraint_error(call, "MCP operation is not allowed"));
        }
        let input = call
            .input
            .as_object()
            .ok_or_else(|| mcp_constraint_error(call, "MCP input must be an object"))?;
        if input
            .keys()
            .any(|field| field != "server" && field != "tool" && field != "arguments")
        {
            return Err(mcp_constraint_error(
                call,
                "MCP input may contain only server, tool, and arguments",
            ));
        }
        let server = required_mcp_string(call, input, "server")?;
        let tool = required_mcp_string(call, input, "tool")?;
        if !self
            .tools
            .iter()
            .any(|target| target.server == server && target.tool == tool)
        {
            return Err(mcp_constraint_error(
                call,
                "MCP server/tool target is not allowed",
            ));
        }
        Ok(())
    }
}

fn mcp_constraint_error(call: &CapabilityCall, reason: &str) -> SandboxError {
    SandboxError::CapabilityConstraintDenied {
        capability: call.capability.clone(),
        reason: reason.to_string(),
    }
}

fn required_mcp_string<'a>(
    call: &CapabilityCall,
    input: &'a serde_json::Map<String, Value>,
    field: &str,
) -> SandboxResult<&'a str> {
    input
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| valid_mcp_name(value))
        .ok_or_else(|| mcp_constraint_error(call, &format!("MCP {field} is invalid")))
}

fn valid_mcp_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && !matches!(value.chars().next(), Some('.' | '-' | '_'))
        && !matches!(value.chars().next_back(), Some('.' | '-' | '_'))
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '-' | '.')
        })
}
