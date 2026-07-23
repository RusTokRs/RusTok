/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use serde::Serialize;
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RichTextErrorCode {
    DocumentTooLarge,
    InvalidJson,
    UnsupportedProfile,
    InvalidRoot,
    UnsupportedNode,
    UnsupportedMark,
    DuplicateMark,
    UnsupportedAttribute,
    InvalidAttribute,
    InvalidStructure,
    LimitExceeded,
    EmptyDocument,
    UnsafeLink,
}

/// Richtext validation error with a stable reason code and a structural path.
///
/// Variants intentionally avoid retaining body text or attribute values so
/// callers can count reason codes without logging tenant content.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum RichTextError {
    #[error("richtext document exceeds the {maximum}-byte limit")]
    DocumentTooLarge { maximum: usize },
    #[error("richtext document is not valid canonical JSON")]
    InvalidJson,
    #[error("richtext profile is not registered")]
    UnsupportedProfile,
    #[error("richtext root must be a `doc` node")]
    InvalidRoot,
    #[error("unsupported richtext node at {path}")]
    UnsupportedNode { path: String },
    #[error("unsupported richtext mark at {path}")]
    UnsupportedMark { path: String },
    #[error("duplicate richtext mark at {path}")]
    DuplicateMark { path: String },
    #[error("unsupported richtext attribute at {path}")]
    UnsupportedAttribute { path: String },
    #[error("invalid richtext attribute at {path}")]
    InvalidAttribute { path: String },
    #[error("invalid richtext tree structure at {path}")]
    InvalidStructure { path: String },
    #[error("richtext {limit} limit exceeded at {path}; maximum is {maximum}")]
    LimitExceeded {
        limit: &'static str,
        maximum: usize,
        path: String,
    },
    #[error("richtext document must contain non-whitespace text")]
    EmptyDocument,
    #[error("unsafe richtext link at {path}")]
    UnsafeLink { path: String },
}

impl RichTextError {
    pub fn code(&self) -> RichTextErrorCode {
        match self {
            Self::DocumentTooLarge { .. } => RichTextErrorCode::DocumentTooLarge,
            Self::InvalidJson => RichTextErrorCode::InvalidJson,
            Self::UnsupportedProfile => RichTextErrorCode::UnsupportedProfile,
            Self::InvalidRoot => RichTextErrorCode::InvalidRoot,
            Self::UnsupportedNode { .. } => RichTextErrorCode::UnsupportedNode,
            Self::UnsupportedMark { .. } => RichTextErrorCode::UnsupportedMark,
            Self::DuplicateMark { .. } => RichTextErrorCode::DuplicateMark,
            Self::UnsupportedAttribute { .. } => RichTextErrorCode::UnsupportedAttribute,
            Self::InvalidAttribute { .. } => RichTextErrorCode::InvalidAttribute,
            Self::InvalidStructure { .. } => RichTextErrorCode::InvalidStructure,
            Self::LimitExceeded { .. } => RichTextErrorCode::LimitExceeded,
            Self::EmptyDocument => RichTextErrorCode::EmptyDocument,
            Self::UnsafeLink { .. } => RichTextErrorCode::UnsafeLink,
        }
    }
}
