use crate::editor::AdminEditorRuntime;
use leptos::prelude::*;

#[cfg(target_arch = "wasm32")]
use fly::EditorCommand;
#[cfg(any(target_arch = "wasm32", test))]
use fly_ui::{CanvasRect, ResizeHandle, ViewportState};
#[cfg(target_arch = "wasm32")]
use fly_ui::{ResizePolicy, ResizeResult, UiIntent};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use web_sys::{Element, EventTarget, PointerEvent};

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
                controller
                    .selected_component_view()
                    .is_none_or(|selected| selected.is_root)
                    || controller.ui().state.overlays.selected.is_none()
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
            if let Some(result) = result {
                up_runtime.dispatch(UiIntent::execute(EditorCommand::Patch {
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
        });

    match (move_listener, up_listener) {
        (Ok(move_listener), Ok(up_listener)) => {
            let _listeners = StoredValue::new_local((move_listener, up_listener));
        }
        (Err(error), _) | (_, Err(error)) => runtime.fail(error.to_string()),
    }
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
