/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusToK Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

mod error;
mod plain_text;
mod profile;
mod render;
mod validate;

use rustok_api::RichTextDocument;

pub use error::{RichTextError, RichTextErrorCode};
pub use plain_text::plain_text;
pub use profile::{
    RichTextLimits, RichTextProfile, RichTextProfileManifest, all_profile_manifests,
    profile_from_id,
};
pub use render::{project, render_html};
pub use validate::{parse_json, validate, validate_and_normalize};

pub fn canonical_json(document: &RichTextDocument) -> Result<String, RichTextError> {
    serde_json::to_string(document).map_err(|_| RichTextError::InvalidJson)
}

#[cfg(test)]
mod tests;
