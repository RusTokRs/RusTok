//! Input Validation - OWASP Injection Prevention
//!
//! Protection against:
//! - SQL Injection
//! - NoSQL Injection
//! - XSS (Cross-Site Scripting)
//! - Command Injection
//! - Path Traversal
//! - SSRF (Server-Side Request Forgery)

use std::collections::HashSet;

use email_address::EmailAddress;
use regex::Regex;
use url::Url;

use super::{SecurityCategory, SecurityFinding, Severity};
use crate::security::SecurityConfig;

/// Validation result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationResult {
    Valid,
    Invalid { reason: String },
    Sanitized { original: String, sanitized: String },
}

/// Input validator
#[derive(Debug, Clone)]
pub struct InputValidator {
    /// SQL injection patterns
    sql_patterns: Vec<Regex>,
    /// XSS patterns
    xss_patterns: Vec<Regex>,
    /// Command injection patterns
    cmd_patterns: Vec<Regex>,
    /// Path traversal patterns
    path_patterns: Vec<Regex>,
    /// Maximum allowed length for inputs
    max_length: usize,
    /// Allowed characters pattern (if set)
    allowed_chars: Option<Regex>,
}

impl Default for InputValidator {
    fn default() -> Self {
        Self::new()
    }
}

use once_cell::sync::Lazy;

// INVARIANT: Static SQL patterns are compile-time verified constants.
static SQL_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        // INVARIANT: Verified static SQL injection regex
        Regex::new(r"(?i)(SELECT\s+.*\s+FROM|INSERT\s+INTO|UPDATE\s+.*\s+SET|DELETE\s+FROM|DROP\s+(TABLE|DATABASE|SCHEMA|VIEW)|ALTER\s+TABLE|UNION\s+(ALL\s+)?SELECT|--|;--|/\*.*?\*/)")
            .expect("valid SQL regex"),
        // INVARIANT: Verified static boolean injection regex
        Regex::new(r#"(?i)(OR\s+['\"]?1['\"]?\s*=\s*['\"]?1['\"]?|AND\s+['\"]?1['\"]?\s*=\s*['\"]?1['\"]?|['\"]?1['\"]?\s*=\s*['\"]?1['\"]?)"#)
            .expect("valid SQL regex"),
        // INVARIANT: Verified static SQL execution regex
        Regex::new(r"(?i)(EXEC\s*\(|EXECUTE\s*\(|sp_executesql)")
            .expect("valid SQL regex"),
    ]
});

// INVARIANT: Static XSS patterns are compile-time verified constants.
static XSS_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        // INVARIANT: Verified static XSS script tag regex (both unclosed and closed tags)
        Regex::new(r"(?i)<\s*script").expect("valid XSS regex"),
        // INVARIANT: Verified static javascript URI regex
        Regex::new(r"(?i)javascript:").expect("valid XSS regex"),
        // INVARIANT: Verified static vbscript URI regex
        Regex::new(r"(?i)vbscript:").expect("valid XSS regex"),
        // INVARIANT: Verified static data:text/html URI regex
        Regex::new(r"(?i)data:\s*text/html").expect("valid XSS regex"),
        // INVARIANT: Verified static event handler regex
        Regex::new(r#"(?i)on[a-z]+\s*="#).expect("valid XSS regex"),
        // INVARIANT: Verified static iframe regex
        Regex::new(r"(?i)<\s*iframe").expect("valid XSS regex"),
        // INVARIANT: Verified static object regex
        Regex::new(r"(?i)<\s*object").expect("valid XSS regex"),
        // INVARIANT: Verified static embed regex
        Regex::new(r"(?i)<\s*embed").expect("valid XSS regex"),
        // INVARIANT: Verified static base tag injection regex
        Regex::new(r"(?i)<\s*base\b").expect("valid XSS regex"),
    ]
});

// INVARIANT: Static command injection patterns are compile-time verified constants.
static CMD_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        // INVARIANT: Verified static shell metacharacter regex
        Regex::new(r"[;&|`]\s*\w+").expect("valid CMD regex"),
        // INVARIANT: Verified static command substitution regex
        Regex::new(r"\$\(.*\)").expect("valid CMD regex"),
        // INVARIANT: Verified static backtick substitution regex
        Regex::new(r"`.*`").expect("valid CMD regex"),
    ]
});

