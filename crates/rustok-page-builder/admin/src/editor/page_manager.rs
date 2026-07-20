use crate::editor::{AdminEditorRuntime, CapabilityFieldset};
use crate::i18n::t;
use fly::{EditorCommand, PageCommand, PageMetadata, PagePatch, blank_page, normalize_slug};
use fly_ui::{EditorCapability, UiIntent};
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;
use serde_json::{Map, Value};
use std::collections::BTreeSet;

#[component]
pub fn PageManagerPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let title = t(locale.as_deref(), "page_builder.panel.pages", "Pages");
    let add_label = t(locale.as_deref(), "page_builder.action.addPage", "Add page");
    let apply_label = t(locale.as_deref(), "page_builder.action.apply", "Apply");
    let move_up_label = t(locale.as_deref(), "page_builder.action.moveUp", "Move up");
    let move_down_label = t(
        locale.as_deref(),
        "page_builder.action.moveDown",
        "Move down",
    );
    let remove_label = t(locale.as_deref(), "page_builder.action.remove", "Remove");
    let name_label = t(
        locale.as_deref(),
        "page_builder.field.pageName",
        "Page name",
    );
    let id_label = t(locale.as_deref(), "page_builder.field.pageId", "Page id");
    let seo_title_label = t(
        locale.as_deref(),
        "page_builder.field.seoTitle",
        "SEO title",
    );
    let description_label = t(
        locale.as_deref(),
        "page_builder.field.seoDescription",
        "SEO description",
    );
    let slug_label = t(locale.as_deref(), "page_builder.field.slug", "Slug");
    let canonical_label = t(
        locale.as_deref(),
        "page_builder.field.canonicalUrl",
        "Canonical URL",
    );
    let og_title_label = t(
        locale.as_deref(),
        "page_builder.field.openGraphTitle",
        "Open Graph title",
    );
    let og_description_label = t(
        locale.as_deref(),
        "page_builder.field.openGraphDescription",
        "Open Graph description",
    );
    let og_image_label = t(
        locale.as_deref(),
        "page_builder.field.openGraphImage",
        "Open Graph image",
    );
    let no_index_label = t(
        locale.as_deref(),
        "page_builder.field.noIndex",
        "Prevent search indexing",
    );

    let new_page_name = RwSignal::new(String::new());
    let page_name = RwSignal::new(String::new());
    let page_id = RwSignal::new(String::new());
    let seo_title = RwSignal::new(String::new());
    let seo_description = RwSignal::new(String::new());
    let slug = RwSignal::new(String::new());
    let canonical_url = RwSignal::new(String::new());
    let open_graph_title = RwSignal::new(String::new());
    let open_graph_description = RwSignal::new(String::new());
    let open_graph_image = RwSignal::new(String::new());
    let no_index = RwSignal::new(false);
    let observed_page = RwSignal::new(None::<String>);

    Effect::new({
        let runtime = runtime.clone();
        move |_| {
            let key = runtime.controller.with(|controller| {
                controller.active_page_summary().map(|summary| {
                    format!(
                        "{}:{}:{}",
                        summary.index,
                        summary.id.clone().unwrap_or_default(),
                        controller.editor().revision().project_hash.hex(),
                    )
                })
            });
            if observed_page.get_untracked() == key {
                return;
            }
            observed_page.set(key);
            let snapshot = runtime.controller.with(|controller| {
                let summary = controller.active_page_summary()?;
                let page = controller
                    .editor()
                    .document()
                    .project
                    .pages
                    .get(summary.index)?;
                Some((summary, PageMetadata::from_page(page)))
            });
            if let Some((summary, metadata)) = snapshot {
                page_name.set(summary.name);
                page_id.set(summary.id.unwrap_or_default());
                seo_title.set(metadata.title.unwrap_or_default());
                seo_description.set(metadata.description.unwrap_or_default());
                slug.set(metadata.slug.unwrap_or_default());
                canonical_url.set(metadata.canonical_url.unwrap_or_default());
                open_graph_title.set(metadata.open_graph_title.unwrap_or_default());
                open_graph_description.set(metadata.open_graph_description.unwrap_or_default());
                open_graph_image.set(metadata.open_graph_image.unwrap_or_default());
                no_index.set(metadata.no_index);
            }
        }
    });

    let list_runtime = runtime.clone();
    let edit_gate_runtime = runtime.clone();
    let properties_gate_runtime = runtime.clone();
    let add_runtime = runtime.clone();
    let identity_runtime = runtime.clone();
    let metadata_runtime = runtime.clone();
    let move_up_disabled_runtime = runtime.clone();
    let move_up_action_runtime = runtime.clone();
    let move_down_disabled_runtime = runtime.clone();
    let move_down_action_runtime = runtime.clone();
    let remove_disabled_runtime = runtime.clone();
    let remove_action_runtime = runtime;

    view! {
        <section class="space-y-3 rounded-xl border border-border bg-card p-3">
            <h2 class="font-semibold">{title}</h2>
            <div class="space-y-1">
                {move || {
                    let active = list_runtime.controller.with(|controller| controller.active_page_index());
                    list_runtime.controller.with(|controller| controller.page_summaries()).into_iter().map(|page| {
                        let runtime = list_runtime.clone();
                        let page_id = page.id.clone();
                        let page_index = page.index;
                        let active = active == page_index;
                        view! {
                            <button
                                type="button"
                                class=if active {
                                    "block w-full rounded bg-primary/10 px-2 py-2 text-left text-sm text-primary"
                                } else {
                                    "block w-full rounded px-2 py-2 text-left text-sm hover:bg-muted"
                                }
                                on:click=move |_| runtime.dispatch(UiIntent::ActivatePage {
                                    page_id: page_id.clone(),
                                    page_index,
                                })
                            >
                                <span class="font-medium">{page.name}</span>
                                <span class="ml-2 text-xs text-muted-foreground">{format!("{} nodes", page.component_count)}</span>
                            </button>
                        }
                    }).collect_view()
                }}
            </div>

            <CapabilityFieldset runtime=edit_gate_runtime capability=EditorCapability::Edit>
                <div class="flex gap-2 border-t border-border pt-3">
                    <input
                        class="min-w-0 flex-1 rounded border border-input bg-background px-2 py-1 text-sm"
                        placeholder=name_label.clone()
                        prop:value=move || new_page_name.get()
                        on:input=move |event| new_page_name.set(event_target_value(&event))
                    />
                    <button
                        type="button"
                        class="rounded border border-border px-2 py-1 text-xs"
                        on:click=move |_| {
                            let name = new_page_name.get_untracked().trim().to_string();
                            let name = if name.is_empty() { "Untitled page".to_string() } else { name };
                            let id = unique_page_id(&add_runtime, &name);
                            let index = add_runtime.controller.with(|controller| controller.page_summaries().len());
                            add_runtime.dispatch(UiIntent::execute(EditorCommand::Page {
                                command: PageCommand::Add {
                                    index,
                                    page: Box::new(blank_page(id.clone(), name)),
                                },
                            }));
                            if add_runtime.last_error.get_untracked().is_none() {
                                add_runtime.dispatch(UiIntent::ActivatePage {
                                    page_id: Some(id),
                                    page_index: index,
                                });
                                new_page_name.set(String::new());
                            }
                        }
                    >{add_label}</button>
                </div>

                <CapabilityFieldset
                    runtime=properties_gate_runtime
                    capability=EditorCapability::Properties
                >
                    <div class="grid gap-2 border-t border-border pt-3">
                        <label class="text-sm font-medium">{name_label.clone()}</label>
                        <input
                            class="rounded border border-input bg-background px-2 py-1 text-sm"
                            prop:value=move || page_name.get()
                            on:input=move |event| page_name.set(event_target_value(&event))
                        />
                        <label class="text-sm font-medium">{id_label}</label>
                        <input
                            class="rounded border border-input bg-background px-2 py-1 text-sm"
                            prop:value=move || page_id.get()
                            on:input=move |event| page_id.set(event_target_value(&event))
                        />
                        <button
                            type="button"
                            class="w-fit rounded border border-border px-2 py-1 text-xs"
                            on:click=move |_| {
                                let id = page_id.get_untracked().trim().to_string();
                                if id.is_empty() {
                                    identity_runtime.fail("page id must not be empty");
                                    return;
                                }
                                identity_runtime.dispatch(UiIntent::execute(EditorCommand::Page {
                                    command: PageCommand::Patch {
                                        locator: identity_runtime.controller.with(|controller| controller.active_page_locator()),
                                        patch: PagePatch {
                                            fields: Map::from_iter([
                                                ("id".to_string(), Value::String(id)),
                                                ("name".to_string(), Value::String(page_name.get_untracked().trim().to_string())),
                                            ]),
                                            ..PagePatch::default()
                                        },
                                    },
                                }));
                            }
                        >{apply_label.clone()}</button>
                    </div>

                    <details class="border-t border-border pt-3" open>
                        <summary class="cursor-pointer text-sm font-medium">"SEO / Open Graph"</summary>
                        <div class="mt-2 grid gap-2">
                            <MetadataInput label=seo_title_label signal=seo_title />
                            <MetadataTextarea label=description_label signal=seo_description />
                            <MetadataInput label=slug_label signal=slug />
                            <MetadataInput label=canonical_label signal=canonical_url />
                            <MetadataInput label=og_title_label signal=open_graph_title />
                            <MetadataTextarea label=og_description_label signal=open_graph_description />
                            <MetadataInput label=og_image_label signal=open_graph_image />
                            <label class="flex items-center gap-2 text-sm">
                                <input
                                    type="checkbox"
                                    prop:checked=move || no_index.get()
                                    on:change=move |event| no_index.set(event_target_checked(&event))
                                />
                                <span>{no_index_label}</span>
                            </label>
                            <button
                                type="button"
                                class="w-fit rounded border border-border px-2 py-1 text-xs"
                                on:click=move |_| {
                                    let mut metadata = current_metadata(&metadata_runtime);
                                    metadata.title = Some(seo_title.get_untracked());
                                    metadata.description = Some(seo_description.get_untracked());
                                    metadata.slug = Some(slug.get_untracked());
                                    metadata.canonical_url = Some(canonical_url.get_untracked());
                                    metadata.open_graph_title = Some(open_graph_title.get_untracked());
                                    metadata.open_graph_description = Some(open_graph_description.get_untracked());
                                    metadata.open_graph_image = Some(open_graph_image.get_untracked());
                                    metadata.no_index = no_index.get_untracked();
                                    metadata_runtime.dispatch(UiIntent::execute(EditorCommand::Page {
                                        command: PageCommand::Patch {
                                            locator: metadata_runtime.controller.with(|controller| controller.active_page_locator()),
                                            patch: metadata.normalized().into_page_patch(),
                                        },
                                    }));
                                }
                            >{apply_label}</button>
                        </div>
                    </details>
                </CapabilityFieldset>

                <div class="flex flex-wrap gap-2 border-t border-border pt-3">
                    <button
                        type="button"
                        class="rounded border border-border px-2 py-1 text-xs"
                        disabled=move || move_up_disabled_runtime.controller.with(|controller| controller.active_page_index() == 0)
                        on:click=move |_| {
                            let index = move_up_action_runtime.controller.with(|controller| controller.active_page_index());
                            if index == 0 {
                                return;
                            }
                            move_up_action_runtime.dispatch(UiIntent::execute(EditorCommand::Page {
                                command: PageCommand::Move {
                                    locator: move_up_action_runtime.controller.with(|controller| controller.active_page_locator()),
                                    index: index - 1,
                                },
                            }));
                        }
                    >{move_up_label}</button>
                    <button
                        type="button"
                        class="rounded border border-border px-2 py-1 text-xs"
                        disabled=move || move_down_disabled_runtime.controller.with(|controller| {
                            controller.active_page_index() + 1 >= controller.page_summaries().len()
                        })
                        on:click=move |_| {
                            let (index, len) = move_down_action_runtime.controller.with(|controller| {
                                (controller.active_page_index(), controller.page_summaries().len())
                            });
                            if index + 1 >= len {
                                return;
                            }
                            move_down_action_runtime.dispatch(UiIntent::execute(EditorCommand::Page {
                                command: PageCommand::Move {
                                    locator: move_down_action_runtime.controller.with(|controller| controller.active_page_locator()),
                                    index: index + 2,
                                },
                            }));
                        }
                    >{move_down_label}</button>
                    <button
                        type="button"
                        class="rounded border border-destructive/40 px-2 py-1 text-xs text-destructive"
                        disabled=move || remove_disabled_runtime.controller.with(|controller| controller.page_summaries().len() <= 1)
                        on:click=move |_| remove_action_runtime.dispatch(UiIntent::execute(EditorCommand::Page {
                            command: PageCommand::Remove {
                                locator: remove_action_runtime.controller.with(|controller| controller.active_page_locator()),
                            },
                        }))
                    >{remove_label}</button>
                </div>
            </CapabilityFieldset>
        </section>
    }
}

