//! Site symbol authoring controls for the browser editor.
//!
//! The panel only emits Fly commands. The Pages host owns the shared catalog
//! and its save-time optimistic revision; no transport or auth lives here.

use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use fly::{
    EditorCommand, SymbolCommand, SymbolDescriptor, convert_component_to_symbol,
    insert_symbol_instance,
};
use fly_ui::{EditorCapability, UiIntent};
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

#[component]
pub(crate) fn SymbolSection(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let title = t(
        locale.as_deref(),
        "page_builder.panel.symbols",
        "Site symbols",
    );
    let id_label = t(locale.as_deref(), "page_builder.symbols.id", "Symbol id");
    let name_label = t(locale.as_deref(), "page_builder.symbols.name", "Name");
    let convert_label = t(
        locale.as_deref(),
        "page_builder.symbols.convert",
        "Convert selection",
    );
    let insert_label = t(
        locale.as_deref(),
        "page_builder.symbols.insert",
        "Insert instance",
    );
    let remove_label = t(
        locale.as_deref(),
        "page_builder.symbols.remove",
        "Remove definition",
    );
    let explainer = t(
        locale.as_deref(),
        "page_builder.symbols.explainer",
        "Convert a selected section into a shared symbol. After editing a definition, re-publish each page that uses it.",
    );

    let symbol_id = RwSignal::new(String::new());
    let symbol_name = RwSignal::new(String::new());
    let convert_runtime = runtime.clone();
    let convert_gate = runtime.clone();
    let list_runtime = runtime;

    view! {
        <section class="space-y-2 border-t border-border pt-3" data-fly-symbols="true">
            <h2 class="font-semibold">{title}</h2>
            <p class="text-xs text-muted-foreground">{explainer}</p>
            <label class="grid gap-1 text-sm">
                <span class="font-medium">{id_label}</span>
                <input
                    class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                    prop:value=move || symbol_id.get()
                    on:input=move |event| symbol_id.set(event_target_value(&event))
                />
            </label>
            <label class="grid gap-1 text-sm">
                <span class="font-medium">{name_label}</span>
                <input
                    class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                    prop:value=move || symbol_name.get()
                    on:input=move |event| symbol_name.set(event_target_value(&event))
                />
            </label>
            <button
                type="button"
                class="rounded border border-border px-2 py-1 text-xs disabled:opacity-50"
                aria-label=convert_label.clone()
                disabled=move || !convert_gate.capability_enabled(EditorCapability::Edit)
                on:click=move |_| {
                    let id = symbol_id.get_untracked().trim().to_string();
                    let name = symbol_name.get_untracked().trim().to_string();
                    let result = convert_runtime.controller.with(|controller| {
                        let editor = controller.editor();
                        let selected = editor.selection().ok_or_else(|| {
                            "Select a non-root section before converting it to a symbol".to_string()
                        })?;
                        convert_component_to_symbol(
                            editor.document(), selected, &id,
                            (!name.is_empty()).then_some(name.clone()),
                        )
                        .map(UiIntent::execute)
                        .map_err(|error| error.to_string())
                    });
                    convert_runtime.dispatch_result(result);
                }
            >{convert_label}</button>
            <div class="space-y-1">
                {move || {
                    let definitions = list_runtime.controller.with(|controller| {
                        SymbolDescriptor::catalog_from_document(controller.editor().document())
                    });
                    definitions.into_values().map(|definition| {
                        let id = definition.id.clone();
                        let display = definition.name.clone().unwrap_or_else(|| id.clone());
                        let insert_id = id.clone();
                        let remove_id = id.clone();
                        let insert_runtime = list_runtime.clone();
                        let insert_gate = list_runtime.clone();
                        let remove_runtime = list_runtime.clone();
                        let remove_gate = list_runtime.clone();
                        let insert_accessible_label = format!("{insert_label}: {display}");
                        let remove_accessible_label = format!("{remove_label}: {display}");
                        view! {
                            <div class="rounded border border-border p-2 text-xs" data-fly-symbol-id=id>
                                <div class="font-medium">{display}</div>
                                <div class="mt-2 flex gap-2">
                                    <button
                                        type="button"
                                        class="rounded border border-border px-2 py-1 disabled:opacity-50"
                                        aria-label=insert_accessible_label
                                        disabled=move || !insert_gate.capability_enabled(EditorCapability::Edit)
                                        on:click=move |_| {
                                            let result = insert_runtime.controller.with(|controller| {
                                                insert_symbol_instance(controller.editor().document(), &insert_id)
                                                    .map(UiIntent::execute)
                                                    .map_err(|error| error.to_string())
                                            });
                                            insert_runtime.dispatch_result(result);
                                        }
                                    >{insert_label.clone()}</button>
                                    <button
                                        type="button"
                                        class="rounded border border-border px-2 py-1 disabled:opacity-50"
                                        aria-label=remove_accessible_label
                                        disabled=move || !remove_gate.capability_enabled(EditorCapability::Edit)
                                        on:click=move |_| {
                                            remove_runtime.dispatch(UiIntent::execute(EditorCommand::Symbol {
                                                command: SymbolCommand::Remove { symbol_id: remove_id.clone() },
                                            }));
                                        }
                                    >{remove_label.clone()}</button>
                                </div>
                            </div>
                        }
                    }).collect_view()
                }}
            </div>
        </section>
    }
}
