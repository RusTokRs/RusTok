use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use sha2::{Digest, Sha256};

use crate::error::{AuthError, Result};

/// Hash a password with Argon2id and a random salt.
pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| AuthError::PasswordHashFailed)
}

/// Verify a password against an Argon2 hash.
pub fn verify_password(password: &str, password_hash: &str) -> Result<bool> {
    let parsed = PasswordHash::new(password_hash).map_err(|_| AuthError::InvalidCredentials)?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// Generate a cryptographically secure 256-bit refresh token (64 hex chars).
pub fn generate_refresh_token() -> String {
    use argon2::password_hash::rand_core::RngCore;
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// SHA-256 hash of a refresh token for secure storage.
pub fn hash_refresh_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_token_256_bit_entropy() {
        let token = generate_refresh_token();
        assert_eq!(
            token.len(),
            64,
            "Refresh token must be 64 hex chars (256 bits)"
        );
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn refresh_token_unique() {
        let t1 = generate_refresh_token();
        let t2 = generate_refresh_token();
        assert_ne!(t1, t2);
    }

    #[test]
    fn refresh_token_hash_sha256() {
        let token = generate_refresh_token();
        let hash = hash_refresh_token(&token);
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
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
}
