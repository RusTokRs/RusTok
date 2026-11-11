//! The page forms of the classic SSR inspector, dispatched the way the browser posts them.
//!
//! `create_page`, `remove_page`, `rename_page` and `patch_page_metadata` have two payload shapes
//! under one wire name: the typed envelopes of the adapter workspaces address a page by
//! `page_index`, the SSR forms by `page_id` alone. These tests send the exact fields the forms in
//! `editor/ssr_inspector.rs` carry — as the host bootstrap serializes them — through
//! [`dispatch_browser_intent`], because resolving a form in `ssr_form_intent` proves nothing when
//! an arm of the dispatcher takes the intent first. The envelope shape is covered next to them
//! so that neither shape is fixed at the cost of the other.

use crate::{AdminCanvasController, dispatch_browser_intent};
use fly::{PageLocator, PageMetadata};
use fly_browser::{BrowserIntentEnvelope, BrowserIntentKind, FLY_BROWSER_PROTOCOL};
use serde_json::{Value, json};

fn controller() -> AdminCanvasController {
    AdminCanvasController::new(
        "home",
        "rev-1",
        json!({
            "pages": [{
                "id": "home",
                "name": "Home",
                "component": { "id": "home-root", "type": "wrapper" }
            }, {
                "id": "about",
                "name": "About",
                "flyPageMeta": { "slug": "about" },
                "component": { "id": "about-root", "type": "wrapper" }
            }]
        }),
    )
    .expect("controller")
}

fn intent(
    controller: &AdminCanvasController,
    kind: BrowserIntentKind,
    payload: Value,
) -> BrowserIntentEnvelope {
    BrowserIntentEnvelope {
        protocol: FLY_BROWSER_PROTOCOL.to_string(),
        instance_id: "ssr-page-forms".to_string(),
        intent: kind.as_str().to_string(),
        payload,
        sequence: Some(1),
        page_id: Some(controller.page_id().to_string()),
        revision: Some(controller.revision_id().to_string()),
        project_hash: Some(controller.editor().revision().project_hash.hex()),
        draft_token: None,
        draft_generation: None,
    }
}

fn page_ids(controller: &AdminCanvasController) -> Vec<String> {
    controller
        .page_summaries()
        .into_iter()
        .filter_map(|page| page.id)
        .collect()
}

#[test]
fn create_page_form_keeps_the_page_id_the_author_typed() {
    let mut controller = controller();

    let request = intent(
        &controller,
        BrowserIntentKind::CreatePage,
        json!({ "page_id": "contact", "name": "Contact us" }),
    );
    dispatch_browser_intent(&mut controller, request).expect("create page form");

    assert_eq!(page_ids(&controller), ["home", "about", "contact"]);
    let created = controller
        .page_summaries()
        .into_iter()
        .find(|page| page.id.as_deref() == Some("contact"))
        .expect("created page");
    assert_eq!(created.name, "Contact us");
}

#[test]
fn remove_page_form_names_the_page_by_id_alone() {
    let mut controller = controller();

    let request = intent(
        &controller,
        BrowserIntentKind::RemovePage,
        json!({ "page_id": "about" }),
    );
    dispatch_browser_intent(&mut controller, request).expect("remove page form");

    assert_eq!(page_ids(&controller), ["home"]);
}

#[test]
fn rename_page_form_changes_the_id_without_a_new_name() {
    let mut controller = controller();

    // The name input of the form is optional; an empty one is still sent.
    let request = intent(
        &controller,
        BrowserIntentKind::RenamePage,
        json!({ "page_id": "about", "new_page_id": "company", "name": "" }),
    );
    dispatch_browser_intent(&mut controller, request).expect("rename page form");

    assert_eq!(page_ids(&controller), ["home", "company"]);
}

#[test]
fn page_metadata_form_sends_flat_fields() {
    let mut controller = controller();

    let request = intent(
        &controller,
        BrowserIntentKind::PatchPageMetadata,
        json!({
            "page_id": "about",
            "title": "About us",
            "slug": "about",
            "description": "",
            "canonical_url": "",
            "og_title": "",
            "og_description": "",
            "og_image": "",
            "no_index": true
        }),
    );
    dispatch_browser_intent(&mut controller, request).expect("page metadata form");

    let document = controller.editor().document();
    let page = document
        .page(&PageLocator::by_id("about"))
        .expect("about page");
    let metadata = PageMetadata::from_page(page);
    assert_eq!(metadata.title.as_deref(), Some("About us"));
    assert_eq!(metadata.slug.as_deref(), Some("about"));
    assert!(metadata.no_index);
}

#[test]
fn page_envelopes_of_the_adapter_workspaces_still_address_a_page_by_index() {
    let mut controller = controller();

    let request = intent(
        &controller,
        BrowserIntentKind::RenamePage,
        json!({ "page_index": 1, "name": "Company" }),
    );
    dispatch_browser_intent(&mut controller, request).expect("rename page envelope");
    let renamed = controller
        .page_summaries()
        .into_iter()
        .find(|page| page.index == 1)
        .expect("second page");
    assert_eq!(renamed.id.as_deref(), Some("about"));
    assert_eq!(renamed.name, "Company");

    let request = intent(
        &controller,
        BrowserIntentKind::RemovePage,
        json!({ "page_index": 1 }),
    );
    dispatch_browser_intent(&mut controller, request).expect("remove page envelope");
    assert_eq!(page_ids(&controller), ["home"]);
}

#[test]
fn a_page_form_without_its_required_field_is_rejected_by_the_form_resolver() {
    let mut controller = controller();

    let request = intent(
        &controller,
        BrowserIntentKind::RenamePage,
        json!({ "page_id": "about", "name": "Company" }),
    );
    let error = dispatch_browser_intent(&mut controller, request)
        .expect_err("rename page form without new_page_id");

    assert!(
        error.to_string().contains("new_page_id"),
        "unexpected error: {error}"
    );
    assert_eq!(page_ids(&controller), ["home", "about"]);
}
