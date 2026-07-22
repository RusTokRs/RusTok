#![cfg_attr(not(feature = "server"), allow(dead_code))]

pub const ALLOY_CODE_TASK_SLUG: &str = "alloy_code";
pub const ALLOY_CODE_TOOL_NAME: &str = "direct.alloy.run_script";

/// Typed Alloy operations owned by the Alloy support adapter.
///
/// The AI runtime dispatches the selected operation but must not define a
/// second operation catalog.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AlloyOperation {
    #[default]
    ListScripts,
    GetScript,
    ValidateScript,
    RunScript,
}

impl AlloyOperation {
    pub const fn slug(self) -> &'static str {
        match self {
            Self::ListScripts => "list_scripts",
            Self::GetScript => "get_script",
            Self::ValidateScript => "validate_script",
            Self::RunScript => "run_script",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlloyCodeAgentDescriptor {
    pub slug: &'static str,
    pub display_name: &'static str,
    pub responsibility: &'static str,
    pub required_permissions: &'static [&'static str],
    pub allowed_operations: &'static [&'static str],
    pub requires_approval: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlloySwarmStageDescriptor {
    pub id: &'static str,
    pub agent_slug: &'static str,
    pub depends_on: &'static [&'static str],
    pub requires_approval: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlloySwarmWorkflowDescriptor {
    pub slug: &'static str,
    pub display_name: &'static str,
    pub stages: &'static [AlloySwarmStageDescriptor],
}

/// Binds an owner-defined code-agent role to the existing Alloy AI task
/// contract. The generic runtime consumes this declaration but never invents
/// an Alloy operation or payload shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlloyStageExecutionDescriptor {
    pub agent_slug: &'static str,
    pub task_slug: &'static str,
    pub required_operation: &'static str,
    pub input_shape: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlloyAiVerticalDescriptor {
    pub task_slug: &'static str,
    pub tool_name: &'static str,
    pub sensitive: bool,
    pub runtime_operation: &'static str,
    pub runtime_payload_json_shape: &'static str,
    pub transport_owner: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlloyScriptExecutionPolicy {
    pub script_runtime: &'static str,
    pub runtime_payload_json_shape: &'static str,
    pub composition_owner: &'static str,
    pub domain_owner: &'static str,
    pub remote_transport: &'static str,
    pub allowed_operations: &'static [&'static str],
    pub sensitive: bool,
}

pub const ALLOY_AI_VERTICALS: &[AlloyAiVerticalDescriptor] = &[AlloyAiVerticalDescriptor {
    task_slug: ALLOY_CODE_TASK_SLUG,
    tool_name: ALLOY_CODE_TOOL_NAME,
    sensitive: false,
    runtime_operation: "run_script",
    runtime_payload_json_shape: "absent_blank_or_json_object",
    transport_owner: "rustok-ai",
}];

pub const ALLOY_SCRIPT_ALLOWED_OPERATIONS: &[&str] = &[
    "list_scripts",
    "get_script",
    "validate_script",
    "run_script",
];

// These values mirror the existing platform permission contract. Alloy-specific
// script read/write/execute permissions must not be invented in this adapter;
// they require an explicit rustok-api/rbac foundation decision first.
const ALLOY_TASK_RUN_PERMISSIONS: &[&str] = &["ai:tasks:alloy:run"];

/// Code-agent roles are owned by Alloy. Their model assignment and invocation
/// lifecycle are intentionally left to the generic AI runtime.
pub const ALLOY_CODE_AGENTS: &[AlloyCodeAgentDescriptor] = &[
    AlloyCodeAgentDescriptor {
        slug: "alloy_code_planner",
        display_name: "Alloy code planner",
        responsibility: "Inspect Alloy scripts and produce a bounded implementation plan.",
        required_permissions: ALLOY_TASK_RUN_PERMISSIONS,
        allowed_operations: &["list_scripts", "get_script", "validate_script"],
        requires_approval: false,
    },
    AlloyCodeAgentDescriptor {
        slug: "alloy_code_implementer",
        display_name: "Alloy code implementer",
        responsibility: "Draft and validate Alloy script changes; applying a change stays approval-gated.",
        required_permissions: ALLOY_TASK_RUN_PERMISSIONS,
        allowed_operations: &["list_scripts", "get_script", "validate_script"],
        requires_approval: true,
    },
    AlloyCodeAgentDescriptor {
        slug: "alloy_code_reviewer",
        display_name: "Alloy code reviewer",
        responsibility: "Review a proposed Alloy script change without applying it.",
        required_permissions: ALLOY_TASK_RUN_PERMISSIONS,
        allowed_operations: &["list_scripts", "get_script", "validate_script"],
        requires_approval: false,
    },
    AlloyCodeAgentDescriptor {
        slug: "alloy_code_verifier",
        display_name: "Alloy code verifier",
        responsibility: "Validate and execute permitted verification scripts.",
        required_permissions: ALLOY_TASK_RUN_PERMISSIONS,
        allowed_operations: &[
            "list_scripts",
            "get_script",
            "validate_script",
            "run_script",
        ],
        requires_approval: true,
    },
];

const ALLOY_CHANGE_REVIEW_STAGES: &[AlloySwarmStageDescriptor] = &[
    AlloySwarmStageDescriptor {
        id: "plan",
        agent_slug: "alloy_code_planner",
        depends_on: &[],
        requires_approval: false,
    },
    AlloySwarmStageDescriptor {
        id: "implement",
        agent_slug: "alloy_code_implementer",
        depends_on: &["plan"],
        requires_approval: true,
    },
    AlloySwarmStageDescriptor {
        id: "review",
        agent_slug: "alloy_code_reviewer",
        depends_on: &["implement"],
        requires_approval: false,
    },
    AlloySwarmStageDescriptor {
        id: "verify",
        agent_slug: "alloy_code_verifier",
        depends_on: &["review"],
        requires_approval: true,
    },
];

pub const ALLOY_SWARM_WORKFLOWS: &[AlloySwarmWorkflowDescriptor] =
    &[AlloySwarmWorkflowDescriptor {
        slug: "alloy_change_review",
        display_name: "Alloy change review",
        stages: ALLOY_CHANGE_REVIEW_STAGES,
    }];

pub const ALLOY_STAGE_EXECUTIONS: &[AlloyStageExecutionDescriptor] = &[
    AlloyStageExecutionDescriptor {
        agent_slug: "alloy_code_planner",
        task_slug: ALLOY_CODE_TASK_SLUG,
        required_operation: "list_scripts",
        input_shape: "alloy_task_input_list_scripts",
    },
    AlloyStageExecutionDescriptor {
        agent_slug: "alloy_code_implementer",
        task_slug: ALLOY_CODE_TASK_SLUG,
        required_operation: "validate_script",
        input_shape: "alloy_task_input_validate_script",
    },
    AlloyStageExecutionDescriptor {
        agent_slug: "alloy_code_reviewer",
        task_slug: ALLOY_CODE_TASK_SLUG,
        required_operation: "validate_script",
        input_shape: "alloy_task_input_validate_script",
    },
    AlloyStageExecutionDescriptor {
        agent_slug: "alloy_code_verifier",
        task_slug: ALLOY_CODE_TASK_SLUG,
        required_operation: "run_script",
        input_shape: "alloy_task_input_run_script",
    },
];

pub const ALLOY_SCRIPT_EXECUTION_POLICY: AlloyScriptExecutionPolicy = AlloyScriptExecutionPolicy {
    script_runtime: "alloy",
    runtime_payload_json_shape: "absent_blank_or_json_object",
    composition_owner: "rustok-ai",
    domain_owner: "rustok-ai-alloy",
    remote_transport: "not_started",
    allowed_operations: ALLOY_SCRIPT_ALLOWED_OPERATIONS,
    sensitive: false,
};

pub fn alloy_ai_verticals() -> &'static [AlloyAiVerticalDescriptor] {
    ALLOY_AI_VERTICALS
}

