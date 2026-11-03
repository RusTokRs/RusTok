use crate::*;

#[test]
fn test_package_metadata_contract() {
    assert_eq!(env!("CARGO_PKG_NAME"), "fly-browser");
    assert!(!env!("CARGO_PKG_VERSION").is_empty());
    assert!(!env!("CARGO_PKG_DESCRIPTION").is_empty());
}

#[test]
fn test_embedded_assets_contract() {
    assert!(
        !FLY_BROWSER_ADAPTER_JS.is_empty(),
        "Browser adapter JS asset must not be empty"
    );
    assert!(
        !FLY_ACTIONS_JS.is_empty(),
        "Actions JS asset must not be empty"
    );
    assert!(
        FLY_BROWSER_ADAPTER_JS.contains("fly"),
        "Browser JS asset must reference fly"
    );
    assert!(
        FLY_ACTIONS_JS.contains("fly"),
        "Actions JS asset must reference fly"
    );
}

#[test]
fn test_protocol_constants_contract() {
    assert_eq!(FLY_BROWSER_PROTOCOL, "fly_iframe");
    assert_eq!(FLY_BROWSER_ADAPTER, "fly_browser");
    assert_eq!(FLY_CSRF_HEADER_NAME, "x-csrf-token");
    const { assert!(DEFAULT_MAX_BROWSER_MESSAGE_BYTES >= 1024) };
    const { assert!(DEFAULT_INTENT_REQUEST_TIMEOUT_MS > 0) };
}

#[test]
fn test_csrf_derivation_contract() {
    let session_token = "secure-fly-session-secret-987654";
    let csrf = derive_csrf_token(session_token);
    assert_eq!(csrf.len(), 64, "CSRF token must be 64-char SHA256 hex");
    assert!(
        verify_csrf_token(session_token, &csrf),
        "Valid CSRF token must verify"
    );
    assert!(
        !verify_csrf_token("tampered-session", &csrf),
        "Tampered session must fail"
    );
    assert!(
        !verify_csrf_token(session_token, "wrong-csrf-hex"),
        "Wrong CSRF token must fail"
    );
    assert!(!verify_csrf_token("", &csrf), "Empty session must fail");
}

#[test]
fn test_intent_kind_classification_contract() {
    // Mutating intents
    assert!(BrowserIntentKind::InsertBlock.is_mutating());
    assert!(BrowserIntentKind::RemoveSelected.is_mutating());
    assert!(BrowserIntentKind::MoveSelected.is_mutating());
    assert!(BrowserIntentKind::PatchSelected.is_mutating());
    assert!(BrowserIntentKind::Undo.is_mutating());
    assert!(BrowserIntentKind::Redo.is_mutating());

    // Non-mutating intents
    assert!(!BrowserIntentKind::Select.is_mutating());
    assert!(!BrowserIntentKind::Hover.is_mutating());
    assert!(!BrowserIntentKind::FocusRequested.is_mutating());
    assert!(!BrowserIntentKind::HoverRequested.is_mutating());
}
