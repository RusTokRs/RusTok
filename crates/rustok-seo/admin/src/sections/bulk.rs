use leptos::prelude::*;
use rustok_seo::{
    SeoBulkApplyMode, SeoBulkFieldPatchMode, SeoBulkJobRecord, SeoBulkPage, SeoBulkSelectionMode,
    SeoBulkSource, SeoTargetRegistryEntry, SeoTargetSlug,
};
use std::str::FromStr;
use uuid::Uuid;

use crate::api::{self, ApiError};
use crate::i18n::t;
use crate::model::{SeoBulkActionForm, SeoBulkFilterForm};

#[component]
pub fn SeoBulkPane(
    ui_locale: Option<String>,
    bulk_filter_form: RwSignal<SeoBulkFilterForm>,
    bulk_action_form: RwSignal<SeoBulkActionForm>,
    bulk_selected_ids: RwSignal<Vec<Uuid>>,
    bulk_selection_preview: RwSignal<Option<i32>>,
    bulk_targets: Resource<Result<Vec<SeoTargetRegistryEntry>, ApiError>>,
    bulk_items: Resource<Result<SeoBulkPage, ApiError>>,
    bulk_jobs: Resource<Result<Vec<SeoBulkJobRecord>, ApiError>>,
    busy_key: RwSignal<Option<String>>,
    on_refresh: Callback<()>,
    on_preview_selection: Callback<()>,
    on_queue_apply: Callback<()>,
    on_queue_export: Callback<()>,
    on_queue_import: Callback<()>,
) -> impl IntoView {
    let busy = Signal::derive(move || busy_key.get().is_some());
    let title = t(ui_locale.as_deref(), "seo.bulk.title", "Bulk Editor");
    let subtitle = t(
        ui_locale.as_deref(),
        "seo.bulk.subtitle",
        "Central SEO operator surface for one target kind and one locale per run. Owner-module SEO editors stay responsible for per-entity authoring.",
    );
    let select_visible = Callback::new(move |_| {
        if let Some(Ok(page)) = bulk_items.get_untracked() {
            bulk_selected_ids.set(page.items.into_iter().map(|item| item.target_id).collect());
        }
    });
    let clear_selected = Callback::new(move |_| bulk_selected_ids.set(Vec::new()));

    view! {
        <div class="space-y-6">
            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-2">
                    <h2 class="text-lg font-semibold text-card-foreground">{title}</h2>
                    <p class="max-w-3xl text-sm text-muted-foreground">{subtitle}</p>
                </div>
            </section>

            <div class="grid gap-6 2xl:grid-cols-[minmax(0,0.95fr)_minmax(0,1.05fr)]">
                <section class="space-y-5 rounded-2xl border border-border bg-card p-6 shadow-sm">
                    <div class="space-y-2">
                        <h3 class="text-base font-semibold text-card-foreground">"Scope"</h3>
                        <p class="text-sm text-muted-foreground">
                            "Each bulk run is bound to one target kind and one normalized locale. Host locale only seeds the default form value."
                        </p>
                    </div>

                    <div class="grid gap-4 md:grid-cols-2">
                        <label class="grid gap-2 text-sm">
                            <span class="font-medium text-foreground">"Target kind"</span>
                            {move || match bulk_targets.get() {
                                Some(Ok(entries)) if !entries.is_empty() => view! {
                                    <select
                                        class="rounded-lg border border-input bg-background px-3 py-2"
                                        prop:value=move || bulk_filter_form.get().target_kind.as_str().to_string()
                                        on:change=move |ev| {
                                            if let Ok(slug) = SeoTargetSlug::new(event_target_value(&ev)) {
                                                bulk_filter_form.update(|draft| draft.target_kind = slug);
                                            }
                                        }
                                    >
                                        {entries
                                            .into_iter()
                                            .map(|entry| {
                                                let slug = entry.slug.as_str().to_string();
                                                let label = format!(
                                                    "{} [{}]",
                                                    entry.display_name,
                                                    entry.owner_module_slug
                                                );
                                                view! { <option value=slug.clone()>{label}</option> }
                                            })
                                            .collect_view()}
                                    </select>
                                }
                                .into_any(),
                                Some(Ok(_)) => view! {
                                    <p class="rounded-lg border border-border/80 bg-background/60 px-3 py-2 text-sm text-muted-foreground">
                                        "No bulk-capable SEO targets are registered in the shared runtime registry."
                                    </p>
                                }
                                .into_any(),
                                Some(Err(err)) => view! {
                                    <p class="rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive">
                                        {format!("Failed to load registered bulk targets: {}", err)}
                                    </p>
                                }
                                .into_any(),
                                None => view! {
                                    <p class="rounded-lg border border-border/80 bg-background/60 px-3 py-2 text-sm text-muted-foreground">
                                        "Loading registered bulk targets..."
                                    </p>
                                }
                                .into_any(),
                            }}
                        </label>

                        <label class="grid gap-2 text-sm">
                            <span class="font-medium text-foreground">"Locale"</span>
                            <input
                                class="rounded-lg border border-input bg-background px-3 py-2"
                                prop:value=move || bulk_filter_form.get().locale.clone()
                                on:input=move |ev| bulk_filter_form.update(|draft| draft.locale = event_target_value(&ev))
                            />
                        </label>

                        <label class="grid gap-2 text-sm">
                            <span class="font-medium text-foreground">"Source"</span>
                            <select
                                class="rounded-lg border border-input bg-background px-3 py-2"
                                prop:value=move || bulk_filter_form.get().source.as_str().to_string()
                                on:change=move |ev| bulk_filter_form.update(|draft| {
                                    draft.source = SeoBulkSource::from_str(event_target_value(&ev).as_str())
                                        .unwrap_or(SeoBulkSource::Any);
                                })
                            >
                                <option value="any">"any"</option>
                                <option value="explicit">"explicit"</option>
                                <option value="generated">"generated"</option>
                                <option value="fallback">"fallback"</option>
                            </select>
                        </label>

                        <label class="grid gap-2 text-sm">
                            <span class="font-medium text-foreground">"Page size"</span>
                            <input
                                type="number"
                                min="1"
                                max="100"
                                class="rounded-lg border border-input bg-background px-3 py-2"
                                prop:value=move || bulk_filter_form.get().per_page.to_string()
                                on:input=move |ev| bulk_filter_form.update(|draft| {
                                    draft.per_page = event_target_value(&ev).parse::<i32>().unwrap_or(20);
                                })
                            />
                        </label>
                    </div>

                    <label class="grid gap-2 text-sm">
                        <span class="font-medium text-foreground">"Query"</span>
                        <input
                            class="rounded-lg border border-input bg-background px-3 py-2"
                            prop:value=move || bulk_filter_form.get().query.clone()
                            on:input=move |ev| bulk_filter_form.update(|draft| draft.query = event_target_value(&ev))
                        />
                    </label>

                    <div class="flex flex-wrap gap-3">
                        <button
                            type="button"
                            class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:opacity-60"
                            disabled=move || busy.get()
                            on:click=move |_| on_refresh.run(())
                        >
                            "Refresh grid"
                        </button>
                        <button
                            type="button"
                            class="rounded-lg border border-border px-4 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-60"
                            disabled=move || busy.get()
                            on:click=move |_| on_preview_selection.run(())
                        >
                            "Preview selection"
                        </button>
                    </div>

                    <div class="rounded-xl border border-border/80 bg-background/60 px-4 py-3 text-sm text-foreground">
                        {move || {
                            let selected = bulk_selected_ids.get().len();
                            let preview = bulk_selection_preview.get().map(|value| value.to_string()).unwrap_or_else(|| "n/a".to_string());
                            format!("Selected IDs: {selected} | Server-side preview: {preview}")
                        }}
                    </div>
                </section>

                <section class="space-y-4 rounded-2xl border border-border bg-card p-6 shadow-sm">
                    <div class="flex flex-wrap items-start justify-between gap-3">
                        <div class="space-y-2">
                            <h3 class="text-base font-semibold text-card-foreground">"Grid"</h3>
                            <p class="text-sm text-muted-foreground">
                                "Resolved values are shown for the current locale-scoped scope. `explicit`, `generated`, and `fallback` reflect the same read precedence used by the SEO runtime."
                            </p>
                        </div>
                        <div class="flex flex-wrap gap-2">
                            <button
                                type="button"
                                class="rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent"
                                on:click=move |_| select_visible.run(())
                            >
                                "Select visible"
                            </button>
                            <button
                                type="button"
                                class="rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent"
                                on:click=move |_| clear_selected.run(())
                            >
                                "Clear selection"
                            </button>
                        </div>
                    </div>

                    <Suspense fallback=move || view! { <p class="text-sm text-muted-foreground">"Loading bulk grid..."</p> }>
                        {move || match bulk_items.get() {
                            Some(Ok(page)) if !page.items.is_empty() => view! { <BulkGridList page=page bulk_selected_ids=bulk_selected_ids /> }.into_any(),
                            Some(Ok(_)) => view! { <p class="text-sm text-muted-foreground">"No items matched the current bulk scope."</p> }.into_any(),
                            Some(Err(err)) => view! { <p class="text-sm text-destructive">{err.to_string()}</p> }.into_any(),
                            None => view! { <p class="text-sm text-muted-foreground">"No bulk scope loaded yet."</p> }.into_any(),
                        }}
                    </Suspense>
                </section>
            </div>

            <div class="grid gap-6 2xl:grid-cols-[minmax(0,1.05fr)_minmax(0,0.95fr)]">
                <section class="space-y-5 rounded-2xl border border-border bg-card p-6 shadow-sm">
                    <div class="space-y-2">
                        <h3 class="text-base font-semibold text-card-foreground">"Action composer"</h3>
                        <p class="text-sm text-muted-foreground">
                            "`apply` uses explicit safe modes over generated/template output. `export` serializes resolved values for the current locale grid. `import` accepts only the same target kind and locale as the current scope."
                        </p>
                    </div>

                    <div class="grid gap-4 xl:grid-cols-3">
                        <label class="grid gap-2 text-sm">
                            <span class="font-medium text-foreground">"Selection mode"</span>
                            <select
                                class="rounded-lg border border-input bg-background px-3 py-2"
                                prop:value=move || bulk_action_form.get().selection_mode.as_str().to_string()
                                on:change=move |ev| bulk_action_form.update(|draft| {
                                    draft.selection_mode = SeoBulkSelectionMode::from_str(event_target_value(&ev).as_str())
                                        .unwrap_or(SeoBulkSelectionMode::CurrentFilterScope);
                                })
                            >
                                <option value="current_filter_scope">"current_filter_scope"</option>
                                <option value="selected_ids">"selected_ids"</option>
                            </select>
                        </label>
                        <label class="grid gap-2 text-sm">
                            <span class="font-medium text-foreground">"Apply mode"</span>
                            <select
                                class="rounded-lg border border-input bg-background px-3 py-2"
                                prop:value=move || bulk_action_form.get().apply_mode.as_str().to_string()
                                on:change=move |ev| bulk_action_form.update(|draft| {
                                    draft.apply_mode = SeoBulkApplyMode::from_str(event_target_value(&ev).as_str())
                                        .unwrap_or(SeoBulkApplyMode::ApplyMissingOnly);
                                })
                            >
                                <option value="preview_only">"preview_only"</option>
                                <option value="apply_missing_only">"apply_missing_only"</option>
                                <option value="overwrite_generated_only">"overwrite_generated_only"</option>
                                <option value="force_overwrite_explicit">"force_overwrite_explicit"</option>
                            </select>
                        </label>
                        <label class="flex items-center justify-between gap-4 rounded-xl border border-border/80 bg-background/60 px-4 py-3 text-sm">
                            <span class="font-medium text-foreground">"Publish after write"</span>
                            <input
                                type="checkbox"
                                class="h-5 w-5 accent-primary"
                                prop:checked=move || bulk_action_form.get().publish_after_write
                                disabled=move || bulk_action_form.get().apply_mode == SeoBulkApplyMode::PreviewOnly
                                on:change=move |ev| bulk_action_form.update(|draft| draft.publish_after_write = event_target_checked(&ev))
                            />
                        </label>
                    </div>

                    <div class="grid gap-4 xl:grid-cols-2">
                        <BulkStringPatchField
                            label="Title".to_string()
                            mode_value=Signal::derive(move || bulk_action_form.get().title.mode.as_str().to_string())
                            value=Signal::derive(move || bulk_action_form.get().title.value.clone())
                            on_mode=Callback::new(move |value: String| bulk_action_form.update(|draft| {
                                draft.title.mode = SeoBulkFieldPatchMode::from_str(value.as_str()).unwrap_or(SeoBulkFieldPatchMode::Keep);
                            }))
                            on_value=Callback::new(move |value: String| bulk_action_form.update(|draft| draft.title.value = value))
                        />
                        <BulkStringPatchField
                            label="Description".to_string()
                            mode_value=Signal::derive(move || bulk_action_form.get().description.mode.as_str().to_string())
                            value=Signal::derive(move || bulk_action_form.get().description.value.clone())
                            on_mode=Callback::new(move |value: String| bulk_action_form.update(|draft| {
                                draft.description.mode = SeoBulkFieldPatchMode::from_str(value.as_str()).unwrap_or(SeoBulkFieldPatchMode::Keep);
                            }))
                            on_value=Callback::new(move |value: String| bulk_action_form.update(|draft| draft.description.value = value))
                        />
                        <BulkStringPatchField
                            label="Keywords".to_string()
                            mode_value=Signal::derive(move || bulk_action_form.get().keywords.mode.as_str().to_string())
                            value=Signal::derive(move || bulk_action_form.get().keywords.value.clone())
                            on_mode=Callback::new(move |value: String| bulk_action_form.update(|draft| {
                                draft.keywords.mode = SeoBulkFieldPatchMode::from_str(value.as_str()).unwrap_or(SeoBulkFieldPatchMode::Keep);
                            }))
                            on_value=Callback::new(move |value: String| bulk_action_form.update(|draft| draft.keywords.value = value))
                        />
                        <BulkStringPatchField
                            label="Canonical URL".to_string()
                            mode_value=Signal::derive(move || bulk_action_form.get().canonical_url.mode.as_str().to_string())
                            value=Signal::derive(move || bulk_action_form.get().canonical_url.value.clone())
                            on_mode=Callback::new(move |value: String| bulk_action_form.update(|draft| {
                                draft.canonical_url.mode = SeoBulkFieldPatchMode::from_str(value.as_str()).unwrap_or(SeoBulkFieldPatchMode::Keep);
                            }))
                            on_value=Callback::new(move |value: String| bulk_action_form.update(|draft| draft.canonical_url.value = value))
                        />
                        <BulkStringPatchField
                            label="OG title".to_string()
                            mode_value=Signal::derive(move || bulk_action_form.get().og_title.mode.as_str().to_string())
                            value=Signal::derive(move || bulk_action_form.get().og_title.value.clone())
                            on_mode=Callback::new(move |value: String| bulk_action_form.update(|draft| {
                                draft.og_title.mode = SeoBulkFieldPatchMode::from_str(value.as_str()).unwrap_or(SeoBulkFieldPatchMode::Keep);
                            }))
                            on_value=Callback::new(move |value: String| bulk_action_form.update(|draft| draft.og_title.value = value))
                        />
                        <BulkStringPatchField
                            label="OG description".to_string()
                            mode_value=Signal::derive(move || bulk_action_form.get().og_description.mode.as_str().to_string())
                            value=Signal::derive(move || bulk_action_form.get().og_description.value.clone())
                            on_mode=Callback::new(move |value: String| bulk_action_form.update(|draft| {
                                draft.og_description.mode = SeoBulkFieldPatchMode::from_str(value.as_str()).unwrap_or(SeoBulkFieldPatchMode::Keep);
                            }))
                            on_value=Callback::new(move |value: String| bulk_action_form.update(|draft| draft.og_description.value = value))
                        />
                        <BulkStringPatchField
                            label="OG image".to_string()
                            mode_value=Signal::derive(move || bulk_action_form.get().og_image.mode.as_str().to_string())
                            value=Signal::derive(move || bulk_action_form.get().og_image.value.clone())
                            on_mode=Callback::new(move |value: String| bulk_action_form.update(|draft| {
                                draft.og_image.mode = SeoBulkFieldPatchMode::from_str(value.as_str()).unwrap_or(SeoBulkFieldPatchMode::Keep);
                            }))
                            on_value=Callback::new(move |value: String| bulk_action_form.update(|draft| draft.og_image.value = value))
                        />
                        <BulkBoolPatchField
                            label="Noindex".to_string()
                            mode_value=Signal::derive(move || bulk_action_form.get().noindex.mode.as_str().to_string())
                            checked=Signal::derive(move || bulk_action_form.get().noindex.value)
                            on_mode=Callback::new(move |value: String| bulk_action_form.update(|draft| {
                                draft.noindex.mode = SeoBulkFieldPatchMode::from_str(value.as_str()).unwrap_or(SeoBulkFieldPatchMode::Keep);
                            }))
                            on_checked=Callback::new(move |value: bool| bulk_action_form.update(|draft| draft.noindex.value = value))
                        />
                        <BulkBoolPatchField
                            label="Nofollow".to_string()
                            mode_value=Signal::derive(move || bulk_action_form.get().nofollow.mode.as_str().to_string())
                            checked=Signal::derive(move || bulk_action_form.get().nofollow.value)
                            on_mode=Callback::new(move |value: String| bulk_action_form.update(|draft| {
                                draft.nofollow.mode = SeoBulkFieldPatchMode::from_str(value.as_str()).unwrap_or(SeoBulkFieldPatchMode::Keep);
                            }))
                            on_checked=Callback::new(move |value: bool| bulk_action_form.update(|draft| draft.nofollow.value = value))
                        />
                    </div>

                    <BulkJsonPatchField
                        label="Structured data JSON".to_string()
                        mode_value=Signal::derive(move || bulk_action_form.get().structured_data.mode.as_str().to_string())
                        value=Signal::derive(move || bulk_action_form.get().structured_data.value.clone())
                        on_mode=Callback::new(move |value: String| bulk_action_form.update(|draft| {
                            draft.structured_data.mode = SeoBulkFieldPatchMode::from_str(value.as_str()).unwrap_or(SeoBulkFieldPatchMode::Keep);
                        }))
                        on_value=Callback::new(move |value: String| bulk_action_form.update(|draft| draft.structured_data.value = value))
                    />

                    <label class="grid gap-2 text-sm">
                        <span class="font-medium text-foreground">"CSV import payload"</span>
                        <textarea
                            class="min-h-40 rounded-lg border border-input bg-background px-3 py-2 font-mono text-xs"
                            prop:value=move || bulk_action_form.get().import_csv.clone()
                            on:input=move |ev| bulk_action_form.update(|draft| draft.import_csv = event_target_value(&ev))
                        />
                    </label>

                    <div class="flex flex-wrap gap-3">
                        <button
                            type="button"
                            class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:opacity-60"
                            disabled=move || busy.get()
                            on:click=move |_| on_queue_apply.run(())
                        >
                            "Queue apply"
                        </button>
                        <button
                            type="button"
                            class="rounded-lg border border-border px-4 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-60"
                            disabled=move || busy.get()
                            on:click=move |_| on_queue_export.run(())
                        >
                            "Queue export CSV"
                        </button>
                        <button
                            type="button"
                            class="rounded-lg border border-border px-4 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-60"
                            disabled=move || busy.get()
                            on:click=move |_| on_queue_import.run(())
                        >
                            "Queue import CSV"
                        </button>
                    </div>
                </section>

                <section class="space-y-4 rounded-2xl border border-border bg-card p-6 shadow-sm">
                    <div class="space-y-2">
                        <h3 class="text-base font-semibold text-card-foreground">"Jobs"</h3>
                        <p class="text-sm text-muted-foreground">
                            "Recent bulk jobs stay tenant-scoped. Artifacts are served from the SEO module HTTP endpoint with tenant and RBAC checks."
                        </p>
                    </div>

                    <Suspense fallback=move || view! { <p class="text-sm text-muted-foreground">"Loading bulk jobs..."</p> }>
                        {move || match bulk_jobs.get() {
                            Some(Ok(items)) if !items.is_empty() => view! {
                                <ul class="space-y-3">
                                    {items.into_iter().map(render_bulk_job).collect_view()}
                                </ul>
                            }.into_any(),
                            Some(Ok(_)) => view! { <p class="text-sm text-muted-foreground">"No bulk jobs yet."</p> }.into_any(),
                            Some(Err(err)) => view! { <p class="text-sm text-destructive">{err.to_string()}</p> }.into_any(),
                            None => view! { <p class="text-sm text-muted-foreground">"No bulk jobs yet."</p> }.into_any(),
                        }}
                    </Suspense>
                </section>
            </div>
        </div>
    }
}

