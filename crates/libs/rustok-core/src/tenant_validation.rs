//! Tenant identifier validation and sanitization
//!
//! This module provides security-focused validation for tenant identifiers
//! to prevent injection attacks and ensure data integrity.

use once_cell::sync::Lazy;
use regex::Regex;
use thiserror::Error;

/// Regex pattern for valid slugs (lowercase alphanumeric with hyphens)
static VALID_SLUG_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-z0-9][a-z0-9-]{0,62}$").unwrap());

/// Regex pattern for valid UUIDs
static VALID_UUID_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$").unwrap()
});

/// Reserved slugs that cannot be used as tenant identifiers
const RESERVED_SLUGS: &[&str] = &[
    // System endpoints
    "api",
    "admin",
    "www",
    "app",
    "cdn",
    "assets",
    "static",
    "health",
    "metrics",
    "docs",
    "swagger",
    // Common subdomains
    "mail",
    "smtp",
    "pop",
    "imap",
    "ftp",
    "sftp",
    "ssh",
    "vpn",
    // Security-related
    "auth",
    "login",
    "logout",
    "register",
    "signup",
    "signin",
    // Development
    "dev",
    "test",
    "staging",
    "prod",
    "production",
    "localhost",
    // Database/cache
    "db",
    "database",
    "cache",
    "redis",
    "postgres",
    // Special
    "root",
    "system",
    "internal",
    "private",
    "public",
];

/// Errors that can occur during tenant identifier validation
#[derive(Debug, Error, Clone, PartialEq)]
pub enum TenantValidationError {
    /// Identifier is too long
    #[error("Identifier is too long (max 64 characters)")]
    TooLong,

    /// Identifier contains invalid characters
    #[error(
        "Identifier contains invalid characters (only lowercase alphanumeric and hyphens allowed)"
    )]
    InvalidCharacters,

    /// Identifier is a reserved keyword
    #[error("Identifier '{0}' is reserved and cannot be used")]
    Reserved(String),

    /// Invalid UUID format
    #[error("Invalid UUID format")]
    InvalidUuid,

    /// Identifier cannot be empty
    #[error("Identifier cannot be empty")]
    Empty,

    /// Hostname is too long (max 253 characters per RFC 1035)
    #[error("Hostname is too long (max 253 characters)")]
    HostnameTooLong,

    /// Invalid hostname format
    #[error("Invalid hostname format")]
    InvalidHostname,
}

/// Validator for tenant identifiers
pub struct TenantIdentifierValidator;

impl TenantIdentifierValidator {
    /// Validates and sanitizes a slug identifier
    ///
    /// # Arguments
    /// * `raw` - The raw slug input to validate
    ///
    /// # Returns
    /// * `Ok(String)` - The sanitized slug (trimmed and lowercased)
    /// * `Err(TenantValidationError)` - If validation fails
    ///
    /// # Security
    /// - Trims whitespace
    /// - Converts to lowercase
    /// - Validates length (1-64 characters)
    /// - Ensures only alphanumeric characters and hyphens
    /// - Blocks reserved keywords
    ///
    /// # Examples
    /// ```
    /// use rustok_core::tenant_validation::TenantIdentifierValidator;
    ///
    /// // Valid slugs
    /// assert!(TenantIdentifierValidator::validate_slug("my-tenant").is_ok());
    /// assert!(TenantIdentifierValidator::validate_slug("tenant123").is_ok());
    ///
    /// // Invalid slugs
    /// assert!(TenantIdentifierValidator::validate_slug("").is_err());
    /// assert!(TenantIdentifierValidator::validate_slug("admin").is_err());
    /// assert!(TenantIdentifierValidator::validate_slug("tenant@123").is_err());
    /// ```
    pub fn validate_slug(raw: &str) -> Result<String, TenantValidationError> {
        let sanitized = raw.trim().to_lowercase();

        // Check for empty
        if sanitized.is_empty() {
            return Err(TenantValidationError::Empty);
        }

        // Check length
        if sanitized.len() > 64 {
            return Err(TenantValidationError::TooLong);
        }

        // Check pattern (alphanumeric + hyphens, must start with alphanumeric)
        if !VALID_SLUG_PATTERN.is_match(&sanitized) {
            return Err(TenantValidationError::InvalidCharacters);
        }

        // Check for reserved names
        if RESERVED_SLUGS.contains(&sanitized.as_str()) {
            return Err(TenantValidationError::Reserved(sanitized));
        }

        Ok(sanitized)
    }

