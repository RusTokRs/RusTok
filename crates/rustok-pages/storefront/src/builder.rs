use crate::model::PageBody;
use leptos::prelude::*;
use rustok_page_builder_storefront::{PageBuilderStorefront, PageSelection};
use serde_json::Value;

pub const GRAPESJS_FORMAT_BODY_FORMAT: &str = "grapesjs";
pub const STATIC_LANDING_URL_BODY_FORMAT: &str = "fly_artifact_url";

pub fn is_page_builder_body(body: &PageBody) -> bool {
    body.format
        .eq_ignore_ascii_case(GRAPESJS_FORMAT_BODY_FORMAT)
        || body
            .format
            .eq_ignore_ascii_case(STATIC_LANDING_URL_BODY_FORMAT)
}

pub fn decode_page_builder_body(body: &PageBody) -> Result<Value, serde_json::Error> {
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

    if body
        .format
        .eq_ignore_ascii_case(STATIC_LANDING_URL_BODY_FORMAT)
    {
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

    match decode_page_builder_body(&body) {
        Ok(project_data) => view! {
            <PageBuilderStorefront
                project_data
                selection=PageSelection::First
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
                <p>{format!("Invalid GrapesJS page body: {error}")}</p>
            </div>
        }
        .into_any(),
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
    fn detects_builder_body_case_insensitively() {
        assert!(is_page_builder_body(&body("GRAPESJS", "{}".to_string())));
        assert!(is_page_builder_body(&body(
            "FLY_ARTIFACT_URL",
            "/api/pages/id/artifact".to_string(),
        )));
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
            GRAPESJS_FORMAT_BODY_FORMAT,
            serde_json::to_string(&project).expect("project json"),
        ))
        .expect("decode project");
        assert_eq!(decoded, project);
    }

    #[test]
    fn invalid_builder_json_is_rejected() {
        assert!(decode_page_builder_body(&body(
            GRAPESJS_FORMAT_BODY_FORMAT,
            "{invalid".to_string(),
        ))
        .is_err());
    }
}