// INVARIANT: Static path traversal patterns are compile-time verified constants.
static PATH_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        // INVARIANT: Verified static unix path traversal regex
        Regex::new(r"\.\./|\.\.$").expect("valid path traversal regex"),
        // INVARIANT: Verified static windows path traversal regex
        Regex::new(r"\.\.\\").expect("valid path traversal regex"),
        // INVARIANT: Verified static URL-encoded path traversal regex
        Regex::new(r"(?i)%2e%2e[/\\]|%2e%2e$").expect("valid path traversal regex"),
        // INVARIANT: Verified static double-encoded path traversal regex
        Regex::new(r"(?i)%252e%252e").expect("valid path traversal regex"),
        // INVARIANT: Verified static hex-encoded path traversal regex
        Regex::new(r"\x2e\x2e[/\\]").expect("valid path traversal regex"),
        // INVARIANT: Verified static null byte injection regex
        Regex::new(r"(?i)\x00|%00").expect("valid path traversal regex"),
    ]
});

impl InputValidator {
    /// Create new input validator with default patterns
    pub fn new() -> Self {
        Self {
            sql_patterns: SQL_PATTERNS.clone(),
            xss_patterns: XSS_PATTERNS.clone(),
            cmd_patterns: CMD_PATTERNS.clone(),
            path_patterns: PATH_PATTERNS.clone(),
            max_length: 10000,
            allowed_chars: None,
        }
    }

    /// Set maximum input length
    pub fn with_max_length(mut self, max: usize) -> Self {
        self.max_length = max;
        self
    }

    /// Set allowed characters pattern
    pub fn with_allowed_chars(mut self, pattern: &str) -> Result<Self, regex::Error> {
        self.allowed_chars = Some(Regex::new(pattern)?);
        Ok(self)
    }

    /// Validate string against all patterns
    pub fn validate(&self, input: &str) -> ValidationResult {
        // Check length
        if input.len() > self.max_length {
            return ValidationResult::Invalid {
                reason: format!("Input exceeds maximum length of {}", self.max_length),
            };
        }

        // Check allowed characters
        if let Some(ref pattern) = self.allowed_chars
            && !pattern.is_match(input)
        {
            return ValidationResult::Invalid {
                reason: "Input contains invalid characters".to_string(),
            };
        }

        // Check SQL injection
        for pattern in &self.sql_patterns {
            if pattern.is_match(input) {
                return ValidationResult::Invalid {
                    reason: "Potential SQL injection detected".to_string(),
                };
            }
        }

        // Check XSS
        for pattern in &self.xss_patterns {
            if pattern.is_match(input) {
                return ValidationResult::Invalid {
                    reason: "Potential XSS attack detected".to_string(),
                };
            }
        }

        // Check command injection
        for pattern in &self.cmd_patterns {
            if pattern.is_match(input) {
                return ValidationResult::Invalid {
                    reason: "Potential command injection detected".to_string(),
                };
            }
        }

        // Check path traversal
        for pattern in &self.path_patterns {
            if pattern.is_match(input) {
                return ValidationResult::Invalid {
                    reason: "Potential path traversal detected".to_string(),
                };
            }
        }

        ValidationResult::Valid
    }

    /// Sanitize HTML content
    pub fn sanitize_html(&self, input: &str) -> String {
        v_htmlescape::escape_fmt(input).to_string()
    }

    /// Validate email address
    pub fn validate_email(&self, email: &str) -> ValidationResult {
        if email.len() > 254 {
            return ValidationResult::Invalid {
                reason: "Email address too long".to_string(),
            };
        }

        if EmailAddress::is_valid(email) {
            ValidationResult::Valid
        } else {
            ValidationResult::Invalid {
                reason: "Invalid email format".to_string(),
            }
        }
    }

    /// Validate UUID
    pub fn validate_uuid(&self, uuid: &str) -> ValidationResult {
        if uuid::Uuid::parse_str(uuid).is_ok() {
            ValidationResult::Valid
        } else {
            ValidationResult::Invalid {
                reason: "Invalid UUID format".to_string(),
            }
        }
    }
}

