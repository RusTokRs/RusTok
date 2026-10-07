use leptos::prelude::*;
use rustok_grid::{
    ColumnAlign, ColumnFilters, FilterOption, FilterValue, GridColumnDef, GridFilterType,
    GridPagination,
};
use rustok_grid_leptos::prelude::*;

use crate::entities::workflow::{ExecutionStatus, WorkflowExecution};

pub fn execution_history_grid_columns() -> Vec<GridColumnDef> {
    vec![
        GridColumnDef::new("status", "Status")
            .width(140)
            .align(ColumnAlign::Center)
            .filter(GridFilterType::select(vec![
                FilterOption {
                    value: "COMPLETED".to_string(),
                    label: "Completed".to_string(),
                },
                FilterOption {
                    value: "FAILED".to_string(),
                    label: "Failed".to_string(),
                },
                FilterOption {
                    value: "RUNNING".to_string(),
                    label: "Running".to_string(),
                },
                FilterOption {
                    value: "TIMED_OUT".to_string(),
                    label: "Timed out".to_string(),
                },
            ])),
        GridColumnDef::new("started", "Started")
            .width(180)
            .align(ColumnAlign::Left)
            .filter(GridFilterType::text_with_placeholder("Filter started...")),
        GridColumnDef::new("completed", "Completed")
            .width(180)
            .align(ColumnAlign::Left),
        GridColumnDef::new("steps", "Steps")
            .width(90)
            .align(ColumnAlign::Center),
        GridColumnDef::new("error", "Error")
            .width(260)
            .align(ColumnAlign::Left)
            .filter(GridFilterType::text_with_placeholder("Filter error...")),
    ]
}

pub fn matches_execution_filter(
    exec: &WorkflowExecution,
    query: &str,
    filters: &ColumnFilters,
) -> bool {
    let q = query.trim().to_lowercase();
    if !q.is_empty() {
        let matches_id = exec.id.to_lowercase().contains(&q);
        let matches_err = exec
            .error
            .as_deref()
            .map(|e| e.to_lowercase().contains(&q))
            .unwrap_or(false);
        let matches_status = exec.status.to_string().to_lowercase().contains(&q);
        let matches_started = exec.started_at.to_lowercase().contains(&q);
        if !matches_id && !matches_err && !matches_status && !matches_started {
            return false;
        }
    }

    for (col, val) in filters.iter() {
        match (col.as_str(), val) {
            ("status", FilterValue::Select(status_val)) => {
                if !status_val.is_empty()
                    && !exec.status.to_string().eq_ignore_ascii_case(status_val)
                {
                    return false;
                }
            }
            ("started", FilterValue::Text(started_val)) => {
                let needle = started_val.trim().to_lowercase();
                if !needle.is_empty() && !exec.started_at.to_lowercase().contains(&needle) {
                    return false;
                }
            }
            ("error", FilterValue::Text(err_val)) => {
                let needle = err_val.trim().to_lowercase();
                if !needle.is_empty() {
                    let matches = exec
                        .error
                        .as_deref()
                        .map(|e| e.to_lowercase().contains(&needle))
                        .unwrap_or(false);
                    if !matches {
                        return false;
                    }
                }
            }
            _ => {}
        }
    }

    true
}

#[component]
pub fn ExecutionHistory(executions: Vec<WorkflowExecution>) -> impl IntoView {
    if executions.is_empty() {
        return view! {
            <p class="text-sm text-muted-foreground">"No executions yet."</p>
        }
        .into_any();
    }

    let (search_query, set_search_query) = signal(String::new());
    let filters = RwSignal::new(ColumnFilters::new());
    let pagination = RwSignal::new(GridPagination::new(1, 10, executions.len() as u64));

    let columns = execution_history_grid_columns();
    let rows = executions.clone();

    let filtered_items = Memo::new(move |_| {
        let q = search_query.get();
        let flt = filters.get();
        rows.iter()
            .filter(|item| matches_execution_filter(item, &q, &flt))
            .cloned()
            .collect::<Vec<_>>()
    });

    let on_filters_change = Callback::new(move |new_filters: ColumnFilters| {
        filters.set(new_filters);
        pagination.update(|p| p.page = 1);
    });

    let cell_renderer =
        Callback::new(
            move |(item, col_id): (WorkflowExecution, String)| match col_id.as_str() {
                "status" => view! {
                    <ExecutionBadge status=item.status />
                }
                .into_any(),
                "started" => view! {
                    <span class="text-xs text-muted-foreground">{item.started_at}</span>
                }
                .into_any(),
                "completed" => view! {
                    <span class="text-xs text-muted-foreground">
                        {item.completed_at.unwrap_or_else(|| "—".into())}
                    </span>
                }
                .into_any(),
                "steps" => view! {
                    <span class="text-muted-foreground">{item.step_executions.len()}</span>
                }
                .into_any(),
                "error" => view! {
                    <span class="max-w-xs truncate text-xs text-destructive">
                        {item.error.unwrap_or_else(|| "—".into())}
                    </span>
                }
                .into_any(),
                _ => view! { <span /> }.into_any(),
            },
        );

    view! {
        <div class="space-y-3">
            <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                <div class="relative max-w-sm flex-1">
                    <input
                        type="text"
                        placeholder="Search executions..."
                        class="w-full rounded-lg border border-border bg-background px-3 py-1.5 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
                        prop:value=move || search_query.get()
                        on:input=move |ev| set_search_query.set(event_target_value(&ev))
                    />
                </div>
            </div>

            <DataGrid
                columns=columns
                data=Signal::derive(move || filtered_items.get())
                key_fn=|item: &WorkflowExecution| item.id.clone()
                cell_renderer=cell_renderer
                empty_message="No executions found matching filter".to_string()
                pagination=pagination
                filters=filters
                on_filter_change=on_filters_change
            />
        </div>
    }
    .into_any()
}

