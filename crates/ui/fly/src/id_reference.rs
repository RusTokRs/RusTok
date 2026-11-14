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
use std::collections::{BTreeMap, BTreeSet};

/// Reserve identities already present in a destination, including provider form IDs which become
/// HTML ids during materialization. The public generator need not know anything about a project.
pub(crate) fn project_identity_ids(document: &crate::ProjectDocument) -> BTreeSet<String> {
    let mut ids = Vec::new();
    for page in &document.project.pages {
        if let Some(root) = &page.component {
            root.collect_ids(&mut ids);
            root.collect_dom_ids(&mut ids);
            root.collect_form_ids(&mut ids);
        }
    }
    ids.into_iter().collect()
}

/// Form identities are document-wide provider resources, unlike per-page HTML ids.
pub(crate) fn project_form_ids(document: &crate::ProjectDocument) -> BTreeSet<String> {
    let mut ids = Vec::new();
    for page in &document.project.pages {
        if let Some(root) = &page.component {
            root.collect_form_ids(&mut ids);
        }
    }
    ids.into_iter().collect()
}

/// HTML ids that the given roots can claim after form materialization. Invalid or opaque
/// form nodes are kept conservative, since Fly cannot safely project them to native forms.
pub(crate) fn materialized_dom_ids<'a>(
    roots: impl IntoIterator<Item = &'a crate::ComponentNode>,
) -> BTreeSet<String> {
    let mut owners = Vec::new();
    for root in roots {
        root.collect_materialized_form_dom_owners(&mut owners);
    }
    owners
        .into_iter()
        .flat_map(|(form, dom)| form.into_iter().chain(dom))
        .collect()
}

/// A materialized form claims its provider id as its HTML id. Only an HTML id on that same
/// node may share it; on another node it would make native form and ARIA targets ambiguous.
pub(crate) fn conflicting_form_dom_id<'a>(
    roots: impl IntoIterator<Item = &'a crate::ComponentNode>,
) -> Option<String> {
    let mut owners = Vec::new();
    for root in roots {
        root.collect_materialized_form_dom_owners(&mut owners);
    }
    let form_owners = owners
        .iter()
        .enumerate()
        .filter_map(|(index, (form, _))| form.as_ref().map(|id| (id.as_str(), index)))
        .collect::<BTreeMap<_, _>>();
    for (index, (_, dom_ids)) in owners.iter().enumerate() {
        for id in dom_ids {
            if form_owners
                .get(id.as_str())
                .is_some_and(|owner| *owner != index)
            {
                return Some(id.clone());
            }
        }
    }
    None
}

/// Re-identification cannot preserve references if two source nodes already own the same id.
pub(crate) fn ambiguous_tree_identity<'a>(
    roots: impl IntoIterator<Item = &'a crate::ComponentNode>,
) -> Option<String> {
    let roots = roots.into_iter().collect::<Vec<_>>();
    let mut component_ids = Vec::new();
    let mut dom_ids = Vec::new();
    let mut form_ids = Vec::new();
    for root in &roots {
        root.collect_ids(&mut component_ids);
        root.collect_dom_ids(&mut dom_ids);
        root.collect_form_ids(&mut form_ids);
    }
    for (kind, ids) in [
        ("component", component_ids),
        ("HTML", dom_ids),
        ("form", form_ids),
    ] {
        let mut seen = BTreeSet::new();
        for id in ids {
            if !seen.insert(id.clone()) {
                return Some(format!("duplicate {kind} id `{id}`"));
            }
        }
    }
    conflicting_form_dom_id(roots)
        .map(|id| format!("form id `{id}` belongs to a different HTML element"))
}

pub(crate) fn fresh_component_id_mapping(
    ids: impl IntoIterator<Item = String>,
    hint: &str,
    reserved: &mut BTreeSet<String>,
    generator: &mut impl crate::IdGenerator,
) -> BTreeMap<String, String> {
    let mut mapping = BTreeMap::new();
    for id in ids {
        if mapping.contains_key(&id) {
            continue;
        }
        let replacement = allocate_generated_id(generator, hint, reserved);
        mapping.insert(id, replacement);
    }
    mapping
}