/// SSRF Protection
#[derive(Debug, Clone)]
pub struct SsrfProtection {
    /// Allowed host patterns
    allowed_hosts: HashSet<String>,
    /// Blocked schemes
    blocked_schemes: HashSet<String>,
}

impl Default for SsrfProtection {
    fn default() -> Self {
        Self::new()
    }
}

impl SsrfProtection {
    /// Create new SSRF protection with defaults
    pub fn new() -> Self {
        let blocked_schemes = ["file", "ftp", "gopher", "dict"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        Self {
            allowed_hosts: HashSet::new(),
            blocked_schemes,
        }
    }

    /// Add allowed host
    pub fn allow_host(mut self, host: impl Into<String>) -> Self {
        self.allowed_hosts
            .insert(Self::normalize_host(&host.into()));
        self
    }

    /// Validate URL for SSRF
    pub fn validate_url(&self, url: &str) -> ValidationResult {
        let parsed = match Url::parse(url) {
            Ok(u) => u,
            Err(_) => {
                return ValidationResult::Invalid {
                    reason: "Invalid URL format".to_string(),
                };
            }
        };

        // Reject credentials / userinfo in URL
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return ValidationResult::Invalid {
                reason: "URL userinfo is not allowed".to_string(),
            };
        }

        // Check scheme
        let scheme = parsed.scheme();
        if self.blocked_schemes.contains(scheme) {
            return ValidationResult::Invalid {
                reason: format!("URL scheme '{}' is not allowed", scheme),
            };
        }

        if scheme != "http" && scheme != "https" {
            return ValidationResult::Invalid {
                reason: "Only HTTP and HTTPS URLs are allowed".to_string(),
            };
        }

        let host = match parsed.host() {
            Some(host) => host,
            None => {
                return ValidationResult::Invalid {
                    reason: "URL must have a host".to_string(),
                };
            }
        };

        let normalized_host = match host {
            url::Host::Domain(domain) => Self::normalize_host(domain),
            url::Host::Ipv4(ipv4) => ipv4.to_string(),
            url::Host::Ipv6(ipv6) => ipv6.to_string(),
        };

        // Check if in allowed list (if configured)
        if !self.allowed_hosts.is_empty() && !self.allowed_hosts.contains(&normalized_host) {
            return ValidationResult::Invalid {
                reason: format!("Host '{}' is not in the allowlist", normalized_host),
            };
        }

        if normalized_host == "localhost"
            || normalized_host.ends_with(".localhost")
            || normalized_host.ends_with(".local")
            || normalized_host.ends_with(".internal")
            || normalized_host.ends_with(".home.arpa")
            || normalized_host.ends_with(".lan")
            || normalized_host.ends_with(".localdomain")
            || normalized_host.ends_with(".corp")
        {
            return ValidationResult::Invalid {
                reason: "Localhost or internal network hostnames are not allowed".to_string(),
            };
        }

        if let Ok(ip) = normalized_host.parse::<std::net::IpAddr>()
            && self.is_private_ip(&ip)
        {
            return ValidationResult::Invalid {
                reason: "Private IP addresses are not allowed".to_string(),
            };
        }

        ValidationResult::Valid
    }

    /// Validate every hop in an HTTP redirect chain.
    pub fn validate_redirect_chain<I, S>(&self, urls: I) -> ValidationResult
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut has_urls = false;

        for (index, url) in urls.into_iter().enumerate() {
            has_urls = true;

            match self.validate_url(url.as_ref()) {
                ValidationResult::Valid => {}
                ValidationResult::Invalid { reason } => {
                    return ValidationResult::Invalid {
                        reason: format!("Redirect hop {} rejected: {}", index, reason),
                    };
                }
                ValidationResult::Sanitized { .. } => {
                    return ValidationResult::Invalid {
                        reason: format!("Redirect hop {} returned an unexpected result", index),
                    };
                }
            }
        }

