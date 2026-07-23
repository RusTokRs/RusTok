/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusToK Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::collections::{BTreeMap, HashSet};

use rustok_api::{RichTextDocument, RichTextMark, RichTextNode};
use serde_json::Value;
use url::Url;

use super::{RichTextError, RichTextProfile};

#[derive(Default)]
struct Stats {
    nodes: usize,
    text_chars: usize,
    links: usize,
    has_meaningful_text: bool,
}

pub fn parse_json(raw: &str, profile: RichTextProfile) -> Result<RichTextDocument, RichTextError> {
    let maximum = profile.limits().max_json_bytes;
    if raw.len() > maximum {
        return Err(RichTextError::DocumentTooLarge { maximum });
    }

    let document =
        serde_json::from_str::<RichTextDocument>(raw).map_err(|_| RichTextError::InvalidJson)?;
    validate_and_normalize(document, profile)
}

pub fn validate(
    document: &RichTextDocument,
    profile: RichTextProfile,
) -> Result<(), RichTextError> {
    validate_and_normalize(document.clone(), profile).map(|_| ())
}

pub fn validate_and_normalize(
    mut document: RichTextDocument,
    profile: RichTextProfile,
) -> Result<RichTextDocument, RichTextError> {
    let maximum = profile.limits().max_json_bytes;
    let serialized_size = serde_json::to_vec(&document)
        .map_err(|_| RichTextError::InvalidJson)?
        .len();
    if serialized_size > maximum {
        return Err(RichTextError::DocumentTooLarge { maximum });
    }
    if document.kind != "doc" {
        return Err(RichTextError::InvalidRoot);
    }

    let mut stats = Stats {
        nodes: 1,
        ..Stats::default()
    };
    check_limit(stats.nodes, profile.limits().max_nodes, "node_count", "$")?;

    for (index, child) in document.content.iter_mut().enumerate() {
        let path = format!("$.content[{index}]");
        validate_node(child, profile, 2, &path, &mut stats)?;
        if !is_block(&child.kind) {
            return Err(RichTextError::InvalidStructure { path });
        }
    }

    if !stats.has_meaningful_text {
        return Err(RichTextError::EmptyDocument);
    }

    Ok(document)
}

fn validate_node(
    node: &mut RichTextNode,
    profile: RichTextProfile,
    depth: usize,
    path: &str,
    stats: &mut Stats,
) -> Result<(), RichTextError> {
    check_limit(depth, profile.limits().max_depth, "depth", path)?;
    stats.nodes += 1;
    check_limit(stats.nodes, profile.limits().max_nodes, "node_count", path)?;

    if !profile.allows_node(&node.kind) || node.kind == "doc" {
        return Err(RichTextError::UnsupportedNode {
            path: path.to_string(),
        });
    }

    if node.kind == "text" {
        validate_text_node(node, profile, path, stats)?;
        return Ok(());
    }

    if node.text.is_some() {
        return Err(RichTextError::InvalidStructure {
            path: field_path(path, "text"),
        });
    }

    if node.kind == "hardBreak" {
        validate_empty_attrs(&node.attrs, path)?;
        validate_marks(&mut node.marks, profile, path, stats)?;
        if !node.content.is_empty() {
            return Err(RichTextError::InvalidStructure {
                path: field_path(path, "content"),
            });
        }
        return Ok(());
    }

    if !node.marks.is_empty() {
        return Err(RichTextError::InvalidStructure {
            path: field_path(path, "marks"),
        });
    }

    match node.kind.as_str() {
        "paragraph" => {
            validate_empty_attrs(&node.attrs, path)?;
            validate_inline_children(node, profile, depth, path, stats)
        }
        "heading" => {
            validate_heading_attrs(&node.attrs, profile, path)?;
            validate_inline_children(node, profile, depth, path, stats)
        }
        "bulletList" => {
            validate_empty_attrs(&node.attrs, path)?;
            validate_list(node, profile, depth, path, stats)
        }
        "orderedList" => {
            normalize_ordered_list_attrs(&mut node.attrs, path)?;
            validate_list(node, profile, depth, path, stats)
        }
        "listItem" => {
            validate_empty_attrs(&node.attrs, path)?;
            validate_list_item(node, profile, depth, path, stats)
        }
        "blockquote" => {
            validate_empty_attrs(&node.attrs, path)?;
            validate_block_children(node, profile, depth, path, stats, true)
        }
        "codeBlock" => {
            validate_empty_attrs(&node.attrs, path)?;
            validate_code_block(node, profile, depth, path, stats)
        }
        "horizontalRule" => {
            validate_empty_attrs(&node.attrs, path)?;
            if node.content.is_empty() {
                Ok(())
            } else {
                Err(RichTextError::InvalidStructure {
                    path: field_path(path, "content"),
                })
            }
        }
        _ => Err(RichTextError::UnsupportedNode {
            path: path.to_string(),
        }),
    }
}

