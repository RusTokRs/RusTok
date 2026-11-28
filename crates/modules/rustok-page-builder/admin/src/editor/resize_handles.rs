use crate::editor::AdminEditorRuntime;
use leptos::prelude::*;

#[cfg(target_arch = "wasm32")]
use crate::AdminCanvasController;
#[cfg(any(target_arch = "wasm32", test))]
use crate::core::page_builder_resize_policy;
#[cfg(target_arch = "wasm32")]
use fly::ComponentPatch;
#[cfg(any(target_arch = "wasm32", test))]
use fly_ui::{CanvasRect, ResizeHandle, ViewportState};
#[cfg(target_arch = "wasm32")]
use fly_ui::{ResizeResult, ResizeSession};
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

#[cfg(any(target_arch = "wasm32", test))]
fn can_start_resize_pointer(listeners_ready: bool, button: i16, is_primary: bool) -> bool {
    listeners_ready && button == 0 && is_primary
}

#[cfg(any(target_arch = "wasm32", test))]
fn can_resize_by_keyboard(
    key: &str,
    alt: bool,
    control: bool,
    meta: bool,
    alt_graph: bool,
) -> bool {
    matches!(key, "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown")
        && !alt
        && !control
        && !meta
        && !alt_graph
}

#[derive(Debug, Clone, PartialEq)]
#[cfg(any(target_arch = "wasm32", test))]
struct ResizeContext {
    page_id: String,
    active_page_id: Option<String>,
    active_page_index: usize,
    revision_id: String,
    project_hash: String,
    command_sequence: u64,
    component_id: String,
    rect: CanvasRect,
    zoom: f64,
}

#[cfg(target_arch = "wasm32")]
fn current_resize_context(controller: &AdminCanvasController) -> Option<ResizeContext> {
    let selected = controller.selected_component_view()?;
    let capabilities = controller.ui().state.capabilities;
    if selected.is_root || selected.is_locked || !capabilities.edit || !capabilities.styles {
        return None;
    }
    let rect = controller.ui().state.overlays.selected?;
    if !rect.is_valid() {
        return None;
    }
    let revision = controller.editor().revision();
    Some(ResizeContext {
        page_id: controller.page_id().to_string(),
        active_page_id: controller.active_page_summary().and_then(|page| page.id),
        active_page_index: controller.active_page_index(),
        revision_id: controller.revision_id().to_string(),
        project_hash: revision.project_hash.hex(),
        command_sequence: revision.command_sequence,
        component_id: selected.id,
        rect,
        zoom: f64::from(controller.ui().state.viewport.zoom.max(0.01)),
    })
}

#[cfg(any(target_arch = "wasm32", test))]
fn resize_context_is_current(captured: &ResizeContext, current: Option<&ResizeContext>) -> bool {
    current == Some(captured)
}

