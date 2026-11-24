use crate::editor::AdminEditorRuntime;
use leptos::prelude::*;

#[cfg(target_arch = "wasm32")]
use fly::ComponentPatch;
#[cfg(any(target_arch = "wasm32", test))]
use fly_ui::{CanvasRect, ResizeHandle, ViewportState};
#[cfg(target_arch = "wasm32")]
use fly_ui::{ResizePolicy, ResizeResult, ResizeSession};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use web_sys::{Element, Event, EventTarget, KeyboardEvent, PointerEvent};

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg(any(target_arch = "wasm32", test))]
struct SvgRectGeometry {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg(any(target_arch = "wasm32", test))]
struct SvgPoint {
    x: f64,
    y: f64,
}

#[cfg(target_arch = "wasm32")]
#[component]
pub fn ResizeHandles(runtime: AdminEditorRuntime) -> impl IntoView {
    let session = StoredValue::new_local(None::<fly_leptos::BrowserResizeSession>);
    let captured_element = StoredValue::new_local(None::<Element>);
    let preview = RwSignal::new(None::<ResizeResult>);

    let listeners_ready =
        install_resize_listeners(runtime.clone(), session, captured_element, preview);

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
    let preview_geometry = Memo::new(move |_| {
        preview_runtime.controller.with(|controller| {
            preview
                .get()
                .map(|result| svg_rect_geometry(result.rect, controller.ui().state.viewport))
        })
    });

    view! {
        <svg
            aria-hidden="false"
            class="pointer-events-none absolute inset-0 h-full w-full overflow-visible"
            class:hidden=move || frame_runtime.controller.with(|controller| {
                controller.selected_component_view().is_none_or(|selected| {
                    selected.is_root
                        || selected.is_locked
                        || !controller.ui().state.capabilities.edit
                        || !controller.ui().state.capabilities.styles
                }) || controller.ui().state.overlays.selected.is_none()
            })
        >
            <rect
                aria-hidden="true"
                class="pointer-events-none fill-violet-500/5 stroke-violet-500 stroke-2"
                class:hidden=move || preview_geometry.get().is_none()
                x=move || preview_geometry.get().map(|value| value.x).unwrap_or_default()
                y=move || preview_geometry.get().map(|value| value.y).unwrap_or_default()
                width=move || preview_geometry.get().map(|value| value.width).unwrap_or_default()
                height=move || preview_geometry.get().map(|value| value.height).unwrap_or_default()
            ></rect>
            {handles.into_iter().map(|handle| {
                let runtime = runtime.clone();
                let position_runtime = runtime.clone();
                let position = Memo::new(move |_| position_runtime.controller.with(|controller| {
                    let viewport = controller.ui().state.viewport;
                    let rect = preview
                        .get()
                        .map(|result| result.rect)
                        .or(controller.ui().state.overlays.selected);
                    rect.map(|rect| svg_handle_position(rect, viewport, handle))
                }));
                let class = format!(
                    "pointer-events-auto fill-white stroke-violet-700 stroke-1 drop-shadow focus-visible:stroke-primary {}",
                    resize_handle_cursor_class(handle),
                );
                view! {
                    <circle
                        role="button"
                        tabindex="0"
                        aria-label=format!("Resize {handle:?}")
                        class=class
                        class:hidden=move || position.get().is_none()
                        cx=move || position.get().map(|value| value.x).unwrap_or_default()
                        cy=move || position.get().map(|value| value.y).unwrap_or_default()
                        r="6"
                        on:keydown={
                            let runtime = runtime.clone();
                            move |event: KeyboardEvent| {
                                let Some((component_id, rect)) = runtime.controller.with(|controller| {
                                    let selected = controller.selected_component_view()?;
                                    if selected.is_root
                                        || selected.is_locked
                                        || !controller.ui().state.capabilities.edit
                                        || !controller.ui().state.capabilities.styles
                                    {
                                        return None;
                                    }
                                    Some((
                                        selected.id,
                                        controller.ui().state.overlays.selected?,
                                    ))
                                }) else {
                                    return;
                                };
                                let resize = ResizeSession {
                                    component_id: component_id.clone(),
                                    handle,
                                    start_rect: rect,
                                    start_x: 0.0,
                                    start_y: 0.0,
                                    policy: ResizePolicy {
                                        grid_size: Some(4.0),
                                        ..ResizePolicy::default()
                                    },
                                };
                                let Some(result) =
                                    resize.update_by_arrow_key(&event.key(), event.shift_key())
                                else {
                                    return;
                                };
                                if !result.width_changed && !result.height_changed {
                                    return;
                                }
                                event.prevent_default();
                                apply_resize_patch(
                                    &runtime,
                                    &component_id,
                                    resize.component_patch(result),
                                );
                            }
                        }
                        on:pointerdown={
                            let runtime = runtime.clone();
                            move |event: PointerEvent| {
                                event.prevent_default();
                                event.stop_propagation();
                                if !listeners_ready || event.button() != 0 || !event.is_primary() {
                                    return;
                                }
                                let Some((component_id, rect, zoom)) = runtime.controller.with(|controller| {
                                    let selected = controller.selected_component_view()?;
                                    if selected.is_root
                                        || selected.is_locked
                                        || !controller.ui().state.capabilities.edit
                                        || !controller.ui().state.capabilities.styles
                                    {
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
                                let Some(element) = event
                                    .current_target()
                                    .and_then(|target| target.dyn_into::<Element>().ok())
                                else {
                                    runtime.fail("resize handle has no pointer-capture target");
                                    return;
                                };
                                if let Err(error) =
                                    fly_leptos::set_pointer_capture(&element, event.pointer_id())
                                {
                                    runtime.fail(error.to_string());
                                    return;
                                }
                                captured_element.set_value(Some(element));
                                session.set_value(Some(resize));
                                preview.set(None);
                            }
                        }
                    ></circle>
                }
            }).collect_view()}
        </svg>
    }
}

#[cfg(target_arch = "wasm32")]
fn install_resize_listeners(
    runtime: AdminEditorRuntime,
    session: StoredValue<Option<fly_leptos::BrowserResizeSession>, LocalStorage>,
    captured_element: StoredValue<Option<Element>, LocalStorage>,
    preview: RwSignal<Option<ResizeResult>>,
) -> bool {
    let Some(window) = web_sys::window() else {
        runtime.fail("browser window is unavailable for resize handles");
        return false;
    };
    let target: EventTarget = window.unchecked_into();

    let move_session = session;
    let move_listener = fly_leptos::EventListenerHandle::new::<PointerEvent>(
        &target,
        "pointermove",
        move |event| {
            if let Some(result) = move_session
                .get_value()
                .and_then(|session| session.update(&event))
            {
                preview.set(Some(result));
            }
        },
    );

    let up_runtime = runtime.clone();
    let up_session = session;
    let up_element = captured_element;
    let up_listener =
        fly_leptos::EventListenerHandle::new::<PointerEvent>(&target, "pointerup", move |event| {
            let Some(active) = up_session.get_value() else {
                return;
            };
            if !active.accepts(&event) {
                return;
            }
            let result = active.update(&event).or_else(|| preview.get_untracked());
            if let Some(result) = result.filter(|result| result.width_changed || result.height_changed) {
                apply_resize_patch(
                    &up_runtime,
                    &active.resize.component_id,
                    active.resize.component_patch(result),
                );
            }
            if let Some(element) = up_element.get_value() {
                if let Err(error) = fly_leptos::release_pointer_capture(&element, active.pointer_id) {
                    up_runtime.fail(error.to_string());
                }
            }
            up_element.set_value(None);
            up_session.set_value(None);
            preview.set(None);
        });

    let cancel_runtime = runtime.clone();
    let cancel_session = session;
    let cancel_element = captured_element;
    let cancel_preview = preview;
    let cancel_listener = fly_leptos::EventListenerHandle::new::<PointerEvent>(
        &target,
        "pointercancel",
        move |event| {
            cancel_active_resize(
                &cancel_runtime,
                cancel_session,
                cancel_element,
                cancel_preview,
                Some(event.pointer_id()),
                true,
            );
        },
    );

    let lost_runtime = runtime.clone();
    let lost_session = session;
    let lost_element = captured_element;
    let lost_preview = preview;
    let lost_listener = fly_leptos::EventListenerHandle::new::<PointerEvent>(
        &target,
        "lostpointercapture",
        move |event| {
            cancel_active_resize(
                &lost_runtime,
                lost_session,
                lost_element,
                lost_preview,
                Some(event.pointer_id()),
                false,
            );
        },
    );

    let blur_runtime = runtime.clone();
    let blur_session = session;
    let blur_element = captured_element;
    let blur_preview = preview;
    let blur_listener = fly_leptos::EventListenerHandle::new::<Event>(
        &target,
        "blur",
        move |_| {
            cancel_active_resize(
                &blur_runtime,
                blur_session,
                blur_element,
                blur_preview,
                None,
                true,
            );
        },
    );

    match (move_listener, up_listener, cancel_listener, lost_listener, blur_listener) {
        (Ok(move_listener), Ok(up_listener), Ok(cancel_listener), Ok(lost_listener), Ok(blur_listener)) => {
            let _listeners = StoredValue::new_local((
                move_listener,
                up_listener,
                cancel_listener,
                lost_listener,
                blur_listener,
            ));
            true
        }
        (move_result, up_result, cancel_result, lost_result, blur_result) => {
            let error = move_result
                .err()
                .or_else(|| up_result.err())
                .or_else(|| cancel_result.err())
                .or_else(|| lost_result.err())
                .or_else(|| blur_result.err());
            if let Some(error) = error {
                runtime.fail(error.to_string());
            }
            false
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn apply_resize_patch(runtime: &AdminEditorRuntime, component_id: &str, patch: ComponentPatch) {
    let intent = runtime.controller.with(|controller| {
        controller.ssr_selected_style_patch_intent(Some(component_id), patch)
    });
    match intent {
        Ok(intent) => runtime.dispatch(intent),
        Err(error) => runtime.fail(error),
    }
}

#[cfg(target_arch = "wasm32")]
fn cancel_active_resize(
    runtime: &AdminEditorRuntime,
    session: StoredValue<Option<fly_leptos::BrowserResizeSession>, LocalStorage>,
    captured_element: StoredValue<Option<Element>, LocalStorage>,
    preview: RwSignal<Option<ResizeResult>>,
    pointer_id: Option<i32>,
    release_capture: bool,
) {
    let Some(active) = session.get_value() else {
        return;
    };
    if pointer_id.is_some_and(|pointer_id| pointer_id != active.pointer_id) {
        return;
    }
    if release_capture {
        if let Some(element) = captured_element.get_value() {
            if let Err(error) =
                fly_leptos::release_pointer_capture(&element, active.pointer_id)
            {
                runtime.fail(error.to_string());
            }
        }
    }
    captured_element.set_value(None);
    session.set_value(None);
    preview.set(None);
}

#[cfg(any(target_arch = "wasm32", test))]
fn svg_rect_geometry(rect: CanvasRect, viewport: ViewportState) -> SvgRectGeometry {
    let zoom = f64::from(viewport.zoom.max(0.01));
    SvgRectGeometry {
        x: rect.x * zoom,
        y: rect.y * zoom,
        width: rect.width * zoom,
        height: rect.height * zoom,
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn svg_handle_position(
    rect: CanvasRect,
    viewport: ViewportState,
    handle: ResizeHandle,
) -> SvgPoint {
    let geometry = svg_rect_geometry(rect, viewport);
    let right = geometry.x + geometry.width;
    let bottom = geometry.y + geometry.height;
    let center_x = geometry.x + geometry.width / 2.0;
    let center_y = geometry.y + geometry.height / 2.0;
    let (x, y) = match handle {
        ResizeHandle::North => (center_x, geometry.y),
        ResizeHandle::NorthEast => (right, geometry.y),
        ResizeHandle::East => (right, center_y),
        ResizeHandle::SouthEast => (right, bottom),
        ResizeHandle::South => (center_x, bottom),
        ResizeHandle::SouthWest => (geometry.x, bottom),
        ResizeHandle::West => (geometry.x, center_y),
        ResizeHandle::NorthWest => (geometry.x, geometry.y),
    };
    SvgPoint { x, y }
}

#[cfg(any(target_arch = "wasm32", test))]
fn resize_handle_cursor_class(handle: ResizeHandle) -> &'static str {
    match handle {
        ResizeHandle::North | ResizeHandle::South => "cursor-ns-resize",
        ResizeHandle::NorthEast | ResizeHandle::SouthWest => "cursor-nesw-resize",
        ResizeHandle::East | ResizeHandle::West => "cursor-ew-resize",
        ResizeHandle::SouthEast | ResizeHandle::NorthWest => "cursor-nwse-resize",
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[component]
pub fn ResizeHandles(runtime: AdminEditorRuntime) -> impl IntoView {
    let _ = runtime;
    view! { <span class="hidden" aria-hidden="true"></span> }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_geometry_uses_svg_attributes_and_bounded_cursor_classes() {
        let viewport = ViewportState {
            zoom: 0.5,
            ..ViewportState::default()
        };
        let rect = CanvasRect {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 40.0,
        };

        assert_eq!(
            svg_rect_geometry(rect, viewport),
            SvgRectGeometry {
                x: 5.0,
                y: 10.0,
                width: 50.0,
                height: 20.0,
            }
        );
        assert_eq!(
            svg_handle_position(rect, viewport, ResizeHandle::SouthEast),
            SvgPoint { x: 55.0, y: 30.0 }
        );
        assert_eq!(
            resize_handle_cursor_class(ResizeHandle::NorthEast),
            "cursor-nesw-resize"
        );
    }
}
