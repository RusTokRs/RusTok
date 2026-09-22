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
    let owner = Owner::new();
    owner.with(|| {
        provide_context(
            PageBuilderAdminHostContext::new(controller()).with_editor_capabilities(capabilities),
        );
        view! { <PageBuilderAdmin /> }.to_html()
    })
}

fn element_containing<'a>(
    html: &'a str,
    marker: &str,
    opening_tag: &str,
    closing_tag: &str,
) -> &'a str {
    let marker_index = html
        .find(marker)
        .unwrap_or_else(|| panic!("missing rendered marker {marker:?}"));
    let start = html[..marker_index]
        .rfind(opening_tag)
        .unwrap_or_else(|| panic!("missing opening tag {opening_tag:?} before {marker:?}"));
    let tail = &html[start..];
    let end = tail
        .find(closing_tag)
        .unwrap_or_else(|| panic!("missing closing tag {closing_tag:?} after {marker:?}"));
    &tail[..end + closing_tag.len()]
}

fn opening_tag_containing<'a>(html: &'a str, marker: &str, opening_tag: &str) -> &'a str {
    let marker_index = html
        .find(marker)
        .unwrap_or_else(|| panic!("missing rendered marker {marker:?}"));
    let start = html[..marker_index]
        .rfind(opening_tag)
        .unwrap_or_else(|| panic!("missing opening tag {opening_tag:?} before {marker:?}"));
    let end = html[marker_index..]
        .find('>')
        .map(|offset| marker_index + offset)
        .unwrap_or_else(|| panic!("missing end of opening tag after {marker:?}"));
    &html[start..=end]
}

fn has_native_disabled_attribute(opening_tag: &str) -> bool {
    opening_tag.contains(" disabled ")
        || opening_tag.contains(" disabled=\"\"")
        || opening_tag.contains(" disabled=\"disabled\"")
        || opening_tag.ends_with(" disabled>")
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

    let home_button = element_containing(&html, "Home", "<button", "</button>");
    assert!(
        home_button.contains("aria-pressed=\"true\""),
        "active Home page control must expose selected state: {home_button}"
    );
    let about_button = element_containing(&html, "About", "<button", "</button>");
    assert!(
        about_button.contains("aria-pressed=\"false\""),
        "inactive About page control must expose unselected state: {about_button}"
    );
    assert!(
        html.matches("Page name").count() >= 2,
        "rendered HTML must retain both the add-page accessible name and visible Page name label"
    );
    assert!(
        html.contains("Page id"),
        "rendered HTML must retain the visible Page id label"
    );
}

#[test]
fn generic_editor_ssr_exposes_denied_capabilities_as_disabled_fieldsets() {
    let html = render_admin(CapabilityState::read_only());

    for capability in ["edit", "properties"] {
        let marker = format!("data-fly-capability=\"{capability}\"");
        let fieldset = opening_tag_containing(&html, &marker, "<fieldset");
        assert!(
            fieldset.contains("aria-disabled=\"true\""),
            "{capability} fieldset must expose aria-disabled when denied: {fieldset}"
        );
        assert!(
            has_native_disabled_attribute(fieldset),
            "{capability} fieldset must use native disabled semantics when denied: {fieldset}"
        );
    }
}
