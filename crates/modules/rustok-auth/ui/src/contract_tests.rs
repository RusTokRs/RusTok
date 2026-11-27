use crate::core::*;

#[test]
fn test_package_metadata_contract() {
    assert_eq!(env!("CARGO_PKG_NAME"), "rustok-auth-ui");
    assert!(!env!("CARGO_PKG_VERSION").is_empty());
    assert!(!env!("CARGO_PKG_DESCRIPTION").is_empty());
}

#[test]
fn test_storage_and_cookie_constants_contract() {
    assert_eq!(ADMIN_TOKEN_KEY, "rustok-admin-token");
    assert_eq!(ADMIN_TENANT_KEY, "rustok-admin-tenant");
    assert_eq!(ADMIN_USER_KEY, "rustok-admin-user");
    assert_eq!(ADMIN_SESSION_KEY, "rustok-admin-session");
    assert_eq!(ADMIN_REFRESH_COOKIE, "rustok-admin-refresh-v1");
}

#[test]
fn test_refresh_cookie_helpers_contract() {
    let tenant = "tenant-acme";
    let cookie_val = format!("{tenant}.{}.{}", "a1".repeat(32), "b2".repeat(32));
    let credential = RefreshCredential::parse(&cookie_val).unwrap();
    assert_eq!(credential.tenant(), tenant);
    assert!(RefreshCredential::parse("invalid").is_err());
    assert!(RefreshCredential::parse("tenant.token").is_err());

    let raw_header = format!("other=1; {ADMIN_REFRESH_COOKIE}={cookie_val}; session=abc");
    assert_eq!(
        inspect_refresh_cookie_headers([raw_header.as_str()]),
        Ok(Some(credential.clone()))
    );
    assert_eq!(inspect_refresh_cookie_headers([]), Ok(None));
    assert!(inspect_refresh_cookie_headers([raw_header.as_str(), raw_header.as_str()]).is_err());

    let set_cookie_insecure = refresh_cookie_header(&credential, false);
    assert!(set_cookie_insecure.contains("HttpOnly"));
    assert!(set_cookie_insecure.contains("SameSite=Lax"));
    assert!(!set_cookie_insecure.contains("Secure"));
    let set_cookie_secure = refresh_cookie_header(&credential, true);
    assert!(set_cookie_secure.contains("; Secure"));

    let clear_cookie = clear_refresh_cookie_header(true);
    assert!(clear_cookie.contains("Max-Age=0"));
    assert!(clear_cookie.contains("; Secure"));
}

#[test]
fn test_session_id_from_access_token_contract() {
    // Construct minimal base64url header and payload
    // Header: {"alg":"HS256","typ":"JWT"} -> eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9
    // Payload: {"sub":"user-1","session_id":"uuid-session-12345"}
    let payload_json = r#"{"sub":"user-1","session_id":"uuid-session-12345"}"#;
    let mut encoded_payload = String::new();
    // Simple base64url encode for test
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let bytes = payload_json.as_bytes();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };
        encoded_payload.push(TABLE[(b0 >> 2) as usize] as char);
        encoded_payload.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            encoded_payload.push(TABLE[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        }
        if chunk.len() > 2 {
            encoded_payload.push(TABLE[(b2 & 0x3f) as usize] as char);
        }
    }

    let fake_jwt = format!("eyJhbGciOiJIUzI1NiJ9.{}.signature", encoded_payload);
    let extracted_id = session_id_from_access_token(&fake_jwt);
    assert_eq!(extracted_id.as_deref(), Some("uuid-session-12345"));

    // Invalid tokens return None
    assert!(session_id_from_access_token("not-a-jwt").is_none());
    assert!(session_id_from_access_token("head.badbase64!!!.sig").is_none());
}