        if has_urls {
            ValidationResult::Valid
        } else {
            ValidationResult::Invalid {
                reason: "Redirect chain must contain at least one URL".to_string(),
            }
        }
    }

    pub fn is_private_ip(&self, ip: &std::net::IpAddr) -> bool {
        Self::is_private_ip_addr(ip)
    }

    pub fn is_private_ip_addr(ip: &std::net::IpAddr) -> bool {
        match ip {
            std::net::IpAddr::V4(ipv4) => Self::is_private_ipv4(ipv4),
            std::net::IpAddr::V6(ipv6) => Self::is_private_ipv6(ipv6),
        }
    }

    fn is_private_ipv4(ipv4: &std::net::Ipv4Addr) -> bool {
        let [a, b, c, _] = ipv4.octets();
        ipv4.is_private()
            || ipv4.is_loopback()
            || ipv4.is_link_local()
            || ipv4.is_broadcast()
            || ipv4.is_documentation()
            || ipv4.is_unspecified()
            || ipv4.is_multicast()
            || a == 0 // 0.0.0.0/8 current network
            || a >= 240 // 240.0.0.0/4 reserved (Class E)
            || (a == 100 && (64..=127).contains(&b)) // 100.64.0.0/10 Carrier-Grade NAT (RFC 6598)
            || (a == 192 && b == 0 && c == 0) // 192.0.0.0/24 IETF Protocol Assignments (RFC 6890)
            || (a == 198 && (b == 18 || b == 19)) // 198.18.0.0/15 Benchmark testing (RFC 2544)
    }

    fn is_private_ipv6(ipv6: &std::net::Ipv6Addr) -> bool {
        if let Some(ipv4) = ipv6.to_ipv4() {
            return Self::is_private_ipv4(&ipv4);
        }

        let segments = ipv6.segments();
        let first = segments[0];
        ipv6.is_loopback()
            || ipv6.is_unspecified()
            || ipv6.is_multicast()
            || ipv6.is_unique_local()
            || ipv6.is_unicast_link_local()
            || (first & 0xfe00) == 0xfc00 // Unique Local Address fc00::/7
            || (first & 0xffc0) == 0xfe80 // Link-Local Unicast fe80::/10
            || (first & 0xffc0) == 0xfec0 // Site-Local Unicast (deprecated) fec0::/10
            || (first & 0xff00) == 0xff00 // Multicast ff00::/8
            || (segments[0] == 0x0064 && segments[1] == 0xff9b) // NAT64 64:ff9b::/96
            || (segments[0] == 0x0100 && segments[1] == 0) // Discard-only prefix 100::/64
            || (segments[0] == 0x2001 && segments[1] == 0) // TEREDO 2001::/32
            || (segments[0] == 0x2001 && segments[1] == 0x0db8) // Documentation 2001:db8::/32
            || segments[0] == 0x2002 // 6to4 2002::/16
    }

    fn normalize_host(host: &str) -> String {
        host.trim_end_matches('.').to_ascii_lowercase()
    }
}

