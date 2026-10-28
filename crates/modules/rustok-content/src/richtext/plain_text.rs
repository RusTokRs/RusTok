/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusToK Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use rustok_api::{RichTextDocument, RichTextNode};

use super::{RichTextError, RichTextProfile, validate_and_normalize};

pub fn plain_text(
    document: &RichTextDocument,
    profile: RichTextProfile,
) -> Result<String, RichTextError> {
    let document = validate_and_normalize(document.clone(), profile)?;
    Ok(document
        .content
        .iter()
        .filter_map(|node| {
            let value = node_text(node);
            (!value.is_empty()).then_some(value)
        })
        .collect::<Vec<_>>()
        .join("\n\n"))
}

fn node_text(node: &RichTextNode) -> String {
    match node.kind.as_str() {
        "text" => node.text.clone().unwrap_or_default(),
        "hardBreak" => "\n".to_string(),
        "horizontalRule" => String::new(),
        "paragraph" | "heading" | "codeBlock" => inline_text(node),
        "blockquote" => join_blocks(node, "\n"),
        "bulletList" => list_text(node, None),
        "orderedList" => {
            let start = node
                .attrs
                .get("start")
                .and_then(|value| value.as_u64())
                .unwrap_or(1);
            list_text(node, Some(start))
        }
        "listItem" => join_blocks(node, "\n"),
        _ => String::new(),
    }
}

fn inline_text(node: &RichTextNode) -> String {
    node.content.iter().map(node_text).collect()
}

fn join_blocks(node: &RichTextNode, separator: &str) -> String {
    node.content
        .iter()
        .filter_map(|child| {
            let value = node_text(child);
            (!value.is_empty()).then_some(value)
        })
        .collect::<Vec<_>>()
        .join(separator)
}

fn list_text(node: &RichTextNode, ordered_start: Option<u64>) -> String {
    node.content
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let value = node_text(item).replace('\n', "\n  ");
            match ordered_start {
                Some(start) => format!("{}. {value}", start + index as u64),
                None => format!("- {value}"),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
