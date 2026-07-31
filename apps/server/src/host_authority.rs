use std::collections::HashSet;

use axum::http::HeaderMap;
use rustok_api::{HostAuthority, HostAuthorityContext};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::error::{Error, Result};

pub const HOST_AUTHORITY_CREDENTIALS_ENV: &str = "RUSTOK_HOST_AUTHORITY_CREDENTIALS";
pub const HOST_AUTHORITY_TOKEN_HEADER: &str = "x-rustok-host-token";
const MAX_CREDENTIALS: usize = 64;
const MIN_TOKEN_BYTES: usize = 32;
const MAX_TOKEN_BYTES: usize = 512;

#[derive(Clone, Debug, Default)]
pub struct HostAuthorityPolicy {
    credentials: Vec<HostAuthorityCredential>,
}

#[derive(Clone, Debug)]
struct HostAuthorityCredential {
    actor_id: Uuid,
    authority: HostAuthority,
    token_sha256: [u8; 32],
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfiguredCredential {
    actor_id: Uuid,
    authority: String,
    token_sha256: String,
}

impl HostAuthorityPolicy {
    pub fn from_env() -> Result<Self> {
        match std::env::var(HOST_AUTHORITY_CREDENTIALS_ENV) {
            Ok(value) => Self::parse(&value),
            Err(std::env::VarError::NotPresent) => Ok(Self::default()),
            Err(std::env::VarError::NotUnicode(_)) => Err(Error::BadRequest(format!(
                "{HOST_AUTHORITY_CREDENTIALS_ENV} must contain valid UTF-8"
            ))),
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        if value.trim().is_empty() {
            return Ok(Self::default());
        }

        let configured: Vec<ConfiguredCredential> = serde_json::from_str(value).map_err(|_| {
            Error::BadRequest(format!(
                "{HOST_AUTHORITY_CREDENTIALS_ENV} must be a JSON array of host credentials"
            ))
        })?;
        if configured.len() > MAX_CREDENTIALS {
            return Err(Error::BadRequest(format!(
                "{HOST_AUTHORITY_CREDENTIALS_ENV} must contain at most {MAX_CREDENTIALS} credentials"
            )));
        }

        let mut credentials = Vec::with_capacity(configured.len());
        let mut token_hashes = HashSet::with_capacity(configured.len());
        for credential in configured {
            if credential.actor_id.is_nil() {
                return Err(Error::BadRequest(format!(
                    "{HOST_AUTHORITY_CREDENTIALS_ENV} must not contain a nil actor_id"
                )));
            }
            let authority = match credential.authority.trim().to_ascii_lowercase().as_str() {
                "read" => HostAuthority::Read,
                "manage" => HostAuthority::Manage,
                _ => {
                    return Err(Error::BadRequest(format!(
                        "{HOST_AUTHORITY_CREDENTIALS_ENV} authority must be `read` or `manage`"
                    )));
                }
            };
            let decoded = hex::decode(credential.token_sha256.trim()).map_err(|_| {
                Error::BadRequest(format!(
                    "{HOST_AUTHORITY_CREDENTIALS_ENV} token_sha256 must be 64 hexadecimal characters"
                ))
            })?;
            let token_sha256: [u8; 32] = decoded.try_into().map_err(|_| {
                Error::BadRequest(format!(
                    "{HOST_AUTHORITY_CREDENTIALS_ENV} token_sha256 must be 64 hexadecimal characters"
                ))
            })?;
            if !token_hashes.insert(token_sha256) {
                return Err(Error::BadRequest(format!(
                    "{HOST_AUTHORITY_CREDENTIALS_ENV} must not contain duplicate token hashes"
                )));
            }
            credentials.push(HostAuthorityCredential {
                actor_id: credential.actor_id,
                authority,
                token_sha256,
            });
        }

        Ok(Self { credentials })
    }

    pub fn authenticate(&self, token: &str) -> Option<HostAuthorityContext> {
        let token = token.trim();
        if !(MIN_TOKEN_BYTES..=MAX_TOKEN_BYTES).contains(&token.len()) {
            return None;
        }

        let presented_hash: [u8; 32] = Sha256::digest(token.as_bytes()).into();
        self.credentials.iter().find_map(|credential| {
            bool::from(presented_hash.ct_eq(&credential.token_sha256)).then(|| {
                HostAuthorityContext::for_actor(credential.authority, credential.actor_id)
                    .expect("validated non-nil host operator actor")
            })
        })
    }

    pub fn is_empty(&self) -> bool {
        self.credentials.is_empty()
    }
}

/// Authenticate the host-only credential header without consulting tenant or
/// OAuth state. Ordinary requests do not parse host credential configuration.
pub fn resolve_host_authority(headers: &HeaderMap) -> Result<Option<HostAuthorityContext>> {
    let Some(value) = headers.get(HOST_AUTHORITY_TOKEN_HEADER) else {
        return Ok(None);
    };
    let policy = HostAuthorityPolicy::from_env()?;
    resolve_with_policy(value.to_str(), &policy)
}

fn resolve_with_policy(
    token: std::result::Result<&str, axum::http::header::ToStrError>,
    policy: &HostAuthorityPolicy,
) -> Result<Option<HostAuthorityContext>> {
    let token = token.map_err(|_| {
        Error::Unauthorized("Host-global authority credential is invalid".to_string())
    })?;
    policy
        .authenticate(token)
        .map(Some)
        .ok_or_else(|| Error::Unauthorized("Host-global authority credential is invalid".to_string()))
}

#[cfg(test)]
mod tests {
    use super::{
        HOST_AUTHORITY_TOKEN_HEADER, HostAuthorityPolicy, resolve_host_authority,
        resolve_with_policy,
    };
    use axum::http::HeaderMap;
    use rustok_api::HostAuthority;
    use sha2::{Digest, Sha256};
    use uuid::Uuid;

