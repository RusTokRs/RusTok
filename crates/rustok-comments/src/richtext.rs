/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusToK Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use rustok_api::{RichTextDocument, RichTextView};
use rustok_content::richtext::{
    RichTextError, RichTextProfile, canonical_json, parse_json, plain_text, project,
    validate_and_normalize,
};

use crate::error::{CommentsError, CommentsResult};

pub(crate) struct CommentBodyProjection {
    pub(crate) view: RichTextView,
    pub(crate) plain_text: String,
}

pub(crate) fn serialize_comment_body(document: RichTextDocument) -> CommentsResult<String> {
    let document =
        validate_and_normalize(document, RichTextProfile::Comment).map_err(map_richtext_error)?;
    canonical_json(&document).map_err(map_richtext_error)
}

pub(crate) fn project_comment_body(raw: &str) -> CommentsResult<CommentBodyProjection> {
    let document = parse_json(raw, RichTextProfile::Comment).map_err(map_richtext_error)?;
    let plain_text = plain_text(&document, RichTextProfile::Comment).map_err(map_richtext_error)?;
    let view = project(&document, RichTextProfile::Comment).map_err(map_richtext_error)?;
    Ok(CommentBodyProjection { view, plain_text })
}

fn map_richtext_error(error: RichTextError) -> CommentsError {
    CommentsError::Validation(format!("Invalid comment richtext: {error}"))
}
