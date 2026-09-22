use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

#[component]
pub(crate) fn DiagnosticsSection(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let diagnostics_label = t(
        locale.as_deref(),
        "page_builder.panel.diagnostics",
        "Diagnostics",
    );
    let project_hash_label = t(
        locale.as_deref(),
        "page_builder.field.projectHash",
        "Project hash",
    );
    let hash_runtime = runtime.clone();
    let diagnostics_runtime = runtime;

    view! {
        <section class="space-y-1 border-t border-border pt-3 text-sm">
            <h2 class="font-semibold">{diagnostics_label}</h2>
            <p class="break-all">{move || hash_runtime.controller.with(|controller| {
                format!(
                    "{project_hash_label}: {}",
                    controller.editor().revision().project_hash.hex(),
                )
            })}</p>
            {move || diagnostics_runtime
                .controller
                .with(|controller| controller.ui().state.diagnostics.clone())
                .into_iter()
                .map(|diagnostic| view! {
                    <div class="rounded bg-muted/50 px-2 py-1 text-xs">
                        <strong>{diagnostic.code}</strong>
                        <div>{diagnostic.message}</div>
                    </div>
                })
                .collect_view()}
        </section>
    }
}
