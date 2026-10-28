use rustok_api::{RichTextDocument, RichTextView};
use rustok_content::richtext::{
    canonical_json, parse_json, plain_text, project, validate_and_normalize, RichTextError,
    RichTextProfile,
};

use crate::error::{ForumError, ForumResult};

#[derive(Debug, Clone)]
pub struct ForumBodyProjection {
    pub view: RichTextView,
    pub plain_text: String,
}

pub fn normalize_discussion(document: RichTextDocument) -> ForumResult<RichTextDocument> {
    validate_and_normalize(document, RichTextProfile::Discussion).map_err(map_richtext_error)
}

pub fn serialize_discussion(document: RichTextDocument) -> ForumResult<String> {
    let document = normalize_discussion(document)?;
    canonical_json(&document).map_err(map_richtext_error)
}

pub fn project_discussion(document: RichTextDocument) -> ForumResult<ForumBodyProjection> {
    let document = normalize_discussion(document)?;
    let plain_text =
        plain_text(&document, RichTextProfile::Discussion).map_err(map_richtext_error)?;
    let view = project(&document, RichTextProfile::Discussion).map_err(map_richtext_error)?;
    Ok(ForumBodyProjection { view, plain_text })
}

pub fn project_stored_discussion(raw: &str) -> ForumResult<ForumBodyProjection> {
    let document = parse_json(raw, RichTextProfile::Discussion).map_err(map_richtext_error)?;
    project_discussion(document)
}

fn map_richtext_error(error: RichTextError) -> ForumError {
    ForumError::Validation(format!("Invalid forum richtext: {error}"))
}

#[cfg(test)]
mod tests {
    use rustok_api::RichTextDocument;

    use super::{project_discussion, project_stored_discussion, serialize_discussion};

    #[test]
    fn discussion_storage_and_projections_share_one_document() {
        let document = RichTextDocument::single_paragraph("Forum <body>");
        let stored = serialize_discussion(document.clone()).expect("serialize discussion");
        let projection = project_stored_discussion(&stored).expect("project stored discussion");

        assert_eq!(projection.view.document, document);
        assert_eq!(projection.plain_text, "Forum <body>");
        assert_eq!(
            projection.view.html,
            "<p class=\"richtext-paragraph\">Forum &lt;body&gt;</p>"
        );
    }

    #[test]
    fn invalid_discussion_fails_closed() {
        let error =
            project_discussion(RichTextDocument::empty()).expect_err("empty discussion must fail");
        assert!(error.to_string().contains("Invalid forum richtext"));
    }
}
