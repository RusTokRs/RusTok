use crate::{AdminCanvasController, AdminCanvasEffect};
use fly_ui::UiIntent;
use leptos::prelude::*;
use rustok_page_builder::dto::PageBuilderCapabilityRequest;

const ADMIN_CANVAS_SRCDOC: &str = r#"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'">
<meta name="viewport" content="width=device-width, initial-scale=1">
<style>
html,body{height:100%;margin:0}body{display:grid;place-items:center;background:#fff;color:#667085;font:14px system-ui,sans-serif}
#fly-canvas-root{padding:24px;border:1px dashed #98a2b3;border-radius:12px;text-align:center}
</style>
</head>
<body><div id="fly-canvas-root" data-fly-canvas-root>Fly isolated canvas</div></body>
</html>"#;

fn dispatch_admin_intent(
    controller: RwSignal<AdminCanvasController>,
    last_error: RwSignal<Option<String>>,
    last_announcement: RwSignal<Option<String>>,
    on_request: Option<Callback<PageBuilderCapabilityRequest>>,
    intent: UiIntent,
) {
    let mut requests = Vec::new();
    let mut announcement = None;
    let mut error = None;

    controller.update(|controller| match controller.dispatch(intent) {
        Ok(effects) => {
            for effect in effects {
                match effect {
                    AdminCanvasEffect::Request { request, .. } => requests.push(request),
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

    for request in requests {
        if let Some(callback) = on_request.as_ref() {
            callback.run(request);
        } else {
            last_error.set(Some(
                "Page Builder admin facade is not mounted for this canvas".to_string(),
            ));
        }
    }
}

#[component]
pub fn AdminCanvas(
    controller: AdminCanvasController,
    #[prop(optional)] on_request: Option<Callback<PageBuilderCapabilityRequest>>,
) -> impl IntoView {
    let controller = RwSignal::new(controller);
    let last_error = RwSignal::new(None::<String>);
    let last_announcement = RwSignal::new(None::<String>);

    let undo_request = on_request.clone();
    let redo_request = on_request.clone();
    let save_request = on_request.clone();

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
                        undo_request.clone(),
                        UiIntent::Undo,
                    )
                >
                    "Undo"
                </button>
                <button
                    type="button"
                    disabled=move || !controller.with(|controller| controller.can_redo())
                    on:click=move |_| dispatch_admin_intent(
                        controller,
                        last_error,
                        last_announcement,
                        redo_request.clone(),
                        UiIntent::Redo,
                    )
                >
                    "Redo"
                </button>
                <button
                    type="button"
                    disabled=move || controller.with(|controller| {
                        controller.ui().state.has_blocking_diagnostics()
                            || !controller.ui().state.dirty.dirty
                    })
                    on:click=move |_| dispatch_admin_intent(
                        controller,
                        last_error,
                        last_announcement,
                        save_request.clone(),
                        UiIntent::RequestSave,
                    )
                >
                    "Save"
                </button>
                <span class="rustok-page-builder-admin__dirty-state" aria-live="polite">
                    {move || controller.with(|controller| {
                        if controller.ui().state.dirty.save_in_progress {
                            "Saving"
                        } else if controller.ui().state.dirty.save_failed {
                            "Save failed"
                        } else if controller.ui().state.dirty.dirty {
                            "Unsaved changes"
                        } else {
                            "Saved"
                        }
                    })}
                </span>
            </div>

            <div class="rustok-page-builder-admin__layout">
                <aside class="rustok-page-builder-admin__panel" aria-label="Fly editor state">
                    <h2>"Layers and selection"</h2>
                    <dl>
                        <dt>"Page"</dt>
                        <dd>{move || controller.with(|controller| controller.page_id().to_string())}</dd>
                        <dt>"Revision"</dt>
                        <dd>{move || controller.with(|controller| controller.revision_id().to_string())}</dd>
                        <dt>"Selected component"</dt>
                        <dd>{move || controller.with(|controller| {
                            controller
                                .ui()
                                .state
                                .selection
                                .component_id
                                .clone()
                                .unwrap_or_else(|| "None".to_string())
                        })}</dd>
                    </dl>
                </aside>

                <main class="rustok-page-builder-admin__canvas" aria-label="Isolated page canvas">
                    <iframe
                        title="Fly page canvas"
                        sandbox="allow-scripts"
                        srcdoc=ADMIN_CANVAS_SRCDOC
                        data-fly-iframe-canvas="true"
                    ></iframe>
                </main>

                <aside class="rustok-page-builder-admin__panel" aria-label="Validation diagnostics">
                    <h2>"Diagnostics"</h2>
                    <p>{move || controller.with(|controller| {
                        format!("{} diagnostic(s)", controller.ui().state.diagnostics.len())
                    })}</p>
                    <p>{move || controller.with(|controller| {
                        format!("Project hash: {}", controller.editor().revision().project_hash.hex())
                    })}</p>
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
