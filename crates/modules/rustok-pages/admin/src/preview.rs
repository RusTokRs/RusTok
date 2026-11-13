//! Server-rendered preview of a Page Builder draft.
//!
//! The preview is one render reached from two places: the editor's `render_server_preview` intent,
//! which answers with the document through the browser-intent endpoint, and a host preview route,
//! which answers with the document as its own response and its own policy — the shape a faithful
//! preview needs, because a `srcdoc` frame inherits the policy of the console it is displayed in.
//!
//! Both need the same three things: the draft session or the saved page, the active page's
//! one-page project, and the runtime context the editor holds. They are assembled once, here.
//!
//! A preview never writes. It reads the session the caller already owns and renders it; it does
//! not advance a revision, commit a draft, or publish.
//!
//! The host owns attribution. A preview route runs in the process that verified the caller, and it
//! passes the owner it verified; this module never guesses one. A preview of a session without an
//! owner would be a preview of whoever's draft the token names, which is why there is no
//! unattributed entry point here.

use crate::browser_intent::PagesBrowserIntentError;
use crate::builder::{self, PagesBuilderSaveSnapshot};
use rustok_page_builder::dto::{
    PageBuilderCapabilityResponse, PageBuilderPreviewRuntime, PreviewPageBuilderInput,
};
use rustok_page_builder_admin::{AdminCanvasController, SsrDraftSessionStore};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// What a preview render produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PagesServerPreview {
    pub page_id: String,
    /// The complete document, as the renderer composed it.
    pub html: String,
    /// The project hash the render answers: a client compares it to the document it holds to know
    /// whether the preview is of the draft it is looking at.
    pub project_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_scenario_id: Option<String>,
}

/// The store the host resolves for draft sessions.
pub fn pages_server_preview_store() -> &'static dyn SsrDraftSessionStore {
    crate::browser_intent::pages_browser_draft_store()
}

/// Renders the preview of the page `snapshot` addresses.
///
/// `draft_token` names the session the editor is using; without one the saved page is previewed,
/// which is what a first render before any edit shows. `snapshot.owner` is the caller the host
/// verified; a session bound to another owner is refused by the store rather than read.
///
/// Nothing is committed: a preview is a read of the draft, and the revision the session carries is
/// left exactly as it was.
#[cfg(feature = "ssr")]
pub async fn render_server_preview(
    snapshot: PagesBuilderSaveSnapshot,
    draft_store: &dyn SsrDraftSessionStore,
    draft_token: Option<&str>,
    runtime_context: Value,
    runtime_scenario_id: Option<String>,
) -> Result<PagesServerPreview, PagesBrowserIntentError> {
    let page_id = snapshot.page_id.clone();
    let loaded_session = match draft_token {
        Some(token) => draft_store.load(token, &page_id, snapshot.owner.as_ref())?,
        None => None,
    };

    let (controller, runtime_context) = match loaded_session {
        Some(session) => (session.controller, session.runtime_context),
        None => {
            let page = crate::transport::fetch_page(
                snapshot.token.clone(),
                snapshot.tenant_slug.clone(),
                page_id.clone(),
            )
            .await?
            .ok_or(PagesBrowserIntentError::PageNotFound)?;
            // The starter document must not stand in for a document this principal is not shown.
            if !page.document_readable {
                return Err(PagesBrowserIntentError::DocumentUnavailable);
            }
            let revision_id = builder::page_revision(&page);
            (
                crate::browser_intent::controller_from_page(
                    &page,
                    &revision_id,
                    &snapshot.default_locale,
                )?,
                runtime_context,
            )
        }
    };
    let project_hash = controller.editor().revision().project_hash.hex();

    let response = crate::builder::execute_preview_capability(
        snapshot,
        PreviewPageBuilderInput::new(page_id, one_page_project(&controller)).with_runtime(
            PageBuilderPreviewRuntime::new(runtime_context, runtime_scenario_id),
        ),
    )
    .await?;
    let PageBuilderCapabilityResponse::Preview(result) = response else {
        return Err(PagesBrowserIntentError::Preview(format!(
            "the Page Builder transport answered a preview request with `{}`",
            response.capability()
        )));
    };
    Ok(PagesServerPreview {
        page_id: result.page_id,
        html: result.html,
        project_hash,
        runtime_scenario_id: result.runtime_scenario_id,
    })
}

