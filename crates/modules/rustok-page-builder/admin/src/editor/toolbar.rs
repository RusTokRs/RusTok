use crate::editor::{AdminEditorRuntime, dispatch_shortcut};
use crate::i18n::t;
use fly_ui::{
    CapabilityState, EditorShortcut, UiIntent, ViewportState, builtin_viewport_presets,
    custom_viewport, viewport_preset,
};
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;
use std::collections::BTreeMap;

/// Route query key that names the previewed viewport preset.
///
/// The previewed viewport is view state, not project state. In the default `ssr + browser-js`
/// runtime the canvas frame is rendered on the server, so the choice has to reach the server
/// with the page request: the browser adapter writes the selected preset id under this key and
/// navigates, and [`route_viewport`] resolves it before the canvas is rendered. Keeping it in
/// the route rather than in the draft session makes it work for a read-only caller, who has no
/// session, and lets it survive the reload that follows every mutating intent.
pub(crate) const VIEWPORT_ROUTE_QUERY_KEY: &str = "fly_viewport";

/// Viewport requested by the route, when it names a built-in preset.
///
/// An unknown or empty value yields `None`: a stale or hand-edited route renders the current
/// viewport instead of failing the page.
pub(crate) fn route_viewport(
    query: &BTreeMap<String, String>,
    current: ViewportState,
) -> Option<ViewportState> {
    let preset = viewport_preset(query.get(VIEWPORT_ROUTE_QUERY_KEY)?.trim())?;
    Some(preset.apply(current))
}

/// Id of the built-in preset whose frame is the given viewport, if any.
fn active_viewport_preset_id(viewport: ViewportState) -> Option<String> {
    builtin_viewport_presets()
        .into_iter()
        .find(|preset| preset.width == viewport.width && preset.height == viewport.height)
        .map(|preset| preset.id)
}

