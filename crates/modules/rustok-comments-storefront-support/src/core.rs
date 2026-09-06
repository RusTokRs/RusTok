use rustok_api::{RichTextDocument, RichTextNode};

pub fn is_richtext_blank(document: &RichTextDocument) -> bool {
    document.content.iter().all(node_is_blank)
}

fn node_is_blank(node: &RichTextNode) -> bool {
    node.text
        .as_deref()
        .is_none_or(|text| text.trim().is_empty())
        && node.content.iter().all(node_is_blank)
}

#[cfg(test)]
mod tests {
    use super::is_richtext_blank;
    use rustok_api::RichTextDocument;

    #[test]
    fn empty_document_is_blank() {
        assert!(is_richtext_blank(&RichTextDocument::empty()));
    }

    #[test]
    fn whitespace_only_document_is_blank() {
        assert!(is_richtext_blank(&RichTextDocument::single_paragraph(
            " \n\t "
        )));
    }

    #[test]
    fn text_document_is_not_blank() {
        assert!(!is_richtext_blank(&RichTextDocument::single_paragraph(
            "Useful comment"
        )));
    }
}
