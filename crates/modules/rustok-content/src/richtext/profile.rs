/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusToK Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use rustok_api::RichTextProfileId;
use serde::Serialize;

use super::RichTextError;

const ARTICLE_NODES: &[&str] = &[
    "doc",
    "paragraph",
    "heading",
    "bulletList",
    "orderedList",
    "listItem",
    "blockquote",
    "codeBlock",
    "horizontalRule",
    "hardBreak",
    "text",
];
const COMMENT_NODES: &[&str] = &[
    "doc",
    "paragraph",
    "bulletList",
    "orderedList",
    "listItem",
    "blockquote",
    "hardBreak",
    "text",
];
const BASE_MARKS: &[&str] = &["bold", "italic", "strike", "code", "link"];
const ARTICLE_HEADINGS: &[u8] = &[2, 3, 4];
const NO_HEADINGS: &[u8] = &[];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RichTextProfile {
    Article,
    Discussion,
    Comment,
}

impl RichTextProfile {
    pub fn id(self) -> RichTextProfileId {
        RichTextProfileId::new(self.id_str()).expect("built-in profile identifiers are valid")
    }

    pub fn id_str(self) -> &'static str {
        match self {
            Self::Article => "article",
            Self::Discussion => "discussion",
            Self::Comment => "comment",
        }
    }

    pub fn limits(self) -> RichTextLimits {
        match self {
            Self::Article => RichTextLimits {
                max_json_bytes: 512 * 1024,
                max_depth: 16,
                max_nodes: 4096,
                max_text_chars: 200_000,
                max_marks_per_node: 8,
                max_links: 512,
                max_attribute_bytes: 2048,
                max_url_bytes: 2048,
            },
            Self::Discussion => RichTextLimits {
                max_json_bytes: 256 * 1024,
                max_depth: 14,
                max_nodes: 2048,
                max_text_chars: 100_000,
                max_marks_per_node: 8,
                max_links: 256,
                max_attribute_bytes: 2048,
                max_url_bytes: 2048,
            },
            Self::Comment => RichTextLimits {
                max_json_bytes: 64 * 1024,
                max_depth: 10,
                max_nodes: 512,
                max_text_chars: 20_000,
                max_marks_per_node: 6,
                max_links: 32,
                max_attribute_bytes: 1024,
                max_url_bytes: 1024,
            },
        }
    }

    pub fn manifest(self) -> RichTextProfileManifest {
        RichTextProfileManifest {
            id: self.id(),
            nodes: self
                .node_kinds()
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            marks: self
                .mark_kinds()
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            heading_levels: self.heading_levels().to_vec(),
            allow_empty: false,
            external_link_rel: self.external_link_rel().to_string(),
            limits: self.limits(),
        }
    }

    pub(crate) fn allows_node(self, kind: &str) -> bool {
        self.node_kinds().contains(&kind)
    }

    pub(crate) fn allows_mark(self, kind: &str) -> bool {
        self.mark_kinds().contains(&kind)
    }

    pub(crate) fn allows_heading_level(self, level: u8) -> bool {
        self.heading_levels().contains(&level)
    }

    pub(crate) fn external_link_rel(self) -> &'static str {
        match self {
            Self::Article => "noopener noreferrer",
            Self::Discussion | Self::Comment => "noopener noreferrer nofollow ugc",
        }
    }

    fn node_kinds(self) -> &'static [&'static str] {
        match self {
            Self::Article | Self::Discussion => ARTICLE_NODES,
            Self::Comment => COMMENT_NODES,
        }
    }

    fn mark_kinds(self) -> &'static [&'static str] {
        BASE_MARKS
    }

    fn heading_levels(self) -> &'static [u8] {
        match self {
            Self::Article | Self::Discussion => ARTICLE_HEADINGS,
            Self::Comment => NO_HEADINGS,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct RichTextLimits {
    pub max_json_bytes: usize,
    pub max_depth: usize,
    pub max_nodes: usize,
    pub max_text_chars: usize,
    pub max_marks_per_node: usize,
    pub max_links: usize,
    pub max_attribute_bytes: usize,
    pub max_url_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RichTextProfileManifest {
    pub id: RichTextProfileId,
    pub nodes: Vec<String>,
    pub marks: Vec<String>,
    pub heading_levels: Vec<u8>,
    pub allow_empty: bool,
    pub external_link_rel: String,
    pub limits: RichTextLimits,
}

pub fn profile_from_id(id: &RichTextProfileId) -> Result<RichTextProfile, RichTextError> {
    match id.as_str() {
        "article" => Ok(RichTextProfile::Article),
        "discussion" => Ok(RichTextProfile::Discussion),
        "comment" => Ok(RichTextProfile::Comment),
        _ => Err(RichTextError::UnsupportedProfile),
    }
}

pub fn all_profile_manifests() -> Vec<RichTextProfileManifest> {
    [
        RichTextProfile::Article,
        RichTextProfile::Discussion,
        RichTextProfile::Comment,
    ]
    .into_iter()
    .map(RichTextProfile::manifest)
    .collect()
}