#[component]
pub fn AuthoringToolbar(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let undo = t(locale.as_deref(), "page_builder.action.undo", "Undo");
    let redo = t(locale.as_deref(), "page_builder.action.redo", "Redo");
    let save = t(locale.as_deref(), "page_builder.action.save", "Save");
    let copy = t(locale.as_deref(), "page_builder.action.copy", "Copy");
    let cut = t(locale.as_deref(), "page_builder.action.cut", "Cut");
    let paste = t(locale.as_deref(), "page_builder.action.paste", "Paste");
    let duplicate = t(
        locale.as_deref(),
        "page_builder.action.duplicate",
        "Duplicate",
    );
    let remove = t(
        locale.as_deref(),
        "page_builder.action.remove",
        "Remove selected",
    );
    let move_up = t(locale.as_deref(), "page_builder.action.moveUp", "Move up");
    let move_down = t(
        locale.as_deref(),
        "page_builder.action.moveDown",
        "Move down",
    );
    let move_mode = t(
        locale.as_deref(),
        "page_builder.action.move",
        "Move selected",
    );
    let cancel_drag = t(
        locale.as_deref(),
        "page_builder.action.cancelDrag",
        "Cancel drag",
    );
    let saving = t(locale.as_deref(), "page_builder.status.saving", "Saving");
    let failed = t(
        locale.as_deref(),
        "page_builder.status.saveFailed",
        "Save failed",
    );
    let dirty = t(
        locale.as_deref(),
        "page_builder.status.dirty",
        "Unsaved changes",
    );
    let saved = t(locale.as_deref(), "page_builder.status.saved", "Saved");
    let conflict = t(
        locale.as_deref(),
        "page_builder.status.saveConflict",
        "Save conflict: changed in another session. Reload before saving.",
    );
    let device = t(locale.as_deref(), "page_builder.field.device", "Device");

    install_keyboard_bindings(runtime.clone());
    let presets = builtin_viewport_presets();
    // The select states the viewport the canvas is rendered with. A frame that is no built-in
    // preset (the initial 1280 × 720 one, or a custom size) gets its own leading option, so the
    // control never names a preset that is not in effect.
    let current_viewport = runtime
        .controller
        .with_untracked(|controller| controller.ui().state.viewport);
    let active_preset_id = active_viewport_preset_id(current_viewport);
    let unlisted_viewport_label = active_preset_id.is_none().then(|| {
        custom_viewport(
            current_viewport.width,
            current_viewport.height,
            current_viewport.zoom,
        )
        .label
    });
    let status_runtime = runtime.clone();

    view! {
        <div class="flex flex-wrap items-center gap-2 rounded-xl border border-border bg-card p-3" role="toolbar" aria-label="Page builder actions">
            <ToolbarButton
                label=undo
                ssr_intent="undo"
                disabled=Signal::derive({
                    let runtime = runtime.clone();
                    move || runtime.controller.with(|controller| {
                        !controller.ui().state.capabilities.history || !controller.can_undo()
                    })
                })
                on_click=Callback::new({
                    let runtime = runtime.clone();
                    move |_| runtime.dispatch(UiIntent::Undo)
                })
            />
            <ToolbarButton
                label=redo
                ssr_intent="redo"
                disabled=Signal::derive({
                    let runtime = runtime.clone();
                    move || runtime.controller.with(|controller| {
                        !controller.ui().state.capabilities.history || !controller.can_redo()
                    })
                })
                on_click=Callback::new({
                    let runtime = runtime.clone();
                    move |_| runtime.dispatch(UiIntent::Redo)
                })
            />
            <ToolbarButton
                label=save
                ssr_intent="save"
                primary=true
                disabled=Signal::derive({
                    let runtime = runtime.clone();
                    move || runtime.controller.with(|controller| {
                        // Saving the working draft is an authoring action
                        // gated by the edit capability; the publish
                        // capability is reserved for the separate
                        // publication flow.
                        !controller.ui().state.capabilities.edit
                            || controller.ui().state.has_blocking_diagnostics()
                            || !controller.ui().state.dirty.dirty
                            || controller.ui().state.dirty.save_in_progress
                    })
                })
                on_click=Callback::new({
                    let runtime = runtime.clone();
                    move |_| runtime.dispatch(UiIntent::RequestSave)
                })
            />
            <span class="mx-1 h-6 w-px bg-border"></span>
            <ToolbarButton
                label=copy
                ssr_intent="copy"
                disabled=selection_disabled(runtime.clone(), true, |capabilities| capabilities.clipboard)
                on_click=shortcut_callback(runtime.clone(), EditorShortcut::Copy)
            />
            <ToolbarButton
                label=cut
                ssr_intent="cut"
                disabled=selection_disabled(runtime.clone(), true, |capabilities| capabilities.clipboard)
                on_click=shortcut_callback(runtime.clone(), EditorShortcut::Cut)
            />
            <ToolbarButton
                label=paste
                ssr_intent="paste"
                disabled=Signal::derive({
                    let runtime = runtime.clone();
                    move || runtime.controller.with(|controller| {
                        !controller.ui().state.capabilities.clipboard || !controller.has_clipboard()
                    })
                })
                on_click=shortcut_callback(runtime.clone(), EditorShortcut::Paste)
            />
            <ToolbarButton
                label=duplicate
                ssr_intent="duplicate"
                disabled=selection_disabled(runtime.clone(), true, |capabilities| capabilities.clipboard)
                on_click=shortcut_callback(runtime.clone(), EditorShortcut::Duplicate)
            />
            <ToolbarButton
                label=remove
                ssr_intent="remove_selected"
                destructive=true
                disabled=selection_disabled(runtime.clone(), true, |capabilities| capabilities.edit)
                on_click=shortcut_callback(runtime.clone(), EditorShortcut::DeleteSelection)
            />
            <span class="mx-1 h-6 w-px bg-border"></span>
            <ToolbarButton
                label=move_up
                ssr_intent="move_selected_up"
                disabled=selection_disabled(runtime.clone(), true, |capabilities| capabilities.edit)
                on_click=shortcut_callback(runtime.clone(), EditorShortcut::MoveSelectionUp)
            />
            <ToolbarButton
                label=move_down
                ssr_intent="move_selected_down"
                disabled=selection_disabled(runtime.clone(), true, |capabilities| capabilities.edit)
                on_click=shortcut_callback(runtime.clone(), EditorShortcut::MoveSelectionDown)
            />
            <ToolbarButton
                label=move_mode
                ssr_intent="begin_selected_move"
                disabled=selection_disabled(runtime.clone(), true, |capabilities| capabilities.drag_drop)
                on_click=Callback::new({
                    let runtime = runtime.clone();
                    move |_| {
                        let intent = runtime
                            .controller
                            .with(|controller| controller.begin_selected_move_intent());
                        runtime.dispatch_result(intent);
                    }
                })
            />
            <ToolbarButton
                label=cancel_drag
                ssr_intent="cancel_drag"
                disabled=Signal::derive({
                    let runtime = runtime.clone();
                    move || runtime.controller.with(|controller| controller.ui().state.drag.is_none())
                })
                on_click=shortcut_callback(runtime.clone(), EditorShortcut::Cancel)
            />

            <label class="ml-auto flex items-center gap-2 text-sm">
                <span class="text-muted-foreground">{device}</span>
                <select
                    class="rounded border border-input bg-background px-2 py-1"
                    data-fly-action="set-viewport"
                    data-fly-route-query=VIEWPORT_ROUTE_QUERY_KEY
                    on:change={
                        let runtime = runtime.clone();
                        move |event| {
                            let id = event_target_value(&event);
                            if let Some(preset) = viewport_preset(&id) {
                                let viewport = runtime.controller.with(|controller| {
                                    preset.apply(controller.ui().state.viewport)
                                });
                                runtime.dispatch(UiIntent::SetViewport(viewport));
                            }
                        }
                    }
                >
                    {unlisted_viewport_label.map(|label| view! {
                        <option value="" selected=true>{label}</option>
                    })}
                    {presets.into_iter().map(|preset| {
                        let selected = active_preset_id.as_deref() == Some(preset.id.as_str());
                        view! {
                            <option value=preset.id selected=selected>{preset.label}</option>
                        }
                    }).collect_view()}
                </select>
            </label>

            <span class="text-sm text-muted-foreground" aria-live="polite">
                {move || status_runtime.controller.with(|controller| {
                    if controller.ui().state.dirty.save_in_progress {
                        saving.clone()
                    } else if controller.ui().state.dirty.save_conflict {
                        conflict.clone()
                    } else if controller.ui().state.dirty.save_failed {
                        failed.clone()
                    } else if controller.ui().state.dirty.dirty {
                        dirty.clone()
                    } else {
                        saved.clone()
                    }
                })}
            </span>
        </div>
    }
}

