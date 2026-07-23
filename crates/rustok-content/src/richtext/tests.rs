/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusToK Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use rustok_api::RichTextDocument;
use serde_json::{Value, json};

use super::{
    RichTextError, RichTextErrorCode, RichTextProfile, all_profile_manifests, canonical_json,
    parse_json, plain_text, project, render_html, validate_and_normalize,
};

fn document(value: Value) -> RichTextDocument {
    serde_json::from_value(value).expect("test document must match the neutral structure")
}

#[test]
fn article_fixture_has_one_canonical_projection() {
    let raw = include_str!("../../fixtures/richtext/article.json");
    let document = parse_json(raw, RichTextProfile::Article).expect("valid article fixture");

    assert_eq!(
        render_html(&document, RichTextProfile::Article).expect("render"),
        include_str!("../../fixtures/richtext/article.html").trim_end()
    );
    assert_eq!(
        plain_text(&document, RichTextProfile::Article).expect("plain text"),
        include_str!("../../fixtures/richtext/article.txt").trim_end()
    );

    let view = project(&document, RichTextProfile::Article).expect("projection");
    assert_eq!(view.document, document);
    assert_eq!(
        canonical_json(&view.document).expect("canonical JSON"),
        serde_json::to_string(&document).expect("serialize")
    );
}

#[test]
fn profile_manifest_matches_the_shared_fixture() {
    let expected: Value =
        serde_json::from_str(include_str!("../../fixtures/richtext/profiles.json"))
            .expect("valid profile fixture");
    let actual = serde_json::to_value(all_profile_manifests()).expect("serialize manifest");
    assert_eq!(actual, expected);
}

#[test]
fn normalization_orders_marks_and_removes_tiptap_defaults() {
    let document = document(json!({
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [{
                "type": "text",
                "text": "linked",
                "marks": [
                    {"type": "italic"},
                    {
                        "type": "link",
                        "attrs": {
                            "href": "https://example.com/?a=1&b=2",
                            "target": null,
                            "rel": null,
                            "class": null
                        }
                    },
                    {"type": "bold"}
                ]
            }]
        }]
    }));

    let normalized =
        validate_and_normalize(document, RichTextProfile::Article).expect("normalizable document");
    let marks = &normalized.content[0].content[0].marks;
    assert_eq!(
        marks
            .iter()
            .map(|mark| mark.kind.as_str())
            .collect::<Vec<_>>(),
        vec!["link", "bold", "italic"]
    );
    assert_eq!(
        marks[0]
            .attrs
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["href"]
    );
}

#[test]
fn invalid_content_fails_closed_without_silent_drops() {
    let unknown = document(json!({
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [{"type": "mystery", "text": "do not drop me"}]
        }]
    }));
    assert_eq!(
        validate_and_normalize(unknown, RichTextProfile::Article)
            .expect_err("unknown node must fail")
            .code(),
        RichTextErrorCode::InvalidStructure
    );

    let duplicate_mark = document(json!({
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [{
                "type": "text",
                "text": "duplicate",
                "marks": [{"type": "bold"}, {"type": "bold"}]
            }]
        }]
    }));
    assert_eq!(
        validate_and_normalize(duplicate_mark, RichTextProfile::Article)
            .expect_err("duplicate mark must fail")
            .code(),
        RichTextErrorCode::DuplicateMark
    );
}

#[test]
fn link_policy_rejects_script_credentials_and_protocol_relative_urls() {
    for href in [
        "javascript:alert(1)",
        "data:text/html,boom",
        "//evil.example/path",
        "https://user:secret@example.com/",
    ] {
        let document = document(json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{
                    "type": "text",
                    "text": "unsafe",
                    "marks": [{"type": "link", "attrs": {"href": href}}]
                }]
            }]
        }));
        assert_eq!(
            validate_and_normalize(document, RichTextProfile::Article)
                .expect_err("unsafe URL must fail")
                .code(),
            RichTextErrorCode::UnsafeLink
        );
    }
}

#[test]
fn renderer_escapes_text_and_link_attributes() {
    let document = document(json!({
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [
                {"type": "text", "text": "<script>alert('x')</script> "},
                {
                    "type": "text",
                    "text": "link",
                    "marks": [{
                        "type": "link",
                        "attrs": {"href": "https://example.com/?a=1&b=%22x%22"}
                    }]
                }
            ]
        }]
    }));

    let html = render_html(&document, RichTextProfile::Comment).expect("safe render");
    assert!(!html.contains("<script>"));
    assert!(html.contains("&lt;script&gt;"));
    assert!(html.contains("&amp;b="));
    assert!(html.contains("rel=\"noopener noreferrer nofollow ugc\""));
}

#[test]
fn comment_profile_rejects_article_only_nodes() {
    let document = document(json!({
        "type": "doc",
        "content": [{
            "type": "heading",
            "attrs": {"level": 2},
            "content": [{"type": "text", "text": "Not allowed"}]
        }]
    }));

    assert_eq!(
        validate_and_normalize(document, RichTextProfile::Comment)
            .expect_err("comment heading must fail")
            .code(),
        RichTextErrorCode::UnsupportedNode
    );
}

#[test]
fn empty_and_oversized_documents_are_rejected() {
    let empty = document(json!({
        "type": "doc",
        "content": [{"type": "paragraph"}]
    }));
    assert_eq!(
        validate_and_normalize(empty, RichTextProfile::Article)
            .expect_err("empty document must fail"),
        RichTextError::EmptyDocument
    );

    let oversized = format!(
        r#"{{"type":"doc","content":[{{"type":"paragraph","content":[{{"type":"text","text":"{}"}}]}}]}}"#,
        "x".repeat(70_000)
    );
    assert_eq!(
        parse_json(&oversized, RichTextProfile::Comment)
            .expect_err("raw input limit must apply")
            .code(),
        RichTextErrorCode::DocumentTooLarge
    );
}

#[test]
fn invalid_tree_grammar_is_rejected() {
    let paragraph_in_paragraph = document(json!({
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [{
                "type": "paragraph",
                "content": [{"type": "text", "text": "nested"}]
            }]
        }]
    }));
    assert_eq!(
        validate_and_normalize(paragraph_in_paragraph, RichTextProfile::Article)
            .expect_err("invalid parent/child pair must fail")
            .code(),
        RichTextErrorCode::InvalidStructure
    );

    let list_without_item = document(json!({
        "type": "doc",
        "content": [{
            "type": "bulletList",
            "content": [{
                "type": "paragraph",
                "content": [{"type": "text", "text": "not an item"}]
            }]
        }]
    }));
    assert_eq!(
        validate_and_normalize(list_without_item, RichTextProfile::Article)
            .expect_err("list grammar must fail")
            .code(),
        RichTextErrorCode::InvalidStructure
    );
}
