---
id: doc://docs/standards/security.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Security Audit Guide

OWASP Top 10 Security Audit Implementation for RusToK Platform

## Overview

The security module provides comprehensive protection against the OWASP Top 10 2021 security risks:

1. **Broken Access Control** - RBAC enforcement
2. **Cryptographic Failures** - HTTPS enforcement, secure headers
3. **Injection** - SQL, XSS, command injection prevention
4. **Insecure Design** - Secure defaults, defense in depth
5. **Security Misconfiguration** - Security headers
6. **Vulnerable Components** - Dependency audit integration
7. **Auth Failures** - Rate limiting, secure sessions
8. **Data Integrity** - Request validation
9. **Logging Failures** - Security audit logging
10. **SSRF** - URL validation, allowlist enforcement

## Quick Start

```rust
use rustok_core::security::{
    SecurityConfig, SecurityHeaders, run_security_audit, InputValidator,
    RateLimiter, AuditLogger, SsrfProtection
};

// Configure security
let config = SecurityConfig::default();

// Run security audit
let result = run_security_audit(&config).await;
println!("Security Score: {}/100", result.score);

if !result.passed {
    for finding in result.findings {
        println!("{:?}: {}", finding.severity, finding.description);
    }
}
```

## Security Headers

Security headers protect against common web attacks:

```rust
use rustok_core::security::{SecurityHeaders, SecurityHeadersConfig, FrameOptions};

let config = SecurityHeadersConfig {
    csp: "default-src 'self'".to_string(),
    frame_options: FrameOptions::Deny,
    hsts_max_age: 31536000,
    hsts_include_subdomains: true,
    referrer_policy: "strict-origin-when-cross-origin".to_string(),
    permissions_policy: "geolocation=(), microphone=()".to_string(),
};

let headers = SecurityHeaders::from_config(&config);
```

### Headers Included

- `Content-Security-Policy` - XSS protection
- `X-Frame-Options` - Clickjacking protection
- `X-Content-Type-Options` - MIME sniffing protection
- `Strict-Transport-Security` - HTTPS enforcement
- `Referrer-Policy` - Privacy protection
- `Permissions-Policy` - Feature policy
- `X-XSS-Protection` - Legacy XSS protection

## Rate Limiting

Protection against brute force and DDoS attacks:

```rust
use rustok_core::security::{RateLimiter, RateLimitConfig, RateLimitResult};

let config = RateLimitConfig {
    requests_per_minute: 60,
    burst_size: 10,
    login_attempts_per_minute: 5,
    api_key_requests_per_minute: 1000,
    block_duration_seconds: 300,
};

let limiter = RateLimiter::new(config);

// Check IP-based rate limit
match limiter.check_ip(client_ip).await {
    RateLimitResult::Allowed => println!("Request allowed"),
    RateLimitResult::Blocked { retry_after } => {
        println!("Rate limited, retry after: {:?}", retry_after);
    }
    RateLimitResult::Limited { remaining, .. } => {
        println!("Requests remaining: {}", remaining);
    }
}
```

## Input Validation

Protection against injection attacks:

```rust
use rustok_core::security::{InputValidator, ValidationResult, SsrfProtection};

let validator = InputValidator::new();

// Validate user input
match validator.validate(user_input) {
    ValidationResult::Valid => println!("Input is safe"),
    ValidationResult::Invalid { reason } => {
        println!("Validation failed: {}", reason);
    }
    ValidationResult::Sanitized { sanitized, .. } => {
        println!("Input sanitized: {}", sanitized);
    }
}

// Validate email
match validator.validate_email(email) {
    ValidationResult::Valid => println!("Valid email"),
    _ => println!("Invalid email"),
}

// SSRF protection
let ssrf = SsrfProtection::new()
    .allow_host("api.example.com");

match ssrf.validate_url(url) {
    ValidationResult::Valid => println!("URL is safe"),
    ValidationResult::Invalid { reason } => {
        println!("SSRF protection triggered: {}", reason);
    }
}

match ssrf.validate_redirect_chain([
    "https://api.example.com/start",
    "https://api.example.com/final",
]) {
    ValidationResult::Valid => println!("Redirect chain is safe"),
    ValidationResult::Invalid { reason } => {
        println!("Unsafe redirect chain: {}", reason);
    }
    ValidationResult::Sanitized { .. } => unreachable!(),
}
```

### Protected Against