    /// Validates a UUID identifier
    ///
    /// # Arguments
    /// * `raw` - The raw UUID string to validate
    ///
    /// # Returns
    /// * `Ok(uuid::Uuid)` - The parsed UUID
    /// * `Err(TenantValidationError)` - If validation fails
    ///
    /// # Security
    /// - Trims whitespace
    /// - Validates UUID format
    /// - Returns parsed UUID for type safety
    ///
    /// # Examples
    /// ```
    /// use rustok_core::tenant_validation::TenantIdentifierValidator;
    /// use uuid::Uuid;
    ///
    /// let uuid = Uuid::new_v4();
    /// assert!(TenantIdentifierValidator::validate_uuid(&uuid.to_string()).is_ok());
    /// assert!(TenantIdentifierValidator::validate_uuid("invalid").is_err());
    /// ```
    pub fn validate_uuid(raw: &str) -> Result<uuid::Uuid, TenantValidationError> {
        let sanitized = raw.trim().to_lowercase();

        // Validate format before parsing
        if !VALID_UUID_PATTERN.is_match(&sanitized) {
            return Err(TenantValidationError::InvalidUuid);
        }

        // Parse UUID
        sanitized
            .parse::<uuid::Uuid>()
            .map_err(|_| TenantValidationError::InvalidUuid)
    }

    /// Validates a hostname identifier
    ///
    /// # Arguments
    /// * `raw` - The raw hostname to validate
    ///
    /// # Returns
    /// * `Ok(String)` - The sanitized hostname (trimmed and lowercased)
    /// * `Err(TenantValidationError)` - If validation fails
    ///
    /// # Security
    /// - Trims whitespace
    /// - Converts to lowercase
    /// - Validates length (max 253 characters per RFC 1035)
    /// - Ensures only valid hostname characters
    /// - Validates domain labels
    ///
    /// # Examples
    /// ```
    /// use rustok_core::tenant_validation::TenantIdentifierValidator;
    ///
    /// assert!(TenantIdentifierValidator::validate_host("example.com").is_ok());
    /// assert!(TenantIdentifierValidator::validate_host("sub.example.com").is_ok());
    /// assert!(TenantIdentifierValidator::validate_host("invalid host").is_err());
    /// ```
    pub fn validate_host(raw: &str) -> Result<String, TenantValidationError> {
        let sanitized = raw.trim().to_lowercase();

        // Check for empty
        if sanitized.is_empty() {
            return Err(TenantValidationError::Empty);
        }

        // Check maximum length (RFC 1035)
        if sanitized.len() > 253 {
            return Err(TenantValidationError::HostnameTooLong);
        }

        // Validate characters (alphanumeric, dots, and hyphens only)
        if !sanitized
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        {
            return Err(TenantValidationError::InvalidHostname);
        }

        // Validate domain labels
        for label in sanitized.split('.') {
            if label.is_empty() {
                return Err(TenantValidationError::InvalidHostname);
            }
            if label.len() > 63 {
                return Err(TenantValidationError::InvalidHostname);
            }
            // Label cannot start or end with hyphen
            if label.starts_with('-') || label.ends_with('-') {
                return Err(TenantValidationError::InvalidHostname);
            }
        }

        Ok(sanitized)
    }

    /// Validates any tenant identifier by auto-detecting the type
    ///
    /// This is a convenience method that attempts to determine if the
    /// identifier is a UUID, hostname, or slug and validates accordingly.
    ///
    /// # Arguments
    /// * `raw` - The raw identifier to validate
    ///
    /// # Returns
    /// * `Ok(String)` - The sanitized identifier
    /// * `Err(TenantValidationError)` - If validation fails
    pub fn validate_any(raw: &str) -> Result<String, TenantValidationError> {
        let trimmed = raw.trim();

        // Try UUID first (most specific)
        if VALID_UUID_PATTERN.is_match(trimmed) {
            return Self::validate_uuid(trimmed).map(|uuid| uuid.to_string());
        }

        // Try hostname (contains dots)
        if trimmed.contains('.') {
            return Self::validate_host(trimmed);
        }

        // Default to slug
        Self::validate_slug(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ═══════════════════════════════════════════════════════════════════
    // SLUG VALIDATION TESTS
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_validate_slug_valid() {
        assert_eq!(
            TenantIdentifierValidator::validate_slug("my-tenant"),
            Ok("my-tenant".to_string())
        );
        assert_eq!(
            TenantIdentifierValidator::validate_slug("tenant123"),
            Ok("tenant123".to_string())
        );
        assert_eq!(
            TenantIdentifierValidator::validate_slug("a"),
            Ok("a".to_string())
        );
    }

    #[test]
    fn test_validate_slug_normalization() {
        // Whitespace trimming
        assert_eq!(
            TenantIdentifierValidator::validate_slug("  my-tenant  "),
            Ok("my-tenant".to_string())
        );
        // Lowercase conversion
        assert_eq!(
            TenantIdentifierValidator::validate_slug("MyTenant123"),
            Ok("mytenant123".to_string())
        );
    }

    #[test]
    fn test_validate_slug_empty() {
        assert!(TenantIdentifierValidator::validate_slug("").is_err());
        assert!(TenantIdentifierValidator::validate_slug("   ").is_err());
    }

    #[test]
    fn test_validate_slug_reserved() {
        assert!(matches!(
            TenantIdentifierValidator::validate_slug("admin"),
            Err(TenantValidationError::Reserved(_))
        ));
        assert!(matches!(
            TenantIdentifierValidator::validate_slug("api"),
            Err(TenantValidationError::Reserved(_))
        ));
        assert!(matches!(
            TenantIdentifierValidator::validate_slug("www"),
            Err(TenantValidationError::Reserved(_))
        ));
    }

    #[test]
    fn test_validate_slug_invalid_characters() {
        assert!(matches!(
            TenantIdentifierValidator::validate_slug("tenant@123"),
            Err(TenantValidationError::InvalidCharacters)
        ));
        assert!(matches!(
            TenantIdentifierValidator::validate_slug("tenant 123"),
            Err(TenantValidationError::InvalidCharacters)
        ));
        assert!(matches!(
            TenantIdentifierValidator::validate_slug("tenant_123"),
            Err(TenantValidationError::InvalidCharacters)
        ));
        assert!(matches!(
            TenantIdentifierValidator::validate_slug("tenant.123"),
            Err(TenantValidationError::InvalidCharacters)
        ));
    }

    #[test]
    fn test_validate_slug_too_long() {
        let long = "a".repeat(65);
        assert!(matches!(
            TenantIdentifierValidator::validate_slug(&long),
            Err(TenantValidationError::TooLong)
        ));
    }

    // ═══════════════════════════════════════════════════════════════════
    // UUID VALIDATION TESTS
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_validate_uuid_valid() {
        let uuid = uuid::Uuid::new_v4();
        let result = TenantIdentifierValidator::validate_uuid(&uuid.to_string());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), uuid);
    }

