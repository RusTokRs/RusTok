use crate::editor::{
    decode_canvas_message, render_canvas_srcdoc, CanvasBridgeMessage, CanvasComponentGeometry,
};
use crate::i18n::t;
use crate::{AdminCanvasController, AdminCanvasEffect, PageBuilderAdminFacade};
use fly::{ComponentPatch, EditorCommand, ProjectHash};
use fly_leptos::{BrowserPoint, CoordinateTransform};
use fly_ui::{CanvasRect, UiIntent, ViewportState};
use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_page_builder::dto::{
    PageBuilderCapabilityRequest, PageBuilderCapabilityResponse,
};
use rustok_ui_core::UiRouteContext;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::rc::Rc;

fn dispatch_admin_intent(
    controller: RwSignal<AdminCanvasController>,
    last_error: RwSignal<Option<String>>,
    last_announcement: RwSignal<Option<String>>,
    facade: Option<Rc<dyn PageBuilderAdminFacade>>,
    on_request: Option<Callback<PageBuilderCapabilityRequest>>,
    facade_missing: String,
    save_succeeded: String,
    intent: UiIntent,
) {
    let mut requests = Vec::new();
    let mut announcements = Vec::new();
    let mut error = None;

    controller.update(|controller| match controller.dispatch(intent) {
        Ok(effects) => {
            for effect in effects {
                match effect {
                    AdminCanvasEffect::Request {
                        request,
                        expected_hash,
                        ..
                    } => requests.push((request, expected_hash)),
                    AdminCanvasEffect::Announce(message) => announcements.push(message),
                }
            }
        }
        Err(dispatch_error) => error = Some(dispatch_error.to_string()),
    });

    last_error.set(error);
    if let Some(message) = announcements.pop() {
        last_announcement.set(Some(message));
    }

    for (request, expected_hash) in requests {
        if let Some(facade) = facade.as_ref() {
            execute_facade_request(
                controller,
                last_error,
                last_announcement,
                Rc::clone(facade),
                request,
                expected_hash,
                save_succeeded.clone(),
            );
        } else if let Some(callback) = on_request.as_ref() {
            callback.run(request);
        } else {
            last_error.set(Some(facade_missing.clone()));
        }
    }
}

fn execute_facade_request(
    controller: RwSignal<AdminCanvasController>,
    last_error: RwSignal<Option<String>>,
    last_announcement: RwSignal<Option<String>>,
    facade: Rc<dyn PageBuilderAdminFacade>,
    request: PageBuilderCapabilityRequest,
    expected_hash: Option<ProjectHash>,
    save_succeeded: String,
) {
    let expected_hash = expected_hash.or_else(|| {
        controller.with(|controller| controller.ui().state.dirty.project_hash)
    });
    let mut start_error = None;
    controller.update(|controller| {
        if let Err(error) = controller.mark_save_started() {
            start_error = Some(error.to_string());
        }
    });
    if let Some(error) = start_error {
        last_error.set(Some(error));
        return;
    }

    spawn_local(async move {
        match facade.execute(request).await {
            Ok(PageBuilderCapabilityResponse::Publish(response)) => {
                let mut acknowledgement_error = None;
                controller.update(|controller| {
                    let expected_hash = expected_hash
                        .unwrap_or(controller.editor().revision().project_hash);
                    if response.page_id != controller.page_id() {
                        acknowledgement_error = Some(format!(
                            "Page Builder facade returned page `{}` for `{}`",
                            response.page_id,
                            controller.page_id()
                        ));
                    } else if let Err(error) = controller
                        .acknowledge_save_for_hash(expected_hash, response.revision_id.clone())
                    {
                        acknowledgement_error = Some(error.to_string());
                    }
                });
                if let Some(error) = acknowledgement_error {
                    controller.update(|controller| {
                        let _ = controller.mark_save_failed();
                    });
                    last_error.set(Some(error));
                } else {
                    last_error.set(None);
                    last_announcement.set(Some(save_succeeded));
                }
            }
            Ok(response) => {
                controller.update(|controller| {
                    let _ = controller.mark_save_failed();
                });
                last_error.set(Some(format!(
                    "Page Builder facade returned `{}` for a publish request",
                    response.capability()
                )));
            }
            Err(error) => {
                controller.update(|controller| {
                    let _ = controller.mark_save_failed();
                });
                last_error.set(Some(error.to_string()));
            }
        }
    });
}

fn dispatch_canvas_intent(
    controller: RwSignal<AdminCanvasController>,
    last_error: RwSignal<Option<String>>,
    last_announcement: RwSignal<Option<String>>,
    intent: UiIntent,
) {
    let mut error = None;
    let mut announcement = None;
    controller.update(|controller| match controller.dispatch(intent) {
        Ok(effects) => {
            for effect in effects {
                if let AdminCanvasEffect::Announce(message) = effect {
                    announcement = Some(message);
                }
            }
        }
        Err(dispatch_error) => error = Some(dispatch_error.to_string()),
    });
    last_error.set(error);
    if let Some(message) = announcement {
        last_announcement.set(Some(message));
    }
}

