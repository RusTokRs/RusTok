use crate::*;
use serde_json::json;

#[test]
fn test_package_metadata_contract() {
    assert_eq!(env!("CARGO_PKG_NAME"), "fly");
    assert!(!env!("CARGO_PKG_VERSION").is_empty());
    assert!(!env!("CARGO_PKG_DESCRIPTION").is_empty());
}

#[test]
fn test_fly_editor_command_undo_redo_contract() {
    let raw_project = json!({
        "assets": [],
        "styles": [],
        "pages": [{
            "id": "home",
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "hero",
                    "type": "section",
                    "style": { "color": "red", "padding": "24px" }
                }]
            }
        }]
    });

    let doc = GrapesJsCodec::decode_value(raw_project).expect("decoding minimal project must succeed");
    let mut editor = FlyEditor::new(doc, RegistrySet::with_builtins());
    assert_eq!(editor.history().undo_len(), 0);
    assert_eq!(editor.history().redo_len(), 0);

    let initial_hash = editor.document().hash();

    // Apply a patch command to modify component style
    let patch_cmd = EditorCommand::Patch {
        component_id: "hero".to_string(),
        patch: ComponentPatch::default().merge_style(json!({ "width": "320px" })),
    };

    editor.apply(patch_cmd).expect("command execution must succeed");
    assert_eq!(editor.history().undo_len(), 1);
    assert_eq!(editor.history().redo_len(), 0);
    assert_ne!(editor.document().hash(), initial_hash);

    // Undo should restore initial hash
    editor.undo().expect("undo must succeed");
    assert_eq!(editor.history().undo_len(), 0);
    assert_eq!(editor.history().redo_len(), 1);
    assert_eq!(editor.document().hash(), initial_hash);

    // Redo should reapply change
    editor.redo().expect("redo must succeed");
    assert_eq!(editor.history().undo_len(), 1);
    assert_eq!(editor.history().redo_len(), 0);
    assert_ne!(editor.document().hash(), initial_hash);
}

#[test]
fn test_depth_limit_rejection_contract() {
    // Generate a payload nested deeper than MAXIMUM_DECODE_DEPTH
    let mut nested = json!({ "type": "text", "content": "leaf" });
    for _ in 0..(MAXIMUM_DECODE_DEPTH + 10) {
        nested = json!({
            "type": "box",
            "components": [nested]
        });
    }

    let deep_payload = json!({
        "pages": [{
            "id": "deep-page",
            "frames": [{
                "id": "deep-frame",
                "component": nested
            }]
        }]
    });

    let result = GrapesJsCodec::decode_value(deep_payload);
    assert!(
        result.is_err(),
        "Payload exceeding MAXIMUM_DECODE_DEPTH must be rejected fail-closed"
    );
}

#[test]
fn test_safe_digest_and_constant_time_comparison() {
    let data = b"rustok-fly-visual-editor-verification";
    let hash = sha256_hex(data);
    assert_eq!(hash.len(), 64);

    let digest1 = ContentDigest::from_bytes(data);
    let digest2 = ContentDigest::from_bytes(data);
    assert!(constant_time_eq(digest1.as_str().as_bytes(), digest2.as_str().as_bytes()));

    let digest3 = ContentDigest::from_bytes(b"other-data");
    assert!(!constant_time_eq(digest1.as_str().as_bytes(), digest3.as_str().as_bytes()));
}

#[test]
fn test_safe_url_validation_contract() {
    let providers = builtin_embed_providers();
    assert!(!providers.is_empty(), "Builtin embed providers must exist");
    let youtube = &providers[0]; // YouTube provider

    let hostile_urls = [
        "javascript:alert(1)",
        "JAVASCRIPT:malicious()",
        "data:text/html,<script>alert(1)</script>",
        "vbscript:msgbox",
        "http://youtube.com/watch?v=123", // Non-HTTPS rejected
    ];

    for hostile in &hostile_urls {
        assert!(
            validate_embed_url(youtube, hostile).is_err(),
            "Hostile embed URL must be rejected: {}",
            hostile
        );
    }

    let benign_urls = [
        "https://www.youtube.com/embed/dQw4w9WgXcQ",
        "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ",
    ];

    for benign in &benign_urls {
        assert!(
            validate_embed_url(youtube, benign).is_ok(),
            "Benign embed URL should be accepted: {}",
            benign
        );
    }
}
