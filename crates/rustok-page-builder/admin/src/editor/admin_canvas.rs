use crate::editor::{decode_canvas_message, render_canvas_srcdoc, CanvasBridgeMessage};
use crate::i18n::t;
use crate::{AdminCanvasController, AdminCanvasEffect, PageBuilderAdminFacade};
use fly::ProjectHash;
use fly_leptos::{BrowserRect, CoordinateTransform};
use fly_ui::{CanvasRect, UiIntent, ViewportState};
use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_page_builder::dto::{
    PageBuilderCapabilityRequest, PageBuilderCapabilityResponse,
};
use rustok_ui_core::UiRouteContext;
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
    let mut announcement = None;
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
                    AdminCanvasEffect::Announce(message) => announcement = Some(message),
                }
            }
        }
        Err(dispatch_error) => error = Some(dispatch_error.to_string()),
    });

    last_error.set(error);
    if let Some(message) = announcement {
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
    intent: UiIntent,
) {
    let mut error = None;
    controller.update(|controller| {
        if let Err(dispatch_error) = controller.dispatch(intent) {
            error = Some(dispatch_error.to_string());
        }
    });
    if error.is_some() {
        last_error.set(error);
    }
}

fn canvas_rect(rect: BrowserRect, viewport: ViewportState) -> CanvasRect {
    rect.to_canvas_rect(CoordinateTransform {
        scroll_x: viewport.scroll_x,
        scroll_y: viewport.scroll_y,
        zoom: f64::from(viewport.zoom),
        ..CoordinateTransform::default()
    })
}

fn synchronize_overlays(
    controller: RwSignal<AdminCanvasController>,
    geometry: RwSignal<BTreeMap<String, BrowserRect>>,
    last_error: RwSignal<Option<String>>,
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
            .with(|geometry| geometry.get(&id).copied())
            .map(|rect| canvas_rect(rect, viewport))
    });
    let hovered = hovered.and_then(|id| {
        geometry
            .with(|geometry| geometry.get(&id).copied())
            .map(|rect| canvas_rect(rect, viewport))
    });
    dispatch_canvas_intent(controller, last_error, UiIntent::SetSelectedOverlay(selected));
    dispatch_canvas_intent(controller, last_error, UiIntent::SetHoveredOverlay(hovered));
}

fn handle_canvas_message(
    controller: RwSignal<AdminCanvasController>,
    geometry: RwSignal<BTreeMap<String, BrowserRect>>,
    ready: RwSignal<bool>,
    pointer: RwSignal<Option<String>>,
    last_error: RwSignal<Option<String>>,
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
                UiIntent::SetViewport(ViewportState {
                    width,
                    height,
                    zoom: zoom as f32,
                    scroll_x,
                    scroll_y,
                }),
            );
            synchronize_overlays(controller, geometry, last_error);
        }
        CanvasBridgeMessage::GeometrySnapshot { components } => {
            geometry.set(
                components
                    .into_iter()
                    .map(|component| (component.component_id, component.rect))
                    .collect(),
            );
            synchronize_overlays(controller, geometry, last_error);
        }
        CanvasBridgeMessage::PointerMoved { sample } => {
            pointer.set(Some(format!(
                "{:.0}, {:.0}",
                sample.position.x, sample.position.y
            )));
        }
        CanvasBridgeMessage::FocusRequested { component_id } => {
            dispatch_canvas_intent(controller, last_error, UiIntent::Select(component_id));
            synchronize_overlays(controller, geometry, last_error);
        }
        CanvasBridgeMessage::HoverRequested { component_id } => {
            dispatch_canvas_intent(controller, last_error, UiIntent::Hover(component_id));
            synchronize_overlays(controller, geometry, last_error);
        }
        CanvasBridgeMessage::Teardown => {
            ready.set(false);
            geometry.set(BTreeMap::new());
            dispatch_canvas_intent(controller, last_error, UiIntent::SetSelectedOverlay(None));
            dispatch_canvas_intent(controller, last_error, UiIntent::SetHoveredOverlay(None));
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
    let layers_label = t(
        locale.as_deref(),
        "page_builder.panel.layers",
        "Layers and selection",
    );
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

    let instance_seed = format!(
        "{}-{}",
        dom_id(controller.page_id()),
        controller.editor().revision().project_hash.hex()
    );
    let instance_id = format!("fly-canvas-{instance_seed}");
    let iframe_id = format!("{instance_id}-frame");
    let controller = RwSignal::new(controller);
    let geometry = RwSignal::new(BTreeMap::<String, BrowserRect>::new());
    let ready = RwSignal::new(false);
    let pointer = RwSignal::new(None::<String>);
    let last_error = RwSignal::new(None::<String>);
    let last_announcement = RwSignal::new(None::<String>);
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
        <div class="rustok-page-builder-admin__workspace">
            <div class="rustok-page-builder-admin__toolbar" role="toolbar" aria-label="Page builder actions">
                <button
                    type="button"
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
                >
                    {undo_label}
                </button>
                <button
                    type="button"
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
                >
                    {redo_label}
                </button>
                <button
                    type="button"
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
                >
                    {save_label}
                </button>
                <span class="rustok-page-builder-admin__dirty-state" aria-live="polite">
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
                <span class="rustok-page-builder-admin__bridge-state" aria-live="polite">
                    {move || if ready.get() {
                        bridge_ready_label.clone()
                    } else {
                        bridge_waiting_label.clone()
                    }}
                </span>
            </div>

            <div class="rustok-page-builder-admin__layout">
                <aside class="rustok-page-builder-admin__panel" aria-label="Fly editor state">
                    <h2>{layers_label}</h2>
                    <dl>
                        <dt>{page_label}</dt>
                        <dd>{move || controller.with(|controller| controller.page_id().to_string())}</dd>
                        <dt>{revision_label}</dt>
                        <dd>{move || controller.with(|controller| controller.revision_id().to_string())}</dd>
                        <dt>{selected_label}</dt>
                        <dd>{move || controller.with(|controller| {
                            controller
                                .ui()
                                .state
                                .selection
                                .component_id
                                .clone()
                                .unwrap_or_else(|| selected_none_label.clone())
                        })}</dd>
                    </dl>
                </aside>

                <main class="rustok-page-builder-admin__canvas" aria-label="Isolated page canvas" style="position:relative;overflow:hidden">
                    <iframe
                        id=iframe_id
                        title="Fly page canvas"
                        sandbox="allow-scripts"
                        srcdoc=move || canvas_srcdoc.get()
                        data-fly-iframe-canvas="true"
                        on:load=on_iframe_load
                        style="display:block;width:100%;min-height:520px;border:0;background:#fff"
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
                </main>

                <aside class="rustok-page-builder-admin__panel" aria-label="Validation diagnostics">
                    <h2>{diagnostics_label}</h2>
                    <p>{move || controller.with(|controller| {
                        format!("{} {count_label}", controller.ui().state.diagnostics.len())
                    })}</p>
                    <p>{move || controller.with(|controller| {
                        format!("{hash_label}: {}", controller.editor().revision().project_hash.hex())
                    })}</p>
                    <p>{move || pointer.get().map(|position| {
                        format!("{pointer_label}: {position}")
                    }).unwrap_or_default()}</p>
                </aside>
            </div>

            <div class="rustok-page-builder-admin__messages" aria-live="polite">
                {move || last_announcement.get().map(|message| view! {
                    <p class="rustok-page-builder-admin__announcement">{message}</p>
                })}
                {move || last_error.get().map(|message| view! {
                    <p class="rustok-page-builder-admin__error" role="alert">{message}</p>
                })}
            </div>
        </div>
    }
}