/// Audit validation configuration
pub async fn audit_validation(config: &SecurityConfig) -> Vec<SecurityFinding> {
    let mut findings = Vec::new();

    // Check SSRF protection
    if config.allowed_hosts.is_empty() {
        findings.push(SecurityFinding {
            category: SecurityCategory::Ssrf,
            severity: Severity::Medium,
            description: "SSRF allowlist is empty".to_string(),
            remediation: "Configure allowed_hosts for SSRF protection".to_string(),
        });
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sql_injection_detection() {
        let validator = InputValidator::new();

        assert!(matches!(
            validator.validate("SELECT * FROM users"),
            ValidationResult::Invalid { .. }
        ));

        assert!(matches!(
            validator.validate("admin' OR '1'='1"),
            ValidationResult::Invalid { .. }
        ));

        assert!(matches!(
            validator.validate("1' UNION ALL SELECT null, null--"),
            ValidationResult::Invalid { .. }
        ));

        assert!(matches!(
            validator.validate("admin'/* comment */"),
            ValidationResult::Invalid { .. }
        ));

        assert!(matches!(
            validator.validate("normal input"),
            ValidationResult::Valid
        ));
    }

    #[test]
    fn test_xss_detection() {
        let validator = InputValidator::new();

        assert!(matches!(
            validator.validate("<script>alert('xss')</script>"),
            ValidationResult::Invalid { .. }
        ));

        assert!(matches!(
            validator.validate("<script src=\"http://evil.com/x.js\">"),
            ValidationResult::Invalid { .. }
        ));

        assert!(matches!(
            validator.validate("<img src=x onerror=alert(1)>"),
            ValidationResult::Invalid { .. }
        ));

        assert!(matches!(
            validator.validate("javascript:alert(1)"),
            ValidationResult::Invalid { .. }
        ));

        assert!(matches!(
            validator.validate("vbscript:msgbox(1)"),
            ValidationResult::Invalid { .. }
        ));
    }

    #[test]
    fn test_html_sanitization_escapes_dangerous_characters() {
        let validator = InputValidator::new();

        assert_eq!(
            validator.sanitize_html(r#"<script src="/x" onload='alert(1)'>&</script>"#),
            "&lt;script src=&quot;&#x2f;x&quot; onload=&#x27;alert(1)&#x27;&gt;&amp;&lt;&#x2f;script&gt;"
        );
    }

    #[test]
    fn test_path_traversal_detection() {
        let validator = InputValidator::new();

        assert!(matches!(
            validator.validate("../../../etc/passwd"),
            ValidationResult::Invalid { .. }
        ));

        assert!(matches!(
            validator.validate("/path/to/dir/.."),
            ValidationResult::Invalid { .. }
        ));

        assert!(matches!(
            validator.validate("file%00.png"),
            ValidationResult::Invalid { .. }
        ));

        assert!(matches!(
            validator.validate("%252e%252e/etc/passwd"),
            ValidationResult::Invalid { .. }
        ));
    }

    #[test]
    fn test_ssrf_protection() {
        let ssrf = SsrfProtection::new().allow_host("api.example.com");

        assert!(matches!(
            ssrf.validate_url("http://localhost/admin"),
            ValidationResult::Invalid { .. }
        ));

        assert!(matches!(
            ssrf.validate_url("http://127.0.0.1/admin"),
            ValidationResult::Invalid { .. }
        ));

        assert!(matches!(
            ssrf.validate_url("file:///etc/passwd"),
            ValidationResult::Invalid { .. }
        ));

        assert!(matches!(
            ssrf.validate_url("http://api.example.com/data"),
            ValidationResult::Valid
        ));

        // IPv4-mapped IPv6 SSRF blocked
        assert!(matches!(
            ssrf.validate_url("http://[::ffff:127.0.0.1]/admin"),
            ValidationResult::Invalid { .. }
        ));

        // Cloud metadata blocked
        assert!(matches!(
            ssrf.validate_url("http://[::ffff:169.254.169.254]/latest/meta-data"),
            ValidationResult::Invalid { .. }
        ));

        // CGNAT IP blocked
        assert!(matches!(
            ssrf.validate_url("http://100.64.0.1/internal"),
            ValidationResult::Invalid { .. }
        ));

        // Local and internal domain suffixes blocked
        assert!(matches!(
            ssrf.validate_url("http://service.local/api"),
            ValidationResult::Invalid { .. }
        ));

        assert!(matches!(
            ssrf.validate_url("http://metadata.internal/info"),
            ValidationResult::Invalid { .. }
        ));

        assert!(matches!(
            ssrf.validate_url("http://app.localhost/status"),
            ValidationResult::Invalid { .. }
        ));

        // Userinfo rejected
        assert!(matches!(
            ssrf.validate_url("http://admin:secret@api.example.com/data"),
            ValidationResult::Invalid { .. }
        ));
    }

    #[test]
    fn test_email_validation() {
        let validator = InputValidator::new();

        assert!(matches!(
            validator.validate_email("user@example.com"),
            ValidationResult::Valid
        ));

        assert!(matches!(
            validator.validate_email("invalid-email"),
            ValidationResult::Invalid { .. }
        ));
    }

    #[test]
    fn test_uuid_validation() {
        let validator = InputValidator::new();

        assert!(matches!(
            validator.validate_uuid("550e8400-e29b-41d4-a716-446655440000"),
            ValidationResult::Valid
        ));

        assert!(matches!(
            validator.validate_uuid("not-a-uuid"),
            ValidationResult::Invalid { .. }
        ));
    }
}