/// The document of the page the editor is on, as one page.
///
/// A preview shows the page being edited, not the whole project: the renderer selects the first
/// page, and a project that still held the others would make the selection depend on an order the
/// caller did not intend.
fn one_page_project(controller: &AdminCanvasController) -> Value {
    let active_page_index = controller.active_page_index();
    let mut document = controller.editor().document().clone();
    document.project.pages = document
        .project
        .pages
        .get(active_page_index)
        .cloned()
        .into_iter()
        .collect();
    fly::GrapesJsCodec::encode_value(&document)
        .unwrap_or_else(|_| Value::Object(Default::default()))
}

/// The response that serves a preview document, with the policy its own bytes require.
///
/// This is the published artifact's policy — `default-src 'none'`, and a `style-src` naming the
/// hash of the stylesheet the document carries — so a preview cannot load anything the published
/// page could not, and the two cannot drift into disagreeing about what a page may contain.
///
/// The document is `no-store`: its body is the caller's working copy, which a proxy must never
/// hand to another caller, and only this origin may frame it ([`PREVIEW_FRAME_ANCESTORS`]).
///
/// A document that does not carry the Page Builder envelope is refused: its policy cannot be
/// derived from what it contains, and guessing one would either blank the preview or admit more
/// than the page may use.
#[cfg(feature = "ssr")]
pub fn pages_server_preview_response(html: &str) -> Result<axum::response::Response, PreviewError> {
    let stylesheet = fly::document_html_stylesheet(html).ok_or(PreviewError::MissingEnvelope)?;
    Ok(pages_server_preview_response_with_policy(
        html,
        &pages_server_preview_policy(stylesheet),
    ))
}

#[cfg(feature = "ssr")]
fn pages_server_preview_response_with_policy(html: &str, policy: &str) -> axum::response::Response {
    use axum::http::{StatusCode, header};
    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::VARY, "Authorization, X-Tenant-Slug, X-Channel-Slug")
        .header(header::CONTENT_SECURITY_POLICY, policy)
        .header(header::REFERRER_POLICY, "strict-origin-when-cross-origin")
        .header("x-content-type-options", "nosniff")
        .body(axum::body::Body::from(html.to_string()))
        .expect("preview response headers are valid")
}

/// Why a preview document could not be served.
#[cfg(feature = "ssr")]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PreviewError {
    #[error(
        "the preview document does not carry the Page Builder envelope, so the policy of its \
         response cannot be derived from the bytes it serves"
    )]
    MissingEnvelope,
}

/// The only origin allowed to frame a preview document.
///
/// A preview is served by the admin host and embedded by the same host, so another origin cannot
/// open the frame. It is deliberately not `'none'`: the policy governs the document, and the one
/// legitimate embedder is the console that asked for it.
#[cfg(feature = "ssr")]
pub const PREVIEW_FRAME_ANCESTORS: &str = "'self'";