pub fn register_alloy_ai_verticals() -> &'static [AlloyAiVerticalDescriptor] {
    alloy_ai_verticals()
}

pub fn register_alloy_ai_vertical_handlers(
    mut register: impl FnMut(&'static AlloyAiVerticalDescriptor),
) {
    for vertical in alloy_ai_verticals() {
        register(vertical);
    }
}

pub fn alloy_script_execution_policy() -> &'static AlloyScriptExecutionPolicy {
    &ALLOY_SCRIPT_EXECUTION_POLICY
}

pub fn alloy_code_agents() -> &'static [AlloyCodeAgentDescriptor] {
    ALLOY_CODE_AGENTS
}

pub fn alloy_swarm_workflows() -> &'static [AlloySwarmWorkflowDescriptor] {
    ALLOY_SWARM_WORKFLOWS
}

pub fn alloy_stage_execution(agent_slug: &str) -> Option<&'static AlloyStageExecutionDescriptor> {
    ALLOY_STAGE_EXECUTIONS
        .iter()
        .find(|descriptor| descriptor.agent_slug == agent_slug)
}

pub fn validate_stage_execution_input(
    agent_slug: &str,
    payload: &serde_json::Value,
) -> Result<&'static AlloyStageExecutionDescriptor, String> {
    let binding = alloy_stage_execution(agent_slug)
        .ok_or_else(|| format!("unknown Alloy code agent `{agent_slug}`"))?;
    let operation = payload
        .get("operation")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("{} requires a task input operation", binding.agent_slug))?;
    if operation != binding.required_operation {
        return Err(format!(
            "{} requires operation `{}` rather than `{operation}`",
            binding.agent_slug, binding.required_operation
        ));
    }
    Ok(binding)
}

