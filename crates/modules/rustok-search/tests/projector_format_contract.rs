#[test]
fn product_channel_json_path_is_escaped_for_format_macro() {
    let source = include_str!("../src/projector_legacy.rs");
    let escaped_path = "p.metadata #> '{{channel_visibility,allowed_channel_slugs}}'";

    assert_eq!(
        source.matches(escaped_path).count(),
        2,
        "both executable JSON-path uses inside format! must escape literal braces"
    );
    assert!(
        !source
            .lines()
            .any(|line| line.trim() == "p.metadata #> '{channel_visibility,allowed_channel_slugs}'"),
        "an unescaped executable JSON path would make rustok-search fail to compile"
    );
}
