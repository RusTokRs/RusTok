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

/// Index and id of the page the editor is on.
fn active_page(controller: &AdminCanvasController) -> (usize, Option<String>) {
    (
        controller.active_page_index(),
        controller.active_page_summary().and_then(|page| page.id),
    )
}

// The envelopes below come from the typed constructors of `fly-browser`, which is how the
// adapter workspaces build them: a field renamed on either side fails here.

#[test]
fn create_page_envelope_derives_the_id_from_the_name_and_activates_the_page() {
    let mut controller = controller();

    let request = BrowserIntentEnvelope::create_page("page-envelopes", "Contact us");
    dispatch_browser_intent(&mut controller, request).expect("create page envelope");
    assert_eq!(page_ids(&controller), ["home", "about", "contact-us"]);
    assert_eq!(
        active_page(&controller),
        (2, Some("contact-us".to_string()))
    );

    // The same name again gets the next free id instead of a duplicate.
    let request = BrowserIntentEnvelope::create_page("page-envelopes", "Contact us");
    dispatch_browser_intent(&mut controller, request).expect("second page of that name");
    assert_eq!(
        page_ids(&controller),
        ["home", "about", "contact-us", "contact-us-2"]
    );
    assert_eq!(
        active_page(&controller),
        (3, Some("contact-us-2".to_string()))
    );

    // A blank name is not an error: the page gets the default one.
    let request = BrowserIntentEnvelope::create_page("page-envelopes", "  ");
    dispatch_browser_intent(&mut controller, request).expect("page without a name");
    let created = controller
        .page_summaries()
        .into_iter()
        .next_back()
        .expect("created page");
    assert_eq!(created.name, "Untitled page");
    assert_eq!(created.id.as_deref(), Some("untitled-page"));
}

#[test]
fn remove_page_envelope_prefers_the_page_id_over_a_stale_index() {
    let mut controller = controller();

    // The index is the one the workspace rendered; the id decides when both are sent.
    let request = BrowserIntentEnvelope::remove_page("page-envelopes", 0, Some("about"));
    dispatch_browser_intent(&mut controller, request).expect("remove page envelope");

    assert_eq!(page_ids(&controller), ["home"]);
    assert_eq!(active_page(&controller), (0, Some("home".to_string())));
}

#[test]
fn rename_page_envelope_changes_the_name_and_the_id_in_one_command() {
    let mut controller = controller();

    let request = BrowserIntentEnvelope::rename_page(
        "page-envelopes",
        1,
        Some("about"),
        "Company",
        Some("company"),
    );
    dispatch_browser_intent(&mut controller, request).expect("rename page envelope");

    let renamed = controller
        .page_summaries()
        .into_iter()
        .find(|page| page.index == 1)
        .expect("second page");
    assert_eq!(renamed.id.as_deref(), Some("company"));
    assert_eq!(renamed.name, "Company");
    assert_eq!(page_ids(&controller), ["home", "company"]);
}

#[test]
fn rename_page_envelope_keeps_the_editor_on_the_page_whose_id_it_changes() {
    let mut controller = controller();

    let request = BrowserIntentEnvelope::rename_page(
        "page-envelopes",
        0,
        Some("home"),
        "Start",
        Some("start"),
    );
    dispatch_browser_intent(&mut controller, request).expect("rename the active page");

    assert_eq!(page_ids(&controller), ["start", "about"]);
    assert_eq!(active_page(&controller), (0, Some("start".to_string())));
}

#[test]
fn page_properties_envelope_applies_identity_and_metadata_together() {
    let mut controller = controller();

    let metadata = serde_json::to_value(PageMetadata {
        title: Some("About us".to_string()),
        no_index: true,
        ..PageMetadata::default()
    })
    .expect("page metadata");
    let request = BrowserIntentEnvelope::page_properties(
        "page-envelopes",
        1,
        Some("about"),
        "Company",
        None,
        metadata,
    );
    dispatch_browser_intent(&mut controller, request).expect("page properties envelope");

    let document = controller.editor().document();
    let page = document
        .page(&PageLocator::by_id("about"))
        .expect("about page");
    let metadata = PageMetadata::from_page(page);
    assert_eq!(metadata.title.as_deref(), Some("About us"));
    assert!(metadata.no_index);
    let renamed = controller
        .page_summaries()
        .into_iter()
        .find(|page| page.index == 1)
        .expect("second page");
    assert_eq!(renamed.name, "Company");
}

#[test]
fn page_metadata_envelope_carries_the_metadata_projection() {
    let mut controller = controller();

    let metadata = serde_json::to_value(PageMetadata {
        title: Some("About us".to_string()),
        slug: Some("About Us".to_string()),
        ..PageMetadata::default()
    })
    .expect("page metadata");
    let request =
        BrowserIntentEnvelope::patch_page_metadata("page-envelopes", 1, Some("about"), metadata);
    dispatch_browser_intent(&mut controller, request).expect("page metadata envelope");

    let document = controller.editor().document();
    let page = document
        .page(&PageLocator::by_id("about"))
        .expect("about page");
    let metadata = PageMetadata::from_page(page);
    assert_eq!(metadata.title.as_deref(), Some("About us"));
    // The server normalizes the slug whichever shape carries it.
    assert_eq!(metadata.slug.as_deref(), Some("about-us"));
}

#[test]
fn page_metadata_envelope_without_metadata_is_rejected() {
    let mut controller = controller();

    let request = intent(
        &controller,
        BrowserIntentKind::PatchPageMetadata,
        json!({ "page_index": 1 }),
    );
    let error = dispatch_browser_intent(&mut controller, request)
        .expect_err("page metadata envelope without metadata");

    assert!(
        error.to_string().contains("metadata"),
        "unexpected error: {error}"
    );
}

#[test]
fn move_page_envelope_activates_the_page_where_it_lands() {
    let mut controller = controller();

    let request = BrowserIntentEnvelope::move_page("page-envelopes", 1, Some("about"), 0);
    dispatch_browser_intent(&mut controller, request).expect("move page to the front");
    assert_eq!(page_ids(&controller), ["about", "home"]);
    assert_eq!(active_page(&controller), (0, Some("about".to_string())));

    // The target is an insertion index of the list before the move: its length means the end.
    let request = BrowserIntentEnvelope::move_page("page-envelopes", 0, Some("about"), 2);
    dispatch_browser_intent(&mut controller, request).expect("move page to the end");
    assert_eq!(page_ids(&controller), ["home", "about"]);
    assert_eq!(active_page(&controller), (1, Some("about".to_string())));
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