#[component]
fn MetadataInput(label: String, signal: RwSignal<String>) -> impl IntoView {
    view! {
        <label class="grid gap-1 text-sm">
            <span class="font-medium">{label}</span>
            <input
                class="rounded border border-input bg-background px-2 py-1"
                prop:value=move || signal.get()
                on:input=move |event| signal.set(event_target_value(&event))
            />
        </label>
    }
}

#[component]
fn MetadataTextarea(label: String, signal: RwSignal<String>) -> impl IntoView {
    view! {
        <label class="grid gap-1 text-sm">
            <span class="font-medium">{label}</span>
            <textarea
                class="min-h-20 rounded border border-input bg-background px-2 py-1"
                prop:value=move || signal.get()
                on:input=move |event| signal.set(event_target_value(&event))
            ></textarea>
        </label>
    }
}

fn current_metadata(runtime: &AdminEditorRuntime) -> PageMetadata {
    runtime.controller.with(|controller| {
        controller
            .editor()
            .document()
            .project
            .pages
            .get(controller.active_page_index())
            .map(PageMetadata::from_page)
            .unwrap_or_default()
    })
}

fn unique_page_id(runtime: &AdminEditorRuntime, name: &str) -> String {
    let existing = runtime.controller.with(|controller| {
        controller
            .page_summaries()
            .into_iter()
            .filter_map(|page| page.id)
            .collect::<BTreeSet<_>>()
    });
    let base = {
        let normalized = normalize_slug(name.to_string());
        if normalized.is_empty() {
            "page".to_string()
        } else {
            normalized
        }
    };
    if !existing.contains(&base) {
        return base;
    }
    for suffix in 2..=10_000 {
        let candidate = format!("{base}-{suffix}");
        if !existing.contains(&candidate) {
            return candidate;
        }
    }
    format!("{base}-{}", existing.len().saturating_add(1))
}
