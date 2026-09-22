//! Integration tests for Security Audit module
//!
//! Tests OWASP Top 10 protection mechanisms:
//! - Security headers
//! - Rate limiting
//! - Input validation
//! - SSRF protection
//! - Audit logging

use rustok_core::security::RateLimitResult;
use rustok_core::security::{
    FrameOptions, InputValidator, RateLimitConfig, RateLimiter, SecurityConfig, SecurityFinding,
    SecurityHeaders, SecurityHeadersConfig, Severity, SsrfProtection, ValidationResult,
    run_security_audit,
};
use std::net::IpAddr;
use std::net::Ipv4Addr;

#[tokio::test]
async fn test_security_audit_passes_with_default_config() {
    let config = SecurityConfig::default();
    let result = run_security_audit(&config).await;

    // Default config should have minor issues but score well
    assert!(
        result.score >= 60,
        "Security score too low: {}",
        result.score
    );
}

#[tokio::test]
async fn test_security_audit_fails_without_https() {
    let config = SecurityConfig {
        enforce_https: false,
        ..Default::default()
    };

    let result = run_security_audit(&config).await;

    let has_https_finding = result
        .findings
        .iter()
        .any(|f| f.description.contains("HTTPS"));

    assert!(has_https_finding, "Should report HTTPS enforcement issue");
}

#[tokio::test]
async fn test_security_headers_generation() {
    let config = SecurityHeadersConfig::default();
    let headers = SecurityHeaders::from_config(&config);

    let header_names: Vec<&str> = headers
        .to_headers()
        .iter()
        .map(|(k, _)| k.as_str())
        .collect();

    assert!(header_names.contains(&"Content-Security-Policy"));
    assert!(header_names.contains(&"X-Frame-Options"));
    assert!(header_names.contains(&"X-Content-Type-Options"));
    assert!(header_names.contains(&"Strict-Transport-Security"));
    assert!(header_names.contains(&"Referrer-Policy"));
    assert!(header_names.contains(&"Permissions-Policy"));
}

#[tokio::test]
async fn test_security_headers_with_custom_csp() {
    let config = SecurityHeadersConfig {
        csp: "default-src 'none'; script-src 'self'".to_string(),
        ..Default::default()
    };
    let headers = SecurityHeaders::from_config(&config);

    let csp_header = headers
        .to_headers()
        .iter()
        .find(|(k, _)| k == "Content-Security-Policy")
        .map(|(_, v)| v.as_str())
        .expect("CSP header should exist");

    assert_eq!(csp_header, "default-src 'none'; script-src 'self'");
}

#[test]
fn test_frame_options_variants() {
    assert_eq!(FrameOptions::Deny.to_header_value(), "DENY");
    assert_eq!(FrameOptions::SameOrigin.to_header_value(), "SAMEORIGIN");
    assert_eq!(
        FrameOptions::AllowFrom("https://example.com".to_string()).to_header_value(),
        "ALLOW-FROM https://example.com"
    );
}

#[tokio::test]
async fn test_rate_limiter_allows_within_limit() {
    let config = RateLimitConfig {
        requests_per_minute: 60,
        burst_size: 5,
        ..Default::default()
    };
    let limiter = RateLimiter::new(config);
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

    // First 5 requests should pass (burst)
    for _ in 0..5 {
        let result = limiter.check_ip(ip).await;
        assert!(
            matches!(result, RateLimitResult::Allowed),
            "Request should be allowed"
        );
    }
}

#[tokio::test]
async fn test_rate_limiter_blocks_excess() {
    let config = RateLimitConfig {
        requests_per_minute: 60,
        burst_size: 2,
        ..Default::default()
    };
    let limiter = RateLimiter::new(config);
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2));

    // Exhaust burst
    for _ in 0..2 {
        limiter.check_ip(ip).await;
    }

    // Next request should be blocked
    let result = limiter.check_ip(ip).await;
    assert!(
        matches!(result, RateLimitResult::Blocked { .. }),
        "Request should be blocked"
    );
}

#[tokio::test]
async fn test_rate_limiter_per_ip_isolation() {
    let config = RateLimitConfig {
        requests_per_minute: 60,
        burst_size: 2,
        ..Default::default()
    };
    let limiter = RateLimiter::new(config);
    let ip1 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10));
    let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 11));

    // Exhaust burst for ip1
    for _ in 0..2 {
        limiter.check_ip(ip1).await;
    }

    // ip1 should be blocked
    assert!(matches!(
        limiter.check_ip(ip1).await,
        RateLimitResult::Blocked { .. }
    ));

    // ip2 should still be allowed
    assert!(matches!(
        limiter.check_ip(ip2).await,
        RateLimitResult::Allowed
    ));
}

