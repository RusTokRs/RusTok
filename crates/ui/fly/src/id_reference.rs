//! Component-id reference rewriting, used when a pasted fragment is reissued fresh ids.
//!
//! # Why this module exists
//!
//! Remapping used to be implemented as "walk the whole JSON subtree and replace *any* string that
//! equals an old id". That is wrong in both directions:
//!
//! * **False positives.** Ids are short, author-chosen words like `hero`, `title` or `email`. A
//!   component whose id was `title` would silently rewrite an unrelated `placeholder: "title"`,
//!   a text node reading `"title"`, an asset filename, or a CSS value — corrupting content that
//!   has nothing to do with identity.
//! * **False negatives.** A reference embedded in a larger string, such as the anchor `#hero` or
//!   a space-separated `aria-labelledby` list, did not match at all and was left dangling.
//!
//! The fix is to rewrite only *positions that are defined to hold an id*, rather than guessing
//! from the value. Two kinds of position exist, and they are enumerated below.

use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// JSON object keys whose string value is a component-id reference.
///
/// `component_id` is the serialized field name shared by `RuntimeBinding`, `RuntimeCondition`,
/// `RuntimeRepeater`, `StyleRuleDescriptor` and the landing contract snapshots; `flyComponentId`
/// is [`crate::FLY_COMPONENT_RULE_FIELD`], which ties a style rule to its component.
pub(crate) const ID_REFERENCE_KEYS: &[&str] = &["component_id", crate::FLY_COMPONENT_RULE_FIELD];

/// HTML attributes whose value references another element's id.
///
/// `for` and the ARIA relationship attributes are the accessibility wiring that `audit.rs` already
/// understands; leaving them unmapped on paste produces a duplicate that silently points at the
/// original, which is an accessibility defect rather than a visible one.
pub(crate) const ID_REFERENCE_ATTRIBUTES: &[&str] = &[
    "for",
    "aria-activedescendant",
    "aria-controls",
    "aria-describedby",
    "aria-details",
    "aria-errormessage",
    "aria-flowto",
    "aria-labelledby",
    "aria-owns",
    "list",
    "headers",
];

/// Attributes whose value is a *space-separated list* of ids rather than a single id.
const ID_REFERENCE_LIST_ATTRIBUTES: &[&str] = &[
    "aria-controls",
    "aria-describedby",
    "aria-details",
    "aria-flowto",
    "aria-labelledby",
    "aria-owns",
    "headers",
];

fn remap_one(id: &str, mapping: &BTreeMap<String, String>) -> Option<String> {
    mapping.get(id).cloned()
}

/// Rewrite a single id held in its entirety by this string.
fn remap_in_place(value: &mut String, mapping: &BTreeMap<String, String>) {
    if let Some(replacement) = remap_one(value, mapping) {
        *value = replacement;
    }
}