#[component]
fn BulkGridList(page: SeoBulkPage, bulk_selected_ids: RwSignal<Vec<Uuid>>) -> impl IntoView {
    view! {
        <div class="space-y-3">
            <div class="text-sm text-muted-foreground">{format!("Total scope items: {}", page.total)}</div>
            <ul class="space-y-3">
                {page.items.into_iter().map(|item| {
                    let target_id = item.target_id;
                    view! {
                        <li class="rounded-xl border border-border/80 bg-background/60 px-4 py-3">
                            <div class="flex gap-3">
                                <input
                                    type="checkbox"
                                    class="mt-1 h-4 w-4 accent-primary"
                                    prop:checked=move || bulk_selected_ids.get().contains(&target_id)
                                    on:change=move |ev| bulk_selected_ids.update(|ids| {
                                        if event_target_checked(&ev) {
                                            if !ids.contains(&target_id) {
                                                ids.push(target_id);
                                            }
                                        } else {
                                            ids.retain(|value| value != &target_id);
                                        }
                                    })
                                />
                                <div class="min-w-0 flex-1 space-y-1">
                                    <div class="flex flex-wrap items-center gap-2">
                                        <span class="font-medium text-foreground">{item.label}</span>
                                        <span class="rounded-full bg-secondary px-2 py-0.5 text-xs font-medium text-secondary-foreground">
                                            {item.source.as_str().to_string()}
                                        </span>
                                        <span class="text-xs text-muted-foreground">{item.effective_locale}</span>
                                    </div>
                                    <div class="break-all text-xs text-muted-foreground">{item.route}</div>
                                    <div class="grid gap-2 text-sm text-foreground md:grid-cols-2">
                                        <div>{format!("title: {}", item.title.unwrap_or_else(|| "n/a".to_string()))}</div>
                                        <div>{format!("description: {}", item.description.unwrap_or_else(|| "n/a".to_string()))}</div>
                                        <div>{format!("canonical: {}", item.canonical_url.unwrap_or_else(|| "n/a".to_string()))}</div>
                                        <div>{format!("noindex={} | nofollow={}", item.noindex, item.nofollow)}</div>
                                    </div>
                                </div>
                            </div>
                        </li>
                    }
                }).collect_view()}
            </ul>
        </div>
    }
}

