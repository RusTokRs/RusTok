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

/// The renderer that embeds the token and the endpoint that verifies the header must derive the
/// secret by one rule. They used to differ: the renderer used the session id while the endpoint
/// fell back to an empty secret when the claim was absent, so a token without the claim produced a
/// page whose every save answered 403 with no cause.
#[test]
fn test_csrf_secret_matches_between_embedding_and_verification() {
    fn base64url(input: &[u8]) -> String {
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut out = String::new();
        for chunk in input.chunks(3) {
            let b = [
                chunk[0],
                *chunk.get(1).unwrap_or(&0),
                *chunk.get(2).unwrap_or(&0),
            ];
            let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
            for index in 0..4 {
                if index <= chunk.len() {
                    out.push(ALPHABET[((n >> (18 - 6 * index)) & 0x3F) as usize] as char);
                }
            }
        }
        out
    }

    let session_id = "5f3a9d0e-2f4b-4c1a-9e77-0b1d2c3a4b5c";
    let payload = format!(r#"{{"sub":"user","session_id":"{session_id}","exp":1}}"#);
    let token = format!(
        "{}.{}.{}",
        base64url(br#"{"alg":"HS256"}"#),
        base64url(payload.as_bytes()),
        base64url(b"signature")
    );

    let secret = csrf_secret_from_access_token(&token)
        .expect("an access token with a session id claim has a CSRF secret");
    assert_eq!(secret, session_id);
    let embedded = derive_csrf_token(&secret);
    assert!(
        verify_csrf_token(&secret, &embedded),
        "the embedded token must verify against the secret the endpoint derives"
    );

    // A token without the claim has no rotation-stable secret: fail closed instead of falling
    // back to the access token or to an empty secret.
    let claimless = format!(
        "{}.{}.{}",
        base64url(br#"{"alg":"HS256"}"#),
        base64url(br#"{"sub":"user"}"#),
        base64url(b"signature")
    );
    assert!(csrf_secret_from_access_token(&claimless).is_none());
    assert!(csrf_secret_from_access_token("not-a-jwt").is_none());
    assert!(csrf_secret_from_access_token("").is_none());
    assert!(csrf_secret_from_access_token("head.badbase64!!!.sig").is_none());
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
