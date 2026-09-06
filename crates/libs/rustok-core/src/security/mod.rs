//! Security Module - OWASP Top 10 Protection
//!
//! Provides security controls for the RusToK platform following OWASP Top 10 2021:
//! 1. Broken Access Control - Role-based access control enforcement
//! 2. Cryptographic Failures - Secure headers, HTTPS enforcement
//! 3. Injection - Input validation and sanitization
//! 4. Insecure Design - Secure defaults, defense in depth
//! 5. Security Misconfiguration - Security headers
//! 6. Vulnerable Components - Dependency audit integration
//! 7. Auth Failures - Rate limiting, secure session handling
//! 8. Data Integrity - Request signing, tamper detection
//! 9. Logging Failures - Security audit logging
//! 10. SSRF - URL validation, allowlist enforcement

pub mod audit;
pub mod headers;
pub mod rate_limit;
pub mod validation;

pub use audit::{AuditEvent, AuditEventType, AuditLogger, SecurityAudit, SiemConfig};
pub use headers::{FrameOptions, SecurityHeaders, SecurityHeadersConfig};
pub use rate_limit::{RateLimitConfig, RateLimitResult, RateLimiter};
pub use validation::{InputValidator, SsrfProtection, ValidationResult};

use serde::{Deserialize, Serialize};

/// Security configuration for the application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Security headers configuration
    pub headers: SecurityHeadersConfig,
    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,
    /// Whether to enforce HTTPS
    pub enforce_https: bool,
    /// Allowed hosts for SSRF protection
    pub allowed_hosts: Vec<String>,
    /// Audit log configuration
    pub audit_logging: bool,
    /// External SIEM endpoint for audit event forwarding
    #[serde(default)]
    pub siem: SiemConfig,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            headers: SecurityHeadersConfig::default(),
            rate_limit: RateLimitConfig::default(),
            enforce_https: true,
            allowed_hosts: vec![],
            audit_logging: true,
            siem: SiemConfig::default(),
        }
    }
}

/// Security audit result
#[derive(Debug, Clone)]
pub struct SecurityAuditResult {
    pub passed: bool,
    pub findings: Vec<SecurityFinding>,
    pub score: u8, // 0-100
}

/// Individual security finding
#[derive(Debug, Clone)]
pub struct SecurityFinding {
    pub category: SecurityCategory,
    pub severity: Severity,
    pub description: String,
    pub remediation: String,
}

/// OWASP Top 10 categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityCategory {
    BrokenAccessControl,
    CryptographicFailures,
    Injection,
    InsecureDesign,
    SecurityMisconfiguration,
    VulnerableComponents,
    AuthFailures,
    DataIntegrity,
    LoggingFailures,
    Ssrf,
    Other,
}

/// Severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Low,
    Warning,
    Medium,
    High,
    Critical,
}

/// Run full OWASP Top 10 security audit
pub async fn run_security_audit(config: &SecurityConfig) -> SecurityAuditResult {
    let mut findings = Vec::new();

    // Check security headers
    findings.extend(headers::audit_headers(config).await);

    // Check rate limiting
    findings.extend(rate_limit::audit_rate_limiting(config).await);

    // Check input validation
    findings.extend(validation::audit_validation(config).await);

    // Check audit logging
    findings.extend(audit::audit_logging(config).await);

    let score = calculate_security_score(&findings);
    let passed = score >= 80;

    SecurityAuditResult {
        passed,
        findings,
        score,
    }
}

pub fn calculate_security_score(findings: &[SecurityFinding]) -> u8 {
    let base_score = 100i16;
    let deductions: i16 = findings
        .iter()
        .map(|f| match f.severity {
            Severity::Critical => 25,
            Severity::High => 15,
            Severity::Medium => 8,
            Severity::Warning => 5,
            Severity::Low => 3,
            Severity::Info => 0,
        })
        .sum();

    (base_score - deductions).clamp(0, 100) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_score_calculation() {
        let findings = vec![
            SecurityFinding {
                category: SecurityCategory::Injection,
                severity: Severity::High,
                description: "Test".to_string(),
                remediation: "Fix".to_string(),
            },
            SecurityFinding {
                category: SecurityCategory::Injection,
                severity: Severity::Medium,
                description: "Test".to_string(),
                remediation: "Fix".to_string(),
            },
        ];

        assert_eq!(calculate_security_score(&findings), 77);
    }

    #[test]
    fn test_security_score_perfect() {
        let findings: Vec<SecurityFinding> = vec![];
        assert_eq!(calculate_security_score(&findings), 100);
    }
}
