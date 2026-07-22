use crate::{
    ActionMaterialization, BindingMaterialization, ContextMaterialization,
    InternalLinkMaterialization, LocalePolicyMaterialization, LocalizedPageMetadataMaterialization,
    ProjectDocument, RuntimeMaterialization, TranslationMaterialization, ValidationDiagnostic,
    extract_runtime_context_contract, materialize_bindings, materialize_component_actions,
    materialize_context, materialize_internal_page_links, materialize_localized_page_metadata,
    materialize_project_locale_context, materialize_project_translations, materialize_runtime,
    materialize_runtime_locale_context, validate_component_actions, validate_internal_page_links,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeProjectMaterialization {
    pub document: ProjectDocument,
    pub effective_context: Value,
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub defaults_applied: usize,
    pub computed_applied: usize,
    pub computed_fallbacks: usize,
    pub unresolved_computed: usize,
    pub context_type_mismatches: usize,
    pub resolved_internal_links: usize,
    pub fallback_internal_links: usize,
    pub unresolved_internal_links: usize,
    #[serde(default)]
    pub materialized_forms: usize,
    #[serde(default)]
    pub native_actions: usize,
    #[serde(default)]
    pub custom_actions: usize,
    #[serde(default)]
    pub fallback_actions: usize,
    #[serde(default)]
    pub unresolved_actions: usize,
    pub applied_bindings: usize,
    pub fallback_bindings: usize,
    pub unresolved_bindings: usize,
    pub evaluated_conditions: usize,
    pub hidden_components: usize,
    pub repeated_nodes: usize,
}

pub fn materialize_project_with_runtime_context(
    document: &ProjectDocument,
    input_context: &Value,
) -> RuntimeProjectMaterialization {
    let LocalePolicyMaterialization {
        context: locale_policy_context,
        diagnostics: locale_policy_diagnostics,
        ..
    } = materialize_project_locale_context(document, input_context);
    let TranslationMaterialization {
        context: translation_context,
        diagnostics: translation_diagnostics,
        ..
    } = materialize_project_translations(document, &locale_policy_context);
    let locale_materialization = materialize_runtime_locale_context(&translation_context);
    let localized_input_context = locale_materialization.context;

    let LocalizedPageMetadataMaterialization {
        document: localized_document,
        diagnostics: metadata_diagnostics,
        ..
    } = materialize_localized_page_metadata(document, &localized_input_context);
    let contract = extract_runtime_context_contract(&localized_document);
    let contract_is_valid = contract.is_valid();
    let mut diagnostics = locale_policy_diagnostics;
    diagnostics.extend(translation_diagnostics);
    diagnostics.extend(locale_materialization.diagnostics);
    diagnostics.extend(metadata_diagnostics);
    diagnostics.extend(contract.definition_diagnostics);

    let (
        effective_context,
        defaults_applied,
        computed_applied,
        computed_fallbacks,
        unresolved_computed,
        context_type_mismatches,
    ) = if contract_is_valid {
        let ContextMaterialization {
            context,
            diagnostics: context_diagnostics,
            defaults_applied,
            computed_applied,
            computed_fallbacks,
            unresolved_computed,
            type_mismatches,
        } = materialize_context(&localized_document, &localized_input_context);
        diagnostics.extend(context_diagnostics);
        (
            context,
            defaults_applied,
            computed_applied,
            computed_fallbacks,
            unresolved_computed,
            type_mismatches,
        )
    } else {
        (localized_input_context.clone(), 0, 0, 0, 0, 0)
    };

    // Runtime bindings are allowed to target component fields such as flyPageLink, flyAction,
    // flyForm, and tagName. Apply them before structural runtime expansion so repeaters clone the
    // bound contract rather than the authoring template.
    let BindingMaterialization {
        document: bound_document,
        diagnostics: binding_diagnostics,
        applied_bindings,
        fallback_bindings,
        unresolved_bindings,
    } = materialize_bindings(&localized_document, &effective_context);
    diagnostics.extend(binding_diagnostics);

    let RuntimeMaterialization {
        document: dynamic_document,
        diagnostics: dynamic_diagnostics,
        evaluated_conditions,
        hidden_components,
        repeated_nodes,
    } = materialize_runtime(&bound_document, &effective_context);
    diagnostics.extend(dynamic_diagnostics);

    // Bindings and repeaters may introduce or duplicate runtime navigation, action, and form
    // contracts. Validate the effective document before lowering those contracts to native HTML.
    diagnostics.extend(validate_internal_page_links(&dynamic_document));
    diagnostics.extend(validate_component_actions(&dynamic_document));

    // Resolve navigation and action contracts only after conditions/repeaters. This guarantees
    // that generated nodes receive native href/form/button attributes and hidden nodes do not
    // contribute stale runtime diagnostics or counters.
    let InternalLinkMaterialization {
        document: linked_document,
        diagnostics: link_diagnostics,
        resolved_links: resolved_internal_links,
        fallback_links: fallback_internal_links,
        unresolved_links: unresolved_internal_links,
    } = materialize_internal_page_links(&dynamic_document, &effective_context);
    diagnostics.extend(link_diagnostics);

    let ActionMaterialization {
        document,
        diagnostics: action_diagnostics,
        materialized_forms,
        native_actions,
        custom_actions,
        fallback_actions,
        unresolved_actions,
    } = materialize_component_actions(&linked_document, &effective_context);
    diagnostics.extend(action_diagnostics);

    RuntimeProjectMaterialization {
        document,
        effective_context,
        diagnostics,
        defaults_applied,
        computed_applied,
        computed_fallbacks,
        unresolved_computed,
        context_type_mismatches,
        resolved_internal_links,
        fallback_internal_links,
        unresolved_internal_links,
        materialized_forms,
        native_actions,
        custom_actions,
        fallback_actions,
        unresolved_actions,
        applied_bindings,
        fallback_bindings,
        unresolved_bindings,
        evaluated_conditions,
        hidden_components,
        repeated_nodes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GrapesJsCodec, PageMetadata, ValidationSeverity};
    use serde_json::json;

    #[test]
    fn pipeline_exposes_effective_context_and_materialized_document() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "title",
                        "type": "text",
                        "content": "Static"
                    }]
                }
            }],
            "flyRuntimeContextSchema": [{
                "id": "prefix",
                "path": "page.prefix",
                "kind": "string",
                "default": "Hello"
            }],
            "flyRuntimeComputed": [{
                "id": "title",
                "path": "page.title",
                "expression": {
                    "op": "format",
                    "template": "{{page.prefix}} world"
                }
            }],
            "flyRuntimeBindings": [{
                "id": "title-content",
                "component_id": "title",
                "path": "page.title",
                "target": "field",
                "name": "content"
            }]
        }))
        .expect("document");
        let materialized = materialize_project_with_runtime_context(&document, &json!({}));
        assert_eq!(
            materialized.effective_context["page"]["title"],
            "Hello world"
        );
        assert_eq!(materialized.defaults_applied, 1);
        assert_eq!(materialized.computed_applied, 1);
        assert_eq!(materialized.applied_bindings, 1);
        assert_eq!(
            materialized
                .document
                .component("title")
                .and_then(|component| component.extensions.get("content"))
                .and_then(Value::as_str),
            Some("Hello world")
        );
    }

    #[test]
    fn project_locale_policy_defaults_before_translation_materialization() {
        let document = GrapesJsCodec::decode_value(json!({
            "flyLocales": {
                "default_locale": "ru",
                "supported_locales": ["ru", "en"],
                "fallback_locales": ["en"]
            },
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "title",
                        "type": "text",
                        "content": "Static"
                    }]
                }
            }],
            "flyTranslations": [{
                "id": "hero_title",
                "values": {
                    "en": "Welcome",
                    "ru": "Добро пожаловать"
                }
            }],
            "flyRuntimeBindings": [{
                "id": "hero-title-content",
                "component_id": "title",
                "path": "translations.hero_title",
                "target": "field",
                "name": "content"
            }]
        }))
        .expect("document");
        let materialized = materialize_project_with_runtime_context(&document, &json!({}));
        assert_eq!(materialized.effective_context["$locale"], "ru");
        assert_eq!(
            materialized.effective_context["$fallback_locales"],
            json!(["en"])
        );
        assert_eq!(
            materialized.effective_context["translations"]["hero_title"],
            "Добро пожаловать"
        );
        assert_eq!(
            materialized
                .document
                .component("title")
                .and_then(|component| component.extensions.get("content"))
                .and_then(Value::as_str),
            Some("Добро пожаловать")
        );
    }

    #[test]
    fn internal_page_links_materialize_after_bindings_and_repeaters() {
        let document = GrapesJsCodec::decode_value(json!({
            "flyLocales": {
                "default_locale": "ru",
                "supported_locales": ["ru", "en"]
            },
            "pages": [{
                "id": "home",
                "flyPageMeta": { "slug": { "$localized": { "en": "home", "ru": "glavnaya" } } },
                "component": {
                    "id": "home-root",
                    "type": "wrapper",
                    "components": [{
                        "id": "about-link",
                        "type": "link",
                        "tagName": "a",
                        "flyPageLink": { "page_id": "about" }
                    }]
                }
            }, {
                "id": "about",
                "flyPageMeta": { "slug": { "$localized": { "en": "about", "ru": "o-nas" } } },
                "component": { "id": "about-root", "type": "wrapper" }
            }]
        }))
        .expect("document");
        let materialized = materialize_project_with_runtime_context(&document, &json!({}));
        assert_eq!(materialized.resolved_internal_links, 1);
        assert_eq!(
            materialized
                .document
                .component("about-link")
                .unwrap()
                .attributes["href"],
            "/o-nas"
        );
        assert!(
            document
                .component("about-link")
                .unwrap()
                .attributes
                .get("href")
                .is_none()
        );
    }

    #[test]
    fn actions_and_forms_materialize_in_the_canonical_runtime_pipeline() {
        let document = GrapesJsCodec::decode_value(json!({
            "flyLocales": {
                "default_locale": "ru",
                "supported_locales": ["ru", "en"]
            },
            "pages": [{
                "id": "home",
                "flyPageMeta": { "slug": { "$localized": { "en": "home", "ru": "glavnaya" } } },
                "component": {
                    "id": "home-root",
                    "type": "wrapper",
                    "components": [{
                        "id": "contact-form",
                        "type": "wrapper",
                        "flyForm": {
                            "id": "contact",
                            "method": "post",
                            "provider": "crm",
                            "action": "create_lead"
                        }
                    }, {
                        "id": "submit",
                        "type": "button",
                        "flyAction": { "kind": "submit_form", "form_id": "contact" }
                    }, {
                        "id": "about",
                        "type": "button",
                        "flyAction": { "kind": "navigate_page", "page_id": "about-page" }
                    }]
                }
            }, {
                "id": "about-page",
                "flyPageMeta": { "slug": { "$localized": { "en": "about", "ru": "o-nas" } } },
                "component": { "id": "about-root", "type": "wrapper" }
            }]
        }))
        .expect("document");

        let materialized = materialize_project_with_runtime_context(&document, &json!({}));
        assert_eq!(materialized.materialized_forms, 1);
        assert_eq!(materialized.native_actions, 2);
        assert_eq!(materialized.custom_actions, 0);
        assert_eq!(materialized.unresolved_actions, 0);
        assert_eq!(
            materialized
                .document
                .component("contact-form")
                .unwrap()
                .tag_name
                .as_deref(),
            Some("form")
        );
        assert_eq!(
            materialized
                .document
                .component("submit")
                .unwrap()
                .attributes["form"],
            "contact"
        );
        assert_eq!(
            materialized.document.component("about").unwrap().attributes["href"],
            "/o-nas"
        );
        assert!(
            document
                .component("contact-form")
                .unwrap()
                .attributes
                .get("data-fly-form-provider")
                .is_none()
        );
    }

    #[test]
    fn runtime_binding_can_supply_action_before_native_materialization() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "flyPageMeta": { "slug": "home" },
                "component": {
                    "id": "home-root",
                    "type": "wrapper",
                    "components": [{
                        "id": "cta",
                        "type": "button",
                        "content": "About"
                    }]
                }
            }, {
                "id": "about",
                "flyPageMeta": { "slug": "about" },
                "component": { "id": "about-root", "type": "wrapper" }
            }],
            "flyRuntimeBindings": [{
                "id": "cta-action",
                "component_id": "cta",
                "path": "cta.action",
                "target": "field",
                "name": "flyAction"
            }]
        }))
        .expect("document");

        let materialized = materialize_project_with_runtime_context(
            &document,
            &json!({
                "cta": {
                    "action": { "kind": "navigate_page", "page_id": "about" }
                }
            }),
        );
        assert_eq!(materialized.applied_bindings, 1);
        assert_eq!(materialized.native_actions, 1);
        assert_eq!(materialized.unresolved_actions, 0);
        assert_eq!(
            materialized.document.component("cta").unwrap().attributes["href"],
            "/about"
        );
    }

    #[test]
    fn runtime_bound_navigation_conflict_is_validated_before_materialization() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "flyPageMeta": { "slug": "home" },
                "component": {
                    "id": "home-root",
                    "type": "wrapper",
                    "components": [{
                        "id": "cta",
                        "type": "link",
                        "content": "About",
                        "flyPageLink": { "page_id": "about" }
                    }]
                }
            }, {
                "id": "about",
                "flyPageMeta": { "slug": "about" },
                "component": { "id": "about-root", "type": "wrapper" }
            }],
            "flyRuntimeBindings": [{
                "id": "cta-action",
                "component_id": "cta",
                "path": "cta.action",
                "target": "field",
                "name": "flyAction"
            }]
        }))
        .expect("document");

        let materialized = materialize_project_with_runtime_context(
            &document,
            &json!({
                "cta": {
                    "action": { "kind": "navigate_page", "page_id": "about" }
                }
            }),
        );
        assert!(materialized.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "component_navigation_contract_conflict"
                && diagnostic.severity == ValidationSeverity::Error
        }));
    }

    #[test]
    fn locale_resolution_runs_before_computed_values_and_bindings() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "title",
                        "type": "text",
                        "content": "Static"
                    }]
                }
            }],
            "flyRuntimeComputed": [{
                "id": "title",
                "path": "page.title",
                "expression": {
                    "op": "format",
                    "template": "{{page.prefix}} мир"
                }
            }],
            "flyRuntimeBindings": [{
                "id": "title-content",
                "component_id": "title",
                "path": "page.title",
                "target": "field",
                "name": "content"
            }]
        }))
        .expect("document");
        let materialized = materialize_project_with_runtime_context(
            &document,
            &json!({
                "$locale": "ru-RU",
                "page": {
                    "prefix": {
                        "$localized": {
                            "en": "Hello",
                            "ru": "Привет"
                        }
                    }
                }
            }),
        );
        assert_eq!(materialized.effective_context["page"]["prefix"], "Привет");
        assert_eq!(
            materialized.effective_context["page"]["title"],
            "Привет мир"
        );
        assert_eq!(
            materialized
                .document
                .component("title")
                .and_then(|component| component.extensions.get("content"))
                .and_then(Value::as_str),
            Some("Привет мир")
        );
        assert!(
            materialized
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "runtime_localized_value_fallback")
        );
    }

    #[test]
    fn project_translation_catalog_materializes_before_bindings() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "title",
                        "type": "text",
                        "content": "Static"
                    }]
                }
            }],
            "flyTranslations": [{
                "id": "hero_title",
                "values": {
                    "en": "Welcome",
                    "ru": "Добро пожаловать"
                },
                "fallback_locale": "en"
            }],
            "flyRuntimeBindings": [{
                "id": "hero-title-content",
                "component_id": "title",
                "path": "translations.hero_title",
                "target": "field",
                "name": "content"
            }]
        }))
        .expect("document");
        let materialized =
            materialize_project_with_runtime_context(&document, &json!({ "$locale": "ru-RU" }));
        assert_eq!(
            materialized.effective_context["translations"]["hero_title"],
            "Добро пожаловать"
        );
        assert_eq!(materialized.applied_bindings, 1);
        assert_eq!(
            materialized
                .document
                .component("title")
                .and_then(|component| component.extensions.get("content"))
                .and_then(Value::as_str),
            Some("Добро пожаловать")
        );
    }

    #[test]
    fn localized_page_metadata_is_materialized_before_render_selection() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "flyPageMeta": {
                    "title": {
                        "$localized": {
                            "en": "Home",
                            "ru": "Главная"
                        }
                    },
                    "description": {
                        "$localized": {
                            "en": "English description",
                            "ru": "Русское описание"
                        }
                    }
                },
                "component": { "id": "root", "type": "wrapper" }
            }]
        }))
        .expect("document");
        let materialized =
            materialize_project_with_runtime_context(&document, &json!({ "$locale": "ru-RU" }));
        let metadata = PageMetadata::from_page(&materialized.document.project.pages[0]);
        assert_eq!(metadata.title.as_deref(), Some("Главная"));
        assert_eq!(metadata.description.as_deref(), Some("Русское описание"));
    }

    #[test]
    fn invalid_context_contract_does_not_replace_localized_root_context() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "title",
                        "type": "text",
                        "content": "Static"
                    }]
                }
            }],
            "flyRuntimeContextSchema": [{
                "id": "invalid-root",
                "path": "",
                "kind": "object",
                "default": { "replaced": true }
            }]
        }))
        .expect("document");
        let input = json!({
            "$locale": "ru",
            "safe": {
                "$localized": {
                    "en": "safe",
                    "ru": "безопасно"
                }
            }
        });
        let materialized = materialize_project_with_runtime_context(&document, &input);
        assert_eq!(materialized.effective_context["safe"], "безопасно");
        assert_eq!(materialized.defaults_applied, 0);
        assert!(
            materialized
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "runtime_context_field_path_invalid")
        );
    }
}
