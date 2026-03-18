use leptos::prelude::*;
use leptos_router::components::A;

use crate::entities::workflow::{WorkflowStatus, WorkflowSummary};

#[component]
pub fn WorkflowList(workflows: Vec<WorkflowSummary>) -> impl IntoView {
    if workflows.is_empty() {
        return view! {
            <div class="rounded-xl border border-dashed border-border p-12 text-center">
                <p class="text-sm text-muted-foreground">"No workflows yet. Create one to get started."</p>
                <A
                    href="/workflows/new"
                    attr:class="mt-4 inline-flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground"
                >
                    "+ New Workflow"
                </A>
            </div>
        }
        .into_any();
    }

    view! {
        <div class="overflow-hidden rounded-xl border border-border">
            <table class="w-full text-sm">
                <thead class="border-b border-border bg-muted/50">
                    <tr>
                        <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Name"</th>
                        <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Status"</th>
                        <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Failures"</th>
                        <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Updated"</th>
                        <th class="px-4 py-3"></th>
                    </tr>
                </thead>
                <tbody class="divide-y divide-border">
                    {workflows.into_iter().map(|wf| {
                        let id = wf.id.clone();
                        view! {
                            <tr class="hover:bg-muted/30 transition-colors">
                                <td class="px-4 py-3 font-medium text-foreground">{wf.name.clone()}</td>
                                <td class="px-4 py-3">
                                    <StatusBadge status=wf.status />
                                </td>
                                <td class="px-4 py-3 text-muted-foreground">{wf.failure_count}</td>
                                <td class="px-4 py-3 text-muted-foreground text-xs">{wf.updated_at.clone()}</td>
                                <td class="px-4 py-3 text-right">
                                    <A
                                        href=format!("/workflows/{}", id)
                                        attr:class="text-xs font-medium text-primary hover:underline"
                                    >
                                        "View →"
                                    </A>
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
pub fn StatusBadge(status: WorkflowStatus) -> impl IntoView {
    let (label, cls) = match status {
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
        <span class=format!("inline-flex rounded-full px-2.5 py-0.5 text-xs font-semibold {}", cls)>
            {label}
        </span>
    }
}
