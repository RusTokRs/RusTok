use crate::SWAGGER_UI_VENDORED;

#[test]
fn test_swagger_ui_vendored_magic_bytes() {
    assert!(!SWAGGER_UI_VENDORED.is_empty());
    // ZIP local file header signature: PK\x03\x04
    assert_eq!(&SWAGGER_UI_VENDORED[..4], &[0x50, 0x4B, 0x03, 0x04]);
}

#[test]
fn test_swagger_ui_vendored_contents() {
    let raw = SWAGGER_UI_VENDORED;
    assert!(raw.len() > 100);

    // Verify key asset names are present in the archive
    let text = String::from_utf8_lossy(raw);
    assert!(text.contains("swagger-ui-5.17.12/dist/swagger-initializer.js"));
    assert!(text.contains("swagger-ui-5.17.12/dist/index.html"));
}
