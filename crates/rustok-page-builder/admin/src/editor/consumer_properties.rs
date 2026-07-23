use crate::{
    ConsumerPropertyEditorRuntime, ConsumerPropertyEditorSnapshot, ConsumerPropertyFieldKind,
};
use fly_ui::ContributionAssemblyResult;
use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::collections::BTreeMap;
use std::sync::Arc;

#[component]
pub(crate) fn ConsumerPropertiesPanel(
    runtime: Option<Arc<ConsumerPropertyEditorRuntime>>,
    contribution_assembly: Option<Arc<ContributionAssemblyResult>>,
) -> impl IntoView {
    let Some(runtime) = runtime else {
        return view! { <span hidden data-fly-consumer-properties="unbound"></span> }.into_any();
    };
    let Some(assembly) = contribution_assembly else {
        return error_view("Consumer properties require a contribution assembly");
    };
    if let Err(error) = runtime.verify_contribution(&assembly) {
        return error_view(&error.to_string());
    }

    let load_runtime = Arc::clone(&runtime);
    let snapshot = LocalResource::new(move || {
        let runtime = Arc::clone(&load_runtime);
        async move { runtime.load().await }
    });

    view! {
        <Suspense fallback=|| view! {
            <section
                class="space-y-3 rounded-xl border border-border bg-card p-3"
                data-fly-consumer-properties="loading"
            >
                <div class="h-5 w-36 animate-pulse rounded bg-muted"></div>
                <div class="h-9 animate-pulse rounded bg-muted"></div>
                <div class="h-9 animate-pulse rounded bg-muted"></div>
            </section>
        }>
            {move || {
                snapshot.get().map(|result| match result {
                    Ok(snapshot) => view! {
                        <LoadedConsumerPropertiesPanel
                            runtime=Arc::clone(&runtime)
                            snapshot
                        />
                    }.into_any(),
                    Err(error) => error_view(&error.to_string()),
                })
            }}
        </Suspense>
    }
    .into_any()
}

