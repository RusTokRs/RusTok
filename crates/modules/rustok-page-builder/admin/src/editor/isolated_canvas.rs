#[cfg(target_arch = "wasm32")]
use crate::editor::decode_canvas_message;
use crate::editor::{AdminEditorRuntime, ResizeHandles, render_canvas_srcdoc_with_context};
#[cfg(target_arch = "wasm32")]
use crate::editor::{CanvasBridgeMessage, CanvasComponentGeometry, dispatch_shortcut};
#[cfg(target_arch = "wasm32")]
use fly_leptos::BrowserPoint;
use fly_ui::{CanvasRect, ViewportState};
#[cfg(target_arch = "wasm32")]
use fly_ui::{UiIntent, resolve_editor_shortcut};
use leptos::prelude::*;
#[cfg(target_arch = "wasm32")]
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
struct ViewportSvgGeometry {
    source_width: u32,
    source_height: u32,
    rendered_width: f64,
    rendered_height: f64,
    view_box: String,
}

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
    #[cfg(target_arch = "wasm32")]
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
            #[cfg(target_arch = "wasm32")]
            {
                geometry.set(BTreeMap::new());
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

    let viewport_runtime = runtime.clone();
    let viewport_geometry = Memo::new(move |_| {
        viewport_runtime
            .controller
            .with(|controller| viewport_svg_geometry(controller.ui().state.viewport))
    });
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
            <div class="relative mx-auto w-fit overflow-hidden bg-white shadow-lg">
                <svg
                    class="block overflow-visible"
                    width=move || viewport_geometry.get().rendered_width
                    height=move || viewport_geometry.get().rendered_height
                    viewBox=move || viewport_geometry.get().view_box
                    preserveAspectRatio="none"
                    data-fly-svg-viewport="true"
                >
                    <foreignObject
                        x="0"
                        y="0"
                        width=move || viewport_geometry.get().source_width
                        height=move || viewport_geometry.get().source_height
                    >
                        <div class="h-full w-full overflow-hidden bg-white">
                            <iframe
                                id=iframe_id
                                title="Fly page canvas"
                                sandbox="allow-scripts"
                                srcdoc=move || canvas_srcdoc.get()
                                data-fly-iframe-canvas="true"
                                width=move || viewport_geometry.get().source_width
                                height=move || viewport_geometry.get().source_height
                                class="block border-0 bg-white"
                                on:load=on_iframe_load
                            ></iframe>
                        </div>
                    </foreignObject>
                </svg>
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

#[derive(Debug, Clone, Copy, PartialEq)]
struct OverlayGeometry {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[component]
fn OverlayLayer(runtime: AdminEditorRuntime, kind: OverlayKind) -> impl IntoView {
    let geometry = Memo::new(move |_| {
        runtime.controller.with(|controller| {
            let rect = match kind {
                OverlayKind::Hovered => controller.ui().state.overlays.hovered,
                OverlayKind::Selected => controller.ui().state.overlays.selected,
                OverlayKind::Insertion => controller.ui().state.overlays.insertion,
            };
            overlay_geometry(rect, controller.ui().state.viewport)
        })
    });
    let rect_class = match kind {
        OverlayKind::Hovered => "fill-transparent stroke-blue-400 stroke-1 [stroke-dasharray:4_4]",
        OverlayKind::Selected => {
            "fill-transparent stroke-blue-600 stroke-2 drop-shadow-[0_0_1px_rgba(255,255,255,.8)]"
        }
        OverlayKind::Insertion => "fill-green-600/10 stroke-green-600 stroke-[3]",
    };

    view! {
        <svg
            aria-hidden="true"
            class="pointer-events-none absolute inset-0 h-full w-full overflow-visible"
            class:hidden=move || geometry.get().is_none()
        >
            <rect
                class=rect_class
                x=move || geometry.get().map(|value| value.x).unwrap_or_default()
                y=move || geometry.get().map(|value| value.y).unwrap_or_default()
                width=move || geometry.get().map(|value| value.width).unwrap_or_default()
                height=move || geometry.get().map(|value| value.height).unwrap_or_default()
            ></rect>
        </svg>
    }
}

#[cfg(target_arch = "wasm32")]
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

#[cfg(target_arch = "wasm32")]
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

#[cfg(target_arch = "wasm32")]
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

#[cfg(target_arch = "wasm32")]
fn canvas_rect(rect: fly_leptos::BrowserRect) -> CanvasRect {
    CanvasRect {
        x: rect.left,
        y: rect.top,
        width: rect.width,
        height: rect.height,
    }
}

fn viewport_svg_geometry(viewport: ViewportState) -> ViewportSvgGeometry {
    let zoom = f64::from(viewport.zoom.max(0.01));
    ViewportSvgGeometry {
        source_width: viewport.width,
        source_height: viewport.height,
        rendered_width: f64::from(viewport.width) * zoom,
        rendered_height: f64::from(viewport.height) * zoom,
        view_box: format!("0 0 {} {}", viewport.width, viewport.height),
    }
}

fn overlay_geometry(rect: Option<CanvasRect>, viewport: ViewportState) -> Option<OverlayGeometry> {
    let rect = rect?;
    let zoom = f64::from(viewport.zoom.max(0.01));
    Some(OverlayGeometry {
        x: rect.x * zoom,
        y: rect.y * zoom,
        width: rect.width * zoom,
        height: rect.height * zoom,
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_geometry_preserves_source_dimensions_and_applies_zoom() {
        let geometry = viewport_svg_geometry(ViewportState {
            width: 390,
            height: 844,
            zoom: 0.8,
            ..ViewportState::default()
        });

        assert_eq!(geometry.source_width, 390);
        assert_eq!(geometry.source_height, 844);
        assert!((geometry.rendered_width - 312.0).abs() < 0.000_1);
        assert!((geometry.rendered_height - 675.2).abs() < 0.000_1);
        assert_eq!(geometry.view_box, "0 0 390 844");
    }

    #[test]
    fn overlay_geometry_uses_svg_coordinates_without_css_text() {
        let geometry = overlay_geometry(
            Some(CanvasRect {
                x: 10.0,
                y: 20.0,
                width: 30.0,
                height: 40.0,
            }),
            ViewportState {
                zoom: 0.5,
                ..ViewportState::default()
            },
        )
        .expect("geometry");

        assert_eq!(geometry.x, 5.0);
        assert_eq!(geometry.y, 10.0);
        assert_eq!(geometry.width, 15.0);
        assert_eq!(geometry.height, 20.0);
        assert!(overlay_geometry(None, ViewportState::default()).is_none());
    }
}