fn dispatch_result_intent(
    controller: RwSignal<AdminCanvasController>,
    last_error: RwSignal<Option<String>>,
    last_announcement: RwSignal<Option<String>>,
    intent: Result<UiIntent, String>,
) {
    match intent {
        Ok(intent) => dispatch_canvas_intent(controller, last_error, last_announcement, intent),
        Err(error) => last_error.set(Some(error)),
    }
}

fn canvas_rect(
    rect: fly_leptos::BrowserRect,
    viewport: ViewportState,
) -> CanvasRect {
    rect.to_canvas_rect(CoordinateTransform {
        scroll_x: viewport.scroll_x,
        scroll_y: viewport.scroll_y,
        zoom: f64::from(viewport.zoom),
        ..CoordinateTransform::default()
    })
}

fn synchronize_overlays(
    controller: RwSignal<AdminCanvasController>,
    geometry: RwSignal<BTreeMap<String, CanvasComponentGeometry>>,
    last_error: RwSignal<Option<String>>,
    last_announcement: RwSignal<Option<String>>,
) {
    let (selected, hovered, viewport) = controller.with(|controller| {
        (
            controller.ui().state.selection.component_id.clone(),
            controller.ui().state.selection.hovered_component_id.clone(),
            controller.ui().state.viewport,
        )
    });
    let selected = selected.and_then(|id| {
        geometry
            .with(|geometry| geometry.get(&id).map(|item| item.rect))
            .map(|rect| canvas_rect(rect, viewport))
    });
    let hovered = hovered.and_then(|id| {
        geometry
            .with(|geometry| geometry.get(&id).map(|item| item.rect))
            .map(|rect| canvas_rect(rect, viewport))
    });
    dispatch_canvas_intent(
        controller,
        last_error,
        last_announcement,
        UiIntent::SetSelectedOverlay(selected),
    );
    dispatch_canvas_intent(
        controller,
        last_error,
        last_announcement,
        UiIntent::SetHoveredOverlay(hovered),
    );
}

fn update_drag_candidates(
    controller: RwSignal<AdminCanvasController>,
    geometry: RwSignal<BTreeMap<String, CanvasComponentGeometry>>,
    last_error: RwSignal<Option<String>>,
    last_announcement: RwSignal<Option<String>>,
    position: BrowserPoint,
) {
    if !controller.with(|controller| controller.ui().state.drag.is_some()) {
        return;
    }
    let geometries = geometry.with(|geometry| geometry.values().cloned().collect::<Vec<_>>());
    let candidates = controller.with(|controller| controller.hit_candidates(position, geometries));
    dispatch_canvas_intent(
        controller,
        last_error,
        last_announcement,
        UiIntent::UpdateHitTest(candidates),
    );
}

fn complete_drag(
    controller: RwSignal<AdminCanvasController>,
    geometry: RwSignal<BTreeMap<String, CanvasComponentGeometry>>,
    last_error: RwSignal<Option<String>>,
    last_announcement: RwSignal<Option<String>>,
    position: BrowserPoint,
) {
    if !controller.with(|controller| controller.ui().state.drag.is_some()) {
        return;
    }
    update_drag_candidates(
        controller,
        geometry,
        last_error,
        last_announcement,
        position,
    );
    dispatch_canvas_intent(
        controller,
        last_error,
        last_announcement,
        UiIntent::Drop,
    );
}

