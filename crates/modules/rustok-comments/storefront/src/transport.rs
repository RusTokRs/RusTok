/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Storefront transport facade and contract definitions for comments submission.

use rustok_api::RichTextDocument;

/// Storefront comment creation command payload.
#[derive(Debug, Clone, PartialEq)]
pub struct SubmitCommentPayload {
    pub content_locale: String,
    pub content: RichTextDocument,
}

/// Transport execution status for storefront comment actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommentTransportStatus {
    Idle,
    Submitting,
    Success,
    Error(String),
}
