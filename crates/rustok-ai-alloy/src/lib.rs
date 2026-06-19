#![cfg_attr(not(feature = "server"), allow(dead_code))]

pub const ALLOY_CODE_TASK_SLUG: &str = "alloy_code";
pub const ALLOY_CODE_TOOL_NAME: &str = "direct.alloy.run_script";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlloyAiVerticalDescriptor {
    pub task_slug: &'static str,
    pub tool_name: &'static str,
    pub sensitive: bool,
}

pub const ALLOY_AI_VERTICALS: &[AlloyAiVerticalDescriptor] = &[AlloyAiVerticalDescriptor {
    task_slug: ALLOY_CODE_TASK_SLUG,
    tool_name: ALLOY_CODE_TOOL_NAME,
    sensitive: false,
}];

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
}
