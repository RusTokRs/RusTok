#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowTemplateCreateCommand {
    pub template_id: String,
    pub workflow_name: String,
}

pub fn workflow_template_create_command(
    template_id: &str,
    entered_name: &str,
    default_name_prefix: &str,
) -> WorkflowTemplateCreateCommand {
    WorkflowTemplateCreateCommand {
        template_id: template_id.to_string(),
        workflow_name: workflow_name_from_template_input(
            entered_name,
            default_name_prefix,
            template_id,
        ),
    }
}

pub fn workflow_name_from_template_input(
    entered_name: &str,
    default_name_prefix: &str,
    template_id: &str,
) -> String {
    let trimmed = entered_name.trim();
    if trimmed.is_empty() {
        format!("{default_name_prefix} {template_id}")
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_name_from_template_input_trims_or_builds_default() {
        assert_eq!(
            workflow_name_from_template_input("  Custom flow  ", "Workflow from", "tpl-1"),
            "Custom flow"
        );
        assert_eq!(
            workflow_name_from_template_input("   ", "Workflow from", "tpl-1"),
            "Workflow from tpl-1"
        );
    }

    #[test]
    fn workflow_template_create_command_owns_name_policy() {
        let explicit = workflow_template_create_command("tpl-1", "  Campaign  ", "Workflow from");
        assert_eq!(explicit.template_id, "tpl-1");
        assert_eq!(explicit.workflow_name, "Campaign");

        let defaulted = workflow_template_create_command("tpl-2", "", "Workflow from");
        assert_eq!(defaulted.template_id, "tpl-2");
        assert_eq!(defaulted.workflow_name, "Workflow from tpl-2");
    }
}
