//! Strict BFF refresh envelope admission and generation-bound recovery.
//! The refresh token and recovery seed are never serialized into browser payloads.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefreshCookieError {
    Malformed,
    Duplicate,
}

#[derive(Clone, PartialEq, Eq)]
pub struct RefreshCredential {
    tenant: String,
    token: String,
    seed: String,
}

impl std::fmt::Debug for RefreshCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RefreshCredential(<redacted>)")
    }
}

fn canonical_secret(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
        && value.bytes().any(|b| b != b'0')
}

fn valid_tenant(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 253
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

impl RefreshCredential {
    pub fn parse(value: &str) -> Result<Self, RefreshCookieError> {
        let mut parts = value.split('.');
        let tenant = parts.next().ok_or(RefreshCookieError::Malformed)?;
        let token = parts.next().ok_or(RefreshCookieError::Malformed)?;
        let seed = parts.next().ok_or(RefreshCookieError::Malformed)?;
        if parts.next().is_some()
            || !valid_tenant(tenant)
            || !canonical_secret(token)
            || !canonical_secret(seed)
        {
            return Err(RefreshCookieError::Malformed);
        }
        Ok(Self {
            tenant: tenant.into(),
            token: token.into(),
            seed: seed.into(),
        })
    }

    pub fn tenant(&self) -> &str {
        &self.tenant
    }

    pub(super) fn cookie_value(&self) -> String {
        format!("{}.{}.{}", self.tenant, self.token, self.seed)
    }

    #[cfg(feature = "ssr")]
    pub(crate) fn new(tenant: &str, token: &str, seed: String) -> Result<Self, RefreshCookieError> {
        Self::parse(&format!("{tenant}.{token}.{seed}"))
    }

    /// The API logout cookie has no BFF-local tenant prefix.
    #[cfg(feature = "ssr")]
    pub(crate) fn api_cookie_value(&self) -> String {
        format!("{}.{}", self.token, self.seed)
    }

    #[cfg(feature = "ssr")]
    pub(crate) fn token(&self) -> &str {
        &self.token
    }

    #[cfg(feature = "ssr")]
    pub(crate) fn rotated(&self, token: &str) -> Result<Self, RefreshCookieError> {
        Self::new(&self.tenant, token, self.seed.clone())
    }

    #[cfg(feature = "ssr")]
    pub(crate) fn generate_seed() -> Result<String, crate::model::AuthError> {
        use rand_core::{OsRng, RngCore};
        let mut seed = [0u8; 32];
        OsRng
            .try_fill_bytes(&mut seed)
            .map_err(|_| crate::model::AuthError::Network)?;
        if seed == [0; 32] {
            return Err(crate::model::AuthError::Network);
        }
        Ok(hex::encode(seed))
    }

    #[cfg(feature = "ssr")]
    pub(crate) fn retry_key(&self) -> Result<String, crate::model::AuthError> {
        use hmac::{Hmac, KeyInit, Mac};
        use sha2::Sha256;
        let seed = hex::decode(&self.seed).map_err(|_| crate::model::AuthError::Network)?;
        let mut mac =
            Hmac::<Sha256>::new_from_slice(&seed).map_err(|_| crate::model::AuthError::Network)?;
        // The tenant excludes NUL, the token is fixed-width canonical hex.
        mac.update(b"rustok/leptos-bff/refresh-retry\0");
        mac.update(self.tenant.as_bytes());
        mac.update(b"\0");
        mac.update(self.token.as_bytes());
        let proof = hex::encode(mac.finalize().into_bytes());
        if !canonical_secret(&proof) {
            return Err(crate::model::AuthError::Network);
        }
        Ok(proof)
    }
}

pub fn inspect_refresh_cookie_headers<'a>(
    headers: impl IntoIterator<Item = &'a str>,
) -> Result<Option<RefreshCredential>, RefreshCookieError> {
    let mut found = None;
    for header in headers {
        for part in header.split(';').map(str::trim) {
            let Some((name, value)) = part.split_once('=') else {
                if part == super::ADMIN_REFRESH_COOKIE {
                    return Err(RefreshCookieError::Malformed);
                }
                continue;
            };
            if name.trim() != super::ADMIN_REFRESH_COOKIE {
                continue;
            }
            if found.is_some() {
                return Err(RefreshCookieError::Duplicate);
            }
            found = Some(RefreshCredential::parse(value)?);
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope() -> String {
        format!("shop.{}.{}", "a1".repeat(32), "b2".repeat(32))
    }

    #[test]
    fn only_an_absent_cookie_is_an_anonymous_observation() {
        assert_eq!(inspect_refresh_cookie_headers([]), Ok(None));
        assert_eq!(inspect_refresh_cookie_headers(["theme=dark"]), Ok(None));
        let header = format!(
            "theme=dark; {}={}",
            super::super::ADMIN_REFRESH_COOKIE,
            envelope()
        );
        let credential = inspect_refresh_cookie_headers([header.as_str()])
            .unwrap()
            .unwrap();
        assert_eq!(credential.tenant(), "shop");
        assert_eq!(credential.cookie_value(), envelope());
        assert!(!format!("{credential:?}").contains(&"a1".repeat(32)));
        assert!(!format!("{credential:?}").contains(&"b2".repeat(32)));
    }

    #[test]
    fn malformed_or_legacy_envelopes_never_become_anonymous() {
        for value in [
            String::new(),
            "shop".into(),
            format!("shop.{}", "a1".repeat(32)),
            format!("shop.{}.{}", "0".repeat(64), "b2".repeat(32)),
            format!("shop.{}.{}", "a1".repeat(32), "0".repeat(64)),
            format!("bad tenant.{}.{}", "a1".repeat(32), "b2".repeat(32)),
            format!("shop.{}.{}", "A1".repeat(32), "b2".repeat(32)),
            format!("{}.extra", envelope()),
        ] {
            let header = format!("{}={value}", super::super::ADMIN_REFRESH_COOKIE);
            assert_eq!(
                inspect_refresh_cookie_headers([header.as_str()]),
                Err(RefreshCookieError::Malformed)
            );
        }
    }

    #[test]
    fn duplicate_cookies_are_ambiguous_even_when_values_match() {
        let header = format!("{}={}", super::super::ADMIN_REFRESH_COOKIE, envelope());
        assert_eq!(
            inspect_refresh_cookie_headers([header.as_str(), header.as_str()]),
            Err(RefreshCookieError::Duplicate)
        );
    }

    #[cfg(feature = "ssr")]
    #[test]
    fn proof_is_stable_for_the_same_envelope_and_changes_with_generation_tenant_or_seed() {
        let credential = RefreshCredential::parse(&envelope()).unwrap();
        let proof = credential.retry_key().unwrap();
        assert!(canonical_secret(&proof));
        assert_eq!(
            proof,
            RefreshCredential::parse(&credential.cookie_value())
                .unwrap()
                .retry_key()
                .unwrap()
        );
        assert_ne!(
            proof,
            credential
                .rotated(&"c3".repeat(32))
                .unwrap()
                .retry_key()
                .unwrap()
        );
        assert_ne!(
            proof,
            RefreshCredential::new("other", credential.token(), credential.seed.clone())
                .unwrap()
                .retry_key()
                .unwrap()
        );
        assert_ne!(
            proof,
            RefreshCredential::new("shop", credential.token(), "d4".repeat(32))
                .unwrap()
                .retry_key()
                .unwrap()
        );
        assert_ne!(
            RefreshCredential::generate_seed().unwrap(),
            RefreshCredential::generate_seed().unwrap()
        );
    }
}
