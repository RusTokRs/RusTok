use crate::editor::AdminEditorRuntime;
use leptos::prelude::*;

#[cfg(target_arch = "wasm32")]
use fly::{EditorCommand};
#[cfg(target_arch = "wasm32")]
use fly_ui::{CanvasRect, ResizeHandle, ResizePolicy, ResizeResult, UiIntent, ViewportState};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use web_sys::{Element, EventTarget, PointerEvent};

#[cfg(target_arch = "wasm32")]
#[component]
pub fn ResizeHandles(runtime: AdminEditorRuntime) -> impl IntoView {
    let session = StoredValue::new_local(None::<fly_leptos::BrowserResizeSession>);
    let captured_element = StoredValue::new_local(None::<Element>);
    let preview = RwSignal::new(None::<ResizeResult>);

    install_resize_listeners(
        runtime.clone(),
        session,
        captured_element,
        preview,
    );

    let handles = [
        ResizeHandle::North,
        ResizeHandle::NorthEast,
        ResizeHandle::East,
        ResizeHandle::SouthEast,
        ResizeHandle::South,
        ResizeHandle::SouthWest,
        ResizeHandle::West,
        ResizeHandle::NorthWest,
    ];
    let frame_runtime = runtime.clone();
    let preview_runtime = runtime.clone();

    view! {
        <div
            class="pointer-events-none absolute inset-0"
            class:hidden=move || frame_runtime.controller.with(|controller| {
                controller
                    .selected_component_view()
                    .is_none_or(|selected| selected.is_root)
                    || controller.ui().state.overlays.selected.is_none()
            })
        >
            <div
                aria-hidden="true"
                class="pointer-events-none absolute border-2 border-violet-500 bg-violet-500/5"
                class:hidden=move || preview.get().is_none()
                style=move || preview_runtime.controller.with(|controller| {
                    preview
                        .get()
                        .map(|result| rect_style(result.rect, controller.ui().state.viewport))
                        .unwrap_or_else(|| "display:none".to_string())
                })
            ></div>
            {handles.into_iter().map(|handle| {
                let runtime = runtime.clone();
                view! {
                    <button
                        type="button"
                        aria-label=format!("Resize {handle:?}")
                        class="pointer-events-auto absolute h-3 w-3 rounded-full border border-violet-700 bg-white shadow"
                        style=move || runtime.controller.with(|controller| {
                            let viewport = controller.ui().state.viewport;
                            let rect = preview
                                .get()
                                .map(|result| result.rect)
                                .or(controller.ui().state.overlays.selected);
                            rect.map(|rect| handle_style(rect, viewport, handle))
                                .unwrap_or_else(|| "display:none".to_string())
                        })
                        on:pointerdown={
                            let runtime = runtime.clone();
                            move |event: PointerEvent| {
                                event.prevent_default();
                                event.stop_propagation();
                                let Some((component_id, rect, zoom)) = runtime.controller.with(|controller| {
                                    let selected = controller.selected_component_view()?;
                                    if selected.is_root {
                                        return None;
                                    }
                                    Some((
                                        selected.id,
                                        controller.ui().state.overlays.selected?,
                                        controller.ui().state.viewport.zoom,
                                    ))
                                }) else {
                                    return;
                                };
                                let resize = fly_leptos::BrowserResizeSession::begin_scaled(
                                    component_id,
                                    handle,
                                    rect,
                                    &event,
                                    ResizePolicy {
                                        grid_size: Some(4.0),
                                        ..ResizePolicy::default()
                                    },
                                    f64::from(zoom),
                                );
                                if let Some(element) = event
                                    .current_target()
                                    .and_then(|target| target.dyn_into::<Element>().ok())
                                {
                                    if let Err(error) = fly_leptos::set_pointer_capture(
                                        &element,
                                        event.pointer_id(),
                                    ) {
                                        runtime.fail(error.to_string());
                                    }
                                    captured_element.set_value(Some(element));
                                }
                                session.set_value(Some(resize));
                                preview.set(None);
                            }
                        }
                    ></button>
                }
            }).collect_view()}
        </div>
    }
}

