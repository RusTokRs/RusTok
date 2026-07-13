use crate::editor::{AdminCanvas, AdminShell};
use crate::AdminCanvasController;
use leptos::prelude::*;
use rustok_page_builder::dto::PageBuilderCapabilityRequest;

/// Host-provided composition context for a concrete consumer document.
///
/// Generated module composition mounts [`PageBuilderAdmin`] without props. Consumer routes such as
/// Pages may provide this context to activate a concrete document and its FFA request callback.
#[derive(Clone)]
pub struct PageBuilderAdminHostContext {
    pub controller: AdminCanvasController,
    pub on_request: Option<Callback<PageBuilderCapabilityRequest>>,
}

impl PageBuilderAdminHostContext {
    pub fn new(controller: AdminCanvasController) -> Self {
        Self {
            controller,
            on_request: None,
        }
    }

    pub fn with_request_callback(
        mut self,
        callback: Callback<PageBuilderCapabilityRequest>,
    ) -> Self {
        self.on_request = Some(callback);
        self
    }
}

/// Generated host entrypoint. It intentionally accepts no props.
///
/// Without a consumer-owned document context the control-plane route remains useful and explicit:
/// it explains that document lifecycle belongs to Pages/Blog/Forum rather than fabricating an
/// unpersisted page inside the generic Page Builder module.
#[component]
pub fn PageBuilderAdmin() -> impl IntoView {
    match use_context::<PageBuilderAdminHostContext>() {
        Some(context) => view! {
            <PageBuilderAdminWithController
                controller=context.controller
                on_request=context.on_request
            />
        }
        .into_any(),
        None => view! {
            <AdminShell
                title="Page Builder".to_string()
                subtitle=Some(
                    "Fly runtime, compatibility, and provider control surface.".to_string(),
                )
            >
                <section class="rustok-page-builder-admin__unbound" role="status">
                    <h2>"No consumer document selected"</h2>
                    <p>
                        "Open a Pages, Blog, Forum, or another consumer-owned document to start "
                        "full visual authoring. Page Builder does not own document persistence."
                    </p>
                </section>
            </AdminShell>
        }
        .into_any(),
    }
}

#[component]
pub fn PageBuilderAdminWithController(
    controller: AdminCanvasController,
    #[prop(optional)] on_request: Option<Callback<PageBuilderCapabilityRequest>>,
) -> impl IntoView {
    let title = format!("Edit {}", controller.page_id());
    let subtitle = Some(
        "Full Fly authoring surface. Persistence remains owned by the consumer module facade."
            .to_string(),
    );

    view! {
        <AdminShell title subtitle>
            <AdminCanvas controller on_request />
        </AdminShell>
    }
}