fn validate_text_node(
    node: &mut RichTextNode,
    profile: RichTextProfile,
    path: &str,
    stats: &mut Stats,
) -> Result<(), RichTextError> {
    validate_empty_attrs(&node.attrs, path)?;
    if !node.content.is_empty() {
        return Err(RichTextError::InvalidStructure {
            path: field_path(path, "content"),
        });
    }
    let text = node
        .text
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| RichTextError::InvalidStructure {
            path: field_path(path, "text"),
        })?;

    stats.text_chars += text.chars().count();
    check_limit(
        stats.text_chars,
        profile.limits().max_text_chars,
        "text_size",
        path,
    )?;
    stats.has_meaningful_text |= text.chars().any(|character| !character.is_whitespace());
    validate_marks(&mut node.marks, profile, path, stats)
}

fn validate_marks(
    marks: &mut Vec<RichTextMark>,
    profile: RichTextProfile,
    path: &str,
    stats: &mut Stats,
) -> Result<(), RichTextError> {
    check_limit(
        marks.len(),
        profile.limits().max_marks_per_node,
        "marks_per_node",
        &field_path(path, "marks"),
    )?;

    let mut seen = HashSet::with_capacity(marks.len());
    for (index, mark) in marks.iter_mut().enumerate() {
        let mark_path = format!("{}.marks[{index}]", path);
        if !profile.allows_mark(&mark.kind) {
            return Err(RichTextError::UnsupportedMark { path: mark_path });
        }
        if !seen.insert(mark.kind.clone()) {
            return Err(RichTextError::DuplicateMark { path: mark_path });
        }

        if mark.kind == "link" {
            normalize_link_attrs(&mut mark.attrs, profile, &mark_path)?;
            stats.links += 1;
            check_limit(
                stats.links,
                profile.limits().max_links,
                "link_count",
                &mark_path,
            )?;
        } else {
            validate_empty_attrs(&mark.attrs, &mark_path)?;
        }
    }

    marks.sort_by_key(|mark| mark_rank(&mark.kind));
    Ok(())
}

fn validate_inline_children(
    node: &mut RichTextNode,
    profile: RichTextProfile,
    depth: usize,
    path: &str,
    stats: &mut Stats,
) -> Result<(), RichTextError> {
    for (index, child) in node.content.iter_mut().enumerate() {
        let child_path = format!("{}.content[{index}]", path);
        if !is_inline(&child.kind) {
            return Err(RichTextError::InvalidStructure { path: child_path });
        }
        validate_node(child, profile, depth + 1, &child_path, stats)?;
    }
    Ok(())
}

fn validate_block_children(
    node: &mut RichTextNode,
    profile: RichTextProfile,
    depth: usize,
    path: &str,
    stats: &mut Stats,
    require_child: bool,
) -> Result<(), RichTextError> {
    if require_child && node.content.is_empty() {
        return Err(RichTextError::InvalidStructure {
            path: field_path(path, "content"),
        });
    }
    for (index, child) in node.content.iter_mut().enumerate() {
        let child_path = format!("{}.content[{index}]", path);
        if !is_block(&child.kind) {
            return Err(RichTextError::InvalidStructure { path: child_path });
        }
        validate_node(child, profile, depth + 1, &child_path, stats)?;
    }
    Ok(())
}

fn validate_list(
    node: &mut RichTextNode,
    profile: RichTextProfile,
    depth: usize,
    path: &str,
    stats: &mut Stats,
) -> Result<(), RichTextError> {
    if node.content.is_empty() {
        return Err(RichTextError::InvalidStructure {
            path: field_path(path, "content"),
        });
    }
    for (index, child) in node.content.iter_mut().enumerate() {
        let child_path = format!("{}.content[{index}]", path);
        if child.kind != "listItem" {
            return Err(RichTextError::InvalidStructure { path: child_path });
        }
        validate_node(child, profile, depth + 1, &child_path, stats)?;
    }
    Ok(())
}

fn validate_list_item(
    node: &mut RichTextNode,
    profile: RichTextProfile,
    depth: usize,
    path: &str,
    stats: &mut Stats,
) -> Result<(), RichTextError> {
    if node
        .content
        .first()
        .is_none_or(|child| child.kind != "paragraph")
    {
        return Err(RichTextError::InvalidStructure {
            path: field_path(path, "content"),
        });
    }
    validate_block_children(node, profile, depth, path, stats, true)
}

fn validate_code_block(
    node: &mut RichTextNode,
    profile: RichTextProfile,
    depth: usize,
    path: &str,
    stats: &mut Stats,
) -> Result<(), RichTextError> {
    for (index, child) in node.content.iter_mut().enumerate() {
        let child_path = format!("{}.content[{index}]", path);
        if child.kind != "text" || !child.marks.is_empty() {
            return Err(RichTextError::InvalidStructure { path: child_path });
        }
        validate_node(child, profile, depth + 1, &child_path, stats)?;
    }
    Ok(())
}