/// Mint HTML identities separately. A shared string means the same identity only when it occurs
/// on the *same* component. Provider form IDs stay unchanged: they also name the server endpoint.
pub(crate) fn fresh_dom_id_mapping(
    ids: impl IntoIterator<Item = String>,
    matching_ids: &BTreeSet<String>,
    form_ids: &BTreeSet<String>,
    component_ids: &BTreeMap<String, String>,
    reserved: &mut BTreeSet<String>,
    generator: &mut impl crate::IdGenerator,
) -> BTreeMap<String, String> {
    let mut mapping = form_ids
        .iter()
        .map(|id| (id.clone(), id.clone()))
        .collect::<BTreeMap<_, _>>();
    for id in ids {
        if mapping.contains_key(&id) {
            continue;
        }
        let replacement = if matching_ids.contains(&id) {
            component_ids.get(&id).cloned()
        } else {
            None
        }
        .unwrap_or_else(|| allocate_generated_id(generator, "dom", reserved));
        mapping.insert(id, replacement);
    }
    mapping
}

/// Callers may supply a custom generator, including one that repeats or yields invalid names.
/// Bound retries and fall back to a valid deterministic name rather than hanging or persisting
/// an identifier that cannot be rendered safely.
pub(crate) fn allocate_generated_id(
    generator: &mut impl crate::IdGenerator,
    hint: &str,
    reserved: &mut BTreeSet<String>,
) -> String {
    for _ in 0..32 {
        let candidate = generator.next_id(hint);
        if crate::validate_identifier(&candidate).is_ok() && reserved.insert(candidate.clone()) {
            return candidate;
        }
    }
    allocate_derived_id("", hint, reserved)
}

/// Deterministic, bounded generated identity. Unlike simply appending `-2` to a maximum-length
/// instance id, this always makes progress when the preferred name is already used or too long.
pub(crate) fn allocate_derived_id(
    preferred: &str,
    hint: &str,
    used: &mut BTreeSet<String>,
) -> String {
    if crate::validate_identifier(preferred).is_ok() && used.insert(preferred.to_string()) {
        return preferred.to_string();
    }
    let prefix = hint
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':'))
        .take(crate::MAXIMUM_IDENTIFIER_LENGTH - 24)
        .collect::<String>();
    let prefix = if prefix.is_empty() { "fly" } else { &prefix };
    let mut counter = 2u64;
    loop {
        let candidate = format!("{prefix}-{counter}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        counter += 1;
    }
}

/// HTML attributes whose value references another element's id.
///
/// `for` and the ARIA relationship attributes are the accessibility wiring that `audit.rs` already
/// understands; leaving them unmapped on paste produces a duplicate that silently points at the
/// original, which is an accessibility defect rather than a visible one.
pub(crate) const ID_REFERENCE_ATTRIBUTES: &[&str] = &[
    "for",
    "form",
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
    "data-fly-tab-target",
];

/// Attributes whose value is a *space-separated list* of ids rather than a single id.
pub(crate) const ID_REFERENCE_LIST_ATTRIBUTES: &[&str] = &[
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

/// While a repeater is materializing, a single id may still contain whitespace inside a
/// `{{ path }}` expression; do not split that expression before its owner is interpolated.
pub(crate) fn id_list_tokens(value: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut start = None;
    let mut in_template = false;
    let mut chars = value.char_indices().peekable();
    while let Some((index, character)) = chars.next() {
        if character.is_whitespace() && !in_template {
            if let Some(token_start) = start.take() {
                tokens.push(&value[token_start..index]);
            }
            continue;
        }
        start.get_or_insert(index);
        if character == '{' && chars.peek().is_some_and(|(_, next)| *next == '{') {
            in_template = true;
            chars.next();
        } else if in_template
            && character == '}'
            && chars.peek().is_some_and(|(_, next)| *next == '}')
        {
            in_template = false;
            chars.next();
        }
    }
    if let Some(token_start) = start {
        tokens.push(&value[token_start..]);
    }
    tokens
}

/// Rewrite a whitespace-separated list of ids, preserving order and dropping no entries.
fn remap_id_list(value: &str, mapping: &BTreeMap<String, String>) -> String {
    id_list_tokens(value)
        .into_iter()
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
        // Other attributes, including nested provider data, are opaque author content.
    }
}