#[component]
fn LoadedConsumerPropertiesPanel(
    runtime: Arc<ConsumerPropertyEditorRuntime>,
    snapshot: ConsumerPropertyEditorSnapshot,
) -> impl IntoView {
    let schema = runtime.schema.clone();
    let revision = RwSignal::new(snapshot.revision);
    let scope_label = snapshot.scope_label;
    let values = RwSignal::new(snapshot.values);
    let busy = RwSignal::new(false);
    let saved = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);

    let submit_runtime = Arc::clone(&runtime);
    let submit = move |event: SubmitEvent| {
        event.prevent_default();
        if busy.get_untracked() {
            return;
        }
        let current_values = values.get_untracked();
        let current_snapshot = ConsumerPropertyEditorSnapshot {
            revision: revision.get_untracked(),
            scope_label: scope_label.clone(),
            values: current_values.clone(),
        };
        let input = match submit_runtime.prepare_save_input(&current_snapshot, current_values) {
            Ok(input) => input,
            Err(validation_error) => {
                saved.set(false);
                error.set(Some(validation_error.to_string()));
                return;
            }
        };

        busy.set(true);
        saved.set(false);
        error.set(None);
        let runtime = Arc::clone(&submit_runtime);
        spawn_local(async move {
            match runtime.save(input).await {
                Ok(receipt) => {
                    revision.set(receipt.revision);
                    values.set(receipt.values);
                    saved.set(true);
                }
                Err(save_error) => error.set(Some(save_error.to_string())),
            }
            busy.set(false);
        });
    };

    view! {
        <section
            class="space-y-3 rounded-xl border border-border bg-card p-3"
            data-fly-consumer-properties="ready"
            data-fly-consumer-property-editor=runtime.property_editor_id.clone()
        >
            <div>
                <div class="flex flex-wrap items-center justify-between gap-2">
                    <h2 class="font-semibold">{schema.title.clone()}</h2>
                    <span class="rounded-full bg-muted px-2 py-1 text-[10px] font-semibold uppercase text-muted-foreground">
                        {scope_label}
                    </span>
                </div>
                {schema.description.clone().map(|description| view! {
                    <p class="mt-1 text-xs text-muted-foreground">{description}</p>
                })}
            </div>

            <form class="space-y-3" on:submit=submit>
                {schema.fields.into_iter().map(|field| {
                    let value_id = field.id.clone();
                    let input_id = format!("fly-consumer-property-{}", field.id);
                    let update_id = field.id.clone();
                    let placeholder = field.placeholder.clone().unwrap_or_default();
                    let help = field.help.clone();
                    let max_length = field.max_bytes.to_string();
                    let control = match field.kind {
                        ConsumerPropertyFieldKind::Text | ConsumerPropertyFieldKind::StringList => {
                            view! {
                                <input
                                    id=input_id.clone()
                                    name=field.id.clone()
                                    class="mt-1 w-full rounded border border-input bg-background px-2 py-1 text-sm"
                                    prop:value=move || values.with(|values| {
                                        values.get(&value_id).cloned().unwrap_or_default()
                                    })
                                    on:input=move |event| {
                                        saved.set(false);
                                        let value = event_target_value(&event);
                                        values.update(|values| {
                                            values.insert(update_id.clone(), value);
                                        });
                                    }
                                    placeholder=placeholder
                                    maxlength=max_length
                                    required=field.required
                                    autocomplete="off"
                                />
                            }.into_any()
                        }
                        ConsumerPropertyFieldKind::TextArea => {
                            view! {
                                <textarea
                                    id=input_id.clone()
                                    name=field.id.clone()
                                    class="mt-1 min-h-24 w-full rounded border border-input bg-background px-2 py-1 text-sm"
                                    prop:value=move || values.with(|values| {
                                        values.get(&value_id).cloned().unwrap_or_default()
                                    })
                                    on:input=move |event| {
                                        saved.set(false);
                                        let value = event_target_value(&event);
                                        values.update(|values| {
                                            values.insert(update_id.clone(), value);
                                        });
                                    }
                                    placeholder=placeholder
                                    maxlength=max_length
                                    required=field.required
                                ></textarea>
                            }.into_any()
                        }
                    };
                    view! {
                        <label class="block text-sm font-medium" for=input_id>
                            {field.label}
                            {control}
                            {help.map(|help| view! {
                                <span class="mt-1 block text-xs font-normal text-muted-foreground">{help}</span>
                            })}
                        </label>
                    }
                }).collect_view()}

                {move || error.get().map(|message| view! {
                    <div
                        class="rounded border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive"
                        role="alert"
                    >{message}</div>
                })}
                {move || saved.get().then(|| view! {
                    <div
                        class="rounded border border-emerald-300/40 bg-emerald-50 px-3 py-2 text-xs text-emerald-800"
                        role="status"
                    >"Properties saved"</div>
                })}

                <button
                    type="submit"
                    class="rounded bg-primary px-3 py-2 text-sm font-medium text-primary-foreground disabled:opacity-50"
                    disabled=move || busy.get()
                >
                    {move || if busy.get() { "Saving..." } else { "Save properties" }}
                </button>
            </form>
        </section>
    }
}

fn error_view(message: &str) -> AnyView {
    view! {
        <section
            class="rounded-xl border border-destructive/30 bg-destructive/10 px-3 py-3 text-xs text-destructive"
            data-fly-consumer-properties="error"
            role="alert"
        >
            {message.to_string()}
        </section>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_values_remain_string_keyed() {
        let values = BTreeMap::from([
            ("title".to_string(), "Home".to_string()),
            ("channels".to_string(), "web, mobile".to_string()),
        ]);
        assert_eq!(values.get("title").map(String::as_str), Some("Home"));
    }
}
