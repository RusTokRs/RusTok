//! Security Headers - OWASP Secure Headers Project
//!
//! Implements security headers to protect against common attacks:
//! - X-Frame-Options: Clickjacking protection
//! - X-Content-Type-Options: MIME sniffing protection
//! - X-XSS-Protection: Legacy XSS protection (deprecated but useful)
//! - Strict-Transport-Security: HTTPS enforcement
//! - Content-Security-Policy: XSS and data injection protection
//! - Referrer-Policy: Privacy protection
//! - Permissions-Policy: Feature policy

use serde::{Deserialize, Serialize};

use super::{SecurityCategory, SecurityFinding, Severity};
use crate::security::SecurityConfig;

/// Security headers configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityHeadersConfig {
    /// Content-Security-Policy directive
    pub csp: String,
    /// Whether to include X-Frame-Options
    pub frame_options: FrameOptions,
    /// Strict-Transport-Security max age in seconds
    pub hsts_max_age: u64,
    /// Whether to include subdomains in HSTS
    pub hsts_include_subdomains: bool,
    /// Referrer-Policy value
    pub referrer_policy: String,
    /// Permissions-Policy value
    pub permissions_policy: String,
}

impl Default for SecurityHeadersConfig {
    fn default() -> Self {
        Self {
            csp: "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; \
                  img-src 'self' data: https:; font-src 'self'; connect-src 'self'; \
                  media-src 'self'; object-src 'none'; frame-ancestors 'self'; \
                  base-uri 'self'; form-action 'self';"
                .to_string(),
            frame_options: FrameOptions::Deny,
            hsts_max_age: 31536000, // 1 year
            hsts_include_subdomains: true,
            referrer_policy: "strict-origin-when-cross-origin".to_string(),
            permissions_policy: "geolocation=(), microphone=(), camera=()".to_string(),
        }
    }
}

/// X-Frame-Options variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FrameOptions {
    Deny,
    SameOrigin,
    AllowFrom(String),
}

impl FrameOptions {
    pub fn to_header_value(&self) -> String {
        match self {
            FrameOptions::Deny => "DENY".to_string(),
            FrameOptions::SameOrigin => "SAMEORIGIN".to_string(),
            FrameOptions::AllowFrom(origin) => format!("ALLOW-FROM {}", origin),
        }
    }
}

/// Security headers container
#[derive(Debug, Clone, Default)]
pub struct SecurityHeaders {
    headers: Vec<(String, String)>,
}

impl SecurityHeaders {
    /// Create security headers from configuration
    pub fn from_config(config: &SecurityHeadersConfig) -> Self {
        let mut headers = vec![
            // Content-Security-Policy
            ("Content-Security-Policy".to_string(), config.csp.clone()),
            // X-Frame-Options
            (
                "X-Frame-Options".to_string(),
                config.frame_options.to_header_value(),
            ),
            // X-Content-Type-Options
            ("X-Content-Type-Options".to_string(), "nosniff".to_string()),
            // X-XSS-Protection (legacy but still useful)
            ("X-XSS-Protection".to_string(), "1; mode=block".to_string()),
        ];

        // Strict-Transport-Security
        let hsts_value = if config.hsts_include_subdomains {
            format!("max-age={}; includeSubDomains", config.hsts_max_age)
        } else {
            format!("max-age={}", config.hsts_max_age)
        };
        headers.push(("Strict-Transport-Security".to_string(), hsts_value));

        // Referrer-Policy
        headers.push((
            "Referrer-Policy".to_string(),
            config.referrer_policy.clone(),
        ));

        // Permissions-Policy
        headers.push((
            "Permissions-Policy".to_string(),
            config.permissions_policy.clone(),
        ));

        // X-Download-Options (IE)
        headers.push(("X-Download-Options".to_string(), "noopen".to_string()));

        // X-Permitted-Cross-Domain-Policies
        headers.push((
            "X-Permitted-Cross-Domain-Policies".to_string(),
            "none".to_string(),
        ));

        Self { headers }
    }

    /// Get all headers as tuples
    pub fn to_headers(&self) -> &[(String, String)] {
        &self.headers
    }

    /// Add a custom header
    pub fn add_header(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.headers.push((name.into(), value.into()))
    }
}

/// Audit security headers configuration
pub async fn audit_headers(config: &SecurityConfig) -> Vec<SecurityFinding> {
    let mut findings = Vec::new();

    // Check CSP
    if config.headers.csp.is_empty() {
        findings.push(SecurityFinding {
            category: SecurityCategory::SecurityMisconfiguration,
            severity: Severity::High,
            description: "Content-Security-Policy header is not set".to_string(),
            remediation: "Configure CSP to prevent XSS and data injection attacks".to_string(),
        });
    } else if !config.headers.csp.contains("default-src") {
        findings.push(SecurityFinding {
            category: SecurityCategory::SecurityMisconfiguration,
            severity: Severity::Medium,
            description: "CSP is missing default-src directive".to_string(),
            remediation: "Add default-src directive to CSP for baseline protection".to_string(),
        });
    }

    // Check frame options
    if matches!(config.headers.frame_options, FrameOptions::AllowFrom(_)) {
        findings.push(SecurityFinding {
            category: SecurityCategory::SecurityMisconfiguration,
            severity: Severity::Low,
            description: "X-Frame-Options allows framing from specific origin".to_string(),
            remediation: "Consider using DENY or SAMEORIGIN for stronger clickjacking protection"
                .to_string(),
        });
    }

    // Check HSTS
    if config.headers.hsts_max_age < 86400 {
        findings.push(SecurityFinding {
            category: SecurityCategory::CryptographicFailures,
            severity: Severity::Medium,
            description: "HSTS max-age is less than 1 day".to_string(),
            remediation: "Increase HSTS max-age to at least 1 year (31536000 seconds)".to_string(),
        });
    }

    // Check HTTPS enforcement
    if !config.enforce_https {
        findings.push(SecurityFinding {
            category: SecurityCategory::CryptographicFailures,
            severity: Severity::Critical,
            description: "HTTPS enforcement is disabled".to_string(),
            remediation: "Enable enforce_https to prevent man-in-the-middle attacks".to_string(),
        });
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_headers_generation() {
        let config = SecurityHeadersConfig::default();
        let headers = SecurityHeaders::from_config(&config);

        let header_names: Vec<&str> = headers
            .to_headers()
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();

        assert!(header_names.contains(&"Content-Security-Policy"));
        assert!(header_names.contains(&"X-Frame-Options"));
        assert!(header_names.contains(&"Strict-Transport-Security"));
    }

    #[test]
    fn test_frame_options_header_value() {
        assert_eq!(FrameOptions::Deny.to_header_value(), "DENY");
        assert_eq!(FrameOptions::SameOrigin.to_header_value(), "SAMEORIGIN");
        assert_eq!(
            FrameOptions::AllowFrom("https://example.com".to_string()).to_header_value(),
            "ALLOW-FROM https://example.com"
        );
    }
}