fn handle_canvas_message(
    controller: RwSignal<AdminCanvasController>,
    geometry: RwSignal<BTreeMap<String, CanvasComponentGeometry>>,
    ready: RwSignal<bool>,
    pointer: RwSignal<Option<String>>,
    last_error: RwSignal<Option<String>>,
    last_announcement: RwSignal<Option<String>>,
    message: CanvasBridgeMessage,
) {
    match message {
        CanvasBridgeMessage::Ready => ready.set(true),
        CanvasBridgeMessage::ViewportChanged {
            width,
            height,
            scroll_x,
            scroll_y,
            zoom,
        } => {
            dispatch_canvas_intent(
                controller,
                last_error,
                last_announcement,
                UiIntent::SetViewport(ViewportState {
                    width,
                    height,
                    zoom: zoom as f32,
                    scroll_x,
                    scroll_y,
                }),
            );
            synchronize_overlays(
                controller,
                geometry,
                last_error,
                last_announcement,
            );
        }
        CanvasBridgeMessage::GeometrySnapshot { components } => {
            geometry.set(
                components
                    .into_iter()
                    .map(|component| (component.component_id.clone(), component))
                    .collect(),
            );
            synchronize_overlays(
                controller,
                geometry,
                last_error,
                last_announcement,
            );
        }
        CanvasBridgeMessage::PointerMoved { sample } => {
            pointer.set(Some(format!(
                "{:.0}, {:.0}",
                sample.position.x, sample.position.y
            )));
            update_drag_candidates(
                controller,
                geometry,
                last_error,
                last_announcement,
                sample.position,
            );
        }
        CanvasBridgeMessage::DragMoved { position } => update_drag_candidates(
            controller,
            geometry,
            last_error,
            last_announcement,
            position,
        ),
        CanvasBridgeMessage::DropRequested { position } => complete_drag(
            controller,
            geometry,
            last_error,
            last_announcement,
            position,
        ),
        CanvasBridgeMessage::CancelDragRequested => {
            if controller.with(|controller| controller.ui().state.drag.is_some()) {
                dispatch_canvas_intent(
                    controller,
                    last_error,
                    last_announcement,
                    UiIntent::CancelDrag,
                );
            }
        }
        CanvasBridgeMessage::FocusRequested { component_id } => {
            if !controller.with(|controller| controller.ui().state.drag.is_some()) {
                dispatch_canvas_intent(
                    controller,
                    last_error,
                    last_announcement,
                    UiIntent::Select(component_id),
                );
                synchronize_overlays(
                    controller,
                    geometry,
                    last_error,
                    last_announcement,
                );
            }
        }
        CanvasBridgeMessage::HoverRequested { component_id } => {
            dispatch_canvas_intent(
                controller,
                last_error,
                last_announcement,
                UiIntent::Hover(component_id),
            );
            synchronize_overlays(
                controller,
                geometry,
                last_error,
                last_announcement,
            );
        }
        CanvasBridgeMessage::Teardown => {
            ready.set(false);
            geometry.set(BTreeMap::new());
            dispatch_canvas_intent(
                controller,
                last_error,
                last_announcement,
                UiIntent::SetSelectedOverlay(None),
            );
            dispatch_canvas_intent(
                controller,
                last_error,
                last_announcement,
                UiIntent::SetHoveredOverlay(None),
            );
            if controller.with(|controller| controller.ui().state.drag.is_some()) {
                dispatch_canvas_intent(
                    controller,
                    last_error,
                    last_announcement,
                    UiIntent::CancelDrag,
                );
            }
        }
    }
}

fn overlay_style(rect: Option<CanvasRect>, viewport: ViewportState) -> String {
    let Some(rect) = rect else {
        return "display:none".to_string();
    };
    let zoom = f64::from(viewport.zoom.max(0.01));
    let left = rect.x * zoom - viewport.scroll_x;
    let top = rect.y * zoom - viewport.scroll_y;
    format!(
        "display:block;left:{left}px;top:{top}px;width:{}px;height:{}px",
        rect.width * zoom,
        rect.height * zoom
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

fn parse_property_value(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_string()))
}

fn selected_patch_intent(
    controller: RwSignal<AdminCanvasController>,
    patch: ComponentPatch,
) -> Result<UiIntent, String> {
    let component_id = controller
        .with(|controller| controller.ui().state.selection.component_id.clone())
        .ok_or_else(|| "select a component before editing properties".to_string())?;
    Ok(UiIntent::execute(EditorCommand::Patch {
        component_id,
        patch,
    }))
}

