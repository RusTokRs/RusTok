//! Rendered markup of the controls that `fly-browser.js` binds in the default
//! `ssr + browser-js` runtime.
//!
//! A server-rendered page has no Leptos handlers. The palette, the layer tree and the viewport
//! switch work only because the views render the `data-fly-*` attributes the adapter binds, and
//! — for the viewport — because the server resolves the route value before it renders the
//! canvas frame. These tests pin that server half on the composed admin surface, so they also
//! cover the `fly-leptos` workspace the admin mounts. The browser half is pinned by
//! `apps/next-admin/tests/e2e/fly-browser-ssr-controls.spec.ts`, and
//! `scripts/verify/verify-fly-ssr-control-contract.mjs` keeps the two in step.

use crate::{AdminCanvasController, PageBuilderAdmin, PageBuilderAdminHostContext};
use fly_ui::{CapabilityState, UiIntent};
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;
use serde_json::json;

fn controller() -> AdminCanvasController {
    AdminCanvasController::new(
        "home",
        "rev-viewport-1",
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
    .expect("viewport route controller")
}

fn render_admin(capabilities: CapabilityState, query: &[(&str, &str)]) -> String {
    render_admin_with(controller(), capabilities, query)
}

fn render_admin_with(
    controller: AdminCanvasController,
    capabilities: CapabilityState,
    query: &[(&str, &str)],
) -> String {
    let owner = Owner::new();
    owner.with(|| {
        provide_context(UiRouteContext {
            query: query
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
            ..UiRouteContext::default()
        });
        provide_context(
            PageBuilderAdminHostContext::new(controller).with_editor_capabilities(capabilities),
        );
        view! { <PageBuilderAdmin /> }.to_html()
    })
}

/// Opening tag of the first `tag` element that itself carries `marker`.
///
/// The marker can also occur outside an opening tag of that kind — in the inlined adapter
/// script or in another element — so every occurrence is tried, not only the first one.
fn opening_tag<'a>(html: &'a str, marker: &str, tag: &str) -> &'a str {
    html.match_indices(marker)
        .find_map(|(marker_index, _)| {
            let start = html[..marker_index].rfind('<')?;
            let end = start + html[start..].find('>')?;
            (html[start..].starts_with(tag) && marker_index < end).then(|| &html[start..=end])
        })
        .unwrap_or_else(|| panic!("no {tag:?} element carries {marker:?}"))
}

/// Native `disabled` attribute, as opposed to a `disabled:` utility class.
fn is_disabled(opening_tag: &str) -> bool {
    opening_tag.contains(" disabled ")
        || opening_tag.contains(" disabled=\"")
        || opening_tag.ends_with(" disabled>")
}

/// Opening tag of the `<option>` whose text is `label`.
fn option_tag<'a>(html: &'a str, label: &str) -> &'a str {
    let label_index = html
        .find(label)
        .unwrap_or_else(|| panic!("missing rendered option {label:?}"));
    let start = html[..label_index]
        .rfind("<option")
        .unwrap_or_else(|| panic!("missing <option before {label:?}"));
    let end = html[start..]
        .find('>')
        .map(|offset| start + offset)
        .unwrap_or_else(|| panic!("missing end of <option for {label:?}"));
    &html[start..=end]
}

fn is_selected(option: &str) -> bool {
    option.contains(" selected ")
        || option.contains(" selected=\"")
        || option.ends_with(" selected>")
}

#[test]
fn viewport_select_renders_the_ssr_control_the_browser_adapter_binds() {
    let html = render_admin(CapabilityState::full(), &[]);

    let select = opening_tag(&html, "data-fly-action=\"set-viewport\"", "<select");
    assert!(
        select.contains("data-fly-route-query=\"fly_viewport\""),
        "the viewport select must name the route key the adapter writes: {select}"
    );
}