/// Rewrite only the identity slots of a recognized action. Event/provider payloads and unknown
/// fields are opaque author data, even when they happen to contain a `component_id` key.
pub(crate) fn remap_action_ids(value: &mut Value, mapping: &BTreeMap<String, String>) {
    let Some(action) = value.as_object_mut() else {
        return;
    };
    let fields: &[&str] = match action.get("kind").and_then(Value::as_str) {
        Some("toggle_visibility" | "scroll_to" | "animate" | "toggle_accordion") => &["target_id"],
        Some("switch_tab") => &["panel_id"],
        Some("switch_slide") => &["slider_id", "target_slide"],
        _ => &[],
    };
    for field in fields {
        if let Some(Value::String(id)) = action.get_mut(*field)
            && !(*field == "slider_id" && id == "self")
        {
            remap_in_place(id, mapping);
        }
    }
}

/// Remap the direct target of recognized project-level runtime definitions. Unknown entries,
/// extension fields, fallbacks, condition values and symbol definitions remain lossless: a key
/// named `component_id` alone is not enough to establish ownership of an arbitrary JSON object.
pub(crate) fn remap_runtime_catalogs(
    extensions: &mut Map<String, Value>,
    mapping: &BTreeMap<String, String>,
) {
    for field in [
        crate::FLY_RUNTIME_BINDINGS_FIELD,
        crate::FLY_RUNTIME_CONDITIONS_FIELD,
        crate::FLY_RUNTIME_REPEATERS_FIELD,
    ] {
        let Some(Value::Array(entries)) = extensions.get_mut(field) else {
            continue;
        };
        for entry in entries {
            let recognized = match field {
                crate::FLY_RUNTIME_BINDINGS_FIELD => {
                    serde_json::from_value::<crate::RuntimeBinding>(entry.clone()).is_ok()
                }
                crate::FLY_RUNTIME_CONDITIONS_FIELD => {
                    serde_json::from_value::<crate::RuntimeCondition>(entry.clone()).is_ok()
                }
                crate::FLY_RUNTIME_REPEATERS_FIELD => {
                    serde_json::from_value::<crate::RuntimeRepeater>(entry.clone()).is_ok()
                }
                _ => false,
            };
            if recognized && let Some(Value::String(id)) = entry.get_mut("component_id") {
                remap_in_place(id, mapping);
            }
        }
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
    fn fresh_ids_avoid_source_and_destination_even_with_a_restarting_generator() {
        let mut generator = crate::SequentialIdGenerator::new("copy");
        let mut reserved = BTreeSet::from([
            "copy-paste-1".to_string(),
            "copy-paste-2".to_string(),
            "copy-dom-4".to_string(),
            "panel".to_string(),
        ]);
        let components = fresh_component_id_mapping(
            ["copy-paste-1".to_string(), "panel".to_string()],
            "paste",
            &mut reserved,
            &mut generator,
        );
        assert_eq!(components["copy-paste-1"], "copy-paste-3");
        assert_eq!(components["panel"], "copy-paste-4");
        let dom = fresh_dom_id_mapping(
            ["panel".to_string(), "other".to_string()],
            &BTreeSet::new(),
            &BTreeSet::from(["external-form".to_string()]),
            &components,
            &mut reserved,
            &mut generator,
        );
        // `panel` is the component id of a different node, not the owner of this HTML id.
        assert_ne!(dom["panel"], components["panel"]);
        assert_eq!(dom["external-form"], "external-form");
        assert_ne!(dom["panel"], dom["other"]);
    }

    #[test]
    fn a_broken_custom_generator_cannot_hang_or_emit_invalid_ids() {
        struct Stuck;
        impl crate::IdGenerator for Stuck {
            fn next_id(&mut self, _hint: &str) -> String {
                "bad id".into()
            }
        }
        let mut reserved = BTreeSet::from(["node-2".to_string()]);
        let mut generator = Stuck;
        let first = allocate_generated_id(&mut generator, "node", &mut reserved);
        let second = allocate_generated_id(&mut generator, "node", &mut reserved);
        assert_ne!(first, second);
        assert_ne!(first, "node-2");
        assert!(crate::validate_identifier(&first).is_ok());
        assert!(crate::validate_identifier(&second).is_ok());
    }

    #[test]
    fn derived_ids_are_bounded_and_collision_safe_for_maximum_length_hints() {
        let hint = "x".repeat(crate::MAXIMUM_IDENTIFIER_LENGTH);
        let mut used = BTreeSet::new();
        let first = allocate_derived_id(&format!("{hint}-child"), &hint, &mut used);
        let second = allocate_derived_id(&format!("{hint}-child"), &hint, &mut used);
        assert_ne!(first, second);
        assert!(crate::validate_identifier(&first).is_ok());
        assert!(crate::validate_identifier(&second).is_ok());
    }

    #[test]
    fn recognized_runtime_catalog_targets_are_rewritten_without_descending_into_values() {
        let mut value = json!({
            "flyRuntimeConditions": [
                {"id": "c1", "component_id": "hero", "path": "flag", "operator": "equals", "expected": {"component_id": "title"}},
                {"id": "unknown", "component_id": "hero", "operator": "future"}
            ],
            "provider": {"component_id": "hero"}
        });
        remap_runtime_catalogs(value.as_object_mut().unwrap(), &mapping());
        assert_eq!(value["flyRuntimeConditions"][0]["component_id"], "paste-1");
        assert_eq!(
            value["flyRuntimeConditions"][0]["expected"]["component_id"],
            "title"
        );
        assert_eq!(value["flyRuntimeConditions"][1]["component_id"], "hero");
        assert_eq!(value["provider"]["component_id"], "hero");
    }

    #[test]
    fn typed_actions_remap_only_internal_targets_and_preserve_opaque_payloads() {
        for (kind, field) in [
            ("toggle_visibility", "target_id"),
            ("scroll_to", "target_id"),
            ("animate", "target_id"),
            ("toggle_accordion", "target_id"),
            ("switch_tab", "panel_id"),
        ] {
            let mut value = json!({"flyAction": {"kind": kind, (field): "hero", "payload": {"component_id": "title"}}});
            remap_action_ids(&mut value["flyAction"], &mapping());
            assert_eq!(value["flyAction"][field], "paste-1", "{kind}");
            assert_eq!(value["flyAction"]["payload"]["component_id"], "title");
        }
        let mut slider = json!({"flyAction": {"kind": "switch_slide", "slider_id": "self", "target_slide": "title"}});
        let mut ids = mapping();
        ids.insert("self".to_string(), "not-the-sentinel".to_string());
        remap_action_ids(&mut slider["flyAction"], &ids);
        assert_eq!(slider["flyAction"]["slider_id"], "self");
        assert_eq!(slider["flyAction"]["target_slide"], "paste-2");

        let mut event = json!({"flyAction": {"kind": "emit_event", "event": "hero", "payload": {"component_id": "hero"}}});
        remap_action_ids(&mut event["flyAction"], &mapping());
        assert_eq!(event["flyAction"]["payload"]["component_id"], "hero");
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
        let mut node = crate::ComponentNode::Opaque(json!({
            "type": "unknown-provider-widget",
            "attributes": { "aria-labelledby": "hero", "placeholder": "hero" },
            "components": [{ "attributes": { "for": "title" } }],
            "providerData": {"attributes": {"for": "hero"}, "component_id": "hero"}
        }));
        node.remap_ids_with_dom(&mapping(), &BTreeMap::new());
        let crate::ComponentNode::Opaque(value) = node else {
            panic!("opaque node")
        };
        assert_eq!(value["attributes"]["aria-labelledby"], "paste-1");
        assert_eq!(value["attributes"]["placeholder"], "hero");
        assert_eq!(value["components"][0]["attributes"]["for"], "paste-2");
        assert_eq!(value["providerData"]["attributes"]["for"], "hero");
        assert_eq!(value["providerData"]["component_id"], "hero");
    }

    #[test]
    fn nested_provider_data_inside_attributes_is_opaque() {
        let mut attributes =
            Map::from_iter([("data-config".to_string(), json!({ "component_id": "hero" }))]);
        remap_attribute_ids(&mut attributes, &mapping());
        assert_eq!(attributes["data-config"]["component_id"], "hero");
    }
}
