use rustok_core::security::{
    run_security_audit, FrameOptions, InputValidator, RateLimitConfig, RateLimitResult,
    RateLimiter, SecurityConfig, SecurityHeaders, SecurityHeadersConfig, SsrfProtection,
    ValidationResult,
};
use rustok_core::utils::{html_escape, is_valid_email, is_valid_url, is_valid_uuid, slugify};
use std::net::{IpAddr, Ipv4Addr};

// ---------------------------------------------------------------------------
// Security contracts
// ---------------------------------------------------------------------------

#[tokio::test]
async fn security_audit_with_default_config_scores_above_threshold() {
    let config = SecurityConfig::default();
    let result = run_security_audit(&config).await;
    assert!(
        result.score >= 60,
        "default security config should score at least 60, got {}",
        result.score
    );
}

#[test]
fn security_headers_from_default_config_includes_x_frame_options() {
    let headers = SecurityHeaders::from_config(&SecurityHeadersConfig::default());
    let tuples = headers.to_headers();
    assert!(tuples.iter().any(|(k, _)| k == "X-Frame-Options"));
}

#[test]
fn security_headers_can_set_deny_frame_options() {
    let config = SecurityHeadersConfig {
        frame_options: FrameOptions::Deny,
        ..Default::default()
    };
    let headers = SecurityHeaders::from_config(&config);
    let tuples = headers.to_headers();
    let frame_opt = tuples.iter().find(|(k, _)| k == "X-Frame-Options");
    assert_eq!(frame_opt.map(|(_, v)| v.as_str()), Some("DENY"));
}

#[tokio::test]
async fn rate_limiter_allows_first_request() {
    let config = RateLimitConfig::default();
    let limiter = RateLimiter::new(config);
    let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

    let result = limiter.check_ip(ip).await;
    assert!(matches!(result, RateLimitResult::Allowed));
}

#[test]
fn input_validator_validates_email() {
    let validator = InputValidator::new();
    assert!(matches!(
        validator.validate_email("test@example.com"),
        ValidationResult::Valid
    ));
    assert!(matches!(
        validator.validate_email("invalid"),
        ValidationResult::Invalid(_)
    ));
}

#[test]
fn ssrf_protection_blocks_private_ips_by_default() {
    let protection = SsrfProtection::default();
    assert!(matches!(
        protection.validate_url("http://192.168.1.1/internal"),
        ValidationResult::Invalid { .. }
    ));
    assert!(matches!(
        protection.validate_url("http://example.com"),
        ValidationResult::Valid
    ));
}

// ---------------------------------------------------------------------------
// Validation helpers (utils)
// ---------------------------------------------------------------------------

#[test]
fn is_valid_email_accepts_common_formats() {
    assert!(is_valid_email("user@example.com"));
    assert!(is_valid_email("first.last@domain.co.uk"));
}

#[test]
fn is_valid_email_rejects_invalid() {
    assert!(!is_valid_email("not-an-email"));
    assert!(!is_valid_email("@example.com"));
    assert!(!is_valid_email("user@"));
    assert!(!is_valid_email(""));
}

#[test]
fn is_valid_uuid_accepts_standard_format() {
    assert!(is_valid_uuid("550e8400-e29b-41d4-a716-446655440000"));
}

#[test]
fn is_valid_uuid_rejects_invalid() {
    assert!(!is_valid_uuid("not-a-uuid"));
    assert!(!is_valid_uuid(""));
}

#[test]
fn is_valid_url_accepts_http_https() {
    assert!(is_valid_url("https://example.com"));
    assert!(is_valid_url("http://localhost:3000"));
}

#[test]
fn is_valid_url_rejects_invalid() {
    assert!(!is_valid_url("not a url"));
    assert!(!is_valid_url(""));
}

#[test]
fn html_escape_sanitizes_special_chars() {
    assert_eq!(
        html_escape("<script>alert('xss')</script>"),
        "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;"
    );
}

#[test]
fn slugify_produces_expected_output() {
    assert_eq!(slugify("Hello World"), "hello-world");
    assert_eq!(slugify("UPPER_CASE"), "upper-case");
    assert_eq!(slugify("multiple---dashes"), "multiple-dashes");
}

// ---------------------------------------------------------------------------
// Compatibility exports (prelude smoke test)
// ---------------------------------------------------------------------------

#[test]
fn prelude_re_exports_are_accessible() {
    // Smoke test that prelude items compile and are usable.
    use rustok_core::prelude::*;

    let _id = generate_id();
    let _permission = Permission::new(Resource::Products, Action::Read);
    let _role = UserRole::Admin;
}
