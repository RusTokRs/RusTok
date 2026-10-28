use super::model::ProjectDiffSummary;
use crate::{AssetCatalog, ProjectDocument, StyleRuleCatalog, visit_project_components};
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
        before_hash: before.hash().hex(),
        after_hash: after.hash().hex(),
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
        project_extensions_changed: before.project.extensions != after.project.extensions,
    }
}

fn page_map(document: &ProjectDocument) -> BTreeMap<String, Value> {
    document
        .project
        .pages
        .iter()
        .enumerate()
        .map(|(index, page)| {
            let key = page.id.clone().unwrap_or_else(|| format!("@index:{index}"));
            (key, serde_json::to_value(page).unwrap_or(Value::Null))
        })
        .collect()
}

fn component_map(document: &ProjectDocument) -> BTreeMap<String, Value> {
    let mut components = BTreeMap::new();
    visit_project_components(&document.project, |component, visit| {
        let key = component
            .id()
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("@path:{}", visit.path()));
        components.insert(key, serde_json::to_value(component).unwrap_or(Value::Null));
    });
    components
}

fn asset_map(document: &ProjectDocument) -> BTreeMap<String, Value> {
    let catalog = AssetCatalog::from_document(document);
    let mut assets = BTreeMap::new();
    for asset in catalog.assets {
        assets.insert(asset.id, asset.raw);
    }
    for (index, asset) in catalog.unknown_entries.into_iter().enumerate() {
        assets.insert(format!("@opaque:{index}"), asset);
    }
    assets
}

fn style_rule_map(document: &ProjectDocument) -> BTreeMap<String, Value> {
    let catalog = StyleRuleCatalog::from_document(document);
    let mut rules = BTreeMap::new();
    for rule in catalog.rules {
        rules.insert(rule.id, rule.raw);
    }
    for (index, rule) in catalog.unknown_entries.into_iter().enumerate() {
        rules.insert(format!("@opaque:{index}"), rule);
    }
    rules
}

fn diff_maps(
    before: &BTreeMap<String, Value>,
    after: &BTreeMap<String, Value>,
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
