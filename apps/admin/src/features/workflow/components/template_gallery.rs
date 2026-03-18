use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::features::workflow::api::{self, WorkflowTemplateDto};

#[component]
pub fn TemplateGallery(
    token: Option<String>,
    tenant_slug: Option<String>,
    /// Called with the new workflow id after creating from a template
    on_created: Callback<String>,
) -> impl IntoView {
    let (pending_id, set_pending_id) = signal(Option::<String>::None);
    let (name_input, set_name_input) = signal(String::new());

    let tok = token.clone();
    let ts = tenant_slug.clone();
    let templates_resource = Resource::new_blocking(
        move || (tok.clone(), ts.clone()),
        move |(tok, ts)| async move { api::fetch_templates(tok, ts).await },
    );

    view! {
        <div class="space-y-4">
            <h2 class="text-lg font-semibold text-foreground">"Marketplace Templates"</h2>
            <p class="text-sm text-muted-foreground">
                "Pick a ready-made workflow template to get started quickly."
            </p>

            <Suspense fallback=move || view! {
                <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
                    {(0..4).map(|_| view! {
                        <div class="h-28 animate-pulse rounded-xl bg-muted"></div>
                    }).collect::<Vec<_>>()}
                </div>
            }>
                {move || {
                    let pending = pending_id.get();
                    let name = name_input.get();

                    templates_resource.get().map(|result| match result {
                        Err(e) => view! {
                            <div class="rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                {e.to_string()}
                            </div>
                        }.into_any(),
                        Ok(templates) => view! {
                            <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
                                {templates.iter().map(|t| {
                                    let template_id = t.id.clone();
                                    let tok2 = token.clone();
                                    let ts2 = tenant_slug.clone();

                                    let is_pending = pending.as_deref() == Some(&template_id);

                                    view! {
                                        <TemplateCard
                                            template=t.clone()
                                            is_pending=is_pending
                                            name=name.clone()
                                            on_name_change=Callback::new(move |v| set_name_input.set(v))
                                            on_use=Callback::new(move |_| {
                                                let id = template_id.clone();
                                                let tok3 = tok2.clone();
                                                let ts3 = ts2.clone();
                                                let on_created = on_created;
                                                let nm = name_input.get_untracked();
                                                set_pending_id.set(Some(id.clone()));

                                                spawn_local(async move {
                                                    let workflow_name = if nm.trim().is_empty() {
                                                        format!("My {id}")
                                                    } else {
                                                        nm
                                                    };
                                                    match api::create_from_template(tok3, ts3, id, workflow_name).await {
                                                        Ok(wf_id) => {
                                                            set_pending_id.set(None);
                                                            on_created.run(wf_id);
                                                        }
                                                        Err(_) => {
                                                            set_pending_id.set(None);
                                                        }
                                                    }
                                                });
                                            })
                                        />
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        }.into_any(),
                    })
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn TemplateCard(
    template: WorkflowTemplateDto,
    is_pending: bool,
    name: String,
    on_name_change: Callback<String>,
    on_use: Callback<()>,
) -> impl IntoView {
    let category_color = match template.category.as_str() {
        "content" => "bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-300",
        "commerce" => "bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-300",
        "auth" => "bg-purple-100 text-purple-700 dark:bg-purple-900/40 dark:text-purple-300",
        "reporting" => "bg-yellow-100 text-yellow-700 dark:bg-yellow-900/40 dark:text-yellow-300",
        "integrations" => {
            "bg-orange-100 text-orange-700 dark:bg-orange-900/40 dark:text-orange-300"
        }
        _ => "bg-muted text-muted-foreground",
    };

    view! {
        <div class="flex flex-col rounded-xl border border-border bg-card p-4 gap-2">
            <div class="flex items-start justify-between gap-2">
                <span class=format!("rounded-full px-2 py-0.5 text-xs font-medium {category_color}")>
                    {template.category.clone()}
                </span>
            </div>
            <h3 class="text-sm font-semibold text-foreground leading-tight">{template.name.clone()}</h3>
            <p class="text-xs text-muted-foreground line-clamp-2">{template.description.clone()}</p>
            <div class="mt-auto pt-2 flex gap-2">
                <input
                    type="text"
                    placeholder="Workflow name…"
                    class="flex-1 min-w-0 rounded-lg border border-input bg-background px-2 py-1 text-xs"
                    prop:value=name.clone()
                    on:input=move |ev| on_name_change.run(event_target_value(&ev))
                />
                <button
                    on:click=move |_| on_use.run(())
                    disabled=is_pending
                    class="rounded-lg bg-primary px-3 py-1 text-xs font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
                >
                    {if is_pending { "…" } else { "Use" }}
                </button>
            </div>
        </div>
    }
}
