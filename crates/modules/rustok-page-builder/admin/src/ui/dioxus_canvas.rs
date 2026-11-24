use std::collections::BTreeMap;

use dioxus::prelude::*;
use fly::EditorCommand;
use fly_browser::{BrowserIntentEnvelope, BrowserIntentKind};
use fly_dioxus::FlyCanvasIframe;
use fly_ui::{
    CanvasRect, ResizeHandle, ResizeResult, UiIntent, ViewportState,
};

use crate::core::session::EditorSessionState;
use crate::editor::CanvasComponentGeometry;

#[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
use crate::editor::{CanvasBridgeMessage, decode_canvas_message};
#[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
use fly_web::{
    BrowserResizeSession, EventListenerHandle, release_pointer_capture,
    set_pointer_capture,
};
#[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
use wasm_bindgen::JsCast;
#[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
use web_sys::{Element, EventTarget, PointerEvent};

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
    instance_id: String,
    session: Signal<EditorSessionState>,
    on_intent: EventHandler<BrowserIntentEnvelope>,
    on_ui_intent: EventHandler<UiIntent>,
    on_error: Option<EventHandler<String>>,
) -> Element {
    let geometry = use_signal(|| BTreeMap::<String, CanvasComponentGeometry>::new());
    let ready = use_signal(|| false);
    let resize_preview = use_signal(|| None::<ResizeResult>);

    let (viewport, selected_id, hovered_id, insertion_rect, selected_is_root) = {
        let current = session.read();
        (
            current.controller.ui().state.viewport,
            current.controller.ui().state.selection.component_id.clone(),
            current.controller.ui().state.selection.hovered_component_id.clone(),
            current.controller.ui().state.overlays.insertion,
            current
                .controller
                .selected_component_view()
                .is_none_or(|selected| selected.is_root),
        )
    };
    let geometry_snapshot = geometry.read().clone();
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
    let preview_rect = resize_preview
        .read()
        .as_ref()
        .map(|result| result.rect)
        .or(selected_rect);
    let rendered_width = (f64::from(viewport.width) * zoom).ceil() as u32;
    let rendered_height = (f64::from(viewport.height) * zoom).ceil() as u32;
    let view_box = format!("0 0 {} {}", viewport.width, viewport.height);
    let iframe_id = format!("{instance_id}-iframe");
    let resize_root_id = format!("{instance_id}-resize-overlay");

    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    let resize_session = use_signal(|| None::<BrowserResizeSession>);
    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    let captured_element = use_signal(|| None::<Element>);
    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    let resize_context = use_signal(|| None::<(String, CanvasRect, f64)>);
    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    let listener_handles = use_signal(|| Vec::<EventListenerHandle>::new());

    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    resize_context.set(
        (!selected_is_root)
            .then(|| selected_id.clone().zip(selected_rect))
            .flatten()
            .map(|(component_id, rect)| (component_id, rect, zoom)),
    );

    let message_handler = {
        let expected_instance = instance_id.clone();
        move |payload: String| {
            #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
            if let Some((_, message)) =
                decode_canvas_message(&payload, &expected_instance, None)
            {
                if matches!(message, CanvasBridgeMessage::Teardown) {
                    if let Some(active) = resize_session.read().as_ref().cloned() {
                        if let Some(element) = captured_element.read().as_ref() {
                            let _ = release_pointer_capture(element, active.pointer_id);
                        }
                    }
                    resize_session.set(None);
                    resize_preview.set(None);
                    captured_element.set(None);
                }
                handle_canvas_message(
                    message,
                    session,
                    geometry,
                    ready,
                    on_ui_intent,
                    on_intent,
                    &expected_instance,
                );
            }
            #[cfg(not(all(target_arch = "wasm32", feature = "dioxus-web")))]
            {
                let _ = payload;
                let _ = &expected_instance;
            }
        }
    };
    let on_message = EventHandler::new(message_handler);

    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    {
        let root_id = resize_root_id.clone();
        use_effect(move || {
            if !listener_handles.read().is_empty() {
                return;
            }
            let Some(window) = web_sys::window() else {
                return;
            };
            let Some(document) = window.document() else {
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
                        resize_context.read().clone()
                    else {
                        return;
                    };
                    let active = BrowserResizeSession::begin_scaled(
                        component_id,
                        handle,
                        rect,
                        &event,
                        fly_ui::ResizePolicy::default(),
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
            match pointer_down {
                Ok(handle) => handles.push(handle),
                Err(error) => {
                    if let Some(handler) = on_error {
                        handler.call(error.to_string());
                    }
                }
            }

            let pointer_move = EventListenerHandle::new::<PointerEvent>(
                &window_target,
                "pointermove",
                move |event| {
                    let active = resize_session.read().as_ref().cloned();
                    if let Some(active) = active
                        && let Some(result) = active.update(&event)
                    {
                        resize_preview.set(Some(result));
                    }
                },
            );
            match pointer_move {
                Ok(handle) => handles.push(handle),
                Err(error) => {
                    if let Some(handler) = on_error {
                        handler.call(error.to_string());
                    }
                }
            }

            let pointer_up = EventListenerHandle::new::<PointerEvent>(
                &window_target,
                "pointerup",
                move |event| {
                    let Some(active) = resize_session.read().as_ref().cloned() else {
                        return;
                    };
                    if !active.accepts(&event) {
                        return;
                    }
                    if let Some(result) = active.update(&event)
                        && (result.width_changed || result.height_changed)
                    {
                        on_ui_intent.call(UiIntent::execute(EditorCommand::Patch {
                            component_id: active.resize.component_id.clone(),
                            patch: active.resize.component_patch(result),
                        }));
                    }
                    if let Some(element) = captured_element.read().as_ref() {
                        let _ = release_pointer_capture(element, active.pointer_id);
                    }
                    captured_element.set(None);
                    resize_session.set(None);
                    resize_preview.set(None);
                },
            );
            match pointer_up {
                Ok(handle) => handles.push(handle),
                Err(error) => {
                    if let Some(handler) = on_error {
                        handler.call(error.to_string());
                    }
                }
            }

            let pointer_cancel = EventListenerHandle::new::<PointerEvent>(
                &window_target,
                "pointercancel",
                move |event| {
                    let Some(active) = resize_session.read().as_ref().cloned() else {
                        return;
                    };
                    if !active.accepts(&event) {
                        return;
                    }
                    if let Some(element) = captured_element.read().as_ref() {
                        let _ = release_pointer_capture(element, active.pointer_id);
                    }
                    captured_element.set(None);
                    resize_session.set(None);
                    resize_preview.set(None);
                },
            );
            match pointer_cancel {
                Ok(handle) => handles.push(handle),
                Err(error) => {
                    if let Some(handler) = on_error {
                        handler.call(error.to_string());
                    }
                }
            }

            listener_handles.set(handles);
        });
    }

    let class = "absolute inset-0 border-0 bg-white";
    let expected_origin = fly_web::OPAQUE_ORIGIN.to_string();

    rsx! {
        div {
            class: "fly-canvas fly-canvas--isolated relative overflow-hidden",
            style: format!(
                "width:{}px;height:{}px;position:relative;overflow:hidden",
                rendered_width, rendered_height
            ),
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
                    if preview_rect.is_some() && !selected_is_root {
                        for handle in RESIZE_HANDLES {
                            {
                                let (x, y) = resize_handle_position(preview_rect.unwrap(), handle);
                                let name = resize_handle_name(handle);
                                let cursor = resize_handle_cursor(handle);
                                rsx! {
                                    circle {
                                        cx: "{x}",
                                        cy: "{y}",
                                        r: "5",
                                        "data-fly-resize-handle": "{name}",
                                        role: "button",
                                        tabindex: "0",
                                        aria_label: "Resize {name}",
                                        class: format!(
                                            "pointer-events-auto fill-white stroke-violet-700 stroke-2 {}",
                                            cursor
                                        ),
                                    }
                                }
                            }
                        }
                    }
                }
                if !*ready.read() {
                    div {
                        class: "absolute inset-0 flex items-center justify-center bg-background/70 text-sm text-muted-foreground pointer-events-none",
                        role: "status",
                        "Loading isolated canvas…"
                    }
                }
            }
        }
    }
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
fn handle_canvas_message(
    message: CanvasBridgeMessage,
    session: Signal<EditorSessionState>,
    geometry: Signal<BTreeMap<String, CanvasComponentGeometry>>,
    ready: Signal<bool>,
    on_ui_intent: EventHandler<UiIntent>,
    on_intent: EventHandler<BrowserIntentEnvelope>,
    instance_id: &str,
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
        CanvasBridgeMessage::GeometrySnapshot { components } => {
            geometry.set(
                components
                    .into_iter()
                    .map(|component| (component.component_id.clone(), component))
                    .collect(),
            );
        }
        CanvasBridgeMessage::PointerMoved { sample } => {
            update_drag_candidates(session, geometry, on_ui_intent, sample.position);
        }
        CanvasBridgeMessage::DragMoved { position } => {
            update_drag_candidates(session, geometry, on_ui_intent, position);
        }
        CanvasBridgeMessage::DropRequested { position } => {
            if session.read().controller.ui().state.drag.is_some() {
                update_drag_candidates(session, geometry, on_ui_intent, position);
                on_ui_intent.call(UiIntent::Drop);
            }
        }
        CanvasBridgeMessage::KeyStroke { stroke } => {
            on_intent.call(BrowserIntentEnvelope::new(
                instance_id.to_string(),
                BrowserIntentKind::KeyStroke,
                serde_json::json!({ "stroke": stroke }),
            ));
        }
        CanvasBridgeMessage::CancelDragRequested => {
            if session.read().controller.ui().state.drag.is_some() {
                on_ui_intent.call(UiIntent::CancelDrag);
            }
        }
        CanvasBridgeMessage::FocusRequested { component_id } => {
            if session.read().controller.ui().state.drag.is_none() {
                on_ui_intent.call(UiIntent::Select(component_id));
            }
        }
        CanvasBridgeMessage::HoverRequested { component_id } => {
            on_ui_intent.call(UiIntent::Hover(component_id));
        }
        CanvasBridgeMessage::Teardown => {
            ready.set(false);
            geometry.set(BTreeMap::new());
            on_ui_intent.call(UiIntent::SetSelectedOverlay(None));
            on_ui_intent.call(UiIntent::SetHoveredOverlay(None));
            if session.read().controller.ui().state.drag.is_some() {
                on_ui_intent.call(UiIntent::CancelDrag);
            }
        }
    }
}

#[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
fn update_drag_candidates(
    session: Signal<EditorSessionState>,
    geometry: Signal<BTreeMap<String, CanvasComponentGeometry>>,
    on_ui_intent: EventHandler<UiIntent>,
    position: fly_web::BrowserPoint,
) {
    if session.read().controller.ui().state.drag.is_none() {
        return;
    }
    let geometries = geometry.read().values().cloned().collect::<Vec<_>>();
    let candidates = session
        .read()
        .controller
        .hit_candidates(position, geometries);
    on_ui_intent.call(UiIntent::UpdateHitTest(candidates));
}
