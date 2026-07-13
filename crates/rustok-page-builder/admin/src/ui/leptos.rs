use crate::editor::{AdminCanvas, AdminShell};
use crate::i18n::t;
use crate::AdminCanvasController;
use leptos::prelude::*;
use rustok_page_builder::dto::PageBuilderCapabilityRequest;
use rustok_ui_core::UiRouteContext;

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
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;

    match use_context::<PageBuilderAdminHostContext>() {
        Some(context) => view! {
            <PageBuilderAdminWithController
                controller=context.controller
                on_request=context.on_request
            />
        }
        .into_any(),
        None => {
            let title = t(locale.as_deref(), "page_builder.title", "Page Builder");
            let subtitle = t(
                locale.as_deref(),
                "page_builder.subtitle",
                "Fly runtime, compatibility, and provider control surface.",
            );
            let unbound_title = t(
                locale.as_deref(),
                "page_builder.unbound.title",
                "No consumer document selected",
            );
            let unbound_body = t(
                locale.as_deref(),
                "page_builder.unbound.body",
                "Open a consumer-owned document to start full visual authoring. Page Builder does not own document persistence.",
            );

            view! {
                <AdminShell title subtitle=Some(subtitle)>
                    <section class="rustok-page-builder-admin__unbound" role="status">
                        <h2>{unbound_title}</h2>
                        <p>{unbound_body}</p>
                    </section>
                </AdminShell>
            }
            .into_any()
        }
    }
}

#[component]
pub fn PageBuilderAdminWithController(
    controller: AdminCanvasController,
    #[prop(optional)] on_request: Option<Callback<PageBuilderCapabilityRequest>>,
) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let title_prefix = t(locale.as_deref(), "page_builder.title", "Page Builder");
    let title = format!("{title_prefix}: {}", controller.page_id());
    let subtitle = Some(t(
        locale.as_deref(),
        "page_builder.editorSubtitle",
        "Full Fly authoring surface. Persistence remains owned by the consumer module facade.",
    ));

    view! {
        <AdminShell title subtitle>
            <AdminCanvas controller on_request />
        </AdminShell>
    }
}
