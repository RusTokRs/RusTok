use super::model::ProjectDiffSummary;
use crate::{
    AssetDescriptor, ProjectDocument, ProjectHash, StyleRuleIdentity, visit_project_components,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub fn compare_projects(before: &ProjectDocument, after: &ProjectDocument) -> ProjectDiffSummary {
    let before_pages = page_map(before);
    let after_pages = page_map(after);
    let before_components = component_map(before);
    let after_components = component_map(after);
    let before_assets = asset_map(before);
    let after_assets = asset_map(after);
    let before_styles = style_rule_map(before);
    let after_styles = style_rule_map(after);

    let (added_pages, removed_pages, changed_pages) = diff_maps(&before_pages, &after_pages);
    let (added_components, removed_components, changed_components) =
        diff_maps(&before_components, &after_components);
    let (added_assets, removed_assets, changed_assets) = diff_maps(&before_assets, &after_assets);
    let (added_style_rules, removed_style_rules, changed_style_rules) =
        diff_maps(&before_styles, &after_styles);

    ProjectDiffSummary {
        // Compare actual payloads rather than potentially stale raw-project caches.
        before_hash: ProjectHash::from_document(before).hex(),
        after_hash: ProjectHash::from_document(after).hex(),
        added_pages,
        removed_pages,
        changed_pages,
        added_components,
        removed_components,
        changed_components,
        added_assets,
        removed_assets,
        changed_assets,
        added_style_rules,
        removed_style_rules,
        changed_style_rules,
        project_extensions_changed: before.project().extensions != after.project().extensions,
    }
}

// Keep positions and all occurrences: a map of id -> single value loses
// both ordering and repeated identities in imported or malformed projects.
type OrderedEntries = BTreeMap<String, Vec<(usize, Value)>>;

fn page_map(document: &ProjectDocument) -> OrderedEntries {
    let mut pages = OrderedEntries::new();
    for (index, page) in document.project().pages.iter().enumerate() {
        let key = page.id.clone().unwrap_or_else(|| format!("@index:{index}"));
        pages.entry(key).or_default().push((
            index,
            serde_json::to_value(page).unwrap_or(Value::Null),
        ));
    }
    pages
}

fn component_map(document: &ProjectDocument) -> BTreeMap<String, Vec<Value>> {
    let mut components = BTreeMap::<String, Vec<Value>>::new();
    visit_project_components(&document.project(), |component, visit| {
        let key = component
            .id()
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("@path:{}", visit.path()));
        components
            .entry(key)
            .or_default()
            .push(serde_json::to_value(component).unwrap_or(Value::Null));
    });
    components
}

fn asset_map(document: &ProjectDocument) -> OrderedEntries {
    ordered_raw_entries(&document.project().assets, |raw| {
        AssetDescriptor::from_value(raw.clone()).map(|asset| asset.id)
    })
}

fn style_rule_map(document: &ProjectDocument) -> OrderedEntries {
    ordered_raw_entries(&document.project().styles, |raw| {
        StyleRuleIdentity::from_value(raw).map(|rule| rule.id)
    })
}

/// Use owner-defined identities while keeping each raw entry in its original
/// position, including opaque entries interleaved with recognized ones.
fn ordered_raw_entries(
    entries: &[Value],
    identity: impl Fn(&Value) -> Option<String>,
) -> OrderedEntries {
    let mut result = OrderedEntries::new();
    let mut opaque_index = 0usize;
    for (index, raw) in entries.iter().enumerate() {
        let key = identity(raw).unwrap_or_else(|| {
            let key = format!("@opaque:{opaque_index}");
            opaque_index += 1;
            key
        });
        result.entry(key).or_default().push((index, raw.clone()));
    }
    result
}

fn diff_maps<T: PartialEq>(
    before: &BTreeMap<String, T>,
    after: &BTreeMap<String, T>,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let before_keys = before.keys().cloned().collect::<BTreeSet<_>>();
    let after_keys = after.keys().cloned().collect::<BTreeSet<_>>();
    let added = after_keys.difference(&before_keys).cloned().collect();
    let removed = before_keys.difference(&after_keys).cloned().collect();
    let changed = before_keys
        .intersection(&after_keys)
        .filter(|key| before.get(*key) != after.get(*key))
        .cloned()
        .collect();
    (added, removed, changed)
}