#[cfg(target_arch = "wasm32")]
#[component]
pub fn ResizeHandles(runtime: AdminEditorRuntime) -> impl IntoView {
    let session = StoredValue::new_local(None::<fly_leptos::BrowserResizeSession>);
    let captured_element = StoredValue::new_local(None::<Element>);
    let resize_context = StoredValue::new_local(None::<ResizeContext>);
    let preview = RwSignal::new(None::<ResizeResult>);

    let listeners_ready = install_resize_listeners(
        runtime.clone(),
        session,
        captured_element,
        resize_context,
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
                    "pointer-events-auto touch-none select-none fill-white stroke-violet-700 stroke-1 drop-shadow focus-visible:stroke-primary {}",
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
                                // Modified arrow chords belong to browser, OS or editor
                                // shortcuts. Only plain arrows (with optional Shift for a
                                // larger step) should be consumed as resize commands.
                                if !can_resize_by_keyboard(
                                    &event.key(),
                                    event.alt_key(),
                                    event.ctrl_key(),
                                    event.meta_key(),
                                    event.get_modifier_state("AltGraph"),
                                ) {
                                    return;
                                }
                                let Some(context) = runtime
                                    .controller
                                    .with(current_resize_context)
                                else {
                                    return;
                                };
                                let resize = ResizeSession {
                                    component_id: context.component_id.clone(),
                                    handle,
                                    start_rect: context.rect,
                                    start_x: 0.0,
                                    start_y: 0.0,
                                    policy: page_builder_resize_policy(),
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
                                    &context.component_id,
                                    resize.component_patch(result),
                                );
                            }
                        }
                        on:pointerdown={
                            let runtime = runtime.clone();
                            move |event: PointerEvent| {
                                if !can_start_resize_pointer(
                                    listeners_ready,
                                    event.button(),
                                    event.is_primary(),
                                ) {
                                    return;
                                }
                                let Some(context) = runtime
                                    .controller
                                    .with(current_resize_context)
                                else {
                                    return;
                                };
                                let resize = fly_leptos::BrowserResizeSession::begin_scaled(
                                    context.component_id.clone(),
                                    handle,
                                    context.rect,
                                    &event,
                                    page_builder_resize_policy(),
                                    context.zoom,
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
                                // Only consume the event after the resize session and pointer
                                // capture are ready. Failed setup must not block canvas clicks.
                                event.prevent_default();
                                event.stop_propagation();
                                captured_element.set_value(Some(element));
                                resize_context.set_value(Some(context));
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
    resize_context: StoredValue<Option<ResizeContext>, LocalStorage>,
    preview: RwSignal<Option<ResizeResult>>,
) -> bool {
    let Some(window) = web_sys::window() else {
        runtime.fail("browser window is unavailable for resize handles");
        return false;
    };
    let target: EventTarget = window.unchecked_into();

    let move_runtime = runtime.clone();
    let move_session = session;
    let move_element = captured_element;
    let move_context = resize_context;
    let move_preview = preview;
    let move_listener = fly_leptos::EventListenerHandle::new::<PointerEvent>(
        &target,
        "pointermove",
        move |event| {
            let Some(active) = move_session.get_value() else {
                return;
            };
            if !active.accepts(&event) {
                return;
            }
            let captured = move_context.get_value();
            let current = move_runtime.controller.with(current_resize_context);
            if !captured
                .as_ref()
                .is_some_and(|captured| resize_context_is_current(captured, current.as_ref()))
            {
                cancel_active_resize(
                    &move_runtime,
                    move_session,
                    move_element,
                    move_context,
                    move_preview,
                    None,
                    true,
                );
                return;
            }
            if let Some(result) = active.update(&event) {
                move_preview.set(Some(result));
            }
        },
    );

    let up_runtime = runtime.clone();
    let up_session = session;
    let up_element = captured_element;
    let up_context = resize_context;
    let up_preview = preview;
    let up_listener =
        fly_leptos::EventListenerHandle::new::<PointerEvent>(&target, "pointerup", move |event| {
            let Some(active) = up_session.get_value() else {
                return;
            };
            if !active.accepts(&event) {
                return;
            }
            let captured = up_context.get_value();
            let current = up_runtime.controller.with(current_resize_context);
            if !captured
                .as_ref()
                .is_some_and(|captured| resize_context_is_current(captured, current.as_ref()))
            {
                cancel_active_resize(
                    &up_runtime,
                    up_session,
                    up_element,
                    up_context,
                    up_preview,
                    None,
                    true,
                );
                return;
            }
            let result = active.update(&event).or_else(|| up_preview.get_untracked());
            if let Some(result) =
                result.filter(|result| result.width_changed || result.height_changed)
            {
                apply_resize_patch(
                    &up_runtime,
                    &active.resize.component_id,
                    active.resize.component_patch(result),
                );
            }
            if let Some(element) = up_element.get_value() {
                if let Err(error) = fly_leptos::release_pointer_capture(&element, active.pointer_id)
                {
                    up_runtime.fail(error.to_string());
                }
            }
            up_element.set_value(None);
            up_context.set_value(None);
            up_session.set_value(None);
            up_preview.set(None);
        });

    let cancel_runtime = runtime.clone();
    let cancel_session = session;
    let cancel_element = captured_element;
    let cancel_context = resize_context;
    let cancel_preview = preview;
    let cancel_listener = fly_leptos::EventListenerHandle::new::<PointerEvent>(
        &target,
        "pointercancel",
        move |event| {
            cancel_active_resize(
                &cancel_runtime,
                cancel_session,
                cancel_element,
                cancel_context,
                cancel_preview,
                Some(event.pointer_id()),
                true,
            );
        },
    );

    let lost_runtime = runtime.clone();
    let lost_session = session;
    let lost_element = captured_element;
    let lost_context = resize_context;
    let lost_preview = preview;
    let lost_listener = fly_leptos::EventListenerHandle::new::<PointerEvent>(
        &target,
        "lostpointercapture",
        move |event| {
            cancel_active_resize(
                &lost_runtime,
                lost_session,
                lost_element,
                lost_context,
                lost_preview,
                Some(event.pointer_id()),
                false,
            );
        },
    );

    let blur_runtime = runtime.clone();
    let blur_session = session;
    let blur_element = captured_element;
    let blur_context = resize_context;
    let blur_preview = preview;
    let blur_listener = fly_leptos::EventListenerHandle::new::<Event>(&target, "blur", move |_| {
        cancel_active_resize(
            &blur_runtime,
            blur_session,
            blur_element,
            blur_context,
            blur_preview,
            None,
            true,
        );
    });

    match (
        move_listener,
        up_listener,
        cancel_listener,
        lost_listener,
        blur_listener,
    ) {
        (
            Ok(move_listener),
            Ok(up_listener),
            Ok(cancel_listener),
            Ok(lost_listener),
            Ok(blur_listener),
        ) => {
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
    let intent = runtime
        .controller
        .with(|controller| controller.ssr_selected_style_patch_intent(Some(component_id), patch));
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
    resize_context: StoredValue<Option<ResizeContext>, LocalStorage>,
    preview: RwSignal<Option<ResizeResult>>,
    pointer_id: Option<i32>,
    release_capture: bool,
) {
    let Some(active) = session.get_value() else {
        captured_element.set_value(None);
        resize_context.set_value(None);
        preview.set(None);
        return;
    };
    if pointer_id.is_some_and(|pointer_id| pointer_id != active.pointer_id) {
        return;
    }
    if release_capture {
        if let Some(element) = captured_element.get_value() {
            if let Err(error) = fly_leptos::release_pointer_capture(&element, active.pointer_id) {
                runtime.fail(error.to_string());
            }
        }
    }
    captured_element.set_value(None);
    resize_context.set_value(None);
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
    use fly_ui::{MAX_RESIZE_DIMENSION_PX, MIN_RESIZE_HEIGHT_PX, MIN_RESIZE_WIDTH_PX};

    fn resize_context() -> ResizeContext {
        ResizeContext {
            page_id: "home".to_string(),
            active_page_id: Some("landing".to_string()),
            active_page_index: 1,
            revision_id: "rev-2".to_string(),
            project_hash: "hash-2".to_string(),
            command_sequence: 7,
            component_id: "hero".to_string(),
            rect: CanvasRect {
                x: 10.0,
                y: 20.0,
                width: 100.0,
                height: 50.0,
            },
            zoom: 0.8,
        }
    }

    #[test]
    fn resize_gesture_is_fenced_to_its_canvas_snapshot_and_target() {
        let captured = resize_context();
        assert!(resize_context_is_current(&captured, Some(&captured)));
        assert!(!resize_context_is_current(&captured, None));

        let mut changed = captured.clone();
        changed.page_id = "about".to_string();
        assert!(!resize_context_is_current(&captured, Some(&changed)));

        let mut changed = captured.clone();
        changed.active_page_id = Some("about".to_string());
        assert!(!resize_context_is_current(&captured, Some(&changed)));

        let mut changed = captured.clone();
        changed.active_page_index += 1;
        assert!(!resize_context_is_current(&captured, Some(&changed)));

        let mut changed = captured.clone();
        changed.revision_id = "rev-3".to_string();
        assert!(!resize_context_is_current(&captured, Some(&changed)));

        let mut changed = captured.clone();
        changed.project_hash = "hash-3".to_string();
        assert!(!resize_context_is_current(&captured, Some(&changed)));

        let mut changed = captured.clone();
        changed.command_sequence += 1;
        assert!(!resize_context_is_current(&captured, Some(&changed)));

        let mut changed = captured.clone();
        changed.component_id = "other".to_string();
        assert!(!resize_context_is_current(&captured, Some(&changed)));

        let mut changed = captured.clone();
        changed.rect.width += 1.0;
        assert!(!resize_context_is_current(&captured, Some(&changed)));

        let mut changed = captured.clone();
        changed.zoom = 1.0;
        assert!(!resize_context_is_current(&captured, Some(&changed)));
    }

    #[test]
    fn modified_arrow_shortcuts_are_not_consumed_by_resize_handles() {
        assert!(can_resize_by_keyboard(
            "ArrowUp", false, false, false, false
        ));
        assert!(can_resize_by_keyboard(
            "ArrowDown",
            false,
            false,
            false,
            false
        ));
        assert!(!can_resize_by_keyboard(
            "ArrowUp", true, false, false, false
        ));
        assert!(!can_resize_by_keyboard(
            "ArrowDown",
            true,
            false,
            false,
            false
        ));
        assert!(!can_resize_by_keyboard(
            "ArrowLeft",
            false,
            true,
            false,
            false
        ));
        assert!(!can_resize_by_keyboard(
            "ArrowRight",
            false,
            false,
            true,
            false
        ));
        assert!(!can_resize_by_keyboard(
            "ArrowRight",
            false,
            false,
            false,
            true
        ));
        assert!(!can_resize_by_keyboard("Enter", false, false, false, false));
    }

    #[test]
    fn resize_pointerdown_only_consumes_primary_left_gestures() {
        assert!(can_start_resize_pointer(true, 0, true));
        assert!(!can_start_resize_pointer(false, 0, true));
        assert!(!can_start_resize_pointer(true, 2, true));
        assert!(!can_start_resize_pointer(true, 0, false));
    }

    #[test]
    fn page_builder_resize_policy_matches_server_dimension_limit() {
        let policy = page_builder_resize_policy();
        assert_eq!(policy.minimum_width, MIN_RESIZE_WIDTH_PX);
        assert_eq!(policy.minimum_height, MIN_RESIZE_HEIGHT_PX);
        assert_eq!(policy.maximum_width, Some(MAX_RESIZE_DIMENSION_PX));
        assert_eq!(policy.maximum_height, Some(MAX_RESIZE_DIMENSION_PX));
        assert_eq!(policy.grid_size, Some(4.0));
    }

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