#[component]
fn BulkStringPatchField(
    label: String,
    mode_value: Signal<String>,
    value: Signal<String>,
    on_mode: Callback<String>,
    on_value: Callback<String>,
) -> impl IntoView {
    view! {
        <div class="space-y-2 rounded-xl border border-border/80 bg-background/60 p-4">
            <div class="text-sm font-medium text-foreground">{label}</div>
            <select
                class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                prop:value=move || mode_value.get()
                on:change=move |ev| on_mode.run(event_target_value(&ev))
            >
                <option value="keep">"keep"</option>
                <option value="set">"set"</option>
                <option value="clear">"clear"</option>
            </select>
            <input
                class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                prop:value=move || value.get()
                disabled=move || mode_value.get() != "set"
                on:input=move |ev| on_value.run(event_target_value(&ev))
            />
        </div>
    }
}

#[component]
fn BulkBoolPatchField(
    label: String,
    mode_value: Signal<String>,
    checked: Signal<bool>,
    on_mode: Callback<String>,
    on_checked: Callback<bool>,
) -> impl IntoView {
    view! {
        <div class="space-y-2 rounded-xl border border-border/80 bg-background/60 p-4">
            <div class="text-sm font-medium text-foreground">{label}</div>
            <select
                class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                prop:value=move || mode_value.get()
                on:change=move |ev| on_mode.run(event_target_value(&ev))
            >
                <option value="keep">"keep"</option>
                <option value="set">"set"</option>
                <option value="clear">"clear"</option>
            </select>
            <label class="flex items-center justify-between rounded-lg border border-border px-3 py-2 text-sm">
                <span>"Value"</span>
                <input
                    type="checkbox"
                    class="h-4 w-4 accent-primary"
                    prop:checked=move || checked.get()
                    disabled=move || mode_value.get() != "set"
                    on:change=move |ev| on_checked.run(event_target_checked(&ev))
                />
            </label>
        </div>
    }
}

