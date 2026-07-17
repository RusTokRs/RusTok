use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use fly::{AssetCatalog, AssetCommand, EditorCommand};
use fly_ui::UiIntent;
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;
use serde_json::json;

#[component]
pub(crate) fn AssetSection(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let assets_label = t(locale.as_deref(), "page_builder.panel.assets", "Assets");
    let add_label = t(locale.as_deref(), "page_builder.action.add", "Add");
    let remove_label = t(locale.as_deref(), "page_builder.action.remove", "Remove");
    let select_label = t(locale.as_deref(), "page_builder.action.select", "Use");
    let asset_id_label = t(locale.as_deref(), "page_builder.field.assetId", "Asset id");
    let asset_url_label = t(
        locale.as_deref(),
        "page_builder.field.assetUrl",
        "Asset URL",
    );
    let asset_id = RwSignal::new(String::new());
    let asset_url = RwSignal::new(String::new());
    let add_runtime = runtime.clone();
    let list_runtime = runtime;

    view! {
        <section class="space-y-2 border-t border-border pt-3">
            <h2 class="font-semibold">{assets_label}</h2>
            <input
                placeholder=asset_id_label
                class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                prop:value=move || asset_id.get()
                on:input=move |event| asset_id.set(event_target_value(&event))
            />
            <input
                placeholder=asset_url_label
                class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                prop:value=move || asset_url.get()
                on:input=move |event| asset_url.set(event_target_value(&event))
            />
            <button
                type="button"
                class="rounded border border-border px-2 py-1 text-xs"
                on:click=move |_| {
                    let source = asset_url.get_untracked().trim().to_string();
                    if source.is_empty() {
                        add_runtime.fail("asset URL must not be empty");
                        return;
                    }
                    let id = asset_id.get_untracked().trim().to_string();
                    let asset = if id.is_empty() {
                        json!({ "src": source })
                    } else {
                        json!({ "id": id, "src": source })
                    };
                    add_runtime.dispatch(UiIntent::execute(EditorCommand::Asset {
                        command: AssetCommand::Upsert { asset },
                    }));
                }
            >{add_label}</button>

            <div class="space-y-1">
                {move || {
                    let catalog = list_runtime.controller.with(|controller| {
                        AssetCatalog::from_document(controller.editor().document())
                    });
                    catalog.assets.into_iter().map(|asset| {
                        let use_runtime = list_runtime.clone();
                        let remove_runtime = list_runtime.clone();
                        let use_id = asset.id.clone();
                        let remove_id = asset.id.clone();
                        let select_label = select_label.clone();
                        let remove_label = remove_label.clone();
                        view! {
                            <div class="rounded border border-border p-2 text-xs">
                                <div class="font-medium">{asset.name.clone().unwrap_or_else(|| asset.id.clone())}</div>
                                <div class="break-all text-muted-foreground">{asset.source}</div>
                                <div class="mt-2 flex gap-2">
                                    <button
                                        type="button"
                                        class="rounded border border-border px-2 py-1"
                                        on:click=move |_| {
                                            let intent = use_runtime.controller.with(|controller| {
                                                controller.apply_asset_to_selected_intent(&use_id, "src")
                                            });
                                            use_runtime.dispatch_result(intent);
                                        }
                                    >{select_label}</button>
                                    <button
                                        type="button"
                                        class="rounded border border-destructive/40 px-2 py-1 text-destructive"
                                        on:click=move |_| remove_runtime.dispatch(UiIntent::execute(
                                            EditorCommand::Asset {
                                                command: AssetCommand::Remove {
                                                    asset_id: remove_id.clone(),
                                                },
                                            },
                                        ))
                                    >{remove_label}</button>
                                </div>
                            </div>
                        }
                    }).collect_view()
                }}
            </div>
        </section>
    }
}