#[cfg(target_arch = "wasm32")]
fn install_resize_listeners(
    runtime: AdminEditorRuntime,
    session: StoredValue<Option<fly_leptos::BrowserResizeSession>, LocalStorage>,
    captured_element: StoredValue<Option<Element>, LocalStorage>,
    preview: RwSignal<Option<ResizeResult>>,
) {
    let Some(window) = web_sys::window() else {
        runtime.fail("browser window is unavailable for resize handles");
        return;
    };
    let target: EventTarget = window.unchecked_into();

    let move_session = session;
    let move_listener = fly_leptos::EventListenerHandle::new::<PointerEvent>(
        &target,
        "pointermove",
        move |event| {
            if let Some(result) = move_session
                .get_value()
                .and_then(|session| session.update(event))
            {
                preview.set(Some(result));
            }
        },
    );

    let up_runtime = runtime.clone();
    let up_session = session;
    let up_element = captured_element;
    let up_listener = fly_leptos::EventListenerHandle::new::<PointerEvent>(
        &target,
        "pointerup",
        move |event| {
            let Some(active) = up_session.get_value() else {
                return;
            };
            if !active.accepts(event) {
                return;
            }
            let result = active.update(event).or_else(|| preview.get_untracked());
            if let Some(result) = result {
                up_runtime.dispatch(UiIntent::Execute(EditorCommand::Patch {
                    component_id: active.resize.component_id.clone(),
                    patch: active.resize.component_patch(result),
                }));
            }
            if let Some(element) = up_element.get_value() {
                let _ = fly_leptos::release_pointer_capture(&element, event.pointer_id());
            }
            up_element.set_value(None);
            up_session.set_value(None);
            preview.set(None);
        },
    );

    match (move_listener, up_listener) {
        (Ok(move_listener), Ok(up_listener)) => {
            let _listeners = StoredValue::new_local((move_listener, up_listener));
        }
        (Err(error), _) | (_, Err(error)) => runtime.fail(error.to_string()),
    }
}

#[cfg(target_arch = "wasm32")]
fn rect_style(rect: CanvasRect, viewport: ViewportState) -> String {
    let zoom = f64::from(viewport.zoom.max(0.01));
    format!(
        "display:block;left:{}px;top:{}px;width:{}px;height:{}px",
        rect.x * zoom,
        rect.y * zoom,
        rect.width * zoom,
        rect.height * zoom,
    )
}

#[cfg(target_arch = "wasm32")]
fn handle_style(rect: CanvasRect, viewport: ViewportState, handle: ResizeHandle) -> String {
    let zoom = f64::from(viewport.zoom.max(0.01));
    let left = rect.x * zoom;
    let top = rect.y * zoom;
    let width = rect.width * zoom;
    let height = rect.height * zoom;
    let (x, y, cursor) = match handle {
        ResizeHandle::North => (left + width / 2.0, top, "ns-resize"),
        ResizeHandle::NorthEast => (left + width, top, "nesw-resize"),
        ResizeHandle::East => (left + width, top + height / 2.0, "ew-resize"),
        ResizeHandle::SouthEast => (left + width, top + height, "nwse-resize"),
        ResizeHandle::South => (left + width / 2.0, top + height, "ns-resize"),
        ResizeHandle::SouthWest => (left, top + height, "nesw-resize"),
        ResizeHandle::West => (left, top + height / 2.0, "ew-resize"),
        ResizeHandle::NorthWest => (left, top, "nwse-resize"),
    };
    format!(
        "display:block;left:{}px;top:{}px;transform:translate(-50%,-50%);cursor:{cursor}",
        x, y,
    )
}

#[cfg(not(target_arch = "wasm32"))]
#[component]
pub fn ResizeHandles(runtime: AdminEditorRuntime) -> impl IntoView {
    let _ = runtime;
    view! { <span class="hidden" aria-hidden="true"></span> }
}
