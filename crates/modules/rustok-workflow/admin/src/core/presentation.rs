use rustok_grid::{
    ColumnAlign, ColumnFilters, FilterOption, FilterValue, GridColumnDef, GridFilterType,
};

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
            class_name: "bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400",
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

pub fn workflow_grid_columns(locale: Option<&str>) -> Vec<GridColumnDef> {
    let is_ru = locale.map(|l| l.starts_with("ru")).unwrap_or(false);
    vec![
        GridColumnDef::new("name", if is_ru { "Название" } else { "Name" })
            .width(260)
            .align(ColumnAlign::Left)
            .filter(GridFilterType::Text {
                placeholder: Some(if is_ru {
                    "Фильтр названия...".to_string()
                } else {
                    "Filter name...".to_string()
                }),
            }),
        GridColumnDef::new("status", if is_ru { "Статус" } else { "Status" })
            .width(140)
            .align(ColumnAlign::Center)
            .filter(GridFilterType::Select {
                options: vec![
                    FilterOption {
                        value: "active".to_string(),
                        label: if is_ru {
                            "Активен".to_string()
                        } else {
                            "Active".to_string()
                        },
                    },
                    FilterOption {
                        value: "paused".to_string(),
                        label: if is_ru {
                            "Приостановлен".to_string()
                        } else {
                            "Paused".to_string()
                        },
                    },
                    FilterOption {
                        value: "archived".to_string(),
                        label: if is_ru {
                            "В архиве".to_string()
                        } else {
                            "Archived".to_string()
                        },
                    },
                    FilterOption {
                        value: "draft".to_string(),
                        label: if is_ru {
                            "Черновик".to_string()
                        } else {
                            "Draft".to_string()
                        },
                    },
                ],
                placeholder: Some(if is_ru {
                    "Все статусы".to_string()
                } else {
                    "All statuses".to_string()
                }),
            }),
        GridColumnDef::new("failures", if is_ru { "Ошибки" } else { "Failures" })
            .width(110)
            .align(ColumnAlign::Right),
        GridColumnDef::new("updated_at", if is_ru { "Обновлен" } else { "Updated" })
            .width(160)
            .align(ColumnAlign::Right),
        GridColumnDef::new("actions", if is_ru { "Действия" } else { "Actions" })
            .width(100)
            .align(ColumnAlign::Right)
            .not_sortable(),
    ]
}

pub fn matches_workflow_filter(workflow: &WorkflowRowViewModel, filters: &ColumnFilters) -> bool {
    for (col_id, filter_val) in filters.iter() {
        match (col_id.as_str(), filter_val) {
            ("name", FilterValue::Text(q)) => {
                if !workflow.name.to_lowercase().contains(&q.to_lowercase()) {
                    return false;
                }
            }
            ("status", FilterValue::Select(s)) => {
                let status_label = workflow.status.fallback_label.to_lowercase();
                if !status_label.contains(&s.to_lowercase()) {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

pub fn filter_workflows(
    workflows: &[WorkflowRowViewModel],
    filters: &ColumnFilters,
    search: Option<&str>,
) -> Vec<WorkflowRowViewModel> {
    let search_term = search
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());

    workflows
        .iter()
        .filter(|workflow| {
            if let Some(ref term) = search_term {
                let matches_global = workflow.name.to_lowercase().contains(term)
                    || workflow.status.fallback_label.to_lowercase().contains(term);
                if !matches_global {
                    return false;
                }
            }

            matches_workflow_filter(workflow, filters)
        })
        .cloned()
        .collect()
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
    fn workflow_grid_columns_localization() {
        let cols_en = workflow_grid_columns(Some("en"));
        assert_eq!(cols_en[0].title, "Name");
        assert_eq!(cols_en[1].title, "Status");
        assert_eq!(cols_en[2].title, "Failures");

        let cols_ru = workflow_grid_columns(Some("ru"));
        assert_eq!(cols_ru[0].title, "Название");
        assert_eq!(cols_ru[1].title, "Статус");
        assert_eq!(cols_ru[2].title, "Ошибки");
    }

    #[test]
    fn filter_workflows_by_search_and_status() {
        let wf1 = WorkflowRowViewModel {
            id: "1".into(),
            name: "Order Processing".into(),
            failure_count: "0".into(),
            updated_at: "2026-06-01".into(),
            detail_href: "/workflows/1".into(),
            status: workflow_status_presentation(&WorkflowStatus::Active),
        };
        let wf2 = WorkflowRowViewModel {
            id: "2".into(),
            name: "User Sync".into(),
            failure_count: "5".into(),
            updated_at: "2026-06-02".into(),
            detail_href: "/workflows/2".into(),
            status: workflow_status_presentation(&WorkflowStatus::Paused),
        };
        let list = vec![wf1.clone(), wf2.clone()];

        // Search filter
        let res = filter_workflows(&list, &ColumnFilters::new(), Some("order"));
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "1");

        // Status filter
        let mut filters = ColumnFilters::new();
        filters.set("status", FilterValue::Select("paused".into()));
        let res = filter_workflows(&list, &filters, None);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "2");
    }
}
