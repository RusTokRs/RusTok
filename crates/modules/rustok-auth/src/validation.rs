/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Canonical identity input validation owned by `rustok-auth`.
//!
//! All transports and host services must run identity input through these
//! helpers so that email canonicalization, display-name bounds, and password
//! policy cannot drift between REST, GraphQL, native adapters, bootstrap, and
//! operator tooling. Before this module existed, email handling was fragmented
//! across at least three incompatible conventions (trim+lowercase,
//! lowercase-only, and case-insensitive comparison) and passwords were only
//! checked for emptiness.

use crate::error::{AuthError, Result};

/// Maximum email length in characters (RFC 5321 path limit). The persisted
/// column is `varchar(255)`, so longer values must be rejected here instead
/// of surfacing as database errors on PostgreSQL.
pub const MAX_EMAIL_LENGTH_CHARS: usize = 254;

/// Maximum display-name length in characters (matches `users.name`
/// `varchar(255)`).
pub const MAX_DISPLAY_NAME_CHARS: usize = 255;

/// Canonicalize and validate an account email address.
///
/// Trims surrounding whitespace, lowercases the whole address (the storage
/// convention for identity emails in this system), and rejects empty,
/// whitespace/control-bearing, oversized, or structurally invalid values.
pub fn normalize_email(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AuthError::Validation(
            "Email must not be empty".to_string(),
        ));
    }
    if trimmed.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(AuthError::Validation(
            "Email must not contain whitespace or control characters".to_string(),
        ));
    }
    let normalized = trimmed.to_lowercase();
    if normalized.chars().count() > MAX_EMAIL_LENGTH_CHARS {
        return Err(AuthError::Validation(format!(
            "Email must be at most {MAX_EMAIL_LENGTH_CHARS} characters"
        )));
    }
    if !rustok_core::utils::is_valid_email(&normalized) {
        return Err(AuthError::Validation("Invalid email format".to_string()));
    }
    Ok(normalized)
}

/// Canonicalize and validate an optional display name.
///
/// Trims surrounding whitespace and maps blank input to `None` so callers
/// never persist empty names. Rejects control characters and values longer
/// than the persisted column.
pub fn normalize_display_name(raw: Option<&str>) -> Result<Option<String>> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().any(|c| c.is_control()) {
        return Err(AuthError::Validation(
            "Name must not contain control characters".to_string(),
        ));
    }
    if trimmed.chars().count() > MAX_DISPLAY_NAME_CHARS {
        return Err(AuthError::Validation(format!(
            "Name must be at most {MAX_DISPLAY_NAME_CHARS} characters"
        )));
    }
    Ok(Some(trimmed.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_email_trims_and_lowercases() {
        assert_eq!(
            normalize_email("  User@Example.COM ").expect("valid email"),
            "user@example.com"
        );
    }

    #[test]
    fn normalize_email_rejects_invalid_shapes() {
        for invalid in [
            "",
            "   ",
            "not-an-email",
            "user@localhost",
            "user@@example.com",
            "user name@example.com",
            "user@exam ple.com",
            "@example.com",
            "user@",
        ] {
            assert!(
                matches!(normalize_email(invalid), Err(AuthError::Validation(_))),
                "email {invalid:?} must be rejected"
            );
        }
    }

    #[test]
    fn normalize_email_rejects_oversized_local_part_and_address() {
        let long_local = format!("{}@example.com", "a".repeat(65));
        assert!(normalize_email(&long_local).is_err());
        let long_address = format!("a@{}.com", "b".repeat(300));
        assert!(normalize_email(&long_address).is_err());
    }

    #[test]
    fn normalize_display_name_trims_and_maps_blank_to_none() {
        assert_eq!(normalize_display_name(None).expect("ok"), None);
        assert_eq!(normalize_display_name(Some("   ")).expect("ok"), None);
        assert_eq!(
            normalize_display_name(Some("  Ada Lovelace ")).expect("ok"),
            Some("Ada Lovelace".to_string())
        );
    }

    #[test]
    fn normalize_display_name_rejects_control_and_oversized() {
        assert!(normalize_display_name(Some("Ada\u{0007}")).is_err());
        assert!(normalize_display_name(Some(&"a".repeat(MAX_DISPLAY_NAME_CHARS + 1))).is_err());
        assert!(normalize_display_name(Some(&"a".repeat(MAX_DISPLAY_NAME_CHARS))).is_ok());
    }
}
