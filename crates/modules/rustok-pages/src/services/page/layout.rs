//! Non-persistent layout projection for an exact localized Fly body.
//!
//! Template headers and footers are thin symbol references. Resolve them from
//! the Pages catalog *after* composing the body's single root so that the
//! existing symbol cycle, identifier and publish sanitization checks still run.
use std::collections::BTreeSet;

use fly::{ComponentNode, ComponentObject, GrapesJsCodec};
use serde_json::Value;

use crate::error::{PagesError, PagesResult};

use super::symbols::{apply_site_symbols_to_content, replace_symbol_block};

pub(crate) fn compose_layout_document(
    content: &str,
    header: &[String],
    footer: &[String],
    symbols: &[Value],
) -> PagesResult<String> {
    if header.is_empty() && footer.is_empty() {
        return apply_site_symbols_to_content(content, symbols);
    }
    // The store, not an embedded authoring copy, is authoritative even when
    // the caller supplies a stale flySymbols block.
    let source = replace_symbol_block(content, symbols.to_vec())?;
    let mut document = GrapesJsCodec::decode_str(&source)
        .map_err(|error| PagesError::validation(format!("Invalid layout document: {error}")))?;
    if document.project.pages.len() != 1 {
        return Err(PagesError::validation("A template layout requires exactly one Fly page"));
    }
    let mut used = BTreeSet::new();
    document.project.visit_components(|component, _, _| {
        if let Some(id) = &component.id {
            used.insert(id.clone());
        }
    });
    let root = document.project.pages[0]
        .component.as_mut()
        .and_then(ComponentNode::as_object_mut)
        .ok_or_else(|| PagesError::validation("A template layout requires an object page root"))?;
    let children = root.children_mut().ok_or_else(|| {
        PagesError::validation("A template layout cannot wrap opaque page children")
    })?;
    let mut combined = Vec::with_capacity(header.len() + children.len() + footer.len());
    for (index, symbol_id) in header.iter().enumerate() {
        combined.push(reference_node("header", index, symbol_id, &mut used)?);
    }
    combined.append(children);
    for (index, symbol_id) in footer.iter().enumerate() {
        combined.push(reference_node("footer", index, symbol_id, &mut used)?);
    }
    *children = combined;
    let value = GrapesJsCodec::encode_value(&document).map_err(|error| {
        PagesError::validation(format!("Cannot encode template layout: {error}"))
    })?;
    let content = serde_json::to_string(&value)
        .map_err(|error| PagesError::validation(format!("Cannot serialize template layout: {error}")))?;
    apply_site_symbols_to_content(&content, symbols)
}

fn reference_node(
    slot: &str,
    index: usize,
    symbol_id: &str,
    used: &mut BTreeSet<String>,
) -> PagesResult<ComponentNode> {
    fly::validate_identifier(symbol_id).map_err(PagesError::validation)?;
    let base = format!("layout-{slot}-{}", index + 1);
    let mut id = base.clone();
    let mut suffix = 2usize;
    while used.contains(&id) {
        id = format!("{base}-{suffix}");
        suffix += 1;
        fly::validate_identifier(&id).map_err(PagesError::validation)?;
    }
    used.insert(id.clone());
    Ok(ComponentNode::Object(Box::new(ComponentObject {
        id: Some(id),
        component_type: Some("symbol".to_string()),
        tag_name: Some(slot.to_string()),
        symbol_id: Some(symbol_id.to_string()),
        ..ComponentObject::default()
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn body() -> String {
        json!({"pages": [{"id": "landing", "component": {"id": "root", "type": "wrapper", "components": [
            {"id": "layout-header-1", "type": "text", "content": "Body"}
        ]}}]}).to_string()
    }

    fn symbols() -> Vec<Value> {
        vec![
            json!({"id": "site-header", "components": [{"id": "heading", "type": "text", "content": "Header"}]}),
            json!({"id": "site-footer", "components": [{"id": "copyright", "type": "text", "content": "Footer"}]})
        ]
    }

    #[test]
    fn layout_wraps_body_without_mutating_it_or_leaking_authoring_state() {
        let source = body();
        let result = compose_layout_document(
            &source, &["site-header".into()], &["site-footer".into()], &symbols()
        ).expect("valid symbols and one page");
        assert_eq!(source, body());
        assert_eq!(result, compose_layout_document(
            &source, &["site-header".into()], &["site-footer".into()], &symbols()
        ).unwrap());
        let project: Value = serde_json::from_str(&result).unwrap();
        assert!(project.get("flySymbols").is_none());
        assert!(project.get("flySymbolsRevision").is_none());
        let nodes = project["pages"][0]["component"]["components"].as_array().unwrap();
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0]["tagName"], "header");
        assert_eq!(nodes[1]["content"], "Body");
        assert_eq!(nodes[2]["tagName"], "footer");
        assert_eq!(nodes[0]["components"][0]["content"], "Header");
        assert_eq!(nodes[2]["components"][0]["content"], "Footer");
        assert!(nodes[0].get("symbolId").is_none());
        assert_ne!(nodes[0]["id"], nodes[1]["id"], "generated IDs cannot shadow body IDs");
    }

    #[test]
    fn missing_symbol_and_multiple_pages_fail_closed() {
        assert!(compose_layout_document(&body(), &["absent".into()], &[], &symbols()).is_err());
        let many = json!({"pages": [{"component": {"id":"one", "type":"wrapper"}}, {"component": {"id":"two", "type":"wrapper"}}]}).to_string();
        assert!(compose_layout_document(&many, &["site-header".into()], &[], &symbols()).is_err());
    }
}