#[test]
fn viewport_select_states_the_initial_frame_instead_of_the_first_preset() {
    let html = render_admin(CapabilityState::full(), &[]);

    let frame = opening_tag(&html, "data-fly-svg-viewport", "<svg");
    assert!(
        frame.contains("0 0 1280 720"),
        "the canvas frame must be the initial viewport: {frame}"
    );
    let current = option_tag(&html, "Custom 1280 × 720");
    assert!(
        is_selected(current),
        "the initial frame is no built-in preset and must be the selected option: {current}"
    );
    assert!(
        current.contains("value=\"\""),
        "the option of an unlisted frame must carry no preset id: {current}"
    );
    let first_preset = option_tag(&html, "Desktop 1440");
    assert!(
        !is_selected(first_preset),
        "a preset that is not in effect must not be selected: {first_preset}"
    );
}

#[test]
fn route_viewport_is_rendered_into_the_canvas_frame_and_the_select() {
    let html = render_admin(CapabilityState::full(), &[("fly_viewport", "tablet")]);

    let frame = opening_tag(&html, "data-fly-svg-viewport", "<svg");
    assert!(
        frame.contains("0 0 768 1024"),
        "the canvas frame must be the tablet preset named by the route: {frame}"
    );
    let tablet = option_tag(&html, "Tablet 768");
    assert!(
        tablet.contains("value=\"tablet\"") && is_selected(tablet),
        "the preset named by the route must be the selected option: {tablet}"
    );
    assert!(
        !html.contains("Custom 768"),
        "a frame that is a built-in preset must not render the unlisted-frame option"
    );
}

#[test]
fn route_preset_keeps_the_option_that_leads_back_to_the_initial_frame() {
    let html = render_admin(CapabilityState::full(), &[("fly_viewport", "tablet")]);

    // The initial frame is no preset, so no preset id can name it. Its option carries the empty
    // value, which the adapter turns into a route without the key.
    let base = option_tag(&html, "Custom 1280 × 720");
    assert!(
        base.contains("value=\"\""),
        "the option of the initial frame must carry no preset id: {base}"
    );
    assert!(
        !is_selected(base),
        "the initial frame is not in effect while the route names a preset: {base}"
    );
    assert!(is_selected(option_tag(&html, "Tablet 768")));
}

#[test]
fn route_viewport_applies_to_a_read_only_caller() {
    // A read-only caller has no draft session, which is why the viewport travels in the route.
    let html = render_admin(CapabilityState::read_only(), &[("fly_viewport", "mobile")]);

    let frame = opening_tag(&html, "data-fly-svg-viewport", "<svg");
    assert!(
        frame.contains("0 0 390 844"),
        "a read-only caller must get the viewport named by the route: {frame}"
    );
    assert!(is_selected(option_tag(&html, "Mobile 390")));
}

#[test]
fn unknown_route_viewport_renders_the_initial_frame() {
    let html = render_admin(CapabilityState::full(), &[("fly_viewport", "watch-ultra")]);

    let frame = opening_tag(&html, "data-fly-svg-viewport", "<svg");
    assert!(
        frame.contains("0 0 1280 720"),
        "an unknown preset id must leave the viewport untouched: {frame}"
    );
    assert!(is_selected(option_tag(&html, "Custom 1280 × 720")));
}

#[test]
fn palette_renders_the_ssr_controls_the_browser_adapter_binds() {
    let html = render_admin(CapabilityState::full(), &[]);

    let item = opening_tag(&html, "data-fly-block-id=", "<div");
    assert!(
        item.contains("data-fly-can-insert=\"true\"")
            && item.contains("data-fly-can-drag=\"true\""),
        "a palette item must state both capabilities: {item}"
    );
    let insert = opening_tag(&html, "data-fly-action=\"insert-block\"", "<button");
    assert!(
        !is_disabled(insert),
        "an allowed insert must be enabled: {insert}"
    );
    // The admin mounts the workspace in intent mode, which is what offers the drag control.
    let drag = opening_tag(&html, "data-fly-action=\"begin-block-drag\"", "<button");
    assert!(
        !is_disabled(drag),
        "an allowed drag must be enabled: {drag}"
    );
}

