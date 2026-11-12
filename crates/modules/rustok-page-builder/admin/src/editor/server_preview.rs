use crate::{PageBuilderAdminProviderStatus, editor::AdminEditorRuntime};
use leptos::prelude::*;
use rustok_ui_core::UiCspNonce;

/// The preview document as the panel embeds it.
///
/// The frame shows it through `srcdoc`, so it inherits the policy of the document that embeds
/// it. Under a host policy that names a nonce the browser drops the stylesheet of the preview
/// unless it carries that nonce. The preview is rendered by the server pipeline, which does not
/// know the nonce of the page that asked for it, so it is written here, where both are known.
///
/// `fly` owns the document envelope and puts the nonce on the stylesheet of that envelope and
/// on nothing else. Markup that is not the envelope is embedded as it arrived.
fn embedded_preview_document(html: String, csp_nonce: Option<&UiCspNonce>) -> String {
    csp_nonce
        .and_then(|nonce| fly::document_html_with_style_nonce(&html, nonce.as_str()))
        .unwrap_or(html)
}

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
    let csp_nonce = use_context::<UiCspNonce>();

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
                        srcdoc=embedded_preview_document(html, csp_nonce.as_ref())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AdminCanvasController;
    use fly::{PageHead, compose_document_html};
    use serde_json::json;

    fn preview_document() -> String {
        compose_document_html(&PageHead::default(), "p{color:red}", "<p>Preview</p>")
    }

    #[test]
    fn preview_document_carries_the_host_nonce_on_its_stylesheet() {
        let nonce = UiCspNonce::new("0f3a9c").expect("nonce");

        let embedded = embedded_preview_document(preview_document(), Some(&nonce));
        assert!(
            embedded.contains("<style nonce=\"0f3a9c\">p{color:red}</style>"),
            "{embedded}"
        );
        assert_eq!(embedded.matches(" nonce=\"").count(), 1, "{embedded}");

        let without_host_policy = embedded_preview_document(preview_document(), None);
        assert_eq!(without_host_policy, preview_document());
    }

    #[test]
    fn markup_that_is_not_the_fly_envelope_is_embedded_as_it_arrived() {
        let nonce = UiCspNonce::new("0f3a9c").expect("nonce");
        let foreign = "<html><head><style>p{color:red}</style></head></html>".to_string();
        assert_eq!(
            embedded_preview_document(foreign.clone(), Some(&nonce)),
            foreign
        );
    }

    /// The `srcdoc` attribute of the preview frame, with its character references resolved.
    fn rendered_srcdoc(csp_nonce: Option<UiCspNonce>, preview: Option<String>) -> Option<String> {
        let owner = Owner::new();
        let html = owner.with(|| {
            if let Some(nonce) = csp_nonce {
                provide_context(nonce);
            }
            let controller = AdminCanvasController::new(
                "home",
                "rev-preview-1",
                json!({
                    "pages": [{
                        "id": "home",
                        "name": "Home",
                        "component": {
                            "id": "home-root",
                            "type": "wrapper",
                            "components": [{ "id": "hero", "type": "text", "content": "Hero" }]
                        }
                    }]
                }),
            )
            .expect("preview controller");
            let runtime = AdminEditorRuntime::new(controller, None, None, "", "");
            runtime.server_preview_html.set(preview);
            view! { <ServerPreviewPanel runtime=runtime provider_status=None /> }.to_html()
        });
        let marker = " srcdoc=\"";
        let start = html.find(marker)? + marker.len();
        let end = start + html[start..].find('"')?;
        Some(
            html[start..end]
                .replace("&quot;", "\"")
                .replace("&#34;", "\"")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&amp;", "&"),
        )
    }

    #[test]
    fn preview_frame_embeds_the_document_with_the_nonce_the_host_provided() {
        let nonce = UiCspNonce::new("0f3a9c").expect("nonce");
        let srcdoc = rendered_srcdoc(Some(nonce), Some(preview_document())).expect("frame");
        assert!(
            srcdoc.contains("<style nonce=\"0f3a9c\">p{color:red}</style>"),
            "{srcdoc}"
        );

        let srcdoc = rendered_srcdoc(None, Some(preview_document())).expect("frame");
        assert_eq!(srcdoc, preview_document());
    }

    #[test]
    fn preview_frame_is_absent_until_a_preview_was_rendered() {
        let nonce = UiCspNonce::new("0f3a9c").expect("nonce");
        assert_eq!(rendered_srcdoc(Some(nonce), None), None);
    }
}