- SQL Injection (`SELECT`, `UNION`, `DROP`, etc.)
- XSS Attacks (`<script>`, `javascript:`, event handlers)
- Command Injection (`;`, `|`, backticks)
- Path Traversal (`../`, `..\`)
- SSRF (localhost, private IPv4/IPv6, link-local/unspecified IPs, unsafe redirect chains, file://)

## Security Audit Logging

Comprehensive security event logging:

```rust
use rustok_core::security::{SecurityAudit, AuditEvent, AuditEventType, Severity};

let (audit, logger) = SecurityAudit::new(true);

// Log authentication
logger.log_auth(user_id, true, client_ip).await;

// Log authorization failure
logger.log_authz_denied(user_id, "/admin", "DELETE", client_ip).await;

// Log rate limit
logger.log_rate_limit(client_ip, "login").await;

// Log suspicious activity
logger.log_suspicious(client_ip, "Multiple failed login attempts").await;

// Custom event
let event = AuditEvent::new(AuditEventType::DataAccess)
    .with_user(user_id)
    .with_resource("user_data")
    .with_action("READ")
    .with_description("Accessed sensitive user data")
    .with_severity(Severity::Info);

logger.log(event).await;
```

### Event Types

- `Authentication` - Login/logout events
- `Authorization` - Access control events
- `DataAccess` - Sensitive data access
- `DataModification` - Data changes
- `ConfigurationChange` - System configuration
- `SecurityViolation` - Security policy violations
- `RateLimitExceeded` - Rate limiting events
- `SuspiciousActivity` - Anomalous behavior

## Security Audit

Run comprehensive security audit:

```rust
use rustok_core::security::{SecurityConfig, run_security_audit};

let config = SecurityConfig {
    headers: SecurityHeadersConfig::default(),
    rate_limit: RateLimitConfig::default(),
    enforce_https: true,
    allowed_hosts: vec!["api.example.com".to_string()],
    audit_logging: true,
};

let result = run_security_audit(&config).await;

println!("Audit Score: {}/100", result.score);
println!("Passed: {}", result.passed);

for finding in &result.findings {
    println!("{:?} - {:?}: {}", 
        finding.category,
        finding.severity,
        finding.description
    );
}
```

### Audit Checks

1. **Security Headers**
   - CSP presence and configuration
   - Frame options
   - HSTS configuration
   - HTTPS enforcement

2. **Rate Limiting**
   - Rate limit enabled
   - Reasonable limits configured
   - Block duration appropriate

3. **Input Validation**
   - SSRF protection configured
   - Allowed hosts configured

4. **Audit Logging**
   - Security logging enabled
   - Appropriate event types logged

## OWASP Coverage

| OWASP Risk | Protection | Status |
|------------|-----------|--------|
| Broken Access Control | RBAC + Audit logging | ✅ |
| Cryptographic Failures | HTTPS + HSTS | ✅ |
| Injection | Input validation | ✅ |
| Insecure Design | Secure defaults | ✅ |
| Security Misconfiguration | Security headers | ✅ |
| Vulnerable Components | Cargo audit | ✅ |
| Auth Failures | Rate limiting | ✅ |
| Data Integrity | Request validation | ✅ |
| Logging Failures | Audit logging | ✅ |
| SSRF | URL validation | ✅ |

## Best Practices

1. **Always use HTTPS** - Enable `enforce_https`
2. **Configure CSP** - Prevent XSS attacks
3. **Rate limit auth** - Prevent brute force
4. **Log security events** - Enable audit logging
5. **Validate all input** - Use InputValidator
6. **SSRF allowlist** - Restrict external URLs
7. **Regular audits** - Run security audit periodically
8. **Monitor logs** - Alert on security events

## Integration with Axum

```rust
use axum::{
    middleware::{self, Next},
    response::Response,
    http::Request,
};
use rustok_core::security::{SecurityHeaders, SecurityHeadersConfig};

async fn security_headers_middleware<B>(
    request: Request<B>,
    next: Next<B>,
) -> Response {
    let mut response = next.run(request).await;
    
    let headers = SecurityHeaders::from_config(&SecurityHeadersConfig::default());
    for (name, value) in headers.to_headers() {
        response.headers_mut().insert(
            name.parse().unwrap(),
            value.parse().unwrap(),
        );
    }
    
    response
}
```

## Testing

Run security tests:

```bash
cargo test -p rustok-core security
```

## References

- [OWASP Top 10 2021](https://owasp.org/Top10/)
- [OWASP Secure Headers](https://owasp.org/www-project-secure-headers/)
- [OWASP Testing Guide](https://owasp.org/www-project-web-security-testing-guide/)

## RusToK runtime notes

- `Forwarded`, `X-Forwarded-Host`, `X-Forwarded-For`, and `X-Forwarded-Proto` are untrusted by default. Production deployments must explicitly configure `settings.rustok.runtime.request_trust.forwarded_headers_mode=trusted_only` plus `trusted_proxy_cidrs` before any middleware may consume forwarded headers.
- Tenant, channel, OAuth secure-cookie detection, and HTTP rate limiting all share the same request-trust helper; per-feature ad hoc parsing of forwarded headers is no longer allowed.
- `settings.rustok.tenant.resolution=header` must be paired with an explicit fallback policy. Production-safe default is `fallback_mode=disabled`, which turns a missing tenant header into `400`.
- Permission-gated REST handlers must distinguish `403 Forbidden` from RBAC backend failure. Denied access is a client-visible authorization result; RBAC storage/cache errors remain `500`.
- CSP is surface-scoped: API/operator routes keep a strict policy, while embedded UI routes must use a policy compatible with shipped JS/CSS assets.