#[component]
pub fn AdminCanvas(
    controller: AdminCanvasController,
    #[prop(optional)] facade: Option<Rc<dyn PageBuilderAdminFacade>>,
    #[prop(optional)] on_request: Option<Callback<PageBuilderCapabilityRequest>>,
) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let undo_label = t(locale.as_deref(), "page_builder.action.undo", "Undo");
    let redo_label = t(locale.as_deref(), "page_builder.action.redo", "Redo");
    let save_label = t(locale.as_deref(), "page_builder.action.save", "Save");
    let add_label = t(locale.as_deref(), "page_builder.action.add", "Add");
    let drag_label = t(locale.as_deref(), "page_builder.action.drag", "Drag");
    let move_label = t(locale.as_deref(), "page_builder.action.move", "Move selected");
    let remove_label = t(locale.as_deref(), "page_builder.action.remove", "Remove selected");
    let cancel_label = t(locale.as_deref(), "page_builder.action.cancelDrag", "Cancel drag");
    let apply_label = t(locale.as_deref(), "page_builder.action.apply", "Apply");
    let clear_label = t(locale.as_deref(), "page_builder.action.clear", "Clear");
    let saving_status = t(locale.as_deref(), "page_builder.status.saving", "Saving");
    let failed_status = t(
        locale.as_deref(),
        "page_builder.status.saveFailed",
        "Save failed",
    );
    let dirty_status = t(
        locale.as_deref(),
        "page_builder.status.dirty",
        "Unsaved changes",
    );
    let saved_status = t(locale.as_deref(), "page_builder.status.saved", "Saved");
    let save_succeeded = t(
        locale.as_deref(),
        "page_builder.status.saveSucceeded",
        "Project saved",
    );
    let palette_label = t(locale.as_deref(), "page_builder.panel.palette", "Blocks");
    let layers_label = t(locale.as_deref(), "page_builder.panel.layers", "Layers");
    let properties_label = t(locale.as_deref(), "page_builder.panel.properties", "Properties");
    let styles_label = t(locale.as_deref(), "page_builder.panel.styles", "Styles");
    let diagnostics_label = t(
        locale.as_deref(),
        "page_builder.panel.diagnostics",
        "Diagnostics",
    );
    let page_label = t(locale.as_deref(), "page_builder.field.page", "Page");
    let revision_label = t(
        locale.as_deref(),
        "page_builder.field.revision",
        "Revision",
    );
    let selected_label = t(
        locale.as_deref(),
        "page_builder.field.selectedComponent",
        "Selected component",
    );
    let none_label = t(locale.as_deref(), "page_builder.field.none", "None");
    let type_label = t(locale.as_deref(), "page_builder.field.type", "Type");
    let tag_label = t(locale.as_deref(), "page_builder.field.tagName", "Tag name");
    let content_label = t(locale.as_deref(), "page_builder.field.content", "Content");
    let attribute_name_label = t(
        locale.as_deref(),
        "page_builder.field.attributeName",
        "Attribute name",
    );
    let attribute_value_label = t(
        locale.as_deref(),
        "page_builder.field.attributeValue",
        "Attribute value or JSON",
    );
    let style_json_label = t(
        locale.as_deref(),
        "page_builder.field.styleJson",
        "Style JSON",
    );
    let project_hash_label = t(
        locale.as_deref(),
        "page_builder.field.projectHash",
        "Project hash",
    );
    let diagnostic_count_label = t(
        locale.as_deref(),
        "page_builder.diagnosticCount",
        "diagnostic(s)",
    );
    let facade_missing = t(
        locale.as_deref(),
        "page_builder.facadeMissing",
        "Page Builder admin facade is not mounted for this canvas",
    );
    let bridge_ready_label = t(
        locale.as_deref(),
        "page_builder.bridge.ready",
        "Canvas connected",
    );
    let bridge_waiting_label = t(
        locale.as_deref(),
        "page_builder.bridge.waiting",
        "Connecting canvas",
    );
    let pointer_label = t(
        locale.as_deref(),
        "page_builder.bridge.pointer",
        "Pointer",
    );
    let dragging_label = t(
        locale.as_deref(),
        "page_builder.status.dragging",
        "Choose a drop position in the canvas",
    );

    let instance_seed = format!(
        "{}-{}",
        dom_id(controller.page_id()),
        controller.editor().revision().project_hash.hex()
    );
    let instance_id = format!("fly-canvas-{instance_seed}");
    let iframe_id = format!("{instance_id}-frame");
    let controller = RwSignal::new(controller);
    let geometry = RwSignal::new(BTreeMap::<String, CanvasComponentGeometry>::new());
    let ready = RwSignal::new(false);
    let pointer = RwSignal::new(None::<String>);
    let last_error = RwSignal::new(None::<String>);
    let last_announcement = RwSignal::new(None::<String>);
    let attribute_name = RwSignal::new(String::new());
    let attribute_value = RwSignal::new(String::new());
    let tag_name = RwSignal::new(String::new());
    let content_value = RwSignal::new(String::new());
    let style_json = RwSignal::new("{}".to_string());
    let property_selection_id = RwSignal::new(None::<String>);

    Effect::new(move |_| {
        let selected = controller.with(|controller| controller.selected_component_view());
        let selected_id = selected.as_ref().map(|selected| selected.id.clone());
        if property_selection_id.get_untracked() == selected_id {
            return;
        }
        property_selection_id.set(selected_id);
        if let Some(selected) = selected {
            tag_name.set(selected.tag_name.unwrap_or_default());
            content_value.set(
                selected
                    .fields
                    .get("content")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            );
            style_json.set(
                selected
                    .style
                    .as_ref()
                    .and_then(|style| serde_json::to_string_pretty(style).ok())
                    .unwrap_or_else(|| "{}".to_string()),
            );
        } else {
            tag_name.set(String::new());
            content_value.set(String::new());
            style_json.set("{}".to_string());
        }
    });

    let canvas_srcdoc = Memo::new({
        let instance_id = instance_id.clone();
        move |_| {
            controller.with(|controller| {
                render_canvas_srcdoc(controller.editor().document(), &instance_id)
            })
        }
    });

    #[cfg(target_arch = "wasm32")]
    let bridge_subscription = StoredValue::new_local(
        None::<fly_leptos::IframeJsonSubscription>,
    );

    let on_iframe_load = {
        let iframe_id = iframe_id.clone();
        let expected_instance_id = instance_id.clone();
        move |_| {
            ready.set(false);
            geometry.set(BTreeMap::new());
            #[cfg(target_arch = "wasm32")]
            {
                bridge_subscription.set_value(None);
                let decoder_instance = expected_instance_id.clone();
                match fly_leptos::IframeJsonSubscription::subscribe_by_element_id(
                    iframe_id.clone(),
                    "null",
                    move |payload, last_sequence| {
                        decode_canvas_message(payload, &decoder_instance, last_sequence)
                    },
                    move |message| {
                        handle_canvas_message(
                            controller,
                            geometry,
                            ready,
                            pointer,
                            last_error,
                            last_announcement,
                            message,
                        )
                    },
                ) {
                    Ok(subscription) => bridge_subscription.set_value(Some(subscription)),
                    Err(error) => last_error.set(Some(error.to_string())),
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let _ = (&iframe_id, &expected_instance_id);
            }
        }
    };

    let undo_request = on_request.clone();
    let redo_request = on_request.clone();
    let save_request = on_request.clone();
    let undo_facade = facade.clone();
    let redo_facade = facade.clone();
    let save_facade = facade;
    let undo_facade_missing = facade_missing.clone();
    let redo_facade_missing = facade_missing.clone();
    let save_facade_missing = facade_missing;
    let undo_save_succeeded = save_succeeded.clone();
    let redo_save_succeeded = save_succeeded.clone();
    let save_save_succeeded = save_succeeded;
    let selected_none_label = none_label.clone();
    let count_label = diagnostic_count_label.clone();
    let hash_label = project_hash_label.clone();

    view! {
        <div class="rustok-page-builder-admin__workspace space-y-3">
            <div class="rustok-page-builder-admin__toolbar flex flex-wrap items-center gap-2 rounded-xl border border-border bg-card p-3" role="toolbar" aria-label="Page builder actions">
                <button
                    type="button"
                    class="rounded border border-border px-3 py-1.5 text-sm"
                    disabled=move || !controller.with(|controller| controller.can_undo())
                    on:click=move |_| dispatch_admin_intent(
                        controller,
                        last_error,
                        last_announcement,
                        undo_facade.clone(),
                        undo_request.clone(),
                        undo_facade_missing.clone(),
                        undo_save_succeeded.clone(),
                        UiIntent::Undo,
                    )
                >{undo_label}</button>
                <button
                    type="button"
                    class="rounded border border-border px-3 py-1.5 text-sm"
                    disabled=move || !controller.with(|controller| controller.can_redo())
                    on:click=move |_| dispatch_admin_intent(
                        controller,
                        last_error,
                        last_announcement,
                        redo_facade.clone(),
                        redo_request.clone(),
                        redo_facade_missing.clone(),
                        redo_save_succeeded.clone(),
                        UiIntent::Redo,
                    )
                >{redo_label}</button>
                <button
                    type="button"
                    class="rounded bg-primary px-3 py-1.5 text-sm text-primary-foreground"
                    disabled=move || controller.with(|controller| {
                        controller.ui().state.has_blocking_diagnostics()
                            || !controller.ui().state.dirty.dirty
                            || controller.ui().state.dirty.save_in_progress
                    })
                    on:click=move |_| dispatch_admin_intent(
                        controller,
                        last_error,
                        last_announcement,
                        save_facade.clone(),
                        save_request.clone(),
                        save_facade_missing.clone(),
                        save_save_succeeded.clone(),
                        UiIntent::RequestSave,
                    )
                >{save_label}</button>
                <button
                    type="button"
                    class="rounded border border-border px-3 py-1.5 text-sm"
                    disabled=move || controller.with(|controller| {
                        controller.selected_component_view().is_none_or(|selected| selected.is_root)
                    })
                    on:click=move |_| {
                        let intent = controller.with(|controller| controller.begin_selected_move_intent());
                        dispatch_result_intent(controller, last_error, last_announcement, intent);
                    }
                >{move_label}</button>
                <button
                    type="button"
                    class="rounded border border-destructive/40 px-3 py-1.5 text-sm text-destructive"
                    disabled=move || controller.with(|controller| {
                        controller.selected_component_view().is_none_or(|selected| selected.is_root)
                    })
                    on:click=move |_| {
                        let intent = controller.with(|controller| controller.remove_selected_intent());
                        dispatch_result_intent(controller, last_error, last_announcement, intent);
                    }
                >{remove_label}</button>
                <button
                    type="button"
                    class="rounded border border-border px-3 py-1.5 text-sm"
                    disabled=move || controller.with(|controller| controller.ui().state.drag.is_none())
                    on:click=move |_| dispatch_canvas_intent(
                        controller,
                        last_error,
                        last_announcement,
                        UiIntent::CancelDrag,
                    )
                >{cancel_label}</button>
                <span class="rustok-page-builder-admin__dirty-state ml-auto text-sm" aria-live="polite">
                    {move || controller.with(|controller| {
                        if controller.ui().state.dirty.save_in_progress {
                            saving_status.clone()
                        } else if controller.ui().state.dirty.save_failed {
                            failed_status.clone()
                        } else if controller.ui().state.dirty.dirty {
                            dirty_status.clone()
                        } else {
                            saved_status.clone()
                        }
                    })}
                </span>
                <span class="rustok-page-builder-admin__bridge-state text-sm text-muted-foreground" aria-live="polite">
                    {move || if ready.get() {
                        bridge_ready_label.clone()
                    } else {
                        bridge_waiting_label.clone()
                    }}
                </span>
            </div>

            <div class="rustok-page-builder-admin__layout grid min-h-[620px] gap-3" style="grid-template-columns:minmax(220px,280px) minmax(420px,1fr) minmax(260px,340px)">
                <aside class="rustok-page-builder-admin__panel space-y-4 overflow-auto rounded-xl border border-border bg-card p-3">
                    <section class="space-y-2">
                        <h2 class="font-semibold">{palette_label}</h2>
                        <div class="space-y-2">
                            {move || controller.with(|controller| controller.palette_blocks()).into_iter().map(|block| {
                                let insert_id = block.id.clone();
                                let drag_id = block.id.clone();
                                let html_drag_id = block.id.clone();
                                view! {
                                    <div
                                        class="rounded-lg border border-border p-2"
                                        draggable="true"
                                        on:dragstart=move |_| {
                                            let intent = controller.with(|controller| controller.begin_palette_drag_intent(&html_drag_id));
                                            dispatch_result_intent(controller, last_error, last_announcement, intent);
                                        }
                                    >
                                        <div class="text-sm font-medium">{block.label}</div>
                                        <div class="text-xs text-muted-foreground">{block.category}</div>
                                        <div class="mt-2 flex gap-2">
                                            <button
                                                type="button"
                                                class="rounded border border-border px-2 py-1 text-xs"
                                                on:click=move |_| {
                                                    let intent = controller.with(|controller| controller.insert_palette_block_intent(&insert_id));
                                                    dispatch_result_intent(controller, last_error, last_announcement, intent);
                                                }
                                            >{add_label.clone()}</button>
                                            <button
                                                type="button"
                                                class="rounded border border-border px-2 py-1 text-xs"
                                                on:click=move |_| {
                                                    let intent = controller.with(|controller| controller.begin_palette_drag_intent(&drag_id));
                                                    dispatch_result_intent(controller, last_error, last_announcement, intent);
                                                }
                                            >{drag_label.clone()}</button>
                                        </div>
                                    </div>
                                }
                            }).collect_view()}
                        </div>
                    </section>

                    <section class="space-y-2 border-t border-border pt-3">
                        <h2 class="font-semibold">{layers_label}</h2>
                        <div class="space-y-1">
                            {move || {
                                let selected = controller.with(|controller| controller.ui().state.selection.component_id.clone());
                                controller.with(|controller| controller.layer_items()).into_iter().map(|layer| {
                                    let component_id = layer.id.clone();
                                    let active = selected.as_deref() == Some(layer.id.as_str());
                                    view! {
                                        <button
                                            type="button"
                                            class=if active {
                                                "block w-full rounded bg-primary/10 px-2 py-1 text-left text-sm text-primary"
                                            } else {
                                                "block w-full rounded px-2 py-1 text-left text-sm hover:bg-muted"
                                            }
                                            style=format!("padding-left:{}px", 8 + layer.depth * 14)
                                            on:click=move |_| dispatch_canvas_intent(
                                                controller,
                                                last_error,
                                                last_announcement,
                                                UiIntent::Select(Some(component_id.clone())),
                                            )
                                        >
                                            <span class="font-medium">{layer.component_type}</span>
                                            <span class="ml-1 text-xs text-muted-foreground">{layer.id}</span>
                                        </button>
                                    }
                                }).collect_view()
                            }}
                        </div>
                    </section>
                </aside>

                <main class="rustok-page-builder-admin__canvas relative overflow-hidden rounded-xl border border-border bg-white" aria-label="Isolated page canvas">
                    <iframe
                        id=iframe_id
                        title="Fly page canvas"
                        sandbox="allow-scripts"
                        srcdoc=move || canvas_srcdoc.get()
                        data-fly-iframe-canvas="true"
                        on:load=on_iframe_load
                        style="display:block;width:100%;min-height:620px;border:0;background:#fff"
                    ></iframe>
                    <div
                        aria-hidden="true"
                        class="rustok-page-builder-admin__hover-overlay"
                        style=move || controller.with(|controller| {
                            format!(
                                "{};position:absolute;pointer-events:none;border:1px dashed rgba(59,130,246,.75)",
                                overlay_style(
                                    controller.ui().state.overlays.hovered,
                                    controller.ui().state.viewport,
                                )
                            )
                        })
                    ></div>
                    <div
                        aria-hidden="true"
                        class="rustok-page-builder-admin__selection-overlay"
                        style=move || controller.with(|controller| {
                            format!(
                                "{};position:absolute;pointer-events:none;border:2px solid #2563eb;box-shadow:0 0 0 1px rgba(255,255,255,.8)",
                                overlay_style(
                                    controller.ui().state.overlays.selected,
                                    controller.ui().state.viewport,
                                )
                            )
                        })
                    ></div>
                    <div
                        aria-hidden="true"
                        class="rustok-page-builder-admin__insertion-overlay"
                        style=move || controller.with(|controller| {
                            format!(
                                "{};position:absolute;pointer-events:none;border:3px solid #16a34a;background:rgba(22,163,74,.08)",
                                overlay_style(
                                    controller.ui().state.overlays.insertion,
                                    controller.ui().state.viewport,
                                )
                            )
                        })
                    ></div>
                    <div
                        class="absolute bottom-3 left-3 rounded bg-background/90 px-2 py-1 text-xs shadow"
                        class:hidden=move || controller.with(|controller| controller.ui().state.drag.is_none())
                    >{dragging_label}</div>
                </main>

                <aside class="rustok-page-builder-admin__panel space-y-4 overflow-auto rounded-xl border border-border bg-card p-3">
                    <section class="space-y-2">
                        <h2 class="font-semibold">{properties_label}</h2>
                        <dl class="grid grid-cols-[auto_1fr] gap-x-2 gap-y-1 text-sm">
                            <dt class="text-muted-foreground">{page_label}</dt>
                            <dd>{move || controller.with(|controller| controller.page_id().to_string())}</dd>
                            <dt class="text-muted-foreground">{revision_label}</dt>
                            <dd class="break-all">{move || controller.with(|controller| controller.revision_id().to_string())}</dd>
                            <dt class="text-muted-foreground">{selected_label}</dt>
                            <dd class="break-all">{move || controller.with(|controller| {
                                controller
                                    .ui()
                                    .state
                                    .selection
                                    .component_id
                                    .clone()
                                    .unwrap_or_else(|| selected_none_label.clone())
                            })}</dd>
                            <dt class="text-muted-foreground">{type_label}</dt>
                            <dd>{move || controller.with(|controller| {
                                controller.selected_component_view().map(|selected| selected.component_type).unwrap_or_default()
                            })}</dd>
                        </dl>
                    </section>

                    <section class="space-y-2 border-t border-border pt-3">
                        <label class="block text-sm font-medium">{tag_label}</label>
                        <div class="flex gap-2">
                            <input
                                class="min-w-0 flex-1 rounded border border-input bg-background px-2 py-1 text-sm"
                                prop:value=move || tag_name.get()
                                on:input=move |event| tag_name.set(event_target_value(&event))
                            />
                            <button
                                type="button"
                                class="rounded border border-border px-2 py-1 text-xs"
                                on:click=move |_| {
                                    let value = tag_name.get_untracked();
                                    let patch = if value.trim().is_empty() {
                                        ComponentPatch { remove_fields: vec!["tagName".to_string()], ..ComponentPatch::default() }
                                    } else {
                                        ComponentPatch { fields: Map::from_iter([("tagName".to_string(), Value::String(value))]), ..ComponentPatch::default() }
                                    };
                                    let intent = selected_patch_intent(controller, patch);
                                    dispatch_result_intent(controller, last_error, last_announcement, intent);
                                }
                            >{apply_label.clone()}</button>
                        </div>

                        <label class="block text-sm font-medium">{content_label}</label>
                        <textarea
                            class="min-h-24 w-full rounded border border-input bg-background px-2 py-1 text-sm"
                            prop:value=move || content_value.get()
                            on:input=move |event| content_value.set(event_target_value(&event))
                        ></textarea>
                        <div class="flex gap-2">
                            <button
                                type="button"
                                class="rounded border border-border px-2 py-1 text-xs"
                                on:click=move |_| {
                                    let patch = ComponentPatch {
                                        fields: Map::from_iter([(
                                            "content".to_string(),
                                            Value::String(content_value.get_untracked()),
                                        )]),
                                        ..ComponentPatch::default()
                                    };
                                    let intent = selected_patch_intent(controller, patch);
                                    dispatch_result_intent(controller, last_error, last_announcement, intent);
                                }
                            >{apply_label.clone()}</button>
                            <button
                                type="button"
                                class="rounded border border-border px-2 py-1 text-xs"
                                on:click=move |_| {
                                    let intent = selected_patch_intent(controller, ComponentPatch {
                                        remove_fields: vec!["content".to_string()],
                                        ..ComponentPatch::default()
                                    });
                                    dispatch_result_intent(controller, last_error, last_announcement, intent);
                                }
                            >{clear_label.clone()}</button>
                        </div>
                    </section>

                    <section class="space-y-2 border-t border-border pt-3">
                        <div class="text-sm font-medium">Attributes</div>
                        <input
                            aria-label=attribute_name_label.clone()
                            placeholder=attribute_name_label.clone()
                            class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                            prop:value=move || attribute_name.get()
                            on:input=move |event| attribute_name.set(event_target_value(&event))
                        />
                        <input
                            aria-label=attribute_value_label.clone()
                            placeholder=attribute_value_label.clone()
                            class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                            prop:value=move || attribute_value.get()
                            on:input=move |event| attribute_value.set(event_target_value(&event))
                        />
                        <button
                            type="button"
                            class="rounded border border-border px-2 py-1 text-xs"
                            on:click=move |_| {
                                let name = attribute_name.get_untracked().trim().to_string();
                                if name.is_empty() {
                                    last_error.set(Some("attribute name must not be empty".to_string()));
                                    return;
                                }
                                let value = parse_property_value(&attribute_value.get_untracked());
                                let intent = selected_patch_intent(controller, ComponentPatch {
                                    attributes: Map::from_iter([(name, value)]),
                                    ..ComponentPatch::default()
                                });
                                dispatch_result_intent(controller, last_error, last_announcement, intent);
                            }
                        >{apply_label.clone()}</button>
                        <div class="space-y-1">
                            {move || controller.with(|controller| controller.selected_component_view()).map(|selected| {
                                selected.attributes.into_iter().map(|(name, value)| {
                                    let remove_name = name.clone();
                                    view! {
                                        <div class="flex items-start gap-2 rounded bg-muted/50 px-2 py-1 text-xs">
                                            <code class="min-w-0 flex-1 break-all">{format!("{name}={value}")}</code>
                                            <button
                                                type="button"
                                                class="text-destructive"
                                                on:click=move |_| {
                                                    let intent = selected_patch_intent(controller, ComponentPatch {
                                                        remove_attributes: vec![remove_name.clone()],
                                                        ..ComponentPatch::default()
                                                    });
                                                    dispatch_result_intent(controller, last_error, last_announcement, intent);
                                                }
                                            >{clear_label.clone()}</button>
                                        </div>
                                    }
                                }).collect_view()
                            })}
                        </div>
                    </section>

                    <section class="space-y-2 border-t border-border pt-3">
                        <h2 class="font-semibold">{styles_label}</h2>
                        <label class="sr-only">{style_json_label}</label>
                        <textarea
                            aria-label=style_json_label
                            class="min-h-32 w-full rounded border border-input bg-background px-2 py-1 font-mono text-xs"
                            prop:value=move || style_json.get()
                            on:input=move |event| style_json.set(event_target_value(&event))
                        ></textarea>
                        <div class="flex gap-2">
                            <button
                                type="button"
                                class="rounded border border-border px-2 py-1 text-xs"
                                on:click=move |_| {
                                    match serde_json::from_str::<Value>(&style_json.get_untracked()) {
                                        Ok(style) => {
                                            let intent = selected_patch_intent(controller, ComponentPatch {
                                                style: Some(style),
                                                ..ComponentPatch::default()
                                            });
                                            dispatch_result_intent(controller, last_error, last_announcement, intent);
                                        }
                                        Err(error) => last_error.set(Some(format!("invalid style JSON: {error}"))),
                                    }
                                }
                            >{apply_label}</button>
                            <button
                                type="button"
                                class="rounded border border-border px-2 py-1 text-xs"
                                on:click=move |_| {
                                    let intent = selected_patch_intent(controller, ComponentPatch {
                                        clear_style: true,
                                        ..ComponentPatch::default()
                                    });
                                    dispatch_result_intent(controller, last_error, last_announcement, intent);
                                }
                            >{clear_label}</button>
                        </div>
                    </section>

                    <section class="space-y-1 border-t border-border pt-3 text-sm">
                        <h2 class="font-semibold">{diagnostics_label}</h2>
                        <p>{move || controller.with(|controller| {
                            format!("{} {count_label}", controller.ui().state.diagnostics.len())
                        })}</p>
                        <p class="break-all">{move || controller.with(|controller| {
                            format!("{hash_label}: {}", controller.editor().revision().project_hash.hex())
                        })}</p>
                        <p>{move || pointer.get().map(|position| {
                            format!("{pointer_label}: {position}")
                        }).unwrap_or_default()}</p>
                        <div class="space-y-1">
                            {move || controller.with(|controller| controller.ui().state.diagnostics.clone()).into_iter().map(|diagnostic| view! {
                                <div class="rounded bg-muted/50 px-2 py-1 text-xs">
                                    <strong>{diagnostic.code}</strong>
                                    <div>{diagnostic.message}</div>
                                </div>
                            }).collect_view()}
                        </div>
                    </section>
                </aside>
            </div>

            <div class="rustok-page-builder-admin__messages" aria-live="polite">
                {move || last_announcement.get().map(|message| view! {
                    <p class="rustok-page-builder-admin__announcement rounded bg-muted px-3 py-2 text-sm">{message}</p>
                })}
                {move || last_error.get().map(|message| view! {
                    <p class="rustok-page-builder-admin__error rounded bg-destructive/10 px-3 py-2 text-sm text-destructive" role="alert">{message}</p>
                })}
            </div>
        </div>
    }
}
