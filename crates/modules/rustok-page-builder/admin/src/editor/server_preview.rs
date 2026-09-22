use crate::{PageBuilderAdminProviderStatus, editor::AdminEditorRuntime};
use leptos::prelude::*;

#[component]
pub fn ServerPreviewPanel(
    runtime: AdminEditorRuntime,
    provider_status: Option<PageBuilderAdminProviderStatus>,
) -> impl IntoView {
    let busy_runtime = runtime.clone();
    let disabled_provider_status = provider_status.clone();
    let request_runtime = runtime.clone();
    let request_provider_status = provider_status.clone();
    let status_runtime = runtime.clone();
    let frame_runtime = runtime;
    let provider_preview_enabled = provider_status
        .as_ref()
        .is_none_or(PageBuilderAdminProviderStatus::preview_enabled);

    view! {
        <section
            class="rounded-xl border border-border bg-card p-3"
            data-page-builder-server-preview="true"
            data-page-builder-provider-preview=provider_preview_enabled.to_string()
        >
            <div class="flex flex-wrap items-center justify-between gap-3">
                <div>
                    <h2 class="text-sm font-semibold text-card-foreground">"Server preview"</h2>
                    <p class="mt-1 text-xs text-muted-foreground">
                        "Rendered by the canonical Page Builder server pipeline."
                    </p>
                </div>
                <button
                    type="button"
                    class="rounded border border-border px-3 py-1.5 text-sm disabled:opacity-50"
                    disabled=move || {
                        disabled_provider_status
                            .as_ref()
                            .is_some_and(|status| !status.preview_enabled())
                            || busy_runtime.preview_in_progress.get()
                            || busy_runtime.controller.with(|controller| {
                                controller.ui().state.has_blocking_diagnostics()
                            })
                    }
                    on:click=move |_| {
                        if request_provider_status
                            .as_ref()
                            .is_some_and(|status| !status.preview_enabled())
                        {
                            request_runtime.fail(
                                "Server preview is unavailable under the current Page Builder provider status",
                            );
                            return;
                        }
                        request_runtime.request_server_preview();
                    }
                >
                    {move || if status_runtime.preview_in_progress.get() {
                        "Rendering..."
                    } else {
                        "Refresh preview"
                    }}
                </button>
            </div>
            {move || match frame_runtime.server_preview_html.get() {
                Some(html) => view! {
                    <iframe
                        class="mt-3 min-h-[360px] w-full rounded-lg border border-border bg-white"
                        title="Server-rendered page preview"
                        sandbox=""
                        srcdoc=html
                        data-page-builder-server-preview-frame="true"
                    ></iframe>
                }.into_any(),
                None => view! {
                    <div
                        class="mt-3 grid min-h-[180px] place-items-center rounded-lg border border-dashed border-border bg-muted/30 px-4 text-center text-sm text-muted-foreground"
                        role="status"
                    >
                        {if provider_preview_enabled {
                            "Refresh to render the current draft on the server."
                        } else {
                            "Server preview is disabled by the current Page Builder provider status."
                        }}
                    </div>
                }.into_any(),
            }}
        </section>
    }
}