#[component]
fn BulkJsonPatchField(
    label: String,
    mode_value: Signal<String>,
    value: Signal<String>,
    on_mode: Callback<String>,
    on_value: Callback<String>,
) -> impl IntoView {
    view! {
        <div class="space-y-2 rounded-xl border border-border/80 bg-background/60 p-4">
            <div class="text-sm font-medium text-foreground">{label}</div>
            <select
                class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                prop:value=move || mode_value.get()
                on:change=move |ev| on_mode.run(event_target_value(&ev))
            >
                <option value="keep">"keep"</option>
                <option value="set">"set"</option>
                <option value="clear">"clear"</option>
            </select>
            <textarea
                class="min-h-28 w-full rounded-lg border border-input bg-background px-3 py-2 font-mono text-xs"
                prop:value=move || value.get()
                disabled=move || mode_value.get() != "set"
                on:input=move |ev| on_value.run(event_target_value(&ev))
            />
        </div>
    }
}

fn render_bulk_job(job: SeoBulkJobRecord) -> impl IntoView {
    let created_at = job.created_at.to_rfc3339();
    let started_at = job
        .started_at
        .map(|value| value.to_rfc3339())
        .unwrap_or_else(|| "n/a".to_string());
    let completed_at = job
        .completed_at
        .map(|value| value.to_rfc3339())
        .unwrap_or_else(|| "n/a".to_string());
    let last_error = job.last_error.clone();
    let error_view = if let Some(message) = last_error.clone() {
        view! { <p class="text-sm text-destructive">{message}</p> }.into_any()
    } else {
        view! { <></> }.into_any()
    };
    let artifacts = job.artifacts.clone();
    let artifacts_view = if artifacts.is_empty() {
        view! { <></> }.into_any()
    } else {
        view! {
            <div class="flex flex-wrap gap-2">
                {artifacts.into_iter().map(|artifact| {
                    let path = api::bulk_artifact_download_path(
                        job.id.to_string().as_str(),
                        artifact.id.to_string().as_str(),
                    );
                    view! {
                        <a
                            class="rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent"
                            href=path
                        >
                            {artifact.file_name}
                        </a>
                    }
                }).collect_view()}
            </div>
        }.into_any()
    };

    view! {
        <li class="space-y-3 rounded-xl border border-border/80 bg-background/60 px-4 py-3">
            <div class="flex flex-wrap items-center gap-2">
                <span class="font-medium text-foreground">{job.operation_kind.as_str().to_string()}</span>
                <span class="rounded-full bg-secondary px-2 py-0.5 text-xs font-medium text-secondary-foreground">
                    {job.status.as_str().to_string()}
                </span>
                <span class="text-xs text-muted-foreground">{format!("{} / {}", job.target_kind.as_str(), job.locale)}</span>
            </div>
            <div class="grid gap-2 text-sm text-foreground md:grid-cols-2">
                <div>{format!("matched={} | processed={}", job.matched_count, job.processed_count)}</div>
                <div>{format!("ok={} | failed={} | artifacts={}", job.succeeded_count, job.failed_count, job.artifact_count)}</div>
                <div>{format!("created: {created_at}")}</div>
                <div>{format!("started: {started_at}")}</div>
                <div>{format!("completed: {completed_at}")}</div>
                <div>{format!("publish_after_write={}", job.publish_after_write)}</div>
            </div>
            {error_view}
            {artifacts_view}
        </li>
    }
}
