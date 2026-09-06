use crate::error::{BlogError, BlogResult};
use rustok_api::{RichTextDocument, RichTextView};
use rustok_content::{
    RichTextProfile, canonical_json, plain_text, project, validate_and_normalize,
};

/// Convert normalized plain text into a canonical Blog article document.
///
/// Blank lines delimit paragraphs and wrapped non-empty lines are joined with a
/// single space. This adapter deliberately accepts text only: Markdown aliases,
/// raw JSON, HTML, and caller-selected richtext profiles remain outside the
/// owner contract.
pub fn article_document_from_plain_text(text: &str) -> RichTextDocument {
    let mut paragraphs = Vec::new();
    let mut paragraph_lines = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            if !paragraph_lines.is_empty() {
                paragraphs.push(paragraph_lines.join(" "));
                paragraph_lines.clear();
            }
        } else {
            paragraph_lines.push(line);
        }
    }

    if !paragraph_lines.is_empty() {
        paragraphs.push(paragraph_lines.join(" "));
    }

    RichTextDocument {
        kind: "doc".to_string(),
        content: paragraphs
            .into_iter()
            .flat_map(|paragraph| RichTextDocument::single_paragraph(paragraph).content)
            .collect(),
    }
}

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
pub fn project_stored_article(body: &str) -> BlogResult<(RichTextView, String)> {
    let document = serde_json::from_str(body)
        .map_err(|_| BlogError::validation("Stored article content is not a document"))?;

    project_article(document)
}

#[cfg(test)]
mod tests {
    use rustok_api::RichTextDocument;

    use super::{
        article_document_from_plain_text, canonical_article_body, normalize_article,
        project_article, project_stored_article,
    };

    #[test]
    fn plain_text_import_builds_canonical_article_paragraphs() {
        let document = article_document_from_plain_text(
            "  First line  \n second line\n\n\n  Third paragraph  ",
        );

        assert_eq!(document.kind, "doc");
        assert_eq!(document.content.len(), 2);
        assert_eq!(document.content[0].kind, "paragraph");
        assert_eq!(
            document.content[0].content[0].text.as_deref(),
            Some("First line second line")
        );
        assert_eq!(
            document.content[1].content[0].text.as_deref(),
            Some("Third paragraph")
        );
        assert_eq!(
            normalize_article(document.clone()).expect("normalize imported document"),
            document
        );
    }

    #[test]
    fn plain_text_import_returns_empty_canonical_document_for_blank_input() {
        assert_eq!(
            article_document_from_plain_text(" \n\n \t"),
            RichTextDocument::empty()
        );
    }

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

        let (view, text) = project_stored_article(&stored.to_string()).expect("projection");
        assert_eq!(view.document.kind, "doc");
        assert_eq!(text, "Article");
    }
}
