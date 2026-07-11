use leptos::prelude::*;

use crate::entities::workflow::WorkflowStatus;

#[component]
pub fn StatusBadge(status: WorkflowStatus) -> impl IntoView {
    let (label, class_name) = match status {
        WorkflowStatus::Active => (
            "Active",
            "bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400",
        ),
        WorkflowStatus::Paused => (
            "Paused",
            "bg-yellow-50 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400",
        ),
        WorkflowStatus::Archived => ("Archived", "bg-muted text-muted-foreground"),
        WorkflowStatus::Draft | WorkflowStatus::Unknown => ("Draft", "bg-primary/10 text-primary"),
    };

    view! {
        <span class=format!("inline-flex rounded-full px-2.5 py-0.5 text-xs font-semibold {}", class_name)>
            {label}
        </span>
    }
}
