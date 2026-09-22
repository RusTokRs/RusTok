use leptos::prelude::*;

use crate::entities::workflow::{ExecutionStatus, WorkflowExecution};

#[component]
pub fn ExecutionHistory(executions: Vec<WorkflowExecution>) -> impl IntoView {
    if executions.is_empty() {
        return view! {
            <p class="text-sm text-muted-foreground">"No executions yet."</p>
        }
        .into_any();
    }

    view! {
        <div class="overflow-hidden rounded-xl border border-border">
            <table class="w-full text-sm">
                <thead class="border-b border-border bg-muted/50">
                    <tr>
                        <th class="px-4 py-2 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Status"</th>
                        <th class="px-4 py-2 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Started"</th>
                        <th class="px-4 py-2 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Completed"</th>
                        <th class="px-4 py-2 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Steps"</th>
                        <th class="px-4 py-2 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Error"</th>
                    </tr>
                </thead>
                <tbody class="divide-y divide-border">
                    {executions.into_iter().map(|exec| {
                        let step_count = exec.step_executions.len();
                        view! {
                            <tr class="hover:bg-muted/30 transition-colors">
                                <td class="px-4 py-2">
                                    <ExecutionBadge status=exec.status />
                                </td>
                                <td class="px-4 py-2 text-xs text-muted-foreground">{exec.started_at.clone()}</td>
                                <td class="px-4 py-2 text-xs text-muted-foreground">
                                    {exec.completed_at.clone().unwrap_or_else(|| "—".into())}
                                </td>
                                <td class="px-4 py-2 text-muted-foreground">{step_count}</td>
                                <td class="px-4 py-2 max-w-xs truncate text-xs text-destructive">
                                    {exec.error.clone().unwrap_or_else(|| "—".into())}
                                </td>
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
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
