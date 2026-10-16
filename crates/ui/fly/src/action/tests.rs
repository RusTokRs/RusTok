use super::*;
use crate::{GrapesJsCodec, ProjectDocument};
use serde_json::json;

fn document() -> ProjectDocument {
    GrapesJsCodec::decode_value(json!({
        "flyLocales": {
            "default_locale": "ru",
            "supported_locales": ["ru", "en"]
        },
        "pages": [{
            "id": "home",
            "flyPageMeta": { "slug": { "$localized": { "en": "home", "ru": "glavnaya" } } },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "contact-form",
                    "type": "wrapper",
                    "flyForm": {
                        "id": "contact",
                        "method": "post",
                        "provider": "crm",
                        "action": "create_lead",
                        "input": { "source": "landing" }
                    }
                }, {
                    "id": "submit",
                    "type": "button",
                    "flyAction": { "kind": "submit_form", "form_id": "contact" }
                }, {
                    "id": "about",
                    "type": "button",
                    "flyAction": { "kind": "navigate_page", "page_id": "about-page" }
                }, {
                    "id": "track",
                    "type": "button",
                    "flyAction": {
                        "kind": "emit_event",
                        "event": "marketing.cta",
                        "payload": { "campaign": "summer" }
                    }
                }]
            }
        }, {
            "id": "about-page",
            "flyPageMeta": { "slug": { "$localized": { "en": "about", "ru": "o-nas" } } },
            "component": { "id": "about-root", "type": "wrapper" }
        }]
    }))
    .expect("document")
}

#[test]
fn actions_and_forms_materialize_to_native_and_custom_contracts() {
    let document = document();
    let result = materialize_component_actions(&document, &json!({ "$locale": "ru" }));
    assert_eq!(result.materialized_forms, 1);
    assert_eq!(result.native_actions, 2);
    assert_eq!(result.custom_actions, 1);
    let form = result.document.component("contact-form").unwrap();
    assert_eq!(form.tag_name.as_deref(), Some("form"));
    assert_eq!(form.attributes["data-fly-form-provider"], "crm");
    assert_eq!(
        result.document.component("submit").unwrap().attributes["form"],
        "contact"
    );
    assert_eq!(
        result.document.component("about").unwrap().attributes["href"],
        "/o-nas"
    );
    let track = result.document.component("track").unwrap();
    assert_eq!(track.tag_name.as_deref(), Some("button"));
    assert_eq!(track.attributes[FLY_ACTION_KIND_ATTRIBUTE], "emit_event");
}

#[test]
fn materialization_clears_stale_interaction_attributes() {
    let document = GrapesJsCodec::decode_value(json!({
        "pages": [{
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "search-form",
                    "type": "wrapper",
                    "attributes": {
                        "action": "/current",
                        "enctype": "multipart/form-data",
                        "novalidate": "",
                        "data-fly-form-provider": "current",
                        "data-fly-form-action": "send",
                        "data-fly-form-input": "{}",
                        "href": "/stale",
                        "target": "_blank",
                        "type": "button"
                    },
                    "flyForm": { "id": "search", "method": "get" }
                }, {
                    "id": "track",
                    "type": "button",
                    "tagName": "a",
                    "attributes": {
                        "href": "/current",
                        "target": "_blank",
                        "rel": "opener",
                        "form": "current-form",
                        "action": "/current-submit",
                        "method": "post",
                        "enctype": "multipart/form-data",
                        "novalidate": "",
                        "data-fly-form-provider": "current",
                        "data-fly-action": "current"
                    },
                    "flyAction": {
                        "kind": "emit_event",
                        "event": "analytics.track"
                    }
                }]
            }
        }]
    }))
    .expect("document");

    let result = materialize_component_actions(&document, &json!({}));
    let form = result.document.component("search-form").unwrap();
    assert_eq!(form.tag_name.as_deref(), Some("form"));
    assert_eq!(form.attributes["method"], "get");
    for attribute in [
        "action",
        "enctype",
        "novalidate",
        "data-fly-form-provider",
        "data-fly-form-action",
        "data-fly-form-input",
        "href",
        "target",
        "type",
    ] {
        assert!(!form.attributes.contains_key(attribute), "{attribute}");
    }

    let action = result.document.component("track").unwrap();
    assert_eq!(action.tag_name.as_deref(), Some("button"));
    assert_eq!(action.attributes["type"], "button");
    assert_eq!(action.attributes[FLY_ACTION_KIND_ATTRIBUTE], "emit_event");
    assert!(action.attributes.contains_key(FLY_ACTION_DATA_ATTRIBUTE));
    for attribute in [
        "href",
        "target",
        "rel",
        "form",
        "action",
        "method",
        "enctype",
        "novalidate",
        "data-fly-form-provider",
    ] {
        assert!(!action.attributes.contains_key(attribute), "{attribute}");
    }
}

