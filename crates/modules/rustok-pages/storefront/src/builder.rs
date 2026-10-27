use crate::model::PageBody;
use leptos::prelude::*;
pub use rustok_page_builder::PAGE_BUILDER_DOCUMENT_FORMAT;
use rustok_page_builder_storefront::{
    PageBuilderStorefront, PageSelection, SharedRichTextRenderer, StaticLandingStorefront,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const STATIC_LANDING_PAYLOAD_BODY_FORMAT: &str = "fly_artifact_payload";
pub const STATIC_LANDING_URL_BODY_FORMAT: &str = "fly_artifact_url";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StaticLandingBodyPayload {
    pub html: String,
    pub css: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_url: Option<String>,
}

pub fn is_page_builder_body(body: &PageBody) -> bool {
    body.format == PAGE_BUILDER_DOCUMENT_FORMAT
        || body.format == STATIC_LANDING_PAYLOAD_BODY_FORMAT
        || body.format == STATIC_LANDING_URL_BODY_FORMAT
}

pub fn decode_page_builder_body(body: &PageBody) -> Result<Value, serde_json::Error> {
    serde_json::from_str(&body.content)
}

pub fn decode_static_landing_payload(
    body: &PageBody,
) -> Result<StaticLandingBodyPayload, serde_json::Error> {
    serde_json::from_str(&body.content)
}

#[component]
pub fn PageBuilderPageBody(
    body: PageBody,
    #[prop(optional)] class: Option<String>,
) -> impl IntoView {
    let class = class.unwrap_or_else(|| "rustok-pages-storefront__builder-body".to_string());
    if !is_page_builder_body(&body) {
        return view! {
            <div class=class data-page-builder-body="false" role="alert">
                <p>{format!(
                    "Page body format `{}` is not supported by the Fly renderer",
                    body.format
                )}</p>
            </div>
        }
        .into_any();
    }

    if body.format == STATIC_LANDING_PAYLOAD_BODY_FORMAT {
        return match decode_static_landing_payload(&body) {
            Ok(payload) => view! {
                <StaticLandingStorefront
                    html=payload.html
                    css=payload.css
                    class=class
                />
            }
            .into_any(),
            Err(error) => view! {
                <div
                    class=class
                    data-page-builder-body="invalid"
                    role="alert"
                >
                    <p>{format!("Invalid static landing payload: {error}")}</p>
                </div>
            }
            .into_any(),
        };
    }

    if body.format == STATIC_LANDING_URL_BODY_FORMAT {
        if let Ok(payload) = decode_static_landing_payload(&body) {
            return view! {
                <StaticLandingStorefront
                    html=payload.html
                    css=payload.css
                    class=class
                />
            }
            .into_any();
        }

        return view! {
            <iframe
                class=class
                title="Published landing page"
                sandbox="allow-forms allow-same-origin"
                src=body.content
                data-page-builder-body="artifact"
            ></iframe>
        }
        .into_any();
    }

    let rich_text_renderer = pages_rich_text_renderer();
    match decode_page_builder_body(&body) {
        Ok(project_data) => match rich_text_renderer {
            Some(renderer) => view! {
                <PageBuilderStorefront
                    project_data
                    selection=PageSelection::First
                    class=class
                    rich_text_renderer=renderer
                />
            }
            .into_any(),
            None => view! {
                <PageBuilderStorefront
                    project_data
                    selection=PageSelection::First
                    class=class
                />
            }
            .into_any(),
        },
        Err(error) => view! {
            <div
                class=class
                data-page-builder-body="invalid"
                role="alert"
            >
                <p>{format!("Invalid GrapesJS page body: {error}")}</p>
            </div>
        }
        .into_any(),
    }
}

/// Host binding for Fly rich-text payloads: the platform rich-text capability
/// (`rustok-content`, Article profile) is the single production renderer, per the
/// richtext capability ADR. Without the `ssr` feature no renderer is bound and Fly's
/// escaped fallback projection is used.
#[cfg(feature = "ssr")]
pub struct PagesRichTextRenderer;

#[cfg(feature = "ssr")]
impl fly::RichTextHtmlRenderer for PagesRichTextRenderer {
    fn render_rich_text(&self, payload: &fly::RichTextPayload) -> Option<String> {
        let document: rustok_api::RichTextDocument =
            serde_json::from_value(payload.payload.clone()).ok()?;
        rustok_content::richtext::render_html(&document, rustok_content::RichTextProfile::Article)
            .ok()
    }
}

fn pages_rich_text_renderer() -> Option<SharedRichTextRenderer> {
    #[cfg(feature = "ssr")]
    {
        Some(std::sync::Arc::new(PagesRichTextRenderer))
    }
    #[cfg(not(feature = "ssr"))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn body(format: &str, content: String) -> PageBody {
        PageBody {
            locale: "en".to_string(),
            content,
            format: format.to_string(),
        }
    }

    #[test]
    fn accepts_only_canonical_builder_body_formats() {
        assert!(is_page_builder_body(&body(
            PAGE_BUILDER_DOCUMENT_FORMAT,
            "{}".to_string()
        )));
        assert!(is_page_builder_body(&body(
            STATIC_LANDING_PAYLOAD_BODY_FORMAT,
            "{}".to_string()
        )));
        assert!(is_page_builder_body(&body(
            STATIC_LANDING_URL_BODY_FORMAT,
            "/api/pages/id/artifact".to_string(),
        )));
        assert!(!is_page_builder_body(&body("GRAPESJS", "{}".to_string())));
        assert!(!is_page_builder_body(&body(
            "markdown",
            "# Hello".to_string()
        )));
    }

    #[test]
    fn decodes_builder_project_json() {
        let project = json!({
            "pages": [{
                "component": { "id": "root", "type": "wrapper" }
            }]
        });
        let decoded = decode_page_builder_body(&body(
            PAGE_BUILDER_DOCUMENT_FORMAT,
            serde_json::to_string(&project).expect("project json"),
        ))
        .expect("decode project");
        assert_eq!(decoded, project);
    }

    #[test]
    fn decodes_static_landing_payload() {
        let payload = StaticLandingBodyPayload {
            html: "<h1>Landing</h1>".to_string(),
            css: "h1 { color: red; }".to_string(),
            artifact_url: Some("/api/pages/123/artifact".to_string()),
        };
        let decoded = decode_static_landing_payload(&body(
            STATIC_LANDING_PAYLOAD_BODY_FORMAT,
            serde_json::to_string(&payload).expect("payload json"),
        ))
        .expect("decode payload");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn invalid_builder_json_is_rejected() {
        assert!(
            decode_page_builder_body(&body(PAGE_BUILDER_DOCUMENT_FORMAT, "{invalid".to_string(),))
                .is_err()
        );
        assert!(
            decode_static_landing_payload(&body(
                STATIC_LANDING_PAYLOAD_BODY_FORMAT,
                "{invalid".to_string(),
            ))
            .is_err()
        );
    }

    #[test]
    fn page_builder_page_body_constructs_views() {
        let payload = StaticLandingBodyPayload {
            html: "<h1>Landing</h1>".to_string(),
            css: "h1 { color: red; }".to_string(),
            artifact_url: Some("/api/pages/123/artifact".to_string()),
        };
        let _payload_view = view! {
            <PageBuilderPageBody
                body=body(
                    STATIC_LANDING_PAYLOAD_BODY_FORMAT,
                    serde_json::to_string(&payload).unwrap(),
                )
            />
        };
        let _url_view = view! {
            <PageBuilderPageBody
                body=body(
                    STATIC_LANDING_URL_BODY_FORMAT,
                    "/api/pages/123/artifact".to_string(),
                )
            />
        };
    }
}