#[component]
pub fn ExecutionBadge(status: ExecutionStatus) -> impl IntoView {
    let (label, cls) = match status {
        ExecutionStatus::Completed => (
            "Completed",
            "bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400",
        ),
        ExecutionStatus::Failed => ("Failed", "bg-destructive/10 text-destructive"),
        ExecutionStatus::Running => ("Running", "bg-primary/10 text-primary"),
        ExecutionStatus::TimedOut => (
            "Timed out",
            "bg-orange-50 text-orange-700 dark:bg-orange-900/30 dark:text-orange-400",
        ),
        ExecutionStatus::Unknown => ("Unknown", "bg-muted text-muted-foreground"),
    };
    view! {
        <span class=format!("inline-flex rounded-full px-2.5 py-0.5 text-xs font-semibold {}", cls)>
            {label}
        </span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::workflow::StepExecution;

    fn sample_execution(
        id: &str,
        status: ExecutionStatus,
        started: &str,
        error: Option<&str>,
    ) -> WorkflowExecution {
        WorkflowExecution {
            id: id.to_string(),
            workflow_id: "wf-1".to_string(),
            status,
            error: error.map(|s| s.to_string()),
            started_at: started.to_string(),
            completed_at: Some("2026-10-05 10:00:05 UTC".to_string()),
            step_executions: vec![StepExecution {
                id: "s-1".to_string(),
                step_id: "step-1".to_string(),
                status: "COMPLETED".to_string(),
                error: None,
                started_at: started.to_string(),
                completed_at: None,
            }],
        }
    }

    #[test]
    fn test_grid_columns_structure() {
        let cols = execution_history_grid_columns();
        assert_eq!(cols.len(), 5);
        assert_eq!(cols[0].id, "status");
        assert_eq!(cols[1].id, "started");
        assert_eq!(cols[2].id, "completed");
        assert_eq!(cols[3].id, "steps");
        assert_eq!(cols[4].id, "error");
    }

    #[test]
    fn test_matches_filter_search_query() {
        let exec = sample_execution(
            "exec-abc",
            ExecutionStatus::Completed,
            "2026-10-05 09:30",
            None,
        );
        let filters = ColumnFilters::new();

        assert!(matches_execution_filter(&exec, "exec-abc", &filters));
        assert!(matches_execution_filter(&exec, "completed", &filters));
        assert!(matches_execution_filter(&exec, "09:30", &filters));
        assert!(!matches_execution_filter(&exec, "failed", &filters));
    }

    #[test]
    fn test_matches_filter_column_filters() {
        let exec_ok = sample_execution(
            "exec-1",
            ExecutionStatus::Completed,
            "2026-10-05 08:00",
            None,
        );
        let exec_fail = sample_execution(
            "exec-2",
            ExecutionStatus::Failed,
            "2026-10-05 09:00",
            Some("Network timeout"),
        );

        let mut filters = ColumnFilters::new();
        filters.set("status", FilterValue::Select("COMPLETED".to_string()));

        assert!(matches_execution_filter(&exec_ok, "", &filters));
        assert!(!matches_execution_filter(&exec_fail, "", &filters));

        let mut err_filters = ColumnFilters::new();
        err_filters.set("error", FilterValue::Text("timeout".to_string()));
        assert!(!matches_execution_filter(&exec_ok, "", &err_filters));
        assert!(matches_execution_filter(&exec_fail, "", &err_filters));
    }
}