#[test]
fn test_input_validator_blocks_sql_injection() {
    let validator = InputValidator::new();

    let sql_patterns = vec![
        "SELECT * FROM users",
        "admin' OR '1'='1",
        "1=1; DROP TABLE users;--",
        "UNION SELECT password FROM admin",
    ];

    for pattern in sql_patterns {
        let result = validator.validate(pattern);
        assert!(
            matches!(result, ValidationResult::Invalid { .. }),
            "SQL injection should be detected: {}",
            pattern
        );
    }
}

#[test]
fn test_input_validator_blocks_xss() {
    let validator = InputValidator::new();

    let xss_patterns = vec![
        "<script>alert('xss')</script>",
        "<SCRIPT>alert(1)</SCRIPT>",
        "javascript:alert(1)",
        "<img src=x onerror=alert(1)>",
        "<iframe src='evil.com'></iframe>",
    ];

    for pattern in xss_patterns {
        let result = validator.validate(pattern);
        assert!(
            matches!(result, ValidationResult::Invalid { .. }),
            "XSS should be detected: {}",
            pattern
        );
    }
}

#[test]
fn test_input_validator_blocks_path_traversal() {
    let validator = InputValidator::new();

    let path_patterns = vec![
        "../../../etc/passwd",
        "..\\\\windows\\\\system32\\\\config\\\\sam",
        "%2e%2e/%2e%2e/%2e%2e/etc/passwd",
    ];

    for pattern in path_patterns {
        let result = validator.validate(pattern);
        assert!(
            matches!(result, ValidationResult::Invalid { .. }),
            "Path traversal should be detected: {}",
            pattern
        );
    }
}

#[test]
fn test_input_validator_allows_safe_input() {
    let validator = InputValidator::new();

    let safe_inputs = vec![
        "Hello, World!",
        "User123",
        "test@example.com",
        "This is a normal message.",
        "Product ID: ABC-123-XYZ",
    ];

    for input in safe_inputs {
        let result = validator.validate(input);
        assert!(
            matches!(result, ValidationResult::Valid),
            "Safe input should be allowed: {}",
            input
        );
    }
}

#[test]
fn test_input_validator_email_validation() {
    let validator = InputValidator::new();

    let valid_emails = vec![
        "user@example.com",
        "test.user@domain.co.uk",
        "user+tag@example.com",
        "123@numeric.com",
    ];

    for email in valid_emails {
        let result = validator.validate_email(email);
        assert!(
            matches!(result, ValidationResult::Valid),
            "Valid email should pass: {}",
            email
        );
    }

    let invalid_emails = vec![
        "not-an-email",
        "@example.com",
        "user@",
        "user@.com",
        "user space@example.com",
    ];

    for email in invalid_emails {
        let result = validator.validate_email(email);
        assert!(
            matches!(result, ValidationResult::Invalid { .. }),
            "Invalid email should be rejected: {}",
            email
        );
    }
}

#[test]
fn test_ssrf_protection_blocks_private_ips() {
    let ssrf = SsrfProtection::new();

    let private_urls = vec![
        "http://localhost/admin",
        "http://localhost./admin",
        "http://127.0.0.1/config",
        "http://0.0.0.0/config",
        "http://10.0.0.1/api",
        "http://192.168.1.1/router",
        "http://172.16.0.1/internal",
        "http://[::1]/admin",
        "http://[fd00::1]/internal",
        "http://[fe80::1]/link-local",
    ];

    for url in private_urls {
        let result = ssrf.validate_url(url);
        assert!(
            matches!(result, ValidationResult::Invalid { .. }),
            "Private IP should be blocked: {}",
            url
        );
    }
}

#[test]
fn test_ssrf_protection_blocks_dangerous_schemes() {
    let ssrf = SsrfProtection::new();

    let dangerous_urls = vec![
        "file:///etc/passwd",
        "ftp://server.com/data",
        "gopher://attacker.com:53/",
        "dict://server.com:1234/",
    ];

    for url in dangerous_urls {
        let result = ssrf.validate_url(url);
        assert!(
            matches!(result, ValidationResult::Invalid { .. }),
            "Dangerous scheme should be blocked: {}",
            url
        );
    }
}

