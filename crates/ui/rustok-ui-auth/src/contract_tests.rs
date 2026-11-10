use super::*;

#[test]
fn test_auth_user_roundtrip() {
    let user = AuthUser {
        id: "usr_123".to_string(),
        email: "alice@example.com".to_string(),
        name: Some("Alice Smith".to_string()),
        role: "admin".to_string(),
        permissions: vec!["pages:update".to_string(), "pages:publish".to_string()],
    };

    let serialized = serde_json::to_string(&user).expect("serialize user");
    let deserialized: AuthUser = serde_json::from_str(&serialized).expect("deserialize user");
    assert_eq!(user, deserialized);
}

#[test]
fn test_auth_user_without_permissions_deserializes_to_empty_snapshot() {
    // Backends predating the permission snapshot must keep deserializing.
    let legacy_payload = r#"{
        "id": "usr_123",
        "email": "alice@example.com",
        "name": null,
        "role": "manager"
    }"#;
    let user: AuthUser = serde_json::from_str(legacy_payload).expect("deserialize legacy user");
    assert!(user.permissions.is_empty());
}

#[test]
fn test_auth_session_debug_redaction() {
    let session = AuthSession {
        token: "super-secret-jwt-token-value".to_string(),
        expires_at: 1800000000,
        tenant: "default".to_string(),
    };

    let debug_output = format!("{:?}", session);
    assert!(!debug_output.contains("super-secret-jwt-token-value"));
    assert!(debug_output.contains("<redacted>"));
    assert!(debug_output.contains("1800000000"));
    assert!(debug_output.contains("default"));
}

#[test]
fn test_auth_session_expiry_calculations() {
    let session = AuthSession {
        token: "token".to_string(),
        expires_at: 1000,
        tenant: "default".to_string(),
    };

    // Remaining seconds
    assert_eq!(session.secs_until_expiry(900), 100);
    assert_eq!(session.secs_until_expiry(1000), 0);
    // Negative when past expiry
    assert_eq!(session.secs_until_expiry(1100), -100);

    // Expiration check with 60s safety margin:
    // expires_at = 1000. expires_at - 60 = 940.
    assert!(!session.is_expired(939));
    assert!(session.is_expired(940));
    assert!(session.is_expired(999));
    assert!(session.is_expired(1000));
    assert!(session.is_expired(1050));
}

#[test]
fn test_auth_error_from_status() {
    // 401 with is_login = true -> InvalidCredentials
    assert_eq!(
        AuthError::from_status(401, true),
        AuthError::InvalidCredentials
    );

    // 401 with is_login = false -> Unauthorized
    assert_eq!(AuthError::from_status(401, false), AuthError::Unauthorized);

    // Other statuses -> Http(status)
    assert_eq!(AuthError::from_status(403, false), AuthError::Http(403));
    assert_eq!(AuthError::from_status(500, true), AuthError::Http(500));
    assert_eq!(AuthError::from_status(404, false), AuthError::Http(404));
}

#[test]
fn test_auth_error_display() {
    assert_eq!(AuthError::Unauthorized.to_string(), "Unauthorized");
    assert_eq!(
        AuthError::InvalidCredentials.to_string(),
        "Invalid credentials"
    );
    assert_eq!(AuthError::Network.to_string(), "Network error");
    assert_eq!(AuthError::Http(503).to_string(), "HTTP error: 503");
}
