use crate::editor::{
    decode_canvas_message, dispatch_shortcut, render_canvas_srcdoc_with_context,
    AdminEditorRuntime, CanvasBridgeMessage, CanvasComponentGeometry, ResizeHandles,
};
use fly_leptos::BrowserPoint;
use fly_ui::{resolve_editor_shortcut, CanvasRect, UiIntent, ViewportState};
use leptos::prelude::*;
use std::collections::BTreeMap;

#[component]
pub fn IsolatedAuthoringCanvas(runtime: AdminEditorRuntime) -> impl IntoView {
    let instance_seed = runtime.controller.with(|controller| {
        format!(
            "{}-{}",
            dom_id(controller.page_id()),
            controller.editor().revision().project_hash.hex()
        )
    });
    let instance_id = format!("fly-canvas-{instance_seed}");
    let iframe_id = format!("{instance_id}-frame");
    let geometry = RwSignal::new(BTreeMap::<String, CanvasComponentGeometry>::new());
    let ready = RwSignal::new(false);
    let pointer = RwSignal::new(None::<String>);

    let canvas_srcdoc = Memo::new({
        let runtime = runtime.clone();
        let instance_id = instance_id.clone();
        move |_| {
            let context = runtime.runtime_context.get();
            runtime.controller.with(|controller| {
                let mut active_document = controller.editor().document().clone();
                active_document.project.pages = controller
                    .editor()
                    .document()
                    .project
                    .pages
                    .get(controller.active_page_index())
                    .cloned()
                    .into_iter()
                    .collect();
                render_canvas_srcdoc_with_context(&active_document, &instance_id, &context)
            })
        }
    });

    #[cfg(target_arch = "wasm32")]
    let bridge_subscription = StoredValue::new_local(None::<fly_leptos::IframeJsonSubscription>);

    let on_iframe_load = {
        let runtime = runtime.clone();
        let iframe_id = iframe_id.clone();
        let expected_instance_id = instance_id.clone();
        move |_| {
            ready.set(false);
            geometry.set(BTreeMap::new());
            #[cfg(target_arch = "wasm32")]
            {
                bridge_subscription.set_value(None);
                let decoder_instance = expected_instance_id.clone();
                let message_runtime = runtime.clone();
                match fly_leptos::IframeJsonSubscription::subscribe_by_element_id(
                    iframe_id.clone(),
                    "null",
                    move |payload, last_sequence| {
                        decode_canvas_message(payload, &decoder_instance, last_sequence)
                    },
                    move |message| {
                        handle_canvas_message(&message_runtime, geometry, ready, pointer, message)
                    },
                ) {
                    Ok(subscription) => bridge_subscription.set_value(Some(subscription)),
                    Err(error) => runtime.fail(error.to_string()),
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let _ = (&iframe_id, &expected_instance_id, &runtime);
            }
        }
    };

    let frame_runtime = runtime.clone();
    let iframe_runtime = runtime.clone();
    let hovered_runtime = runtime.clone();
    let selected_runtime = runtime.clone();
    let insertion_runtime = runtime.clone();
    let resize_runtime = runtime;

    view! {
        <main class="relative min-w-0 overflow-auto rounded-xl border border-border bg-muted/40 p-4" aria-label="Isolated page canvas">
            <div class="mb-2 flex items-center justify-between text-xs text-muted-foreground">
                <span>{move || if ready.get() { "Canvas connected" } else { "Connecting canvas" }}</span>
                <span>{move || pointer.get().unwrap_or_default()}</span>
            </div>
            <div
                class="relative mx-auto origin-top overflow-hidden bg-white shadow-lg"
                style=move || frame_runtime.controller.with(|controller| {
                    let viewport = controller.ui().state.viewport;
                    format!(
                        "width:{}px;height:{}px",
                        f64::from(viewport.width) * f64::from(viewport.zoom),
                        f64::from(viewport.height) * f64::from(viewport.zoom),
                    )
                })
            >
                <iframe
                    id=iframe_id
                    title="Fly page canvas"
                    sandbox="allow-scripts"
                    srcdoc=move || canvas_srcdoc.get()
                    data-fly-iframe-canvas="true"
                    on:load=on_iframe_load
                    style=move || iframe_runtime.controller.with(|controller| {
                        let viewport = controller.ui().state.viewport;
                        format!(
                            "display:block;width:{}px;height:{}px;border:0;background:#fff;transform:scale({});transform-origin:0 0",
                            viewport.width,
                            viewport.height,
                            viewport.zoom,
                        )
                    })
                ></iframe>
                <OverlayLayer runtime=hovered_runtime kind=OverlayKind::Hovered />
                <OverlayLayer runtime=selected_runtime kind=OverlayKind::Selected />
                <OverlayLayer runtime=insertion_runtime kind=OverlayKind::Insertion />
                <ResizeHandles runtime=resize_runtime />
            </div>
        </main>
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OverlayKind {
    Hovered,
    Selected,
    Insertion,
}

#[component]
fn OverlayLayer(runtime: AdminEditorRuntime, kind: OverlayKind) -> impl IntoView {
    let class = match kind {
        OverlayKind::Hovered => "border border-dashed border-blue-400",
        OverlayKind::Selected => "border-2 border-blue-600 shadow-[0_0_0_1px_rgba(255,255,255,.8)]",
        OverlayKind::Insertion => "border-[3px] border-green-600 bg-green-600/10",
    };
    view! {
        <div
            aria-hidden="true"
            class=format!("pointer-events-none absolute {class}")
            style=move || runtime.controller.with(|controller| {
                let rect = match kind {
                    OverlayKind::Hovered => controller.ui().state.overlays.hovered,
                    OverlayKind::Selected => controller.ui().state.overlays.selected,
                    OverlayKind::Insertion => controller.ui().state.overlays.insertion,
                };
                overlay_style(rect, controller.ui().state.viewport)
            })
        ></div>
    }
}

fn handle_canvas_message(
    runtime: &AdminEditorRuntime,
    geometry: RwSignal<BTreeMap<String, CanvasComponentGeometry>>,
    ready: RwSignal<bool>,
    pointer: RwSignal<Option<String>>,
    message: CanvasBridgeMessage,
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
            let current = runtime
                .controller
                .with(|controller| controller.ui().state.viewport);
            runtime.dispatch(UiIntent::SetViewport(ViewportState {
                width: current.width,
                height: current.height,
                zoom: current.zoom,
                scroll_x,
                scroll_y,
            }));
            synchronize_overlays(runtime, geometry);
        }
        CanvasBridgeMessage::GeometrySnapshot { components } => {
            geometry.set(
                components
                    .into_iter()
                    .map(|component| (component.component_id.clone(), component))
                    .collect(),
            );
            synchronize_overlays(runtime, geometry);
        }
        CanvasBridgeMessage::PointerMoved { sample } => {
            pointer.set(Some(format!(
                "{:.0}, {:.0}",
                sample.position.x, sample.position.y
            )));
            update_drag_candidates(runtime, geometry, sample.position);
        }
        CanvasBridgeMessage::DragMoved { position } => {
            update_drag_candidates(runtime, geometry, position)
        }
        CanvasBridgeMessage::DropRequested { position } => {
            if runtime
                .controller
                .with(|controller| controller.ui().state.drag.is_some())
            {
                update_drag_candidates(runtime, geometry, position);
                runtime.dispatch(UiIntent::Drop);
            }
        }
        CanvasBridgeMessage::KeyStroke { stroke } => {
            if let Some(shortcut) = resolve_editor_shortcut(&stroke) {
                dispatch_shortcut(runtime, shortcut);
            }
        }
        CanvasBridgeMessage::CancelDragRequested => {
            if runtime
                .controller
                .with(|controller| controller.ui().state.drag.is_some())
            {
                runtime.dispatch(UiIntent::CancelDrag);
            }
        }
        CanvasBridgeMessage::FocusRequested { component_id } => {
            if runtime
                .controller
                .with(|controller| controller.ui().state.drag.is_none())
            {
                runtime.dispatch(UiIntent::Select(component_id));
                synchronize_overlays(runtime, geometry);
            }
        }
        CanvasBridgeMessage::HoverRequested { component_id } => {
            runtime.dispatch(UiIntent::Hover(component_id));
            synchronize_overlays(runtime, geometry);
        }
        CanvasBridgeMessage::Teardown => {
            ready.set(false);
            geometry.set(BTreeMap::new());
            runtime.dispatch(UiIntent::SetSelectedOverlay(None));
            runtime.dispatch(UiIntent::SetHoveredOverlay(None));
            if runtime
                .controller
                .with(|controller| controller.ui().state.drag.is_some())
            {
                runtime.dispatch(UiIntent::CancelDrag);
            }
        }
    }
}

fn update_drag_candidates(
    runtime: &AdminEditorRuntime,
    geometry: RwSignal<BTreeMap<String, CanvasComponentGeometry>>,
    position: BrowserPoint,
) {
    if runtime
        .controller
        .with(|controller| controller.ui().state.drag.is_none())
    {
        return;
    }
    let geometries = geometry.with(|geometry| geometry.values().cloned().collect::<Vec<_>>());
    let candidates = runtime
        .controller
        .with(|controller| controller.hit_candidates(position, geometries));
    runtime.dispatch(UiIntent::UpdateHitTest(candidates));
}

fn synchronize_overlays(
    runtime: &AdminEditorRuntime,
    geometry: RwSignal<BTreeMap<String, CanvasComponentGeometry>>,
) {
    let (selected, hovered) = runtime.controller.with(|controller| {
        (
            controller.ui().state.selection.component_id.clone(),
            controller.ui().state.selection.hovered_component_id.clone(),
        )
    });
    let selected = selected.and_then(|id| {
        geometry
            .with(|geometry| geometry.get(&id).map(|item| item.rect))
            .map(canvas_rect)
    });
    let hovered = hovered.and_then(|id| {
        geometry
            .with(|geometry| geometry.get(&id).map(|item| item.rect))
            .map(canvas_rect)
    });
    runtime.dispatch(UiIntent::SetSelectedOverlay(selected));
    runtime.dispatch(UiIntent::SetHoveredOverlay(hovered));
}

fn canvas_rect(rect: fly_leptos::BrowserRect) -> CanvasRect {
    CanvasRect {
        x: rect.left,
        y: rect.top,
        width: rect.width,
        height: rect.height,
    }
}

fn overlay_style(rect: Option<CanvasRect>, viewport: ViewportState) -> String {
    let Some(rect) = rect else {
        return "display:none".to_string();
    };
    let zoom = f64::from(viewport.zoom.max(0.01));
    format!(
        "display:block;left:{}px;top:{}px;width:{}px;height:{}px",
        rect.x * zoom,
        rect.y * zoom,
        rect.width * zoom,
        rect.height * zoom,
    )
}

fn dom_id(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect()
}
