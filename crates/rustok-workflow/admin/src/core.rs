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
    fn workflow_status_presentations_are_framework_agnostic() {
        assert_eq!(
            workflow_status_presentation(&WorkflowStatus::Active).i18n_key,
            "workflow.status.active"
        );
        assert_eq!(
            workflow_status_presentation(&WorkflowStatus::Paused).fallback_label,
            "Paused"
        );
        assert_eq!(
            workflow_status_presentation(&WorkflowStatus::Archived).class_name,
            "bg-muted text-muted-foreground"
        );
        assert_eq!(
            workflow_status_presentation(&WorkflowStatus::Unknown).i18n_key,
            "workflow.status.draft"
        );
    }

    #[test]
    fn workflow_row_view_model_formats_operator_fields() {
        let row = workflow_row_view_model(WorkflowSummary {
            id: "wf-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            name: "Publish flow".to_string(),
            status: WorkflowStatus::Active,
            failure_count: 3,
            created_at: "2026-05-01T00:00:00Z".to_string(),
            updated_at: "2026-05-02T00:00:00Z".to_string(),
        });

        assert_eq!(row.id, "wf-1");
        assert_eq!(row.name, "Publish flow");
        assert_eq!(row.failure_count, "3");
        assert_eq!(row.updated_at, "2026-05-02T00:00:00Z");
        assert_eq!(row.detail_href, "/workflows/wf-1");
        assert_eq!(row.status.i18n_key, "workflow.status.active");
    }

    #[test]
    fn template_view_model_maps_known_and_unknown_category_styles() {
        let template = workflow_template_card_view_model(WorkflowTemplateDto {
            id: "tpl-1".to_string(),
            name: "Content moderation".to_string(),
            description: "Review new content".to_string(),
            category: "content".to_string(),
            trigger_config: serde_json::json!({"type": "manual"}),
        });

        assert_eq!(template.id, "tpl-1");
        assert_eq!(
            template.category_class_name,
            "bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-300"
        );
        assert_eq!(
            template_category_class_name("custom"),
            "bg-muted text-muted-foreground"
        );
    }

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
}