#[cfg(feature = "ssr")]
pub fn pages_server_preview_policy(stylesheet: &str) -> String {
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
    use sha2::{Digest, Sha256};
    let style_hash = BASE64.encode(Sha256::digest(stylesheet.as_bytes()));
    format!(
        "default-src 'none'; style-src 'sha256-{style_hash}'; img-src 'self' https: data:; \
         media-src 'self' https:; font-src 'self' https: data:; connect-src 'self'; \
         form-action 'self'; base-uri 'none'; frame-ancestors {PREVIEW_FRAME_ANCESTORS}"
    )
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::{PREVIEW_FRAME_ANCESTORS, pages_server_preview_policy};

    fn directive<'a>(policy: &'a str, name: &str) -> &'a str {
        policy
            .split(';')
            .map(str::trim)
            .find(|item| item.starts_with(name))
            .unwrap_or_else(|| panic!("no {name} in {policy}"))
    }

    fn object_sources_denied(policy: &str) -> bool {
        let directives = policy.split(';').map(str::trim).collect::<Vec<_>>();
        let objects = directives
            .iter()
            .copied()
            .filter(|item| item.split_ascii_whitespace().next() == Some("object-src"))
            .collect::<Vec<_>>();
        if objects.is_empty() {
            let defaults = directives
                .iter()
                .copied()
                .filter(|item| item.split_ascii_whitespace().next() == Some("default-src"))
                .collect::<Vec<_>>();
            defaults.len() == 1 && defaults[0] == "default-src 'none'"
        } else {
            objects.len() == 1 && objects[0] == "object-src 'none'"
        }
    }

    #[test]
    fn object_source_check_handles_default_fallback_and_explicit_override() {
        for (policy, denied) in [
            ("default-src 'none'", true),
            ("default-src 'none'; object-src 'none'", true),
            ("default-src 'self'; object-src 'none'", true),
            ("default-src 'none'; object-src https:", false),
            ("default-src 'self'", false),
            ("default-src 'none'; object-src", false),
            ("default-src 'self'; default-src 'none'", false),
            (
                "default-src 'none'; object-src https:; object-src 'none'",
                false,
            ),
        ] {
            assert_eq!(object_sources_denied(policy), denied, "{policy}");
        }
    }

    #[test]
    fn the_policy_names_the_hash_of_the_stylesheet_the_document_carries() {
        let policy = pages_server_preview_policy("body{margin:0}");
        // The base64 of SHA-256("body{margin:0}"), so the assertion names the value a browser
        // computes rather than the shape of an expression.
        let expected = {
            use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
            use sha2::{Digest, Sha256};
            BASE64.encode(Sha256::digest(b"body{margin:0}"))
        };
        assert_eq!(
            directive(&policy, "style-src"),
            format!("style-src 'sha256-{expected}'")
        );
        assert_ne!(
            pages_server_preview_policy("body{margin:1px}"),
            policy,
            "a different stylesheet must name a different hash"
        );
    }

    #[test]
    fn the_policy_admits_nothing_the_published_page_could_not_load() {
        let policy = pages_server_preview_policy("");
        assert_eq!(directive(&policy, "default-src"), "default-src 'none'");
        // No script source at all: a preview is a document, not an application.
        assert_eq!(
            policy
                .split(';')
                .map(str::trim)
                .filter(|item| item.starts_with("script-src"))
                .count(),
            0,
            "{policy}"
        );
        // An absent object-src inherits the already-checked default-src 'none'. An explicit
        // object-src must be at least as restrictive, rather than override that fallback.
        assert!(
            object_sources_denied(&policy),
            "object-src must deny plugins explicitly or inherit default-src: {policy}"
        );
        assert_eq!(directive(&policy, "base-uri"), "base-uri 'none'");
    }

    #[test]
    fn the_policy_lets_only_the_console_frame_it() {
        let policy = pages_server_preview_policy("");
        assert_eq!(PREVIEW_FRAME_ANCESTORS, "'self'");
        assert_eq!(
            directive(&policy, "frame-ancestors"),
            "frame-ancestors 'self'"
        );
        assert!(!policy.contains("*"), "{policy}");
    }

    #[test]
    fn a_document_without_the_envelope_is_refused_rather_than_policied() {
        // The policy is derived from the stylesheet the document carries. Markup that carries
        // none cannot be policied from its own bytes, and guessing would either blank the preview
        // or admit a source the page may not use.
        let foreign = "<html><head><style>a{b:c}</style></head><body></body></html>";
        assert_eq!(
            super::pages_server_preview_response(foreign).unwrap_err(),
            super::PreviewError::MissingEnvelope
        );
    }

    #[test]
    fn a_composed_document_is_served_under_the_hash_of_its_own_stylesheet() {
        let html = fly::compose_document_html(&fly::PageHead::default(), "body{margin:0}", "<p/>");
        let response = super::pages_server_preview_response(&html).expect("preview response");
        let headers = response.headers();
        assert_eq!(headers.get("cache-control").unwrap(), "no-store");
        assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
        let policy = headers
            .get("content-security-policy")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(
            policy.contains(&super::pages_server_preview_policy("body{margin:0}")),
            "{policy}"
        );
    }

    #[test]
    fn the_policy_is_a_header_value() {
        // A policy that cannot be sent is a policy that does not exist: `HttpError` falls back to
        // the API deny policy on an invalid header, which would blank the preview silently.
        let policy = pages_server_preview_policy("a{b:c}");
        assert!(axum::http::HeaderValue::try_from(policy.as_str()).is_ok());
    }
}
