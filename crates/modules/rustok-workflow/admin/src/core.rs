use crate::model::{WorkflowStatus, WorkflowSummary, WorkflowTemplateDto};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowStatusPresentation {
    pub i18n_key: &'static str,
    pub fallback_label: &'static str,
    pub class_name: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowRowViewModel {
    pub id: String,
    pub name: String,
    pub failure_count: String,
    pub updated_at: String,
    pub detail_href: String,
    pub status: WorkflowStatusPresentation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowTemplateCardViewModel {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub category_class_name: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowAdminTransportContext {
    pub token: Option<String>,
    pub tenant_slug: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowTemplateCreateCommand {
    pub template_id: String,
    pub workflow_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowAdminNavViewModel {
    pub toggle_href: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowErrorViewModel {
    pub message: String,
}

pub fn workflow_status_presentation(status: &WorkflowStatus) -> WorkflowStatusPresentation {
    match status {
        WorkflowStatus::Active => WorkflowStatusPresentation {
            i18n_key: "workflow.status.active",
            fallback_label: "Active",
            class_name:
                "bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400",
        },
        WorkflowStatus::Paused => WorkflowStatusPresentation {
            i18n_key: "workflow.status.paused",
            fallback_label: "Paused",
            class_name: "bg-yellow-50 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400",
        },
        WorkflowStatus::Archived => WorkflowStatusPresentation {
            i18n_key: "workflow.status.archived",
            fallback_label: "Archived",
            class_name: "bg-muted text-muted-foreground",
        },
        WorkflowStatus::Draft | WorkflowStatus::Unknown => WorkflowStatusPresentation {
            i18n_key: "workflow.status.draft",
            fallback_label: "Draft",
            class_name: "bg-primary/10 text-primary",
        },
    }
}

pub fn workflow_row_view_model(workflow: WorkflowSummary) -> WorkflowRowViewModel {
    WorkflowRowViewModel {
        id: workflow.id.clone(),
        name: workflow.name,
        failure_count: workflow.failure_count.to_string(),
        updated_at: workflow.updated_at,
        detail_href: workflow_detail_href(&workflow.id),
        status: workflow_status_presentation(&workflow.status),
    }
}

pub fn workflow_detail_href(workflow_id: &str) -> String {
    format!("/workflows/{workflow_id}")
}

pub fn workflow_template_card_view_model(
    template: WorkflowTemplateDto,
) -> WorkflowTemplateCardViewModel {
    WorkflowTemplateCardViewModel {
        id: template.id,
        name: template.name,
        description: template.description,
        category_class_name: template_category_class_name(&template.category),
        category: template.category,
    }
}

pub fn template_category_class_name(category: &str) -> &'static str {
    match category {
        "content" => "bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-300",
        "commerce" => "bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-300",
        "auth" => "bg-purple-100 text-purple-700 dark:bg-purple-900/40 dark:text-purple-300",
        "reporting" => "bg-yellow-100 text-yellow-700 dark:bg-yellow-900/40 dark:text-yellow-300",
        "integrations" => {
            "bg-orange-100 text-orange-700 dark:bg-orange-900/40 dark:text-orange-300"
        }
        _ => "bg-muted text-muted-foreground",
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

pub fn workflow_admin_transport_context(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> WorkflowAdminTransportContext {
    WorkflowAdminTransportContext {
        token,
        tenant_slug,
    }
}

pub fn workflow_template_create_command(
    template_id: &str,
    entered_name: &str,
    default_name_prefix: &str,
) -> WorkflowTemplateCreateCommand {
    WorkflowTemplateCreateCommand {
        template_id: template_id.trim().to_string(),
        workflow_name: workflow_name_from_template_input(
            entered_name,
            default_name_prefix,
            template_id,
        ),
    }
}

pub fn workflow_admin_nav_view_model(
    route_segment: Option<&str>,
    showing_templates: bool,
) -> WorkflowAdminNavViewModel {
    let route = route_segment.unwrap_or("workflow");
    let toggle_href = if showing_templates {
        format!("/modules/{route}")
    } else {
        format!("/modules/{route}/templates")
    };
    WorkflowAdminNavViewModel { toggle_href }
}

pub fn workflow_error_view_model(
    context: &str,
    error: impl std::fmt::Display,
) -> WorkflowErrorViewModel {
    WorkflowErrorViewModel {
        message: format!("{context}: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_transport_context_keeps_runtime_credentials_scoped_to_the_ui_adapter() {
        let context = workflow_admin_transport_context(
            Some("token".to_string()),
            Some("tenant-a".to_string()),
        );
        assert_eq!(context.token.as_deref(), Some("token"));
        assert_eq!(context.tenant_slug.as_deref(), Some("tenant-a"));
    }

    #[test]
    fn workflow_template_command_trims_and_builds_default_name() {
        let command = workflow_template_create_command(" tpl-1 ", "  ", "Workflow from");
        assert_eq!(command.template_id, "tpl-1");
        assert_eq!(command.workflow_name, "Workflow from  tpl-1 ");
    }

    #[test]
    fn workflow_status_and_navigation_are_framework_agnostic() {
        assert_eq!(
            workflow_status_presentation(&WorkflowStatus::Active).i18n_key,
            "workflow.status.active"
        );
        assert_eq!(
            workflow_admin_nav_view_model(Some("workflow"), false).toggle_href,
            "/modules/workflow/templates"
        );
    }
}