#[test]
fn missing_form_and_unsafe_url_are_blocking_validation() {
    let document = GrapesJsCodec::decode_value(json!({
        "pages": [{
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "submit",
                    "type": "button",
                    "flyAction": { "kind": "submit_form", "form_id": "missing" }
                }, {
                    "id": "bad-link",
                    "type": "link",
                    "flyAction": { "kind": "navigate_url", "href": "javascript:alert(1)" }
                }]
            }
        }]
    }))
    .expect("document");
    let diagnostics = validate_component_actions(&document);
    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == ValidationSeverity::Error)
            .count(),
        2
    );
}

#[test]
fn materialization_does_not_emit_invalid_action_attributes() {
    let document = GrapesJsCodec::decode_value(json!({
        "pages": [{
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "bad-link",
                    "type": "link",
                    "tagName": "a",
                    "attributes": {
                        "href": "/stale",
                        "target": "_blank",
                        "rel": "opener",
                        "data-fly-action-kind": "navigate_url"
                    },
                    "flyAction": { "kind": "navigate_url", "href": "javascript:alert(1)" }
                }]
            }
        }]
    }))
    .expect("document");
    let result = materialize_component_actions(&document, &json!({}));
    assert_eq!(result.unresolved_actions, 1);
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "runtime_action_invalid" && diagnostic.path == "component:bad-link"
    }));
    let component = result.document.component("bad-link").unwrap();
    assert!(!component.attributes.contains_key("href"));
    assert!(!component.attributes.contains_key("target"));
    assert!(!component.attributes.contains_key("rel"));
    assert!(!component.attributes.contains_key(FLY_ACTION_KIND_ATTRIBUTE));
}

#[test]
fn materialization_clears_conflicting_interaction_contracts() {
    let document = GrapesJsCodec::decode_value(json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": { "slug": "home" },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "conflict",
                    "type": "link",
                    "tagName": "a",
                    "attributes": { "href": "/stale", "data-fly-action-kind": "navigate_page" },
                    "flyPageLink": { "page_id": "home" },
                    "flyAction": { "kind": "navigate_page", "page_id": "home" }
                }]
            }
        }]
    }))
    .expect("document");
    let result = materialize_component_actions(&document, &json!({}));
    assert_eq!(result.unresolved_actions, 1);
    let component = result.document.component("conflict").unwrap();
    assert!(!component.attributes.contains_key("href"));
    assert!(!component.attributes.contains_key(FLY_ACTION_KIND_ATTRIBUTE));
}

#[test]
fn network_paths_and_backslash_urls_are_blocking_validation() {
    let document = GrapesJsCodec::decode_value(json!({
        "pages": [{
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "network-link",
                    "type": "link",
                    "flyAction": {
                        "kind": "navigate_url",
                        "href": "//attacker.example/path"
                    }
                }, {
                    "id": "unsafe-form",
                    "type": "wrapper",
                    "flyForm": {
                        "id": "unsafe",
                        "method": "post",
                        "action_url": "/\\attacker.example/submit"
                    }
                }]
            }
        }]
    }))
    .expect("document");
    let diagnostics = validate_component_actions(&document);
    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == ValidationSeverity::Error)
            .count(),
        2
    );
}

#[test]
fn duplicate_forms_and_interaction_conflicts_are_rejected() {
    let document = GrapesJsCodec::decode_value(json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": { "slug": "home" },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "one",
                    "type": "wrapper",
                    "flyForm": { "id": "same" }
                }, {
                    "id": "two",
                    "type": "wrapper",
                    "flyForm": { "id": "same" }
                }, {
                    "id": "navigation-conflict",
                    "type": "link",
                    "flyPageLink": { "page_id": "home" },
                    "flyAction": { "kind": "navigate_page", "page_id": "home" }
                }, {
                    "id": "form-action-conflict",
                    "type": "wrapper",
                    "flyForm": { "id": "combined" },
                    "flyAction": { "kind": "emit_event", "event": "submit" }
                }]
            }
        }]
    }))
    .expect("document");
    let diagnostics = validate_component_actions(&document);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "duplicate_form_id")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "component_navigation_contract_conflict" })
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "component_form_interaction_contract_conflict"
        })
    );
}