fn validate_heading_attrs(
    attrs: &BTreeMap<String, Value>,
    profile: RichTextProfile,
    path: &str,
) -> Result<(), RichTextError> {
    ensure_only_keys(attrs, &["level"], path)?;
    let level = attrs
        .get("level")
        .and_then(Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .filter(|level| profile.allows_heading_level(*level))
        .ok_or_else(|| RichTextError::InvalidAttribute {
            path: field_path(path, "attrs.level"),
        })?;
    if !profile.allows_heading_level(level) {
        return Err(RichTextError::InvalidAttribute {
            path: field_path(path, "attrs.level"),
        });
    }
    Ok(())
}

fn normalize_ordered_list_attrs(
    attrs: &mut BTreeMap<String, Value>,
    path: &str,
) -> Result<(), RichTextError> {
    ensure_only_keys(attrs, &["start"], path)?;
    let Some(start) = attrs.get("start") else {
        return Ok(());
    };
    if start.is_null() {
        attrs.remove("start");
        return Ok(());
    }
    let start = start
        .as_u64()
        .filter(|value| (1..=1_000_000).contains(value))
        .ok_or_else(|| RichTextError::InvalidAttribute {
            path: field_path(path, "attrs.start"),
        })?;
    if start == 1 {
        attrs.remove("start");
    }
    Ok(())
}

fn normalize_link_attrs(
    attrs: &mut BTreeMap<String, Value>,
    profile: RichTextProfile,
    path: &str,
) -> Result<(), RichTextError> {
    ensure_only_keys(attrs, &["href", "target", "rel", "class"], path)?;

    for default_key in ["target", "rel", "class"] {
        if let Some(value) = attrs.get(default_key) {
            if !value.is_null() {
                return Err(RichTextError::InvalidAttribute {
                    path: field_path(path, &format!("attrs.{default_key}")),
                });
            }
        }
        attrs.remove(default_key);
    }

    let href = attrs.get("href").and_then(Value::as_str).ok_or_else(|| {
        RichTextError::InvalidAttribute {
            path: field_path(path, "attrs.href"),
        }
    })?;
    check_limit(
        href.len(),
        profile.limits().max_attribute_bytes,
        "attribute_size",
        &field_path(path, "attrs.href"),
    )?;
    check_limit(
        href.len(),
        profile.limits().max_url_bytes,
        "url_size",
        &field_path(path, "attrs.href"),
    )?;
    if !is_safe_link(href) {
        return Err(RichTextError::UnsafeLink {
            path: field_path(path, "attrs.href"),
        });
    }
    Ok(())
}

fn validate_empty_attrs(attrs: &BTreeMap<String, Value>, path: &str) -> Result<(), RichTextError> {
    if attrs.is_empty() {
        Ok(())
    } else {
        Err(RichTextError::UnsupportedAttribute {
            path: field_path(path, "attrs"),
        })
    }
}

fn ensure_only_keys(
    attrs: &BTreeMap<String, Value>,
    allowed: &[&str],
    path: &str,
) -> Result<(), RichTextError> {
    if attrs.keys().all(|key| allowed.contains(&key.as_str())) {
        Ok(())
    } else {
        Err(RichTextError::UnsupportedAttribute {
            path: field_path(path, "attrs"),
        })
    }
}

pub(crate) fn is_safe_link(href: &str) -> bool {
    if href.is_empty()
        || href.chars().any(char::is_control)
        || href.contains('\\')
        || href.starts_with("//")
    {
        return false;
    }

    if href.starts_with('/') {
        return true;
    }
    if href.starts_with('#') {
        return href.len() > 1;
    }

    let Ok(url) = Url::parse(href) else {
        return false;
    };
    match url.scheme() {
        "http" | "https" => {
            url.username().is_empty() && url.password().is_none() && url.host_str().is_some()
        }
        "mailto" => true,
        _ => false,
    }
}

fn is_block(kind: &str) -> bool {
    matches!(
        kind,
        "paragraph"
            | "heading"
            | "bulletList"
            | "orderedList"
            | "blockquote"
            | "codeBlock"
            | "horizontalRule"
    )
}

fn is_inline(kind: &str) -> bool {
    matches!(kind, "text" | "hardBreak")
}

fn mark_rank(kind: &str) -> usize {
    match kind {
        "link" => 0,
        "bold" => 1,
        "italic" => 2,
        "strike" => 3,
        "code" => 4,
        _ => usize::MAX,
    }
}

fn check_limit(
    actual: usize,
    maximum: usize,
    limit: &'static str,
    path: &str,
) -> Result<(), RichTextError> {
    if actual <= maximum {
        Ok(())
    } else {
        Err(RichTextError::LimitExceeded {
            limit,
            maximum,
            path: path.to_string(),
        })
    }
}

fn field_path(path: &str, field: &str) -> String {
    format!("{path}.{field}")
}
