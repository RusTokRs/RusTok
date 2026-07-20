use crate::editor::{AdminEditorRuntime, dispatch_shortcut};
use crate::i18n::t;
use fly_ui::{
    CapabilityState, EditorShortcut, UiIntent, builtin_viewport_presets, viewport_preset,
};
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

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
    let device = t(locale.as_deref(), "page_builder.field.device", "Device");

    install_keyboard_bindings(runtime.clone());
    let presets = builtin_viewport_presets();
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
                        !controller.ui().state.capabilities.publish
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
                    {presets.into_iter().map(|preset| view! {
                        <option value=preset.id>{preset.label}</option>
                    }).collect_view()}
                </select>
            </label>

            <span class="text-sm text-muted-foreground" aria-live="polite">
                {move || status_runtime.controller.with(|controller| {
                    if controller.ui().state.dirty.save_in_progress {
                        saving.clone()
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
                    .is_none_or(|selected| reject_root && selected.is_root)
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
                let Some(shortcut) = fly_leptos::shortcut_from_event(event) else {
                    return;
                };
                fly_leptos::prevent_editor_shortcut_default(event, shortcut);
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
