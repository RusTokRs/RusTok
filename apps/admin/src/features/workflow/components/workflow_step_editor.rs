use leptos::prelude::*;

use crate::entities::workflow::WorkflowStep;
use crate::features::workflow::api;

const STEP_TYPES: &[&str] = &[
    "ACTION",
    "CONDITION",
    "DELAY",
    "ALLOY_SCRIPT",
    "EMIT_EVENT",
    "HTTP",
    "NOTIFY",
    "TRANSFORM",
];

#[component]
pub fn WorkflowStepEditor(
    workflow_id: String,
    steps: Vec<WorkflowStep>,
    token: Option<String>,
    tenant_slug: Option<String>,
    #[prop(into)] on_change: Callback<()>,
) -> impl IntoView {
    let (steps_sig, set_steps) = signal(steps);
    let (new_step_type, set_new_step_type) = signal("ACTION".to_string());
    let (new_on_error, set_new_on_error) = signal("STOP".to_string());
    let (adding, set_adding) = signal(false);
    let (error_msg, set_error_msg) = signal(Option::<String>::None);

    let wf_id = workflow_id.clone();
    let tok = token.clone();
    let ts = tenant_slug.clone();

    let handle_add = move |_| {
        let wf_id = wf_id.clone();
        let tok = tok.clone();
        let ts = ts.clone();
        let position = (steps_sig.get().len() as i32 + 1) * 10;
        let step_type = new_step_type.get();
        let on_error = new_on_error.get();

        set_adding.set(true);
        set_error_msg.set(None);

        leptos::task::spawn_local(async move {
            let result = api::add_step(
                tok,
                ts,
                wf_id,
                api::CreateStepInput {
                    position,
                    step_type,
                    config: serde_json::json!({}),
                    on_error,
                    timeout_ms: None,
                },
            )
            .await;

            set_adding.set(false);
            match result {
                Ok(_) => on_change.run(()),
                Err(e) => set_error_msg.set(Some(e.to_string())),
            }
        });
    };

    let wf_id2 = workflow_id.clone();
    let tok2 = token.clone();
    let ts2 = tenant_slug.clone();

    let handle_delete = move |step_id: String| {
        let wf_id = wf_id2.clone();
        let tok = tok2.clone();
        let ts = ts2.clone();
        let sid = step_id.clone();

        set_error_msg.set(None);

        leptos::task::spawn_local(async move {
            let result = api::delete_step(tok, ts, wf_id, sid.clone()).await;
            match result {
                Ok(_) => {
                    set_steps.update(|s| s.retain(|step| step.id != sid));
                    on_change.run(());
                }
                Err(e) => set_error_msg.set(Some(e.to_string())),
            }
        });
    };

    view! {
        <div class="space-y-3">
            {move || {
                let steps = steps_sig.get();
                if steps.is_empty() {
                    view! {
                        <p class="text-sm text-muted-foreground">"No steps yet. Add one below."</p>
                    }.into_any()
                } else {
                    view! {
                        <ol class="space-y-2">
                            {steps.into_iter().enumerate().map(|(idx, step)| {
                                let sid = step.id.clone();
                                let del = handle_delete.clone();
                                view! {
                                    <li class="flex items-center gap-3 rounded-lg border border-border bg-card px-4 py-3 text-sm shadow-sm">
                                        <span class="flex h-6 w-6 flex-shrink-0 items-center justify-center rounded-full bg-primary/10 text-xs font-bold text-primary">
                                            {idx + 1}
                                        </span>
                                        <span class="flex-1 font-mono text-xs text-foreground">
                                            {format!("{}", step.step_type)}
                                        </span>
                                        <span class="text-xs text-muted-foreground">
                                            {format!("on_error: {}", step.on_error)}
                                        </span>
                                        <button
                                            on:click=move |_| (del)(sid.clone())
                                            class="text-xs font-medium text-destructive hover:underline"
                                        >
                                            "Remove"
                                        </button>
                                    </li>
                                }
                            }).collect_view()}
                        </ol>
                    }.into_any()
                }
            }}

            {move || error_msg.get().map(|msg| view! {
                <p class="text-xs text-destructive">{msg}</p>
            })}

            <div class="flex flex-wrap items-end gap-2 pt-2">
                <div>
                    <label class="mb-1 block text-xs font-medium text-muted-foreground">"Step type"</label>
                    <select
                        on:change=move |ev| set_new_step_type.set(event_target_value(&ev))
                        class="rounded-md border border-input bg-background px-2 py-1.5 text-sm"
                    >
                        {STEP_TYPES.iter().map(|t| view! {
                            <option value=*t>{*t}</option>
                        }).collect_view()}
                    </select>
                </div>
                <div>
                    <label class="mb-1 block text-xs font-medium text-muted-foreground">"On error"</label>
                    <select
                        on:change=move |ev| set_new_on_error.set(event_target_value(&ev))
                        class="rounded-md border border-input bg-background px-2 py-1.5 text-sm"
                    >
                        <option value="STOP">"Stop"</option>
                        <option value="SKIP">"Skip"</option>
                        <option value="RETRY">"Retry"</option>
                    </select>
                </div>
                <button
                    on:click=handle_add
                    disabled=move || adding.get()
                    class="rounded-lg bg-primary px-4 py-1.5 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50"
                >
                    {move || if adding.get() { "Adding…" } else { "+ Add Step" }}
                </button>
            </div>
        </div>
    }
}
