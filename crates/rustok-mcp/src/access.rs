use std::collections::BTreeSet;

use rustok_api::Permission;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::alloy_tools::{
    TOOL_ALLOY_APPLY_MODULE_SCAFFOLD, TOOL_ALLOY_CREATE_SCRIPT, TOOL_ALLOY_DELETE_SCRIPT,
    TOOL_ALLOY_GET_SCRIPT, TOOL_ALLOY_LIST_ENTITY_TYPES, TOOL_ALLOY_LIST_SCRIPTS,
    TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD, TOOL_ALLOY_RUN_SCRIPT, TOOL_ALLOY_SCAFFOLD_MODULE,
    TOOL_ALLOY_SCRIPT_HELPERS, TOOL_ALLOY_UPDATE_SCRIPT, TOOL_ALLOY_VALIDATE_SCRIPT,
};
use crate::tools::{
    TOOL_BLOG_MODULE, TOOL_CONTENT_MODULE, TOOL_FORUM_MODULE, TOOL_LIST_MODULES, TOOL_MCP_HEALTH,
    TOOL_MCP_WHOAMI, TOOL_MODULE_DETAILS, TOOL_MODULE_EXISTS, TOOL_PAGES_MODULE,
    TOOL_QUERY_MODULES,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum McpActorType {
    HumanUser,
    ServiceClient,
    ModelAgent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct McpIdentity {
    /// Stable identifier for the MCP actor.
    pub actor_id: String,
    /// Actor kind in the MCP access model.
    pub actor_type: McpActorType,
    /// Optional tenant binding for this actor.
    pub tenant_id: Option<String>,
    /// Optional delegated RusToK user when the actor works on behalf of a person.
    pub delegated_user_id: Option<String>,
    /// Optional display label for introspection/debugging.
    pub display_name: Option<String>,
    /// Scopes granted to the actor at MCP boundary.
    #[serde(default)]
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct McpAccessPolicy {
    /// Optional allow-list of tools exposed to the actor.
    pub allowed_tools: Option<Vec<String>>,
    /// Explicitly denied tools.
    #[serde(default)]
    pub denied_tools: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct McpAccessContext {
    /// Identity attached to this MCP execution context.
    pub identity: Option<McpIdentity>,
    /// Effective RusToK permissions granted to the actor.
    #[serde(default)]
    pub granted_permissions: Vec<String>,
    /// Tool-level policy gates.
    #[serde(default)]
    pub policy: McpAccessPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct McpToolRequirement {
    pub tool_name: String,
    #[serde(default)]
    pub required_permissions: Vec<String>,
    #[serde(default)]
    pub required_scopes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct McpAuthorizationDecision {
    pub allowed: bool,
    pub code: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct McpWhoAmIResponse {
    pub identity: Option<McpIdentity>,
    #[serde(default)]
    pub granted_permissions: Vec<String>,
    #[serde(default)]
    pub effective_scopes: Vec<String>,
    pub allowed_tools: Option<Vec<String>>,
    #[serde(default)]
    pub denied_tools: Vec<String>,
}

impl McpAuthorizationDecision {
    pub fn allow() -> Self {
        Self {
            allowed: true,
            code: None,
            message: None,
        }
    }

    pub fn deny(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            allowed: false,
            code: Some(code.into()),
            message: Some(message.into()),
        }
    }
}

impl McpAccessContext {
    pub fn from_permissions<I>(permissions: I) -> Self
    where
        I: IntoIterator<Item = Permission>,
    {
        Self {
            identity: None,
            granted_permissions: permissions
                .into_iter()
                .map(|permission| permission.to_string())
                .collect(),
            policy: McpAccessPolicy::default(),
        }
    }

    pub fn authorize_tool(&self, requirement: &McpToolRequirement) -> McpAuthorizationDecision {
        if self
            .policy
            .denied_tools
            .iter()
            .any(|tool| tool == &requirement.tool_name)
        {
            return McpAuthorizationDecision::deny(
                "tool_denied",
                format!(
                    "Tool '{}' is denied by MCP access policy",
                    requirement.tool_name
                ),
            );
        }

        if let Some(allowed_tools) = &self.policy.allowed_tools {
            if !allowed_tools
                .iter()
                .any(|tool| tool == &requirement.tool_name)
            {
                return McpAuthorizationDecision::deny(
                    "tool_not_allowed",
                    format!(
                        "Tool '{}' is not allowed for this MCP actor",
                        requirement.tool_name
                    ),
                );
            }
        }

        let granted_permissions = dedupe_sorted(self.granted_permissions.iter().cloned());
        let missing_permissions = requirement
            .required_permissions
            .iter()
            .filter(|permission| !granted_permissions.contains(permission))
            .cloned()
            .collect::<Vec<_>>();
        if !missing_permissions.is_empty() {
            return McpAuthorizationDecision::deny(
                "missing_permissions",
                format!(
                    "Missing required permissions for '{}': {}",
                    requirement.tool_name,
                    missing_permissions.join(", ")
                ),
            );
        }

        let granted_scopes = self
            .identity
            .as_ref()
            .map(|identity| dedupe_sorted(identity.scopes.iter().cloned()))
            .unwrap_or_default();
        let missing_scopes = requirement
            .required_scopes
            .iter()
            .filter(|scope| !granted_scopes.contains(scope))
            .cloned()
            .collect::<Vec<_>>();
        if !missing_scopes.is_empty() {
            return McpAuthorizationDecision::deny(
                "missing_scopes",
                format!(
                    "Missing required scopes for '{}': {}",
                    requirement.tool_name,
                    missing_scopes.join(", ")
                ),
            );
        }

        McpAuthorizationDecision::allow()
    }

    pub fn whoami(&self) -> McpWhoAmIResponse {
        McpWhoAmIResponse {
            identity: self.identity.clone(),
            granted_permissions: dedupe_sorted(self.granted_permissions.iter().cloned()),
            effective_scopes: self
                .identity
                .as_ref()
                .map(|identity| dedupe_sorted(identity.scopes.iter().cloned()))
                .unwrap_or_default(),
            allowed_tools: self
                .policy
                .allowed_tools
                .as_ref()
                .map(|tools| dedupe_sorted(tools.iter().cloned())),
            denied_tools: dedupe_sorted(self.policy.denied_tools.iter().cloned()),
        }
    }
}

impl McpWhoAmIResponse {
    pub fn anonymous() -> Self {
        Self {
            identity: None,
            granted_permissions: Vec::new(),
            effective_scopes: Vec::new(),
            allowed_tools: None,
            denied_tools: Vec::new(),
        }
    }
}

pub fn default_tool_requirement(tool_name: &str) -> McpToolRequirement {
    let required_permissions = match tool_name {
        TOOL_LIST_MODULES | TOOL_QUERY_MODULES => vec![Permission::MODULES_LIST.to_string()],
        TOOL_MODULE_EXISTS | TOOL_MODULE_DETAILS | TOOL_CONTENT_MODULE | TOOL_BLOG_MODULE
        | TOOL_FORUM_MODULE | TOOL_PAGES_MODULE => vec![Permission::MODULES_READ.to_string()],
        TOOL_ALLOY_LIST_SCRIPTS => vec![Permission::SCRIPTS_LIST.to_string()],
        TOOL_ALLOY_GET_SCRIPT
        | TOOL_ALLOY_LIST_ENTITY_TYPES
        | TOOL_ALLOY_SCRIPT_HELPERS
        | TOOL_ALLOY_VALIDATE_SCRIPT => vec![Permission::SCRIPTS_READ.to_string()],
        TOOL_ALLOY_CREATE_SCRIPT => vec![Permission::SCRIPTS_CREATE.to_string()],
        TOOL_ALLOY_UPDATE_SCRIPT => vec![Permission::SCRIPTS_UPDATE.to_string()],
        TOOL_ALLOY_DELETE_SCRIPT => vec![Permission::SCRIPTS_DELETE.to_string()],
        TOOL_ALLOY_RUN_SCRIPT => vec![Permission::SCRIPTS_EXECUTE.to_string()],
        TOOL_ALLOY_SCAFFOLD_MODULE
        | TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD
        | TOOL_ALLOY_APPLY_MODULE_SCAFFOLD => vec![Permission::MODULES_MANAGE.to_string()],
        TOOL_MCP_HEALTH | TOOL_MCP_WHOAMI => Vec::new(),
        _ => Vec::new(),
    };

    McpToolRequirement {
        tool_name: tool_name.to_string(),
        required_permissions,
        required_scopes: Vec::new(),
    }
}

fn dedupe_sorted<I>(values: I) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denies_tool_outside_allow_list() {
        let context = McpAccessContext {
            identity: None,
            granted_permissions: vec![Permission::MODULES_LIST.to_string()],
            policy: McpAccessPolicy {
                allowed_tools: Some(vec![TOOL_MCP_WHOAMI.to_string()]),
                denied_tools: Vec::new(),
            },
        };

        let decision = context.authorize_tool(&default_tool_requirement(TOOL_LIST_MODULES));

        assert!(!decision.allowed);
        assert_eq!(decision.code.as_deref(), Some("tool_not_allowed"));
    }

    #[test]
    fn denies_missing_permission() {
        let context = McpAccessContext {
            identity: None,
            granted_permissions: vec![Permission::MODULES_READ.to_string()],
            policy: McpAccessPolicy::default(),
        };

        let decision = context.authorize_tool(&default_tool_requirement(TOOL_QUERY_MODULES));

        assert!(!decision.allowed);
        assert_eq!(decision.code.as_deref(), Some("missing_permissions"));
    }

    #[test]
    fn denies_missing_scope() {
        let context = McpAccessContext {
            identity: Some(McpIdentity {
                actor_id: "model-1".to_string(),
                actor_type: McpActorType::ModelAgent,
                tenant_id: Some("tenant-1".to_string()),
                delegated_user_id: None,
                display_name: Some("Writer".to_string()),
                scopes: vec!["content.read".to_string()],
            }),
            granted_permissions: vec![Permission::MODULES_READ.to_string()],
            policy: McpAccessPolicy::default(),
        };
        let requirement = McpToolRequirement {
            tool_name: TOOL_MODULE_DETAILS.to_string(),
            required_permissions: vec![Permission::MODULES_READ.to_string()],
            required_scopes: vec!["modules.read".to_string()],
        };

        let decision = context.authorize_tool(&requirement);

        assert!(!decision.allowed);
        assert_eq!(decision.code.as_deref(), Some("missing_scopes"));
    }

    #[test]
    fn allows_when_permissions_are_present() {
        let context = McpAccessContext {
            identity: Some(McpIdentity {
                actor_id: "svc-1".to_string(),
                actor_type: McpActorType::ServiceClient,
                tenant_id: Some("tenant-1".to_string()),
                delegated_user_id: None,
                display_name: Some("CI".to_string()),
                scopes: vec!["modules.read".to_string()],
            }),
            granted_permissions: vec![
                Permission::MODULES_READ.to_string(),
                Permission::MODULES_LIST.to_string(),
            ],
            policy: McpAccessPolicy::default(),
        };
        let requirement = McpToolRequirement {
            tool_name: TOOL_QUERY_MODULES.to_string(),
            required_permissions: vec![Permission::MODULES_LIST.to_string()],
            required_scopes: Vec::new(),
        };

        let decision = context.authorize_tool(&requirement);

        assert!(decision.allowed);
    }

    #[test]
    fn maps_run_script_to_execute_permission() {
        let requirement = default_tool_requirement(TOOL_ALLOY_RUN_SCRIPT);

        assert_eq!(
            requirement.required_permissions,
            vec![Permission::SCRIPTS_EXECUTE.to_string()]
        );
    }
}