#[component]
fn ToolbarButton(
    label: String,
    ssr_intent: &'static str,
    disabled: Signal<bool>,
    on_click: Callback<()>,
    #[prop(optional)] primary: bool,
    #[prop(optional)] destructive: bool,
) -> impl IntoView {
    let class = if primary {
        "rounded bg-primary px-3 py-1.5 text-sm text-primary-foreground disabled:opacity-50"
    } else if destructive {
        "rounded border border-destructive/40 px-3 py-1.5 text-sm text-destructive disabled:opacity-50"
    } else {
        "rounded border border-border px-3 py-1.5 text-sm disabled:opacity-50"
    };
    view! {
        <button
            type="button"
            class=class
            data-fly-action=format!("intent:{ssr_intent}")
            disabled=move || disabled.get()
            on:click=move |_| on_click.run(())
        >{label}</button>
    }
}

fn selection_disabled(
    runtime: AdminEditorRuntime,
    reject_root: bool,
    capability: fn(&CapabilityState) -> bool,
) -> Signal<bool> {
    Signal::derive(move || {
        runtime.controller.with(|controller| {
            !capability(&controller.ui().state.capabilities)
                || controller
                    .selected_component_view()
                    .is_none_or(|selected| (reject_root && selected.is_root) || selected.is_locked)
        })
    })
}

fn shortcut_callback(runtime: AdminEditorRuntime, shortcut: EditorShortcut) -> Callback<()> {
    Callback::new(move |_| dispatch_shortcut(&runtime, shortcut))
}

fn install_keyboard_bindings(runtime: AdminEditorRuntime) {
    #[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
    {
        use wasm_bindgen::JsCast;
        use web_sys::{EventTarget, KeyboardEvent};

        let Some(window) = web_sys::window() else {
            return;
        };
        let target: EventTarget = window.unchecked_into();
        let keyboard_runtime = runtime.clone();
        match fly_leptos::EventListenerHandle::new::<KeyboardEvent>(
            &target,
            "keydown",
            move |event| {
                let Some(shortcut) = fly_leptos::shortcut_from_event(&event) else {
                    return;
                };
                fly_leptos::prevent_editor_shortcut_default(&event, shortcut);
                dispatch_shortcut(&keyboard_runtime, shortcut);
            },
        ) {
            Ok(handle) => {
                let _keyboard_handle = StoredValue::new_local(handle);
            }
            Err(error) => runtime.fail(error.to_string()),
        }
    }
    #[cfg(not(all(target_arch = "wasm32", feature = "wasm-client")))]
    {
        let _ = runtime;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(value: &str) -> BTreeMap<String, String> {
        BTreeMap::from([(VIEWPORT_ROUTE_QUERY_KEY.to_string(), value.to_string())])
    }

    #[test]
    fn route_viewport_resolves_a_built_in_preset() {
        let viewport = route_viewport(&query("tablet"), ViewportState::default())
            .expect("tablet is a built-in preset");
        let preset = viewport_preset("tablet").expect("tablet preset");

        assert_eq!(viewport.width, preset.width);
        assert_eq!(viewport.height, preset.height);
        assert_eq!(viewport, preset.apply(ViewportState::default()));
        assert_eq!(
            active_viewport_preset_id(viewport).as_deref(),
            Some("tablet")
        );
    }

    #[test]
    fn route_viewport_ignores_missing_empty_and_unknown_values() {
        let current = ViewportState::default();

        assert_eq!(route_viewport(&BTreeMap::new(), current), None);
        assert_eq!(route_viewport(&query(""), current), None);
        assert_eq!(route_viewport(&query("watch-ultra"), current), None);
        // Another key with a valid preset id is not the viewport key.
        let other = BTreeMap::from([("fly_draft".to_string(), "tablet".to_string())]);
        assert_eq!(route_viewport(&other, current), None);
    }

    #[test]
    fn every_built_in_preset_round_trips_through_the_route() {
        for preset in builtin_viewport_presets() {
            let viewport = route_viewport(&query(&preset.id), ViewportState::default())
                .unwrap_or_else(|| panic!("preset {} must resolve", preset.id));
            assert_eq!(
                active_viewport_preset_id(viewport).as_deref(),
                Some(preset.id.as_str()),
                "preset {} must be reported as the active one",
                preset.id
            );
        }
    }

    #[test]
    fn the_initial_viewport_is_not_reported_as_a_preset() {
        // 1280 × 720 is no built-in preset; the toolbar renders it as its own option instead of
        // presenting the first preset as if it were in effect.
        assert_eq!(active_viewport_preset_id(ViewportState::default()), None);
    }
}