    fn token_hash(token: &str) -> String {
        hex::encode(Sha256::digest(token.as_bytes()))
    }

    fn policy_json(actor_id: Uuid, authority: &str, token: &str) -> String {
        serde_json::json!([{
            "actor_id": actor_id,
            "authority": authority,
            "token_sha256": token_hash(token),
        }])
        .to_string()
    }

    #[test]
    fn absent_configuration_has_no_host_credentials() {
        assert!(HostAuthorityPolicy::parse(" ").expect("empty policy").is_empty());
    }

    #[test]
    fn exact_high_entropy_token_admits_configured_operator() {
        let actor_id = Uuid::new_v4();
        let token = "operator-token-with-at-least-32-bytes-of-entropy";
        let policy = HostAuthorityPolicy::parse(&policy_json(actor_id, "manage", token))
            .expect("valid host policy");

        let authority = policy.authenticate(token).expect("configured host operator");
        assert_eq!(authority.actor_id(), actor_id);
        assert_eq!(authority.authority(), HostAuthority::Manage);
        assert!(policy.authenticate("wrong-operator-token-with-at-least-32-bytes").is_none());
        assert!(policy.authenticate("too-short").is_none());
    }

    #[test]
    fn header_resolution_uses_the_same_exact_policy() {
        let actor_id = Uuid::new_v4();
        let token = "operator-token-with-at-least-32-bytes-of-entropy";
        let policy = HostAuthorityPolicy::parse(&policy_json(actor_id, "read", token))
            .expect("valid host policy");

        let authority = resolve_with_policy(Ok(token), &policy)
            .expect("valid credential")
            .expect("configured authority");
        assert_eq!(authority.actor_id(), actor_id);
        assert_eq!(authority.authority(), HostAuthority::Read);
        assert!(resolve_with_policy(Ok("wrong-operator-token-with-at-least-32-bytes"), &policy).is_err());
    }

    #[test]
    fn rotation_can_overlap_distinct_tokens_for_the_same_actor() {
        let actor_id = Uuid::new_v4();
        let old_token = "old-operator-token-with-at-least-32-bytes";
        let new_token = "new-operator-token-with-at-least-32-bytes";
        let policy = HostAuthorityPolicy::parse(
            &serde_json::json!([
                {
                    "actor_id": actor_id,
                    "authority": "read",
                    "token_sha256": token_hash(old_token),
                },
                {
                    "actor_id": actor_id,
                    "authority": "manage",
                    "token_sha256": token_hash(new_token),
                }
            ])
            .to_string(),
        )
        .expect("overlapping rotation policy");

        assert_eq!(
            policy.authenticate(old_token).map(|value| value.authority()),
            Some(HostAuthority::Read)
        );
        assert_eq!(
            policy.authenticate(new_token).map(|value| value.authority()),
            Some(HostAuthority::Manage)
        );
    }

    #[test]
    fn malformed_or_ambiguous_configuration_is_rejected() {
        let actor_id = Uuid::new_v4();
        let hash = token_hash("operator-token-with-at-least-32-bytes-of-entropy");
        assert!(HostAuthorityPolicy::parse("{}").is_err());
        assert!(
            HostAuthorityPolicy::parse(&policy_json(
                Uuid::nil(),
                "manage",
                "operator-token-with-at-least-32-bytes"
            ))
            .is_err()
        );
        assert!(
            HostAuthorityPolicy::parse(&policy_json(
                actor_id,
                "owner",
                "operator-token-with-at-least-32-bytes"
            ))
            .is_err()
        );
        assert!(
            HostAuthorityPolicy::parse(
                &serde_json::json!([
                    {"actor_id": actor_id, "authority": "read", "token_sha256": hash.clone()},
                    {"actor_id": Uuid::new_v4(), "authority": "manage", "token_sha256": hash}
                ])
                .to_string()
            )
            .is_err()
        );
    }

    #[test]
    fn absent_header_does_not_consult_host_configuration() {
        let headers = HeaderMap::new();
        assert!(resolve_host_authority(&headers).expect("no header").is_none());
    }

    #[test]
    fn header_name_is_static_and_lowercase() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HOST_AUTHORITY_TOKEN_HEADER,
            "operator-token-with-at-least-32-bytes-of-entropy"
                .parse()
                .expect("valid header"),
        );
        assert!(headers.contains_key(HOST_AUTHORITY_TOKEN_HEADER));
    }
}