/// Rewrite a whitespace-separated list of ids, preserving order and dropping no entries.
fn remap_id_list(value: &str, mapping: &BTreeMap<String, String>) -> String {
    value
        .split_whitespace()
        .map(|id| remap_one(id, mapping).unwrap_or_else(|| id.to_string()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Rewrite a same-document anchor (`#id`), leaving every other URL form untouched.
fn remap_anchor(value: &str, mapping: &BTreeMap<String, String>) -> Option<String> {
    let target = value.strip_prefix('#')?;
    let replacement = remap_one(target, mapping)?;
    Some(format!("#{replacement}"))
}

/// Rewrite id references inside a component's HTML attribute map.
pub(crate) fn remap_attribute_ids(
    attributes: &mut Map<String, Value>,
    mapping: &BTreeMap<String, String>,
) {
    for (name, value) in attributes.iter_mut() {
        let lowered = name.to_ascii_lowercase();

        if lowered == "href" {
            if let Value::String(string) = value
                && let Some(replacement) = remap_anchor(string, mapping)
            {
                *string = replacement;
            }
            continue;
        }

        if ID_REFERENCE_LIST_ATTRIBUTES.contains(&lowered.as_str()) {
            if let Value::String(string) = value {
                *string = remap_id_list(string, mapping);
            }
            continue;
        }

        if ID_REFERENCE_ATTRIBUTES.contains(&lowered.as_str()) {
            if let Value::String(string) = value {
                remap_in_place(string, mapping);
            }
            continue;
        }

        // Any other attribute is author content, not identity. Descend anyway, because a nested
        // object may still carry a `component_id` key.
        remap_value_ids(value, mapping);
    }
}

/// Rewrite id references anywhere under `value`, keyed strictly by [`ID_REFERENCE_KEYS`].
pub(crate) fn remap_value_ids(value: &mut Value, mapping: &BTreeMap<String, String>) {
    match value {
        Value::Object(object) => remap_map_ids(object, mapping),
        Value::Array(values) => {
            for value in values {
                remap_value_ids(value, mapping);
            }
        }
        Value::String(_) | Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

/// Rewrite id references in a JSON object, descending into nested structures.
pub(crate) fn remap_map_ids(map: &mut Map<String, Value>, mapping: &BTreeMap<String, String>) {
    for (key, value) in map.iter_mut() {
        // A component serialized as raw JSON (`ComponentNode::Opaque`) still carries an
        // `attributes` object, and the accessibility references inside it must follow the paste
        // just as they do for a typed component.
        if key == "attributes"
            && let Value::Object(attributes) = value
        {
            remap_attribute_ids(attributes, mapping);
            continue;
        }

        if ID_REFERENCE_KEYS.contains(&key.as_str()) {
            match value {
                Value::String(string) => {
                    remap_in_place(string, mapping);
                    continue;
                }
                // A few schemas allow a list of targets.
                Value::Array(values) => {
                    for entry in values.iter_mut() {
                        if let Value::String(string) = entry {
                            remap_in_place(string, mapping);
                        }
                    }
                    continue;
                }
                _ => {}
            }
        }
        remap_value_ids(value, mapping);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn mapping() -> BTreeMap<String, String> {
        BTreeMap::from([
            ("hero".to_string(), "paste-1".to_string()),
            ("title".to_string(), "paste-2".to_string()),
        ])
    }

    #[test]
    fn declared_reference_keys_are_rewritten() {
        let mut value = json!({
            "flyRuntimeConditions": [{ "id": "c1", "component_id": "hero" }],
            "flyComponentId": "title"
        });
        remap_value_ids(&mut value, &mapping());
        assert_eq!(value["flyRuntimeConditions"][0]["component_id"], "paste-1");
        assert_eq!(value["flyComponentId"], "paste-2");
    }

    #[test]
    fn author_content_that_merely_looks_like_an_id_is_left_alone() {
        // The regression this module exists to prevent: a component id of `title` must not
        // rewrite unrelated prose, placeholders, or asset names.
        let mut attributes = Map::from_iter([
            ("placeholder".to_string(), json!("title")),
            ("alt".to_string(), json!("hero")),
            ("data-label".to_string(), json!("title")),
            ("src".to_string(), json!("hero")),
        ]);
        remap_attribute_ids(&mut attributes, &mapping());
        assert_eq!(attributes["placeholder"], "title");
        assert_eq!(attributes["alt"], "hero");
        assert_eq!(attributes["data-label"], "title");
        assert_eq!(attributes["src"], "hero");
    }

    #[test]
    fn accessibility_relationships_follow_the_paste() {
        let mut attributes = Map::from_iter([
            ("for".to_string(), json!("hero")),
            ("aria-labelledby".to_string(), json!("hero title")),
            ("aria-controls".to_string(), json!("hero unknown title")),
        ]);
        remap_attribute_ids(&mut attributes, &mapping());
        assert_eq!(attributes["for"], "paste-1");
        assert_eq!(attributes["aria-labelledby"], "paste-1 paste-2");
        // Unknown ids are preserved rather than dropped.
        assert_eq!(attributes["aria-controls"], "paste-1 unknown paste-2");
    }

    #[test]
    fn same_document_anchors_are_rewritten_but_other_urls_are_not() {
        let mut attributes = Map::from_iter([("href".to_string(), json!("#hero"))]);
        remap_attribute_ids(&mut attributes, &mapping());
        assert_eq!(attributes["href"], "#paste-1");

        for untouched in [
            "/hero",
            "https://example.com/hero",
            "mailto:hero@example.com",
        ] {
            let mut attributes = Map::from_iter([("href".to_string(), json!(untouched))]);
            remap_attribute_ids(&mut attributes, &mapping());
            assert_eq!(attributes["href"], untouched);
        }
    }

    #[test]
    fn attribute_case_is_ignored() {
        let mut attributes = Map::from_iter([("ARIA-Labelledby".to_string(), json!("hero"))]);
        remap_attribute_ids(&mut attributes, &mapping());
        assert_eq!(attributes["ARIA-Labelledby"], "paste-1");
    }

    #[test]
    fn opaque_components_still_get_their_accessibility_references_remapped() {
        // `collect_ids` never collects ids from an opaque node, so an opaque component keeps its
        // own id; what must still be rewritten are its references to components that were
        // remapped.
        let mut value = json!({
            "type": "unknown-provider-widget",
            "attributes": { "aria-labelledby": "hero", "placeholder": "hero" },
            "components": [{ "attributes": { "for": "title" } }]
        });
        remap_value_ids(&mut value, &mapping());
        assert_eq!(value["attributes"]["aria-labelledby"], "paste-1");
        assert_eq!(value["attributes"]["placeholder"], "hero");
        assert_eq!(value["components"][0]["attributes"]["for"], "paste-2");
    }

    #[test]
    fn nested_reference_keys_inside_attributes_still_resolve() {
        let mut attributes =
            Map::from_iter([("data-config".to_string(), json!({ "component_id": "hero" }))]);
        remap_attribute_ids(&mut attributes, &mapping());
        assert_eq!(attributes["data-config"]["component_id"], "paste-1");
    }
}
