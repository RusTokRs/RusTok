//! # rustok-docs
//!
//! Collaborative documents, wiki and knowledge base module powered by Loro CRDT.

pub use loro::LoroDoc;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loro_smoke() {
        let doc = LoroDoc::new();
        let text = doc.get_text("content");
        text.insert(0, "Hello, RusToK Wiki!").unwrap();
        assert_eq!(text.to_string(), "Hello, RusToK Wiki!");
    }
}