#[test]
fn palette_controls_follow_the_capabilities_of_a_read_only_caller() {
    let html = render_admin(CapabilityState::read_only(), &[]);

    let item = opening_tag(&html, "data-fly-block-id=", "<div");
    assert!(
        item.contains("data-fly-can-insert=\"false\"")
            && item.contains("data-fly-can-drag=\"false\""),
        "a read-only palette item must state both denials: {item}"
    );
    let insert = opening_tag(&html, "data-fly-action=\"insert-block\"", "<button");
    assert!(
        is_disabled(insert),
        "a denied insert must be disabled: {insert}"
    );
    let drag = opening_tag(&html, "data-fly-action=\"begin-block-drag\"", "<button");
    assert!(is_disabled(drag), "a denied drag must be disabled: {drag}");
}

#[test]
fn layer_rows_render_the_ssr_select_control() {
    let html = render_admin(CapabilityState::full(), &[]);

    let row = opening_tag(&html, "data-fly-component-id=\"hero\"", "<button");
    assert!(
        row.contains("data-fly-action=\"select-component\""),
        "a layer row must carry the select action next to its component id: {row}"
    );
    assert!(
        !is_disabled(row),
        "a selectable layer row must be enabled: {row}"
    );
}

// The classic SSR inspector. Its controls are bound by the host bootstrap
// (`PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS`), not by `fly-browser.js`: a form posts its fields
// as an intent, the picker retargets every form. The browser half is pinned by
// `apps/next-admin/tests/e2e/fly-browser-host-bootstrap.spec.ts`.

/// The rendered SSR inspector, from its section marker to the end of the document.
fn ssr_inspector(html: &str) -> &str {
    let start = html
        .find("data-fly-ssr-inspector=\"true\"")
        .expect("the composed admin must render the SSR inspector");
    &html[start..]
}

/// The `<form>` that posts `intent`, from its opening tag to its closing tag.
fn intent_form<'a>(html: &'a str, intent: &str) -> &'a str {
    let marker = format!("data-fly-intent-form=\"{intent}\"");
    let opening = opening_tag(html, &marker, "<form");
    let start = opening.as_ptr() as usize - html.as_ptr() as usize;
    let end = html[start..]
        .find("</form>")
        .map(|offset| start + offset)
        .unwrap_or_else(|| panic!("the {intent:?} form is never closed"));
    &html[start..end]
}

/// Value of the attribute `name` of an opening tag.
fn attribute<'a>(opening_tag: &'a str, name: &str) -> Option<&'a str> {
    let marker = format!(" {name}=\"");
    let start = opening_tag.find(&marker)? + marker.len();
    let end = start + opening_tag[start..].find('"')?;
    Some(&opening_tag[start..end])
}

#[test]
fn ssr_inspector_renders_the_forms_and_the_picker_the_host_bootstrap_binds() {
    let html = render_admin(CapabilityState::full(), &[]);
    let inspector = ssr_inspector(&html);

    for intent in [
        "patch_component_property",
        "patch_page_metadata",
        "create_page",
        "rename_page",
        "remove_page",
    ] {
        // Panics with the name of the form when the bootstrap would find nothing to bind.
        intent_form(inspector, intent);
    }
    let picker = opening_tag(inspector, "data-fly-component-picker=\"true\"", "<select");
    assert!(
        attribute(picker, "name").is_none(),
        "the picker retargets the forms and is not itself a field of one: {picker}"
    );
}

#[test]
fn selected_component_input_is_rendered_with_the_component_the_picker_shows() {
    let html = render_admin(CapabilityState::full(), &[]);
    let inspector = ssr_inspector(&html);

    let form = intent_form(inspector, "patch_component_property");
    let target = opening_tag(form, "data-fly-selected-component-input=\"true\"", "<input");
    assert_eq!(
        attribute(target, "name"),
        Some("component_id"),
        "the selection input is the `component_id` field of its form: {target}"
    );
    let rendered = attribute(target, "value").unwrap_or_default();
    assert!(
        !rendered.is_empty(),
        "without a selection the form must still name a component: {target}"
    );

    // A select without a selected option shows its first one.
    let picker_start = inspector
        .find("data-fly-component-picker=\"true\"")
        .expect("component picker");
    let options = &inspector[picker_start..];
    let options = &options[..options.find("</select>").expect("end of the picker")];
    let shown = opening_tag(options, "value=\"", "<option");
    assert_eq!(
        attribute(shown, "value"),
        Some(rendered),
        "the form must target the component the picker shows: {shown}"
    );
    assert!(
        options.contains("value=\"hero\""),
        "the picker must list every component of the active page: {options}"
    );
}

