use crate::error::{BlogError, BlogResult};
use rustok_api::{RichTextDocument, RichTextView};
use rustok_content::{
    RichTextProfile, canonical_json, plain_text, project, validate_and_normalize,
};

/// Normalize a Blog article document at the owner boundary.
///
/// The profile is deliberately fixed here: callers cannot select a
/// formatter-specific alias for article content.
pub fn normalize_article(document: RichTextDocument) -> BlogResult<RichTextDocument> {
    validate_and_normalize(document, RichTextProfile::Article)
        .map_err(|error| BlogError::validation(error.to_string()))
}

pub fn canonical_article_body(document: &RichTextDocument) -> BlogResult<String> {
    canonical_json(document).map_err(|error| BlogError::validation(error.to_string()))
}

pub fn project_article(document: RichTextDocument) -> BlogResult<(RichTextView, String)> {
    let document = normalize_article(document)?;
    let view = project(&document, RichTextProfile::Article)
        .map_err(|error| BlogError::validation(error.to_string()))?;
    let text = plain_text(&document, RichTextProfile::Article)
        .map_err(|error| BlogError::validation(error.to_string()))?;
    Ok((view, text))
}

/// Project a canonical storage row through the current article policy.
pub fn project_stored_article(body: &str, format: &str) -> BlogResult<(RichTextView, String)> {
    if format != "richtext" {
        return Err(BlogError::validation(
            "Stored article content does not use the canonical richtext format",
        ));
    }
    let document = serde_json::from_str(body)
        .map_err(|_| BlogError::validation("Stored article content is not a document"))?;

    project_article(document)
}

#[cfg(test)]
mod tests {
    use rustok_api::RichTextDocument;

    use super::{canonical_article_body, project_article, project_stored_article};

    #[test]
    fn article_projection_returns_canonical_document_html_and_text() {
        let document: RichTextDocument = serde_json::from_value(serde_json::json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{"type": "text", "text": "Hello <world>"}]
            }]
        }))
        .expect("document");

        let normalized = super::normalize_article(document.clone()).expect("normalize");
        assert_eq!(
            canonical_article_body(&normalized).expect("canonical body"),
            serde_json::to_string(&document).expect("document JSON")
        );

        let (view, text) = project_article(document).expect("projection");
        assert_eq!(
            view.html,
            "<p class=\"richtext-paragraph\">Hello &lt;world&gt;</p>"
        );
        assert_eq!(text, "Hello <world>");
    }

    #[test]
    fn canonical_stored_document_is_projected() {
        let stored = serde_json::json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{"type": "text", "text": "Article"}]
            }]
        });

        let (view, text) =
            project_stored_article(&stored.to_string(), "richtext").expect("projection");
        assert_eq!(view.document.kind, "doc");
        assert_eq!(text, "Article");
    }
}
