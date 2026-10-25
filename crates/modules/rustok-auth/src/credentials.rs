use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};

use crate::error::{AuthError, Result};

/// Maximum allowed password length in bytes to prevent CPU exhaustion DoS on Argon2.
pub const MAX_PASSWORD_LENGTH_BYTES: usize = 1024;

/// Absolute floor for the configurable minimum password length. Operators may
/// raise the minimum through `AuthConfig`, but it can never be configured
/// below the NIST SP 800-63B floor of 8 characters.
pub const MIN_PASSWORD_LENGTH_FLOOR_CHARS: usize = 8;

/// Default minimum password length in characters applied to new credentials.
pub const DEFAULT_MIN_PASSWORD_LENGTH_CHARS: usize = 12;

/// Upper bound for the configurable minimum password length. The minimum is a
/// length-only policy (no composition rules, per NIST SP 800-63B), and 128
/// characters leave ample room for passphrases.
pub const MAX_MIN_PASSWORD_LENGTH_CHARS: usize = 128;

/// Validate a candidate password for a *new* credential (registration,
/// password change/reset, admin provisioning, invite acceptance).
///
/// This is intentionally separate from [`hash_password`]: hashing is also used
/// for generated client secrets and legacy credentials, where rejecting input
/// would be wrong. Verification of existing passwords never applies this
/// policy either, so previously-issued shorter passwords keep working until
/// they are rotated through a policy-enforced path.
pub fn validate_password_policy(password: &str, min_length_chars: usize) -> Result<()> {
    if password.len() > MAX_PASSWORD_LENGTH_BYTES {
        return Err(AuthError::Validation(format!(
            "Password must be at most {MAX_PASSWORD_LENGTH_BYTES} bytes"
        )));
    }
    if password.trim().is_empty() {
        return Err(AuthError::Validation(
            "Password must not be empty or blank".to_string(),
        ));
    }
    let min_length_chars = min_length_chars.max(MIN_PASSWORD_LENGTH_FLOOR_CHARS);
    if password.chars().count() < min_length_chars {
        return Err(AuthError::Validation(format!(
            "Password must be at least {min_length_chars} characters"
        )));
    }
    Ok(())
}

/// Hash a password with Argon2id and a random salt.
pub fn hash_password(password: &str) -> Result<String> {
    if password.is_empty() || password.len() > MAX_PASSWORD_LENGTH_BYTES {
        return Err(AuthError::PasswordHashFailed);
    }
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|_| AuthError::PasswordHashFailed)
}

/// Verify a password against an Argon2 hash.
pub fn verify_password(password: &str, password_hash: &str) -> Result<bool> {
    if password.is_empty() || password.len() > MAX_PASSWORD_LENGTH_BYTES {
        return Ok(false);
    }
    let parsed = match PasswordHash::new(password_hash) {
        Ok(p) => p,
        Err(_) => return Ok(false),
    };
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// Generate a cryptographically secure 256-bit refresh token (64 hex chars).
pub fn generate_refresh_token() -> Result<String> {
    tracing::debug!("Generating 256-bit cryptographically secure refresh token");
    let mut bytes = [0u8; 32];
    OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| AuthError::RefreshTokenGenerationFailed)?;
    Ok(hex::encode(bytes))
}

/// SHA-256 hash of a refresh token for secure storage.
pub fn hash_refresh_token(token: &str) -> String {
    tracing::debug!("Hashing refresh token with SHA-256 for secure storage");
    let token = token.trim();
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_token_256_bit_entropy() {
        let token = generate_refresh_token().expect("refresh token");
        assert_eq!(
            token.len(),
            64,
            "Refresh token must be 64 hex chars (256 bits)"
        );
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn refresh_token_unique() {
        let t1 = generate_refresh_token().expect("refresh token");
        let t2 = generate_refresh_token().expect("refresh token");
        assert_ne!(t1, t2);
    }

    #[test]
    fn refresh_token_hash_sha256() {
        let token = generate_refresh_token().expect("refresh token");
        let hash = hash_refresh_token(&token);
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn refresh_token_hash_trims_whitespace() {
        let token = "  abcdef0123456789  ";
        assert_eq!(
            hash_refresh_token(token),
            hash_refresh_token("abcdef0123456789")
        );
    }

    #[test]
    fn password_hash_argon2() {
        let hash = hash_password("test_password_123!").unwrap();
        assert!(hash.starts_with("$argon2"));
    }

    #[test]
    fn password_verify_roundtrip() {
        let password = "SecureP@ssw0rd!";
        let hash = hash_password(password).unwrap();
        assert!(verify_password(password, &hash).unwrap());
        assert!(!verify_password("wrong_password", &hash).unwrap());
    }

    #[test]
    fn password_hash_unique_salt() {
        let h1 = hash_password("same_password").unwrap();
        let h2 = hash_password("same_password").unwrap();
        assert_ne!(
            h1, h2,
            "Same password must produce different hashes (unique salt)"
        );
    }

    #[test]
    fn password_hash_long_secret() {
        let secret = format!("sk_live_{}", "0123456789abcdef".repeat(8));
        let hash = hash_password(&secret).unwrap();
        assert!(verify_password(&secret, &hash).unwrap());
    }

    #[test]
    fn password_hash_rejects_empty_and_oversized() {
        assert!(hash_password("").is_err());
        let oversized = "a".repeat(MAX_PASSWORD_LENGTH_BYTES + 1);
        assert!(hash_password(&oversized).is_err());
        assert!(!verify_password("", "$argon2id$v=19$m=19456,t=2,p=1$fake$fake").unwrap_or(true));
        assert!(
            !verify_password(&oversized, "$argon2id$v=19$m=19456,t=2,p=1$fake$fake")
                .unwrap_or(true)
        );
    }

    #[test]
    fn password_policy_accepts_passwords_meeting_configured_minimum() {
        assert!(
            validate_password_policy("correct horse battery", DEFAULT_MIN_PASSWORD_LENGTH_CHARS)
                .is_ok()
        );
        assert!(validate_password_policy("12345678", MIN_PASSWORD_LENGTH_FLOOR_CHARS).is_ok());
    }

    #[test]
    fn password_policy_rejects_short_blank_and_oversized() {
        assert!(matches!(
            validate_password_policy("short", DEFAULT_MIN_PASSWORD_LENGTH_CHARS),
            Err(AuthError::Validation(_))
        ));
        assert!(matches!(
            validate_password_policy("            ", DEFAULT_MIN_PASSWORD_LENGTH_CHARS),
            Err(AuthError::Validation(_))
        ));
        let oversized = "a".repeat(MAX_PASSWORD_LENGTH_BYTES + 1);
        assert!(matches!(
            validate_password_policy(&oversized, DEFAULT_MIN_PASSWORD_LENGTH_CHARS),
            Err(AuthError::Validation(_))
        ));
    }

    #[test]
    fn password_policy_never_drops_below_floor() {
        // A misconfigured minimum of 0 must not weaken policy below the floor.
        assert!(validate_password_policy("1234567", 0).is_err());
        assert!(validate_password_policy("12345678", 0).is_ok());
    }

    #[test]
    fn password_verify_handles_malformed_hash_gracefully() {
        assert!(!verify_password("password", "not-a-valid-argon2-hash").unwrap_or(true));
        assert!(!verify_password("password", "").unwrap_or(true));
        assert!(!verify_password("password", "   ").unwrap_or(true));
    }
}