#[test]
fn non_post_encoding_is_rejected() {
    let document = GrapesJsCodec::decode_value(json!({
        "pages": [{
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "invalid-form",
                    "type": "wrapper",
                    "flyForm": {
                        "id": "search",
                        "method": "get",
                        "encoding": "multipart"
                    }
                }]
            }
        }]
    }))
    .expect("document");
    let diagnostics = validate_component_actions(&document);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "form_definition_invalid"
            && diagnostic.message.contains("encoding requires post")
    }));
}

#[test]
fn form_action_url_matches_renderer_policy() {
    let document = GrapesJsCodec::decode_value(json!({
        "pages": [{
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "mailto-form",
                    "type": "form",
                    "flyForm": { "id": "contact", "action_url": "mailto:sales@example.com" }
                }, {
                    "id": "absolute-form",
                    "type": "form",
                    "flyForm": { "id": "signup", "action_url": "https://example.com/signup" }
                }]
            }
        }]
    }))
    .expect("document");
    let diagnostics = validate_component_actions(&document);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "form_definition_invalid" && diagnostic.path == "component:mailto-form"
    }));
    assert!(!diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "form_definition_invalid" && diagnostic.path == "component:absolute-form"
    }));
}

#[test]
fn anonymous_action_diagnostics_use_the_shared_canonical_path() {
    let document = GrapesJsCodec::decode_value(json!({
        "pages": [{
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "type": "button",
                    "flyAction": { "kind": "submit_form", "form_id": "missing" }
                }]
            }
        }]
    }))
    .expect("document");
    let diagnostics = validate_component_actions(&document);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.path == "project.pages[0].component.components[0]" })
    );
}

#[test]
fn switch_tab_and_toggle_accordion_materialize_correctly() {
    let document = GrapesJsCodec::decode_value(json!({
        "pages": [{
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [
                    {
                        "id": "tab-btn",
                        "type": "button",
                        "flyAction": { "kind": "switch_tab", "panel_id": "overview-panel" }
                    },
                    {
                        "id": "accordion-trigger",
                        "type": "button",
                        "flyAction": { "kind": "toggle_accordion", "target_id": "faq-content", "exclusive": true }
                    }
                ]
            }
        }]
    }))
    .expect("document");
    let result = materialize_component_actions(&document, &json!({}));
    assert_eq!(result.custom_actions, 2);

    let tab_btn = result.document.component("tab-btn").expect("tab-btn");
    assert_eq!(tab_btn.attributes[FLY_ACTION_KIND_ATTRIBUTE], "switch_tab");
    assert_eq!(tab_btn.attributes["data-fly-action-target"], "overview-panel");
    let tab_action: serde_json::Value = serde_json::from_str(
        tab_btn.attributes[FLY_ACTION_DATA_ATTRIBUTE]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(tab_action["kind"], "switch_tab");
    assert_eq!(tab_action["panel_id"], "overview-panel");

    let accordion_trigger = result
        .document
        .component("accordion-trigger")
        .expect("accordion-trigger");
    assert_eq!(
        accordion_trigger.attributes[FLY_ACTION_KIND_ATTRIBUTE],
        "toggle_accordion"
    );
    assert_eq!(
        accordion_trigger.attributes["data-fly-action-target"],
        "faq-content"
    );
    let accordion_action: serde_json::Value = serde_json::from_str(
        accordion_trigger.attributes[FLY_ACTION_DATA_ATTRIBUTE]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(accordion_action["kind"], "toggle_accordion");
    assert_eq!(accordion_action["target_id"], "faq-content");
    assert_eq!(accordion_action["exclusive"], true);
}

#[test]
fn switch_tab_and_toggle_accordion_validation_rejects_empty_targets() {
    let document = GrapesJsCodec::decode_value(json!({
        "pages": [{
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [
                    {
                        "id": "bad-tab",
                        "type": "button",
                        "flyAction": { "kind": "switch_tab", "panel_id": "   " }
                    },
                    {
                        "id": "bad-accordion",
                        "type": "button",
                        "flyAction": { "kind": "toggle_accordion", "target_id": "" }
                    }
                ]
            }
        }]
    }))
    .expect("document");
    let diagnostics = validate_component_actions(&document);
    assert!(diagnostics.iter().any(|d| d.code == "action_definition_invalid" && d.path == "component:bad-tab"));
    assert!(diagnostics.iter().any(|d| d.code == "action_definition_invalid" && d.path == "component:bad-accordion"));
}

