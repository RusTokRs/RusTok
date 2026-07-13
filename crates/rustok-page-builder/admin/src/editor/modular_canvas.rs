use crate::editor::{
    AdminEditorRuntime, AuthoringToolbar, IsolatedAuthoringCanvas, PaletteLayersPanel,
    PropertiesAssetsPanel,
};
use crate::i18n::t;
use crate::{AdminCanvasController, PageBuilderAdminFacade};
use leptos::prelude::*;
use rustok_page_builder::dto::PageBuilderCapabilityRequest;
use rustok_ui_core::UiRouteContext;
use std::sync::Arc;

#[component]
pub fn AdminCanvas(
    controller: AdminCanvasController,
    #[prop(optional)] facade: Option<Arc<dyn PageBuilderAdminFacade>>,
    #[prop(optional)] on_request: Option<Callback<PageBuilderCapabilityRequest>>,
) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let facade_missing = t(
        locale.as_deref(),
        "page_builder.facadeMissing",
        "Page Builder admin facade is not mounted for this canvas",
    );
    let save_succeeded = t(
        locale.as_deref(),
        "page_builder.status.saveSucceeded",
        "Project saved",
    );
    let runtime = AdminEditorRuntime::new(
        controller,
        facade,
        on_request,
        facade_missing,
        save_succeeded,
    );

    view! {
        <div class="rustok-page-builder-admin__workspace space-y-3">
            <AuthoringToolbar runtime=runtime.clone() />
            <div
                class="grid min-h-[680px] gap-3"
                style="grid-template-columns:minmax(220px,280px) minmax(420px,1fr) minmax(280px,360px)"
            >
                <PaletteLayersPanel runtime=runtime.clone() />
                <IsolatedAuthoringCanvas runtime=runtime.clone() />
                <PropertiesAssetsPanel runtime=runtime.clone() />
            </div>
            <div class="space-y-2" aria-live="polite">
                {move || runtime.last_announcement.get().map(|message| view! {
                    <p class="rounded bg-muted px-3 py-2 text-sm">{message}</p>
                })}
                {move || runtime.last_error.get().map(|message| view! {
                    <p class="rounded bg-destructive/10 px-3 py-2 text-sm text-destructive" role="alert">{message}</p>
                })}
            </div>
        </div>
    }
}
