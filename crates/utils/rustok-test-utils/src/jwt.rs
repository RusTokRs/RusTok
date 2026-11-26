//! Explicit in-memory RSA material for isolated JWT test stores.
//! No production consumer, persistent fixture or implicit signing fallback.

use aws_lc_rs::{
    encoding::{AsDer, Pkcs8V1Der, PublicKeyX509Der},
    error::Unspecified,
    rsa::{KeyPair, KeySize},
    signature::KeyPair as _,
};
use serde_json::{Value, json};

/// A fresh RSA pair for a single test purpose or rotation candidate.
/// Deliberately not Debug: private key material must not appear in logs.
pub struct RsaKeyPairFixture {
    private_pem: String,
    public_pem: String,
}

impl RsaKeyPairFixture {
    pub fn generate() -> Result<Self, Unspecified> {
        let pair = KeyPair::generate(KeySize::Rsa2048)?;
        let private = AsDer::<Pkcs8V1Der>::as_der(&pair)?;
        let public = AsDer::<PublicKeyX509Der>::as_der(pair.public_key())?;
        Ok(Self {
            private_pem: pem::encode(&pem::Pem::new("PRIVATE KEY", private.as_ref())),
            public_pem: pem::encode(&pem::Pem::new("PUBLIC KEY", public.as_ref())),
        })
    }

    /// Test-only access to the private encoding for negative key admission
    /// and explicit environment-source fixtures. Never log this value.
    pub fn private_pem(&self) -> &str {
        &self.private_pem
    }

    pub fn public_pem(&self) -> &str {
        &self.public_pem
    }

    /// Construct one explicitly configured purpose ring.
    pub fn ring(&self, kid: &str) -> Value {
        json!({
            "keyring": {"active_kid": kid},
            "rsa_private_key_pem": self.private_pem,
            "rsa_public_key_pem": self.public_pem,
        })
    }
}

/// One isolated store's four independent RSA signing pairs.
/// Keep the same fixture only for tests sharing that store's key identity.
pub struct JwtKeyringFixture {
    pub access: RsaKeyPairFixture,
    pub password_reset: RsaKeyPairFixture,
    pub email_verification: RsaKeyPairFixture,
    pub invite: RsaKeyPairFixture,
}

impl JwtKeyringFixture {
    pub fn generate() -> Result<Self, Unspecified> {
        Ok(Self {
            access: RsaKeyPairFixture::generate()?,
            password_reset: RsaKeyPairFixture::generate()?,
            email_verification: RsaKeyPairFixture::generate()?,
            invite: RsaKeyPairFixture::generate()?,
        })
    }

    /// Replace only the access ring for a rotation scenario. The three
    /// special-purpose identities remain stable across the generations.
    pub fn document_with_access(
        &self, generation: u64, kid: &str, pair: &RsaKeyPairFixture,
        history: Option<(&str, &RsaKeyPairFixture, i64)>,
    ) -> Value {
        let mut document = self.document(generation);
        let mut access = pair.ring(kid);
        if let Some((kid, pair, until)) = history {
            access["keyring"]["verification_keys"] = json!([{
                "kid": kid, "material": pair.public_pem(), "verify_until": until,
            }]);
        }
        document["access"] = access;
        document
    }

    /// Auth remains responsible for validating all document invariants.
    pub fn document(&self, generation: u64) -> Value {
        json!({
            "generation": generation,
            "access": self.access.ring("access"),
            "password_reset": self.password_reset.ring("password-reset"),
            "email_verification": self.email_verification.ring("email-verification"),
            "invite": self.invite.ring("invite"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn independent_keys_are_generated_for_each_purpose_and_store() {
        let first = JwtKeyringFixture::generate().unwrap();
        let second = JwtKeyringFixture::generate().unwrap();
        let keys = [
            first.access.public_pem(), first.password_reset.public_pem(),
            first.email_verification.public_pem(), first.invite.public_pem(),
            second.access.public_pem(), second.password_reset.public_pem(),
            second.email_verification.public_pem(), second.invite.public_pem(),
        ];
        assert_eq!(keys.into_iter().collect::<HashSet<_>>().len(), 8);
        let doc = first.document(9);
        assert_eq!(doc["generation"], 9);
        assert_eq!(doc["access"]["keyring"]["active_kid"], "access");
        let rotated = first.document_with_access(
            10, "next", &second.access, Some(("access", &first.access, 1234)),
        );
        assert_eq!(rotated["generation"], 10);
        assert_eq!(rotated["access"]["keyring"]["active_kid"], "next");
        assert_eq!(rotated["access"]["keyring"]["verification_keys"][0]["material"],
            first.access.public_pem());
        for purpose in ["password_reset", "email_verification", "invite"] {
            assert!(rotated[purpose] == doc[purpose], "special purpose changed: {purpose}");
        }
    }
}
