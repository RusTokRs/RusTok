use rustok_api::{RichTextDocument, RichTextNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentComposerValidationError {
    Empty,
}

impl CommentComposerValidationError {
    pub const fn error_key(&self) -> &'static str {
        match self {
            Self::Empty => "comments.composer.empty_error",
        }
    }
}

impl std::fmt::Display for CommentComposerValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "Comment body cannot be blank"),
        }
    }
}

impl std::error::Error for CommentComposerValidationError {}

pub fn validate_comment_submission(
    document: &RichTextDocument,
) -> Result<(), CommentComposerValidationError> {
    if is_richtext_blank(document) {
        return Err(CommentComposerValidationError::Empty);
    }
    Ok(())
}

pub fn is_richtext_blank(document: &RichTextDocument) -> bool {
    document.content.is_empty() || document.content.iter().all(node_is_blank)
}

fn node_is_blank(node: &RichTextNode) -> bool {
    node.text
        .as_deref()
        .is_none_or(|text| text.trim().is_empty())
        && node.content.iter().all(node_is_blank)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::{RichTextDocument, RichTextNode};

    #[test]
    fn empty_document_is_blank() {
        assert!(is_richtext_blank(&RichTextDocument::empty()));
        assert_eq!(
            validate_comment_submission(&RichTextDocument::empty()),
            Err(CommentComposerValidationError::Empty)
        );
    }

    #[test]
    fn whitespace_only_document_is_blank() {
        let doc = RichTextDocument::single_paragraph(" \n\t ");
        assert!(is_richtext_blank(&doc));
        assert_eq!(
            validate_comment_submission(&doc),
            Err(CommentComposerValidationError::Empty)
        );
    }

    #[test]
    fn text_document_is_not_blank() {
        let doc = RichTextDocument::single_paragraph("Useful comment");
        assert!(!is_richtext_blank(&doc));
        assert!(validate_comment_submission(&doc).is_ok());
    }

    #[test]
    fn nested_empty_nodes_are_blank() {
        let doc = RichTextDocument {
            kind: "doc".to_string(),
            content: vec![RichTextNode {
                kind: "paragraph".to_string(),
                text: None,
                attrs: Default::default(),
                marks: Vec::new(),
                content: vec![RichTextNode {
                    kind: "text".to_string(),
                    text: Some("   ".to_string()),
                    attrs: Default::default(),
                    marks: Vec::new(),
                    content: Vec::new(),
                }],
            }],
        };
        assert!(is_richtext_blank(&doc));
        assert_eq!(
            validate_comment_submission(&doc),
            Err(CommentComposerValidationError::Empty)
        );
    }

    #[test]
    fn validation_error_display_and_key() {
        let err = CommentComposerValidationError::Empty;
        assert_eq!(err.error_key(), "comments.composer.empty_error");
        assert_eq!(err.to_string(), "Comment body cannot be blank");
    }
}
