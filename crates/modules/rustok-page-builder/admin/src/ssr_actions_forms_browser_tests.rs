use crate::{AdminCanvasController, dispatch_browser_intent};
use fly_browser::{BrowserIntentEnvelope, FLY_BROWSER_PROTOCOL};
use serde_json::{Value, json};

fn controller() -> AdminCanvasController {
    AdminCanvasController::new(
        "home",
        "rev-1",
        json!({
            "pages": [{
                "id": "home",
                "flyPageMeta": { "slug": "home" },
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [
                        { "id": "cta", "type": "button" },
                        { "id": "form", "type": "wrapper" },
                        { "id": "field", "type": "input" }
                    ]
                }
            }, {
                "id": "about",
                "flyPageMeta": { "slug": "about" },
                "component": { "id": "about-root", "type": "wrapper" }
            }]
        }),
    )
    .expect("controller")
}

fn intent(controller: &AdminCanvasController, name: &str, payload: Value) -> BrowserIntentEnvelope {
    BrowserIntentEnvelope {
        protocol: FLY_BROWSER_PROTOCOL.to_string(),
        instance_id: "ssr-actions-forms".to_string(),
        intent: name.to_string(),
        payload,
        sequence: Some(1),
        page_id: Some(controller.page_id().to_string()),
        revision: Some(controller.revision_id().to_string()),
        project_hash: Some(controller.editor().revision().project_hash.hex()),
        draft_token: None,
        draft_generation: None,
        operation_id: None,
    }
}

#[test]
fn browser_dispatches_action_form_and_native_field_contracts() {
    let mut controller = controller();

    let request = intent(
        &controller,
        "set_component_action",
        json!({
            "component_id": "cta",
            "kind": "navigate_page",
            "page_id": "about"
        }),
    );
    let action = dispatch_browser_intent(&mut controller, request).expect("action dispatch");
    assert!(action.dirty);
    assert_eq!(
        controller
            .editor()
            .document()
            .component("cta")
            .unwrap()
            .extensions["flyAction"]["page_id"],
        "about"
    );

    let request = intent(
        &controller,
        "set_component_form",
        json!({
            "component_id": "form",
            "form_id": "contact",
            "method": "post",
            "provider": "crm",
            "action": "create_lead",
            "input_json": "{\"source\":\"landing\"}"
        }),
    );
    dispatch_browser_intent(&mut controller, request).expect("form dispatch");
    assert_eq!(
        controller
            .editor()
            .document()
            .component("form")
            .unwrap()
            .extensions["flyForm"]["id"],
        "contact"
    );

    let request = intent(
        &controller,
        "set_native_form_field",
        json!({
            "component_id": "field",
            "tag_name": "input",
            "name": "email",
            "field_type": "email",
            "required": true,
            "min_length": 3,
            "max_length": 120,
            "pattern": ".+@.+",
            "autocomplete": "email",
            "aria_label": "Email"
        }),
    );
    dispatch_browser_intent(&mut controller, request).expect("field dispatch");
    let field = controller.editor().document().component("field").unwrap();
    assert_eq!(field.attributes["name"], "email");
    assert_eq!(field.attributes["minlength"], "3");
    assert_eq!(field.attributes["autocomplete"], "email");
    assert!(field.attributes.contains_key("required"));
}

#[test]
fn stale_action_form_mutation_is_rejected_before_dispatch() {
    let mut controller = controller();
    let mut request = intent(
        &controller,
        "set_component_action",
        json!({
            "component_id": "cta",
            "kind": "navigate_page",
            "page_id": "about"
        }),
    );
    request.project_hash = Some("stale".to_string());
    let error = dispatch_browser_intent(&mut controller, request).expect_err("stale hash");
    assert!(error.to_string().contains("project hash conflict"));
}

#[test]
fn switch_slide_form_rejects_missing_malformed_and_ambiguous_selectors_atomically() {
    let invalid_payloads = [
        json!({
            "component_id": "cta",
            "kind": "switch_slide",
            "slider_id": "carousel"
        }),
        json!({
            "component_id": "cta",
            "kind": "switch_slide",
            "slider_id": "carousel",
            "direction": "sideways"
        }),
        json!({
            "component_id": "cta",
            "kind": "switch_slide",
            "slider_id": "carousel",
            "index": "not-an-index"
        }),
        json!({
            "component_id": "cta",
            "kind": "switch_slide",
            "slider_id": "carousel",
            "direction": "next",
            "index": "1"
        }),
        json!({
            "component_id": "cta",
            "kind": "switch_slide",
            "slider_id": "carousel",
            "direction": "prev",
            "target_slide": "slide-2"
        }),
    ];

    for payload in invalid_payloads {
        let mut controller = controller();
        let before = controller.clone();
        let request = intent(&controller, "set_component_action", payload);
        assert!(
            dispatch_browser_intent(&mut controller, request).is_err(),
            "invalid slide selector payload must be rejected"
        );
        assert_eq!(
            controller, before,
            "a rejected slide action must not mutate the document"
        );
    }
}

#[test]
fn switch_slide_form_accepts_each_supported_selector_individually() {
    for payload in [
        json!({
            "component_id": "cta",
            "kind": "switch_slide",
            "slider_id": "carousel",
            "direction": "next"
        }),
        json!({
            "component_id": "cta",
            "kind": "switch_slide",
            "slider_id": "carousel",
            "index": "1"
        }),
        json!({
            "component_id": "cta",
            "kind": "switch_slide",
            "slider_id": "carousel",
            "target_slide": "slide-2"
        }),
    ] {
        let mut controller = controller();
        let request = intent(&controller, "set_component_action", payload);
        let result =
            dispatch_browser_intent(&mut controller, request).expect("valid slide selector");
        assert!(result.dirty);
    }
}

#[test]
fn ssr_action_form_preserves_exclusive_accordion_actions() {
    let source = include_str!("editor/ssr_actions_forms.rs");
    assert!(source.contains("name=\"exclusive\""));
    assert!(source.contains("checked=action_values.exclusive"));

    let mut controller = controller();
    let request = intent(
        &controller,
        "set_component_action",
        json!({
            "component_id": "cta",
            "kind": "toggle_accordion",
            "target_id": "panel",
            "exclusive": true
        }),
    );
    dispatch_browser_intent(&mut controller, request).expect("accordion action dispatch");
    assert_eq!(
        controller.editor().document().component("cta").unwrap().extensions[fly::FLY_ACTION_FIELD]["exclusive"],
        true
    );
}