    #[test]
    fn test_validate_uuid_invalid() {
        assert!(TenantIdentifierValidator::validate_uuid("invalid").is_err());
        assert!(TenantIdentifierValidator::validate_uuid("12345").is_err());
        assert!(TenantIdentifierValidator::validate_uuid("not-a-uuid").is_err());
    }

    #[test]
    fn test_validate_uuid_normalization() {
        let uuid = uuid::Uuid::new_v4();
        let uppercase = uuid.to_string().to_uppercase();
        let result = TenantIdentifierValidator::validate_uuid(&uppercase);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), uuid);
    }

    // ═══════════════════════════════════════════════════════════════════
    // HOSTNAME VALIDATION TESTS
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_validate_host_valid() {
        assert_eq!(
            TenantIdentifierValidator::validate_host("example.com"),
            Ok("example.com".to_string())
        );
        assert_eq!(
            TenantIdentifierValidator::validate_host("sub.example.com"),
            Ok("sub.example.com".to_string())
        );
        assert_eq!(
            TenantIdentifierValidator::validate_host("my-tenant.app.com"),
            Ok("my-tenant.app.com".to_string())
        );
    }

    #[test]
    fn test_validate_host_normalization() {
        assert_eq!(
            TenantIdentifierValidator::validate_host("  Example.COM  "),
            Ok("example.com".to_string())
        );
    }

    #[test]
    fn test_validate_host_invalid() {
        assert!(TenantIdentifierValidator::validate_host("").is_err());
        assert!(TenantIdentifierValidator::validate_host("invalid host").is_err());
        assert!(TenantIdentifierValidator::validate_host("example..com").is_err());
        assert!(TenantIdentifierValidator::validate_host("-example.com").is_err());
        assert!(TenantIdentifierValidator::validate_host("example-.com").is_err());
    }

    #[test]
    fn test_validate_host_too_long() {
        let long = format!("{}.com", "a".repeat(250));
        assert!(matches!(
            TenantIdentifierValidator::validate_host(&long),
            Err(TenantValidationError::HostnameTooLong)
        ));
    }

    // ═══════════════════════════════════════════════════════════════════
    // AUTO-DETECT VALIDATION TESTS
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_validate_any_slug() {
        assert_eq!(
            TenantIdentifierValidator::validate_any("my-tenant"),
            Ok("my-tenant".to_string())
        );
    }

    #[test]
    fn test_validate_any_uuid() {
        let uuid = uuid::Uuid::new_v4();
        let result = TenantIdentifierValidator::validate_any(&uuid.to_string());
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_any_hostname() {
        assert_eq!(
            TenantIdentifierValidator::validate_any("example.com"),
            Ok("example.com".to_string())
        );
    }

    // ═══════════════════════════════════════════════════════════════════
    // SECURITY TESTS
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_sql_injection_attempts() {
        // Common SQL injection patterns
        assert!(TenantIdentifierValidator::validate_slug("'; DROP TABLE--").is_err());
        assert!(TenantIdentifierValidator::validate_slug("1' OR '1'='1").is_err());
        assert!(TenantIdentifierValidator::validate_slug("admin'--").is_err());
    }

    #[test]
    fn test_xss_attempts() {
        // Common XSS patterns
        assert!(TenantIdentifierValidator::validate_slug("<script>alert(1)</script>").is_err());
        assert!(TenantIdentifierValidator::validate_slug("javascript:alert(1)").is_err());
        assert!(TenantIdentifierValidator::validate_slug("<img src=x>").is_err());
    }

    #[test]
    fn test_path_traversal_attempts() {
        // Path traversal patterns
        assert!(TenantIdentifierValidator::validate_slug("../../../etc/passwd").is_err());
        assert!(TenantIdentifierValidator::validate_slug("..\\windows\\system32").is_err());
    }
}
