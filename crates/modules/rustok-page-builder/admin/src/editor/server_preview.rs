use crate::{PageBuilderAdminProviderStatus, editor::AdminEditorRuntime};
#[cfg(not(target_arch = "wasm32"))]
use crate::i18n::t;
use leptos::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use rustok_ui_core::UiRouteContext;
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
    // The form's admission is decided before `runtime` is moved into `frame_runtime`; it is a
    // plain read of the capability state, not a signal, so it is taken here with the rest.
    #[cfg(not(target_arch = "wasm32"))]
    let preview_form_enabled = runtime.form_enabled("render_server_preview");
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
    // The hydrated build has no capability state to consult here; the control it renders is the
    // handler, not the form.
    #[cfg(target_arch = "wasm32")]
    let preview_form_enabled = true;
    #[cfg(not(target_arch = "wasm32"))]
    let preview_form_label = {
        let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
        t(
            locale.as_deref(),
            "page_builder.serverPreview.render",
            "Render preview",
        )
    };
    #[cfg(target_arch = "wasm32")]
    let preview_form_label = "Render preview";
    // The document the frame starts with, if the host already rendered one. A host that answers
    // the form instead of rendering it is filled in by the browser bootstrap.
    let preview_srcdoc = frame_runtime
        .server_preview_html
        .get()
        .map(|html| embedded_preview_document(html, csp_nonce.as_ref()))
        .unwrap_or_default();
    let preview_rendered = !preview_srcdoc.is_empty();

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
                <div class="flex items-center gap-2">
                    // The server-rendered control. A rendered document has no handler to call, so
                    // this form is the preview's transport in the default `ssr + browser-js`
                    // runtime: the host bootstrap posts it, `render_server_preview` reaches the
                    // dispatch and the answer carries the document. It is disabled exactly when
                    // the endpoint would refuse it, by the endpoint's own rule.
                    <form data-fly-intent-form="render_server_preview">
                        <button
                            type="submit"
                            class="rounded border border-border px-3 py-1.5 text-sm disabled:opacity-50"
                            disabled=!preview_form_enabled
                        >
                            {preview_form_label}
                        </button>
                    </form>
                    // The hydrated control. Only a hydrated admin has the handler this needs, so
                    // it is not the path a server render relies on.
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
            </div>
            // The frame is rendered even before a document exists: the host bootstrap fills it
            // from the response to the form above, and a frame created only after a re-render is
            // a frame the response has nothing to fill. `hidden` is how the empty state is shown,
            // so the two are one element rather than a branch that can disagree with the script.
            <iframe
                class="mt-3 min-h-[360px] w-full rounded-lg border border-border bg-white"
                title="Server-rendered page preview"
                sandbox=""
                hidden=!preview_rendered
                srcdoc=preview_srcdoc.clone()
                data-page-builder-server-preview-frame="true"
            ></iframe>
            <div
                class="mt-3 grid min-h-[180px] place-items-center rounded-lg border border-dashed border-border bg-muted/30 px-4 text-center text-sm text-muted-foreground"
                role="status"
                hidden=preview_rendered
            >
                {if provider_preview_enabled {
                    "Render the current draft on the server to see what publication will produce."
                } else {
                    "Server preview is disabled by the current Page Builder provider status."
                }}
            </div>
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

    /// The panel's markup for one preview state.
    fn panel_html(csp_nonce: Option<UiCspNonce>, preview: Option<String>) -> String {
        let owner = Owner::new();
        owner.with(|| {
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
        })
    }

    /// The `srcdoc` attribute of a panel render, with its character references resolved.
    fn rendered_srcdoc(html: &str) -> String {
        let marker = " srcdoc=\"";
        let start = html.find(marker).expect("frame") + marker.len();
        let end = start + html[start..].find('"').expect("attribute end");
        html[start..end]
            .replace("&quot;", "\"")
            .replace("&#34;", "\"")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&")
    }

    /// Whether a rendered element carries `hidden` among its attributes, in either form a renderer
    /// may produce for a boolean Leptos attribute.
    fn is_hidden(html: &str, needle: &str) -> bool {
        let start = html.find(needle).unwrap_or_else(|| panic!("{needle} not rendered"));
        let tag_start = html[..start].rfind('<').expect("tag start");
        let tag_end = start + html[start..].find('>').expect("tag end");
        html[tag_start..tag_end]
            .split_whitespace()
            .any(|attribute| attribute == "hidden" || attribute == "hidden=\"true\"")
    }

    #[test]
    fn preview_frame_embeds_the_document_with_the_nonce_the_host_provided() {
        let nonce = UiCspNonce::new("0f3a9c").expect("nonce");
        let html = panel_html(Some(nonce), Some(preview_document()));
        let srcdoc = rendered_srcdoc(&html);
        assert!(
            srcdoc.contains("<style nonce=\"0f3a9c\">p{color:red}</style>"),
            "{srcdoc}"
        );
        assert!(!is_hidden(&html, "data-page-builder-server-preview-frame"), "{html}");

        let html = panel_html(None, Some(preview_document()));
        assert_eq!(rendered_srcdoc(&html), preview_document());
    }

    #[test]
    fn the_frame_exists_before_a_document_does() {
        // The host bootstrap fills the frame from the preview response, so the frame has to be in
        // the first render: a frame created only after a re-render is one the response cannot
        // reach. The empty state is the same element, hidden.
        let nonce = UiCspNonce::new("0f3a9c").expect("nonce");
        let html = panel_html(Some(nonce), None);
        assert!(
            html.contains("data-page-builder-server-preview-frame"),
            "the frame is not rendered before a document exists: {html}"
        );
        assert!(is_hidden(&html, "data-page-builder-server-preview-frame"), "{html}");
        assert_eq!(rendered_srcdoc(&html), "");
        // The empty state, by contrast, is the visible one.
        assert!(!is_hidden(&html, "role=\"status\""), "{html}");
    }

    #[test]
    fn the_preview_form_is_the_ssr_transport_and_is_rendered_when_admitted() {
        let nonce = UiCspNonce::new("0f3a9c").expect("nonce");
        let html = panel_html(Some(nonce), None);
        assert!(
            html.contains("data-fly-intent-form=\"render_server_preview\""),
            "the SSR-rendered preview control has no transport: {html}"
        );
        assert!(!html.contains("type=\"submit\" disabled"), "{html}");
    }
}