#[test]
fn test_ssrf_protection_allows_safe_urls() {
    let ssrf = SsrfProtection::new()
        .allow_host("API.EXAMPLE.COM")
        .allow_host("trusted-partner.com");

    let safe_urls = vec![
        "http://api.example.com/data",
        "https://api.example.com./trailing-dot",
        "https://api.example.com/secure",
        "https://trusted-partner.com/webhook",
    ];

    for url in safe_urls {
        let result = ssrf.validate_url(url);
        assert!(
            matches!(result, ValidationResult::Valid),
            "Safe URL should be allowed: {}",
            url
        );
    }
}

#[test]
fn test_ssrf_protection_blocks_non_allowlisted() {
    let ssrf = SsrfProtection::new().allow_host("api.example.com");

    let blocked_urls = vec![
        "http://evil.com/steal",
        "https://attacker.com/exfil",
        "http://unknown-host.com/data",
    ];

    for url in blocked_urls {
        let result = ssrf.validate_url(url);
        assert!(
            matches!(result, ValidationResult::Invalid { .. }),
            "Non-allowlisted URL should be blocked: {}",
            url
        );
    }
}

#[test]
fn test_ssrf_redirect_chain_requires_all_hops_to_be_safe() {
    let ssrf = SsrfProtection::new().allow_host("api.example.com");

    let result = ssrf.validate_redirect_chain([
        "https://api.example.com/start",
        "https://api.example.com/final",
    ]);

    assert!(matches!(result, ValidationResult::Valid));
}

#[test]
fn test_ssrf_redirect_chain_blocks_unsafe_redirect_target() {
    let ssrf = SsrfProtection::new().allow_host("api.example.com");

    let result = ssrf
        .validate_redirect_chain(["https://api.example.com/start", "http://localhost/internal"]);

    assert!(matches!(result, ValidationResult::Invalid { .. }));
}

#[test]
fn test_ssrf_redirect_chain_rejects_empty_chain() {
    let ssrf = SsrfProtection::new().allow_host("api.example.com");

    let result = ssrf.validate_redirect_chain(Vec::<&str>::new());

    assert!(matches!(result, ValidationResult::Invalid { .. }));
}

#[test]
fn test_input_validator_sanitizes_html() {
    let validator = InputValidator::new();

    let input = "<script>alert('xss')</script>";
    let sanitized = validator.sanitize_html(input);

    assert_eq!(
        sanitized,
        "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;&#x2f;script&gt;"
    );
}

#[test]
fn test_input_validator_length_limit() {
    let validator = InputValidator::new().with_max_length(10);

    let short = "short";
    let long = "this is a very long string";

    assert!(matches!(validator.validate(short), ValidationResult::Valid));
    assert!(matches!(
        validator.validate(long),
        ValidationResult::Invalid { .. }
    ));
}

#[tokio::test]
async fn test_security_audit_detects_disabled_logging() {
    let config = SecurityConfig {
        audit_logging: false,
        ..Default::default()
    };

    let result = run_security_audit(&config).await;

    let has_logging_finding = result
        .findings
        .iter()
        .any(|f| f.description.contains("audit logging"));

    assert!(has_logging_finding, "Should report disabled audit logging");
}

#[test]
fn test_security_score_calculation() {
    use rustok_core::security::SecurityCategory;

    // Empty findings = perfect score
    let empty: Vec<SecurityFinding> = vec![];
    assert_eq!(rustok_core::security::calculate_security_score(&empty), 100);

    // Critical finding = -25
    let critical = vec![SecurityFinding {
        category: SecurityCategory::Injection,
        severity: Severity::Critical,
        description: "Test".to_string(),
        remediation: "Fix".to_string(),
    }];
    assert_eq!(
        rustok_core::security::calculate_security_score(&critical),
        75
    );

    // High finding = -15
    let high = vec![SecurityFinding {
        category: SecurityCategory::Injection,
        severity: Severity::High,
        description: "Test".to_string(),
        remediation: "Fix".to_string(),
    }];
    assert_eq!(rustok_core::security::calculate_security_score(&high), 85);

    // Combined
    let combined = vec![
        SecurityFinding {
            category: SecurityCategory::Injection,
            severity: Severity::Critical,
            description: "Test".to_string(),
            remediation: "Fix".to_string(),
        },
        SecurityFinding {
            category: SecurityCategory::Injection,
            severity: Severity::High,
            description: "Test".to_string(),
            remediation: "Fix".to_string(),
        },
    ];
    assert_eq!(
        rustok_core::security::calculate_security_score(&combined),
        60
    );
}
