use std::collections::BTreeMap;

use dioxus::prelude::*;
use fly_browser::{BrowserIntentEnvelope, BrowserIntentKind};
use fly_dioxus::FlyCanvasIframe;
use fly_ui::{
    CanvasRect, ResizeHandle, ResizePolicy, ResizeResult, ResizeSession, UiIntent, ViewportState,
};

use crate::core::session::EditorSessionState;
use crate::editor::{CanvasComponentGeometry, CanvasResourceLimit};

#[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
use crate::editor::{CanvasBridgeMessage, decode_canvas_message};
#[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
use fly_web::{
    BrowserResizeSession, EventListenerHandle, release_pointer_capture, set_pointer_capture,
};
#[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
use wasm_bindgen::JsCast;
#[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
use web_sys::{Element, Event, EventTarget, PointerEvent};

const RESIZE_HANDLES: [ResizeHandle; 8] = [
    ResizeHandle::NorthWest,
    ResizeHandle::North,
    ResizeHandle::NorthEast,
    ResizeHandle::East,
    ResizeHandle::SouthEast,
    ResizeHandle::South,
    ResizeHandle::SouthWest,
    ResizeHandle::West,
];

#[component]
pub fn DioxusIsolatedCanvas(
    srcdoc: String,
    /// Per-document iframe identity. It changes whenever the document or its revision changes.
    instance_id: String,
    /// Stable page identity used for server-side browser intents.
    intent_instance_id: String,
    can_resize: bool,
    session: Signal<EditorSessionState>,
    on_intent: EventHandler<BrowserIntentEnvelope>,
    on_ui_intent: EventHandler<UiIntent>,
    on_error: Option<EventHandler<String>>,
) -> Element {
    let mut geometry = use_signal(|| BTreeMap::<String, CanvasComponentGeometry>::new());
    let mut geometry_limit = use_signal(|| None::<CanvasResourceLimit>);
    let mut ready = use_signal(|| false);
    let mut pointer = use_signal(|| None::<String>);
    let mut resize_preview = use_signal(|| None::<ResizeResult>);
    // Geometry and pointer state belong to one rendered iframe document only.
    let mut active_instance_id = use_signal(|| instance_id.clone());

    let (viewport, selected_id, hovered_id, insertion_rect, selected_can_resize) = {
        let current = session.read();
        let selected = current.controller.selected_component_view();
        (
            current.controller.ui().state.viewport,
            current.controller.ui().state.selection.component_id.clone(),
            current.controller.ui().state.selection.hovered_component_id.clone(),
            current.controller.ui().state.overlays.insertion,
            can_resize
                && selected
                    .as_ref()
                    .is_some_and(|selected| !selected.is_root && !selected.is_locked),
        )
    };
    let instance_is_current =
        active_instance_id.read().as_str() == instance_id.as_str();
    let geometry_snapshot = if instance_is_current {
        geometry.read().clone()
    } else {
        BTreeMap::new()
    };
    let insertion_rect = if instance_is_current {
        insertion_rect
    } else {
        None
    };
    let selected_rect = selected_id.as_ref().and_then(|id| {
        geometry_snapshot
            .get(id)
            .map(|item| canvas_rect(item.rect))
    });
    let hovered_rect = hovered_id.as_ref().and_then(|id| {
        geometry_snapshot
            .get(id)
            .map(|item| canvas_rect(item.rect))
    });
    let zoom = f64::from(viewport.zoom.max(0.01));
    let preview_rect = if instance_is_current {
        resize_preview
            .read()
            .as_ref()
            .map(|result| result.rect)
            .or(selected_rect)
    } else {
        None
    };
    let rendered_width = (f64::from(viewport.width) * zoom).ceil() as u32;
    let rendered_height = (f64::from(viewport.height) * zoom).ceil() as u32;
    let view_box = format!("0 0 {} {}", viewport.width, viewport.height);
    let iframe_id = format!("{instance_id}-iframe");
    let resize_root_id = format!("{instance_id}-resize-overlay");

    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    let mut resize_session = use_signal(|| None::<BrowserResizeSession>);
    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    let mut captured_element = use_signal(|| None::<Element>);
    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    let mut resize_context = use_signal(|| None::<(String, CanvasRect, f64)>);
    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    let mut listener_handles = use_signal(|| Vec::<EventListenerHandle>::new());
    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    let mut listener_root_id = use_signal(|| None::<String>);

    use_effect(use_reactive!(|instance_id| {
        if active_instance_id.peek().as_str() == instance_id.as_str() {
            return;
        }
        active_instance_id.set(instance_id.clone());
        ready.set(false);
        geometry.set(BTreeMap::new());
        geometry_limit.set(None);
        pointer.set(None);
        resize_preview.set(None);
        on_ui_intent.call(UiIntent::SetSelectedOverlay(None));
        on_ui_intent.call(UiIntent::SetHoveredOverlay(None));
        if session.read().controller.ui().state.drag.is_some() {
            on_ui_intent.call(UiIntent::CancelDrag);
        }
        #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
        cancel_resize_session(
            resize_session,
            resize_preview,
            captured_element,
            None,
            true,
            on_error,
        );
    }));

    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    let next_resize_context = (instance_is_current && selected_can_resize)
        .then(|| selected_id.clone().zip(selected_rect))
        .flatten()
        .map(|(component_id, rect)| (component_id, rect, zoom));

    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    let resize_effect_inputs = (resize_root_id.clone(), next_resize_context.clone());

    let message_handler = {
        let expected_instance = instance_id.clone();
        let intent_instance = intent_instance_id.clone();
        move |payload: String| {
            #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
            if let Some((_, message)) =
                decode_canvas_message(&payload, &expected_instance, None)
            {
                handle_canvas_message(
                    message,
                    session,
                    geometry,
                    geometry_limit,
                    ready,
                    pointer,
                    resize_session,
                    resize_preview,
                    captured_element,
                    on_ui_intent,
                    on_intent,
                    &intent_instance,
                    on_error,
                );
            }
            #[cfg(not(all(target_arch = "wasm32", feature = "dioxus-web")))]
            {
                let _ = payload;
                let _ = &expected_instance;
                let _ = &intent_instance;
            }
        }
    };
    let on_message = EventHandler::new(message_handler);

    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    let on_iframe_load = Some(EventHandler::new({
        let mut geometry = geometry;
        let mut geometry_limit = geometry_limit;
        let mut ready = ready;
        let mut pointer = pointer;
        let mut active_instance_id = active_instance_id;
        let expected_instance_for_load = expected_instance.clone();
        move |_| {
            active_instance_id.set(expected_instance_for_load.clone());
            ready.set(false);
            geometry.set(BTreeMap::new());
            geometry_limit.set(None);
            pointer.set(None);
            cancel_resize_session(
                resize_session,
                resize_preview,
                captured_element,
                None,
                true,
                on_error,
            );
            on_ui_intent.call(UiIntent::SetSelectedOverlay(None));
            on_ui_intent.call(UiIntent::SetHoveredOverlay(None));
            if session_has_active_drag(session) {
                on_ui_intent.call(UiIntent::CancelDrag);
            }
        }
    }));
    #[cfg(not(all(target_arch = "wasm32", feature = "dioxus-web")))]
    let on_iframe_load: Option<EventHandler<()>> = None;

    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    {
        use_effect(use_reactive!(|resize_effect_inputs| {
            let (root_id, next_context) = resize_effect_inputs.clone();

            if listener_root_id.peek().as_ref() != Some(&root_id) {
                // Dropping the old handles unregisters listeners attached to the old SVG.
                listener_handles.set(Vec::new());
                listener_root_id.set(Some(root_id.clone()));
            }

            let current_context = resize_context.peek().clone();
            if current_context != next_context {
                if resize_session.peek().is_some() {
                    cancel_resize_session(
                        resize_session,
                        resize_preview,
                        captured_element,
                        None,
                        true,
                        on_error,
                    );
                }
                resize_context.set(next_context);
            }

            if !listener_handles.peek().is_empty() {
                return;
            }

            let Some(window) = web_sys::window() else {
                if let Some(handler) = on_error {
                    handler.call("browser window is unavailable for resize handles".to_string());
                }
                return;
            };
            let Some(document) = window.document() else {
                if let Some(handler) = on_error {
                    handler.call("browser document is unavailable for resize handles".to_string());
                }
                return;
            };
            let Some(root) = document.get_element_by_id(&root_id) else {
                return;
            };

            let root_target: EventTarget = root.unchecked_into();
            let window_target: EventTarget = window.clone().unchecked_into();
            let mut handles = Vec::new();

            let pointer_down = EventListenerHandle::new::<PointerEvent>(
                &root_target,
                "pointerdown",
                move |event| {
                    if event.button() != 0 || !event.is_primary() {
                        return;
                    }
                    let Some(target) = event
                        .target()
                        .and_then(|target| target.dyn_into::<Element>().ok())
                    else {
                        return;
                    };
                    let Ok(Some(handle_element)) =
                        target.closest("[data-fly-resize-handle]")
                    else {
                        return;
                    };
                    let Some(handle_name) =
                        handle_element.get_attribute("data-fly-resize-handle")
                    else {
                        return;
                    };
                    let Some(handle) = parse_resize_handle(&handle_name) else {
                        return;
                    };
                    let Some((component_id, rect, coordinate_scale)) =
                        resize_context.peek().clone()
                    else {
                        return;
                    };
                    let policy = ResizePolicy {
                        grid_size: Some(4.0),
                        ..ResizePolicy::default()
                    };
                    let active = BrowserResizeSession::begin_scaled(
                        component_id,
                        handle,
                        rect,
                        &event,
                        policy,
                        coordinate_scale,
                    );
                    if let Err(error) =
                        set_pointer_capture(&handle_element, active.pointer_id)
                    {
                        if let Some(handler) = on_error {
                            handler.call(error.to_string());
                        }
                        return;
                    }
                    captured_element.set(Some(handle_element));
                    resize_preview.set(active.update(&event));
                    resize_session.set(Some(active));
                    event.prevent_default();
                    event.stop_propagation();
                },
            );

            let pointer_move = EventListenerHandle::new::<PointerEvent>(
                &window_target,
                "pointermove",
                move |event| {
                    let Some(active) = resize_session.peek().clone() else {
                        return;
                    };
                    if active.accepts(&event) {
                        resize_preview.set(active.update(&event));
                    }
                },
            );

            let pointer_up = EventListenerHandle::new::<PointerEvent>(
                &window_target,
                "pointerup",
                move |event| {
                    let Some(active) = resize_session.peek().clone() else {
                        // Palette/layer HTML5 drag completion is owned by the drag source's
                        // dragend event. Parent pointer events are not a cross-iframe drop result.
                        return;
                    };
                    if !active.accepts(&event) {
                        return;
                    }
                    let result = active
                        .update(&event)
                        .or_else(|| resize_preview.peek().clone());
                    if let Some(result) =
                        result.filter(|result| result.width_changed || result.height_changed)
                    {
                        let component_id = active.resize.component_id.clone();
                        let patch = active.resize.component_patch(result);
                        on_intent.call(BrowserIntentEnvelope::new(
                            intent_instance_id.clone(),
                            BrowserIntentKind::PatchSelected,
                            serde_json::json!({
                                "component_id": component_id,
                                "patch": patch,
                            }),
                        ));
                    }
                    if let Some(element) = captured_element.peek().clone() {
                        if let Err(error) =
                            release_pointer_capture(&element, active.pointer_id)
                        {
                            if let Some(handler) = on_error {
                                handler.call(error.to_string());
                            }
                        }
                    }
                    captured_element.set(None);
                    resize_session.set(None);
                    resize_preview.set(None);
                },
            );

            let pointer_cancel = EventListenerHandle::new::<PointerEvent>(
                &window_target,
                "pointercancel",
                move |event| {
                    let Some(active) = resize_session.peek().clone() else {
                        // Palette/layer HTML5 drag completion is owned by the drag source's
                        // dragend event. Parent pointer events are not a cross-iframe drop result.
                        return;
                    };
                    if active.accepts(&event) {
                        cancel_resize_session(
                            resize_session,
                            resize_preview,
                            captured_element,
                            Some(event.pointer_id()),
                            true,
                            on_error,
                        );
                    }
                },
            );

            let lost_pointer_capture = EventListenerHandle::new::<PointerEvent>(
                &window_target,
                "lostpointercapture",
                move |event| {
                    let Some(active) = resize_session.peek().clone() else {
                        return;
                    };
                    if active.accepts(&event) {
                        cancel_resize_session(
                            resize_session,
                            resize_preview,
                            captured_element,
                            Some(event.pointer_id()),
                            false,
                            on_error,
                        );
                    }
                },
            );

            let window_blur = EventListenerHandle::new::<Event>(
                &window_target,
                "blur",
                move |_| {
                    cancel_resize_session(
                        resize_session,
                        resize_preview,
                        captured_element,
                        None,
                        true,
                        on_error,
                    );
                    if session_has_active_drag(session) {
                        on_ui_intent.call(UiIntent::CancelDrag);
                    }
                },
            );

            let registration = (|| {
                handles.push(pointer_down?);
                handles.push(pointer_move?);
                handles.push(pointer_up?);
                handles.push(pointer_cancel?);
                handles.push(lost_pointer_capture?);
                handles.push(window_blur?);
                Ok::<(), fly_web::BrowserRuntimeError>(())
            })();

            match registration {
                Ok(()) => listener_handles.set(handles),
                Err(error) => {
                    drop(handles);
                    if let Some(handler) = on_error {
                        handler.call(error.to_string());
                    }
                }
            }
        }));
    }

    let class = "absolute inset-0 border-0 bg-white";
    let expected_origin = fly_web::OPAQUE_ORIGIN.to_string();
    let pointer_label = if instance_is_current {
        pointer.read().clone().unwrap_or_default()
    } else {
        String::new()
    };
    let geometry_limit_message = instance_is_current
        .then(|| geometry_limit.read().as_ref().map(|limit| {
        format!(
            "Canvas geometry limit reached: {} components observed, limit {}. Selection and drag/drop overlays are unavailable until the page is simplified.",
            limit.observed, limit.limit
        }))
        .flatten();

    rsx! {
        div {
            class: "fly-canvas fly-canvas--isolated relative overflow-hidden",
            style: format!(
                "width:{}px;height:{}px;position:relative;overflow:hidden",
                rendered_width, rendered_height
            ),
            if let Some(message) = geometry_limit_message {
                div {
                    class: "absolute left-2 right-2 top-2 z-20 rounded-md border border-amber-500/40 bg-amber-500/10 p-3 text-sm",
                    role: "alert",
                    "{message}"
                }
            }
            div {
                class: "fly-canvas__stage relative",
                style: format!(
                    "width:{}px;height:{}px;position:relative;transform:scale({});transform-origin:top left",
                    viewport.width, viewport.height, zoom
                ),
                FlyCanvasIframe {
                    id: iframe_id,
                    instance_id: instance_id.clone(),
                    srcdoc,
                    expected_origin,
                    on_message,
                    on_error,
                    on_load: on_iframe_load,
                    title: Some("Page Builder canvas".to_string()),
                    width: Some(viewport.width),
                    height: Some(viewport.height),
                    class: Some(class.to_string()),
                }
                svg {
                    id: "{resize_root_id}",
                    view_box: "{view_box}",
                    width: "{viewport.width}",
                    height: "{viewport.height}",
                    style: "position:absolute;inset:0;overflow:visible;pointer-events:none",
                    if let Some(rect) = hovered_rect.filter(|rect| selected_rect != Some(*rect)) {
                        rect {
                            x: "{rect.x}",
                            y: "{rect.y}",
                            width: "{rect.width}",
                            height: "{rect.height}",
                            class: "fill-blue-500/5 stroke-blue-500 stroke-1",
                        }
                    }
                    if let Some(rect) = preview_rect {
                        rect {
                            x: "{rect.x}",
                            y: "{rect.y}",
                            width: "{rect.width}",
                            height: "{rect.height}",
                            class: "fill-violet-500/5 stroke-violet-600 stroke-2",
                        }
                    }
                    if let Some(rect) = insertion_rect {
                        rect {
                            x: "{rect.x}",
                            y: "{rect.y}",
                            width: "{rect.width}",
                            height: "{rect.height}",
                            class: "fill-emerald-500/10 stroke-emerald-500 stroke-2 stroke-dasharray-4",
                        }
                    }
                    if let Some(rect) = preview_rect.filter(|_| selected_can_resize) {
                        for handle in RESIZE_HANDLES {
                            {
                                let (x, y) = resize_handle_position(rect, handle);
                                let name = resize_handle_name(handle);
                                let cursor = resize_handle_cursor(handle);
                                let keyboard_resize_context = resize_context_for_keyboard(
                                    selected_id.clone(),
                                    selected_rect,
                                    zoom,
                                );
                                rsx! {
                                    circle {
                                        cx: "{x}",
                                        cy: "{y}",
                                        r: "5",
                                        "data-fly-resize-handle": "{name}",
                                        role: "button",
                                        tabindex: "0",
                                        aria_label: "Resize {name}",
                                        aria_keyshortcuts: "ArrowUp ArrowDown ArrowLeft ArrowRight Shift+ArrowUp Shift+ArrowDown Shift+ArrowLeft Shift+ArrowRight",
                                        class: format!(
                                            "pointer-events-auto fill-white stroke-violet-700 stroke-2 {}",
                                            cursor
                                        ),
                                        onkeydown: {
                                            let intent_instance_id = intent_instance_id.clone();
                                            move |event| {
                                                let Some((component_id, rect, _zoom)) =
                                                    keyboard_resize_context.clone()
                                                else {
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
                                                let Some(result) = resize
                                                    .update_by_arrow_key(&event.key(), event.shift_key())
                                                else {
                                                    return;
                                                };
                                                if !result.width_changed && !result.height_changed {
                                                    return;
                                                }
                                                event.prevent_default();
                                                on_intent.call(BrowserIntentEnvelope::new(
                                                    intent_instance_id.clone(),
                                                    BrowserIntentKind::PatchSelected,
                                                    serde_json::json!({
                                                        "component_id": component_id,
                                                        "patch": resize.component_patch(result),
                                                    }),
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if !instance_is_current || !*ready.read() {
                    div {
                        class: "absolute inset-0 flex items-center justify-center bg-background/70 text-sm text-muted-foreground pointer-events-none",
                        role: "status",
                        "Loading isolated canvas…"
                    }
                }
                if !pointer_label.is_empty() {
                    div {
                        class: "absolute bottom-2 right-2 rounded bg-background/80 px-2 py-1 text-xs text-muted-foreground pointer-events-none",
                        "{pointer_label}"
                    }
                }
            }
        }
    }
}

fn resize_context_for_keyboard(
    selected_id: Option<String>,
    selected_rect: Option<CanvasRect>,
    zoom: f64,
) -> Option<(String, CanvasRect, f64)> {
    selected_id.zip(selected_rect).map(|(id, rect)| (id, rect, zoom))
}

fn canvas_rect(rect: fly_web::BrowserRect) -> CanvasRect {
    CanvasRect {
        x: rect.left,
        y: rect.top,
        width: rect.width,
        height: rect.height,
    }
}

fn resize_handle_position(rect: CanvasRect, handle: ResizeHandle) -> (f64, f64) {
    let x = match handle {
        ResizeHandle::NorthWest | ResizeHandle::West | ResizeHandle::SouthWest => rect.x,
        ResizeHandle::North | ResizeHandle::South => rect.x + rect.width / 2.0,
        ResizeHandle::NorthEast | ResizeHandle::East | ResizeHandle::SouthEast => {
            rect.x + rect.width
        }
    };
    let y = match handle {
        ResizeHandle::NorthWest | ResizeHandle::North | ResizeHandle::NorthEast => rect.y,
        ResizeHandle::East | ResizeHandle::West => rect.y + rect.height / 2.0,
        ResizeHandle::SouthWest | ResizeHandle::South | ResizeHandle::SouthEast => {
            rect.y + rect.height
        }
    };
    (x, y)
}

fn resize_handle_name(handle: ResizeHandle) -> &'static str {
    match handle {
        ResizeHandle::North => "north",
        ResizeHandle::NorthEast => "north_east",
        ResizeHandle::East => "east",
        ResizeHandle::SouthEast => "south_east",
        ResizeHandle::South => "south",
        ResizeHandle::SouthWest => "south_west",
        ResizeHandle::West => "west",
        ResizeHandle::NorthWest => "north_west",
    }
}

fn resize_handle_cursor(handle: ResizeHandle) -> &'static str {
    match handle {
        ResizeHandle::NorthWest | ResizeHandle::SouthEast => "cursor-nwse-resize",
        ResizeHandle::NorthEast | ResizeHandle::SouthWest => "cursor-nesw-resize",
        ResizeHandle::North | ResizeHandle::South => "cursor-ns-resize",
        ResizeHandle::East | ResizeHandle::West => "cursor-ew-resize",
    }
}

fn parse_resize_handle(value: &str) -> Option<ResizeHandle> {
    match value {
        "north" => Some(ResizeHandle::North),
        "north_east" => Some(ResizeHandle::NorthEast),
        "east" => Some(ResizeHandle::East),
        "south_east" => Some(ResizeHandle::SouthEast),
        "south" => Some(ResizeHandle::South),
        "south_west" => Some(ResizeHandle::SouthWest),
        "west" => Some(ResizeHandle::West),
        "north_west" => Some(ResizeHandle::NorthWest),
        _ => None,
    }
}

#[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
fn cancel_resize_session(
    mut resize_session: Signal<Option<BrowserResizeSession>>,
    mut resize_preview: Signal<Option<ResizeResult>>,
    mut captured_element: Signal<Option<Element>>,
    pointer_id: Option<i32>,
    release_capture: bool,
    on_error: Option<EventHandler<String>>,
) {
    let Some(active) = resize_session.peek().clone() else {
        return;
    };
    if pointer_id.is_some_and(|pointer_id| pointer_id != active.pointer_id) {
        return;
    }
    if release_capture {
        if let Some(element) = captured_element.peek().clone() {
            if let Err(error) = release_pointer_capture(&element, active.pointer_id) {
                if let Some(handler) = on_error {
                    handler.call(error.to_string());
                }
            }
        }
    }
    captured_element.set(None);
    resize_session.set(None);
    resize_preview.set(None);
}

#[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
fn handle_canvas_message(
    message: CanvasBridgeMessage,
    session: Signal<EditorSessionState>,
    mut geometry: Signal<BTreeMap<String, CanvasComponentGeometry>>,
    mut geometry_limit: Signal<Option<CanvasResourceLimit>>,
    mut ready: Signal<bool>,
    mut pointer: Signal<Option<String>>,
    resize_session: Signal<Option<BrowserResizeSession>>,
    resize_preview: Signal<Option<ResizeResult>>,
    captured_element: Signal<Option<Element>>,
    on_ui_intent: EventHandler<UiIntent>,
    on_intent: EventHandler<BrowserIntentEnvelope>,
    intent_instance_id: &str,
    on_error: Option<EventHandler<String>>,
) {
    match message {
        CanvasBridgeMessage::Ready => ready.set(true),
        CanvasBridgeMessage::ViewportChanged {
            width: _,
            height: _,
            scroll_x,
            scroll_y,
            zoom: _,
        } => {
            let current = session.read().controller.ui().state.viewport;
            on_ui_intent.call(UiIntent::SetViewport(ViewportState {
                width: current.width,
                height: current.height,
                zoom: current.zoom,
                scroll_x,
                scroll_y,
            }));
        }
        CanvasBridgeMessage::GeometrySnapshot {
            components,
            resource_limit,
        } => {
            let exceeded_limit = resource_limit.is_some();
            geometry_limit.set(resource_limit);
            if exceeded_limit {
                // A truncated snapshot is not a safe basis for hit testing: a missing
                // component could otherwise leave a stale/incorrect drop candidate active.
                geometry.set(BTreeMap::new());
                on_ui_intent.call(UiIntent::SetSelectedOverlay(None));
                on_ui_intent.call(UiIntent::SetHoveredOverlay(None));
                if session_has_active_drag(session) {
                    on_ui_intent.call(UiIntent::CancelDrag);
                }
            } else {
                geometry.set(
                    components
                        .into_iter()
                        .map(|component| (component.component_id.clone(), component))
                        .collect(),
                );
            }
        }
        CanvasBridgeMessage::PointerMoved { sample } => {
            pointer.set(Some(format!("{:.0}, {:.0}", sample.position.x, sample.position.y)));
            update_drag_candidates(session, geometry, on_ui_intent, sample.position);
        }
        CanvasBridgeMessage::DragMoved { position } => {
            pointer.set(Some(format!("{:.0}, {:.0}", position.x, position.y)));
            update_drag_candidates(session, geometry, on_ui_intent, position);
        }
        CanvasBridgeMessage::DropRequested { position } => {
            pointer.set(Some(format!("{:.0}, {:.0}", position.x, position.y)));
            if !*ready.read() || geometry.read().is_empty() || geometry_limit.read().is_some() {
                if session_has_active_drag(session) {
                    on_ui_intent.call(UiIntent::CancelDrag);
                }
                return;
            }
            if session_has_active_drag(session) {
                update_drag_candidates(session, geometry, on_ui_intent, position);
                let has_legal_candidate = session.read().controller.ui().state.drag
                    .as_ref()
                    .and_then(|drag| drag.active_candidate())
                    .is_some_and(|candidate| candidate.legal);
                if has_legal_candidate {
                    on_ui_intent.call(UiIntent::Drop);
                } else {
                    // Pointer-up is terminal. Keep the state machine's retryable Drop
                    // semantics for explicit callers, but do not strand a pointer-ended drag.
                    on_ui_intent.call(UiIntent::CancelDrag);
                }
            }
        }
        CanvasBridgeMessage::KeyStroke { stroke } => {
            on_intent.call(BrowserIntentEnvelope::new(
                intent_instance_id.to_string(),
                BrowserIntentKind::KeyStroke,
                serde_json::json!({ "stroke": stroke }),
            ));
        }
        CanvasBridgeMessage::CancelDragRequested => {
            pointer.set(None);
            if session_has_active_drag(session) {
                on_ui_intent.call(UiIntent::CancelDrag);
            }
        }
        CanvasBridgeMessage::FocusRequested { component_id } => {
            if geometry_limit.read().is_none() && !session_has_active_drag(session) {
                on_ui_intent.call(UiIntent::Select(component_id));
            }
        }
        CanvasBridgeMessage::HoverRequested { component_id } => {
            if geometry_limit.read().is_none() {
                on_ui_intent.call(UiIntent::Hover(component_id));
            } else {
                on_ui_intent.call(UiIntent::SetHoveredOverlay(None));
            }
        }
        CanvasBridgeMessage::Teardown => {
            ready.set(false);
            geometry.set(BTreeMap::new());
            geometry_limit.set(None);
            pointer.set(None);
            cancel_resize_session(
                resize_session,
                resize_preview,
                captured_element,
                None,
                true,
                on_error,
            );
            on_ui_intent.call(UiIntent::SetSelectedOverlay(None));
            on_ui_intent.call(UiIntent::SetHoveredOverlay(None));
            if session.read().controller.ui().state.drag.is_some() {
                on_ui_intent.call(UiIntent::CancelDrag);
            }
        }
    }
}

#[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
fn session_has_active_drag(session: Signal<EditorSessionState>) -> bool {
    session.read().controller.ui().state.drag.is_some()
}

#[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
fn update_drag_candidates(
    session: Signal<EditorSessionState>,
    geometry: Signal<BTreeMap<String, CanvasComponentGeometry>>,
    on_ui_intent: EventHandler<UiIntent>,
    position: fly_web::BrowserPoint,
) {
    if !session_has_active_drag(session) {
        return;
    }
    let geometries = geometry.read().values().cloned().collect::<Vec<_>>();
    let candidates = session
        .read()
        .controller
        .hit_candidates(position, geometries);
    on_ui_intent.call(UiIntent::UpdateHitTest(candidates));
}
