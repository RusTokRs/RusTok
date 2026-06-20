#![cfg_attr(not(feature = "server"), allow(dead_code))]

pub const ALLOY_CODE_TASK_SLUG: &str = "alloy_code";
pub const ALLOY_CODE_TOOL_NAME: &str = "direct.alloy.run_script";

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
        assert_eq!(vertical.runtime_payload_json_shape, "absent_blank_or_json_object");
        assert_eq!(vertical.transport_owner, "rustok-ai");
    }

    #[test]
    fn test_alloy_execution_policy_records_allowed_operations() {
        let policy = alloy_script_execution_policy();
        assert_eq!(policy.script_runtime, "alloy");
        assert_eq!(policy.remote_transport, "not_started");
        assert_eq!(
            policy.allowed_operations,
            ["list_scripts", "get_script", "validate_script", "run_script"]
        );
    }
}
