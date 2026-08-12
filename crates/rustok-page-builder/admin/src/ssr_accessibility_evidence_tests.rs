use crate::{AdminCanvasController, PageBuilderAdmin, PageBuilderAdminHostContext};
use fly_ui::CapabilityState;
use leptos::prelude::*;
use serde_json::json;

fn controller() -> AdminCanvasController {
    AdminCanvasController::new(
        "home",
        "rev-accessibility-1",
        json!({
            "pages": [
                {
                    "id": "home",
                    "name": "Home",
                    "component": {
                        "id": "home-root",
                        "type": "wrapper",
                        "components": [{ "id": "hero", "type": "text", "content": "Hero" }]
                    }
                },
                {
                    "id": "about",
                    "name": "About",
                    "component": {
                        "id": "about-root",
                        "type": "wrapper",
                        "components": []
                    }
                }
            ]
        }),
    )
    .expect("accessibility evidence controller")
}

fn render_admin(capabilities: CapabilityState) -> String {
    leptos::ssr::render_to_string(move || {
        provide_context(
            PageBuilderAdminHostContext::new(controller()).with_editor_capabilities(capabilities),
        );
        view! { <PageBuilderAdmin /> }
    })
    .to_string()
}

fn element_slice<'a>(html: &'a str, marker: &str, closing_tag: &str) -> &'a str {
    let start = html
        .find(marker)
        .unwrap_or_else(|| panic!("missing rendered marker {marker:?}"));
    let tail = &html[start..];
    let end = tail
        .find(closing_tag)
        .unwrap_or_else(|| panic!("missing closing tag {closing_tag:?} after {marker:?}"));
    &tail[..end + closing_tag.len()]
}

#[test]
fn generic_editor_ssr_exposes_selected_page_and_programmatic_page_name() {
    let html = render_admin(CapabilityState::full());

    assert!(
        html.contains("data-fly-browser-root=\"true\""),
        "expected the rendered Page Builder workspace root"
    );
    assert!(
        html.contains("aria-label=\"Add page: Page name\""),
        "new-page input must have a programmatic name in rendered HTML"
    );
    assert!(
        html.contains("aria-pressed=\"true\""),
        "active page must expose selected state in rendered HTML"
    );
    assert!(
        html.contains("aria-pressed=\"false\""),
        "inactive page must expose unselected state in rendered HTML"
    );
    assert!(html.contains(">Page name</span>"));
    assert!(html.contains(">Page id</span>"));
}

#[test]
fn generic_editor_ssr_exposes_denied_capabilities_as_disabled_fieldsets() {
    let html = render_admin(CapabilityState::read_only());

    for capability in ["edit", "properties"] {
        let marker = format!("data-fly-capability=\"{capability}\"");
        let fieldset = element_slice(&html, &marker, ">");
        assert!(
            fieldset.contains("aria-disabled=\"true\""),
            "{capability} fieldset must expose aria-disabled when denied: {fieldset}"
        );
        assert!(
            fieldset.contains("disabled"),
            "{capability} fieldset must use native disabled semantics when denied: {fieldset}"
        );
    }
}
