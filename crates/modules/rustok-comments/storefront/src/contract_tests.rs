use rustok_api::{RichTextDocument, RichTextNode};

use crate::core::{CommentComposerValidationError, is_richtext_blank, validate_comment_submission};
use crate::ui::CommentComposerCopy;

#[test]
fn test_crate_api_compliance() {
    let api_doc = include_str!("../CRATE_API.md");
    assert!(api_doc.contains("## Platform Capability Bindings"));
    assert!(api_doc.contains("## Domain Collaborations"));
    assert!(api_doc.contains("## Minimum Contract Set"));
    assert!(api_doc.contains("### Input DTOs/Commands"));
    assert!(api_doc.contains("### Domain Invariants"));
    assert!(api_doc.contains("### Events / Outbox Side Effects"));
    assert!(api_doc.contains("### Errors / Failure Codes"));
}

#[test]
fn ffa_core_boundary_invariants() {
    let core_source = include_str!("core.rs");
    assert!(
        !core_source.contains("use leptos"),
        "core.rs must not depend on leptos"
    );
    assert!(
        !core_source.contains("leptos::"),
        "core.rs must not invoke leptos APIs"
    );
    assert!(
        !core_source.contains("#[component]"),
        "core.rs must not define leptos components"
    );
}

#[test]
fn validation_error_traits_and_display() {
    let err = CommentComposerValidationError::Empty;
    assert_eq!(err.to_string(), "Comment body cannot be blank");
    assert_eq!(err.error_key(), "comments.composer.empty_error");
    assert_eq!(err, CommentComposerValidationError::Empty);

    let cloned = err;
    assert_eq!(err, cloned);
    assert_eq!(format!("{err:?}"), "Empty");
}

#[test]
fn is_richtext_blank_extended_cases() {
    // Multi-paragraph with only spaces, tabs, and newlines
    let doc = RichTextDocument {
        kind: "doc".to_string(),
        content: vec![
            RichTextNode {
                kind: "paragraph".to_string(),
                text: None,
                attrs: Default::default(),
                marks: Vec::new(),
                content: vec![RichTextNode {
                    kind: "text".to_string(),
                    text: Some(" \t ".to_string()),
                    attrs: Default::default(),
                    marks: Vec::new(),
                    content: Vec::new(),
                }],
            },
            RichTextNode {
                kind: "paragraph".to_string(),
                text: None,
                attrs: Default::default(),
                marks: Vec::new(),
                content: vec![RichTextNode {
                    kind: "text".to_string(),
                    text: Some("\n".to_string()),
                    attrs: Default::default(),
                    marks: Vec::new(),
                    content: Vec::new(),
                }],
            },
        ],
    };
    assert!(is_richtext_blank(&doc));
    assert_eq!(
        validate_comment_submission(&doc),
        Err(CommentComposerValidationError::Empty)
    );

    // One non-empty paragraph among empty ones
    let mut valid_doc = doc.clone();
    valid_doc.content.push(RichTextNode {
        kind: "paragraph".to_string(),
        text: None,
        attrs: Default::default(),
        marks: Vec::new(),
        content: vec![RichTextNode {
            kind: "text".to_string(),
            text: Some("Real comment content".to_string()),
            attrs: Default::default(),
            marks: Vec::new(),
            content: Vec::new(),
        }],
    });
    assert!(!is_richtext_blank(&valid_doc));
    assert!(validate_comment_submission(&valid_doc).is_ok());
}

#[test]
fn comment_composer_copy_traits() {
    let copy = CommentComposerCopy {
        title: "Leave a comment".to_string(),
        editor_label: "Comment text".to_string(),
        hint: "Be respectful".to_string(),
        submit: "Post".to_string(),
        submitting: "Posting...".to_string(),
        success: "Posted successfully".to_string(),
        sign_in_required: "Sign in to comment".to_string(),
        empty_error: "Body cannot be empty".to_string(),
        richtext: leptos_ui::RichTextFrameCopy {
            messages: serde_json::Value::Null,
            serialization_error: "Serialization error".to_string(),
            invalid_payload_error: "Invalid payload".to_string(),
            frame_error: "Frame error".to_string(),
        },
    };

    let cloned = copy.clone();
    assert_eq!(copy.title, cloned.title);
    assert_eq!(copy.submit, cloned.submit);
    assert!(format!("{copy:?}").contains("Leave a comment"));
}
