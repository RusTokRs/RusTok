use crate::editor::AdminEditorRuntime;
use leptos::prelude::*;

#[component]
pub fn ServerPreviewPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    let busy_runtime = runtime.clone();
    let request_runtime = runtime.clone();
    let status_runtime = runtime.clone();
    let frame_runtime = runtime;

    view! {
        <section
            class="rounded-xl border border-border bg-card p-3"
            data-page-builder-server-preview="true"
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
                        busy_runtime.preview_in_progress.get()
                            || busy_runtime.controller.with(|controller| {
                                controller.ui().state.has_blocking_diagnostics()
                            })
                    }
                    on:click=move |_| request_runtime.request_server_preview()
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
                        "Refresh to render the current draft on the server."
                    </div>
                }.into_any(),
            }}
        </section>
    }
}
