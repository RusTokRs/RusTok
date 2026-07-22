use rustok_core::{
    RtJsonValidationConfig, sanitize_rt_json_before_html_render, validate_and_sanitize_rt_json,
};
use serde_json::json;

#[test]
fn rt_json_sanitizes_marks_and_attrs_before_rendering() {
    let payload = json!({
        "version": "rt_json_v1",
        "locale": "en",
        "doc": {
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "attrs": {"class": "should-be-removed"},
                "content": [{
                    "type": "text",
                    "text": "Hello",
                    "marks": [
                        {"type": "bold", "attrs": {"ignored": true}},
                        {"type": "unknown"},
                        {"type": "link", "attrs": {"href": "mailto:team@example.com", "target": "_blank"}}
                    ]
                }]
            }]
        }
    });

    let sanitized =
        sanitize_rt_json_before_html_render(&payload, &RtJsonValidationConfig::for_locale("en"))
            .unwrap();

    let paragraph = &sanitized["doc"]["content"][0];
    assert!(paragraph.get("attrs").is_none());
    let marks = sanitized["doc"]["content"][0]["content"][0]["marks"]
        .as_array()
        .unwrap();
    assert_eq!(marks.len(), 2);
    assert_eq!(marks[0], json!({"type": "bold"}));
    assert_eq!(
        marks[1],
        json!({"type": "link", "attrs": {"href": "mailto:team@example.com"}})
    );
}

#[test]
fn rt_json_rejects_locale_mismatch_and_invalid_locale_tags() {
    let mismatched = json!({
        "version": "rt_json_v1",
        "locale": "ru-RU",
        "doc": {"type": "doc", "content": []}
    });
    let err = validate_and_sanitize_rt_json(&mismatched, &RtJsonValidationConfig::for_locale("en"))
        .unwrap_err();
    assert!(err.contains("must match request locale"));

    let invalid = json!({
        "version": "rt_json_v1",
        "locale": "en_us",
        "doc": {"type": "doc", "content": []}
    });
    let err = validate_and_sanitize_rt_json(&invalid, &RtJsonValidationConfig::for_locale("en_us"))
        .unwrap_err();
    assert!(err.contains("must be a valid locale"));
}

#[test]
fn rt_json_rejects_disallowed_image_and_embed_urls() {
    let image = json!({
        "version": "rt_json_v1",
        "locale": "en",
        "doc": {"type": "doc", "content": [{"type": "image", "attrs": {"src": "data:text/html,<svg>"}}]}
    });
    assert!(
        validate_and_sanitize_rt_json(&image, &RtJsonValidationConfig::for_locale("en")).is_err()
    );

    let embed = json!({
        "version": "rt_json_v1",
        "locale": "en",
        "doc": {"type": "doc", "content": [{"type": "embed", "attrs": {"provider": "youtube", "url": "https://evil.example/watch?v=1"}}]}
    });
    assert!(
        validate_and_sanitize_rt_json(&embed, &RtJsonValidationConfig::for_locale("en")).is_err()
    );
}

#[test]
fn rt_json_transforms_wrapped_legacy_payload_and_preserves_doc() {
    let legacy = json!({
        "locale": "en",
        "doc": {"type": "doc", "content": [{"type": "unsupported"}, {"type": "paragraph"}]}
    });

    let result =
        validate_and_sanitize_rt_json(&legacy, &RtJsonValidationConfig::for_locale("en")).unwrap();

    assert!(result.transformed_from_legacy);
    assert_eq!(result.sanitized["version"], "rt_json_v1");
    assert_eq!(
        result.sanitized["doc"]["content"].as_array().unwrap().len(),
        1
    );
}

#[test]
fn rt_json_respects_custom_depth_and_node_limits() {
    let payload = json!({
        "version": "rt_json_v1",
        "locale": "en",
        "doc": {"type": "doc", "content": [{"type": "paragraph"}, {"type": "paragraph"}]}
    });

    let mut config = RtJsonValidationConfig::for_locale("en");
    config.max_nodes = 2;
    let err = validate_and_sanitize_rt_json(&payload, &config).unwrap_err();
    assert!(err.contains("max node count"));

    let mut config = RtJsonValidationConfig::for_locale("en");
    config.max_depth = 1;
    let err = validate_and_sanitize_rt_json(&payload, &config).unwrap_err();
    assert!(err.contains("max depth"));
}