pub fn validate_runtime_payload(payload: Option<&str>) -> Result<(), String> {
    let Some(payload) = payload.filter(|value| !value.trim().is_empty()) else {
        return Ok(());
    };
    let parsed: serde_json::Value = serde_json::from_str(payload)
        .map_err(|err| format!("runtime_payload_json must be valid JSON: {err}"))?;
    if !parsed.is_object() {
        return Err("runtime_payload_json must be a JSON object".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_runtime_payload() {
        assert!(validate_runtime_payload(None).is_ok());
        assert!(validate_runtime_payload(Some("{}")).is_ok());
        assert!(validate_runtime_payload(Some("[]")).is_err());
        assert!(validate_runtime_payload(Some("not json")).is_err());
    }

    #[test]
    fn test_alloy_descriptor_records_runtime_policy() {
        let [vertical] = alloy_ai_verticals() else {
            panic!("expected exactly one alloy vertical");
        };
        assert_eq!(vertical.task_slug, ALLOY_CODE_TASK_SLUG);
        assert_eq!(vertical.tool_name, ALLOY_CODE_TOOL_NAME);
        assert_eq!(vertical.runtime_operation, "run_script");
        assert_eq!(
            vertical.runtime_payload_json_shape,
            "absent_blank_or_json_object"
        );
        assert_eq!(vertical.transport_owner, "rustok-ai");
    }

    #[test]
    fn test_alloy_execution_policy_records_allowed_operations() {
        let policy = alloy_script_execution_policy();
        assert_eq!(policy.script_runtime, "alloy");
        assert_eq!(policy.remote_transport, "not_started");
        assert_eq!(
            policy.allowed_operations,
            [
                "list_scripts",
                "get_script",
                "validate_script",
                "run_script"
            ]
        );
    }

    #[test]
    fn alloy_operation_catalog_uses_the_policy_slugs() {
        let operations = [
            AlloyOperation::ListScripts,
            AlloyOperation::GetScript,
            AlloyOperation::ValidateScript,
            AlloyOperation::RunScript,
        ];

        for operation in operations {
            assert!(ALLOY_SCRIPT_ALLOWED_OPERATIONS.contains(&operation.slug()));
        }
    }

    #[test]
    fn code_agents_and_swarm_are_owner_owned_and_bounded() {
        assert_eq!(alloy_code_agents().len(), 4);
        let workflow = alloy_swarm_workflows()
            .iter()
            .find(|workflow| workflow.slug == "alloy_change_review")
            .expect("Alloy change workflow must be registered");
        assert_eq!(workflow.stages[0].id, "plan");
        assert_eq!(workflow.stages[3].depends_on, ["review"]);
        assert!(workflow.stages[1].requires_approval);
        assert!(workflow.stages[3].requires_approval);
    }

    #[test]
    fn every_code_agent_has_one_explicit_execution_binding() {
        for agent in alloy_code_agents() {
            let binding = alloy_stage_execution(agent.slug)
                .expect("every Alloy code agent must publish execution metadata");
            assert_eq!(binding.task_slug, ALLOY_CODE_TASK_SLUG);
            assert!(ALLOY_SCRIPT_ALLOWED_OPERATIONS.contains(&binding.required_operation));
        }
    }

    #[test]
    fn stage_input_cannot_select_an_operation_outside_its_role_binding() {
        assert!(
            validate_stage_execution_input(
                "alloy_code_verifier",
                &serde_json::json!({"operation":"run_script"})
            )
            .is_ok()
        );
        assert!(
            validate_stage_execution_input(
                "alloy_code_verifier",
                &serde_json::json!({"operation":"validate_script"})
            )
            .is_err()
        );
    }
}