/// A project with one translation, so that the translations panel renders its bind form.
fn controller_with_translation() -> AdminCanvasController {
    AdminCanvasController::new(
        "home",
        "rev-translation-1",
        json!({
            "flyTranslations": [{ "id": "hero-title", "values": { "en": "Hero" } }],
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
    .expect("translation controller")
}

/// The selection input of the translation bind form, which is the only selection input of the
/// translations panel.
fn translation_bind_target(html: &str) -> &str {
    let start = html
        .find("data-fly-ssr-translations=\"true\"")
        .expect("the composed admin must render the SSR translations panel");
    let panel = &html[start..];
    let panel = &panel[..panel
        .find("</section>")
        .expect("end of the translations panel")];
    opening_tag(
        panel,
        "data-fly-selected-component-input=\"true\"",
        "<input",
    )
}

#[test]
fn translation_bind_form_is_rendered_with_the_selection_the_server_holds() {
    let mut controller = controller_with_translation();
    controller
        .dispatch(UiIntent::Select(Some("hero".to_string())))
        .expect("select the component to bind");
    let html = render_admin_with(controller, CapabilityState::full(), &[]);

    let target = translation_bind_target(&html);
    assert_eq!(
        attribute(target, "name"),
        Some("component_id"),
        "the selection input is the `component_id` field of the bind form: {target}"
    );
    // The adapter of a new document has no selection: without the rendered value the form
    // posts no target after a reload and the server refuses the binding.
    assert_eq!(
        attribute(target, "value"),
        Some("hero"),
        "the bind form must name the component the server holds as selected: {target}"
    );
}

#[test]
fn translation_bind_form_names_no_component_without_a_selection() {
    let html = render_admin_with(controller_with_translation(), CapabilityState::full(), &[]);

    let target = translation_bind_target(&html);
    assert_eq!(
        attribute(target, "value").unwrap_or_default(),
        "",
        "a binding has no default target; the server refuses the form by name: {target}"
    );
}

#[test]
fn page_forms_render_the_fields_their_request_types_read() {
    let html = render_admin(CapabilityState::full(), &[]);
    let inspector = ssr_inspector(&html);

    let create = intent_form(inspector, "create_page");
    let page_id = opening_tag(create, "name=\"page_id\"", "<input");
    assert!(
        page_id.contains(" required"),
        "a new page needs the id the author types: {page_id}"
    );
    opening_tag(create, "name=\"name\"", "<input");

    let rename = intent_form(inspector, "rename_page");
    opening_tag(rename, "name=\"page_id\"", "<select");
    assert!(
        rename.contains("value=\"home\""),
        "the page select must offer the pages of the project by id: {rename}"
    );
    let new_page_id = opening_tag(rename, "name=\"new_page_id\"", "<input");
    assert!(
        new_page_id.contains(" required"),
        "a rename must state the new id: {new_page_id}"
    );
    opening_tag(rename, "name=\"name\"", "<input");

    let remove = intent_form(inspector, "remove_page");
    opening_tag(remove, "name=\"page_id\"", "<select");

    let metadata = intent_form(inspector, "patch_page_metadata");
    let page = opening_tag(metadata, "name=\"page_id\"", "<input");
    assert_eq!(attribute(page, "value"), Some("home"));
    let no_index = opening_tag(metadata, "name=\"no_index\"", "<input");
    assert_eq!(
        attribute(no_index, "type"),
        Some("checkbox"),
        "`no_index` is a boolean of the request type: {no_index}"
    );
}
