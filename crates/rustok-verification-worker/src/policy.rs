use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Platform-owned trust inputs mounted into the isolated worker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationPolicy {
    pub trust_policy_revision: u64,
    pub capability_policy_revision: u64,
    pub trust_root: VerificationTrustRoots,
    pub allowed_builders: Vec<String>,
    pub allowed_source_repositories: Vec<String>,
    pub allowed_source_refs: Vec<String>,
    pub allowed_build_types: Vec<String>,
    pub allowed_licenses: Vec<String>,
    pub allowed_cyclonedx_spec_versions: Vec<String>,
    pub maximum_vulnerability_severity: String,
}

/// Explicit active/retiring trust roots for a bounded key-rotation window.
///
/// Only the active root is required. A retiring root is a deliberate overlap,
/// never an implicit fallback: it has a hard Unix-second expiry and is ignored
/// at and after that instant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationTrustRoots {
    pub active: VerificationTrustRoot,
    #[serde(default)]
    pub retiring: Option<VerificationRetiringTrustRoot>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationRetiringTrustRoot {
    pub root: VerificationTrustRoot,
    pub retire_after_unix_seconds: u64,
}

/// One concrete trust-root mode. Keyless Sigstore and first-party KMS keys
/// never share an implicit fallback path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VerificationTrustRoot {
    KeylessSigstore {
        allowed_signer_identities: Vec<String>,
        allowed_oidc_issuers: Vec<String>,
        require_transparency_bundle: bool,
    },
    KmsKey {
        key_reference: String,
        signer_identity: String,
        require_transparency_bundle: bool,
    },
}

impl VerificationPolicy {
    /// Reject incomplete policy rather than silently broadening an admission
    /// decision. Every configured allow-list is part of the admission AND.
    pub fn validate(&self) -> Result<(), String> {
        let required = [
            ("allowed_builders", &self.allowed_builders),
            (
                "allowed_source_repositories",
                &self.allowed_source_repositories,
            ),
            ("allowed_source_refs", &self.allowed_source_refs),
            ("allowed_build_types", &self.allowed_build_types),
            ("allowed_licenses", &self.allowed_licenses),
            (
                "allowed_cyclonedx_spec_versions",
                &self.allowed_cyclonedx_spec_versions,
            ),
        ];
        validate_trust_root(&self.trust_root.active)?;
        if let Some(retiring) = &self.trust_root.retiring {
            if retiring.retire_after_unix_seconds == 0 || retiring.root == self.trust_root.active {
                return Err(
                    "verification policy retiring trust root must differ from active and have an expiry"
                        .into(),
                );
            }
            validate_trust_root(&retiring.root)?;
        }
        for (name, values) in required {
            validate_values(name, values)?;
        }
        if vulnerability_severity_rank(&self.maximum_vulnerability_severity).is_none() {
            return Err("verification policy has an invalid maximum_vulnerability_severity".into());
        }
        Ok(())
    }

    pub(crate) fn trust_roots_at(&self, unix_seconds: u64) -> Vec<&VerificationTrustRoot> {
        let mut roots = vec![&self.trust_root.active];
        if let Some(retiring) = &self.trust_root.retiring {
            if unix_seconds < retiring.retire_after_unix_seconds {
                roots.push(&retiring.root);
            }
        }
        roots
    }

    pub(crate) fn current_unix_seconds() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(u64::MAX)
    }

    pub fn signer_is_allowed(&self, signer_identity: &str) -> bool {
        self.signer_is_allowed_at(signer_identity, Self::current_unix_seconds())
    }

    fn signer_is_allowed_at(&self, signer_identity: &str, unix_seconds: u64) -> bool {
        self.trust_roots_at(unix_seconds)
            .into_iter()
            .any(|root| signer_is_allowed(root, signer_identity))
    }
}

fn validate_trust_root(trust_root: &VerificationTrustRoot) -> Result<(), String> {
    match trust_root {
        VerificationTrustRoot::KeylessSigstore {
            allowed_signer_identities,
            allowed_oidc_issuers,
            ..
        } => {
            validate_values("allowed_signer_identities", allowed_signer_identities)?;
            validate_values("allowed_oidc_issuers", allowed_oidc_issuers)?;
        }
        VerificationTrustRoot::KmsKey {
            key_reference,
            signer_identity,
            ..
        } if key_reference.trim().is_empty() || signer_identity.trim().is_empty() => {
            return Err(
                "verification policy requires non-empty KMS key reference and signer identity"
                    .into(),
            );
        }
        VerificationTrustRoot::KmsKey { .. } => {}
    }
    Ok(())
}

fn signer_is_allowed(trust_root: &VerificationTrustRoot, signer_identity: &str) -> bool {
    match trust_root {
        VerificationTrustRoot::KeylessSigstore {
            allowed_signer_identities,
            ..
        } => allowed_signer_identities
            .iter()
            .any(|identity| identity == signer_identity),
        VerificationTrustRoot::KmsKey {
            signer_identity: configured,
            ..
        } => configured == signer_identity,
    }
}

fn validate_values(name: &str, values: &[String]) -> Result<(), String> {
    if values.is_empty() || values.iter().any(|value| value.trim().is_empty()) {
        return Err(format!("verification policy requires non-empty {name}"));
    }
    Ok(())
}

pub(crate) fn vulnerability_severity_rank(severity: &str) -> Option<u8> {
    match severity.to_ascii_lowercase().as_str() {
        "none" | "info" => Some(0),
        "low" => Some(1),
        "medium" | "moderate" => Some(2),
        "high" => Some(3),
        "critical" => Some(4),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        VerificationPolicy, VerificationRetiringTrustRoot, VerificationTrustRoot,
        VerificationTrustRoots,
    };

    fn policy(trust_root: VerificationTrustRoot) -> VerificationPolicy {
        VerificationPolicy {
            trust_policy_revision: 1,
            capability_policy_revision: 1,
            trust_root: VerificationTrustRoots {
                active: trust_root,
                retiring: None,
            },
            allowed_builders: vec!["https://build.rustok.dev/worker".into()],
            allowed_source_repositories: vec!["https://github.com/rustok/module".into()],
            allowed_source_refs: vec!["refs/heads/main".into()],
            allowed_build_types: vec!["https://rustok.dev/build/wasm/v1".into()],
            allowed_licenses: vec!["MIT".into()],
            allowed_cyclonedx_spec_versions: vec!["1.6".into()],
            maximum_vulnerability_severity: "high".into(),
        }
    }

    #[test]
    fn kms_trust_root_does_not_require_keyless_identity_inputs() {
        let policy = policy(VerificationTrustRoot::KmsKey {
            key_reference: "awskms:///alias/rustok-module-signing".into(),
            signer_identity: "first-party-release".into(),
            require_transparency_bundle: true,
        });
        assert!(policy.validate().is_ok());
        assert!(policy.signer_is_allowed("first-party-release"));
        assert!(!policy.signer_is_allowed("builder@rustok.dev"));
    }

    #[test]
    fn keyless_trust_root_rejects_empty_issuer_allow_list() {
        let policy = policy(VerificationTrustRoot::KeylessSigstore {
            allowed_signer_identities: vec!["builder@rustok.dev".into()],
            allowed_oidc_issuers: Vec::new(),
            require_transparency_bundle: true,
        });
        assert!(policy.validate().is_err());
    }

    #[test]
    fn retiring_root_is_accepted_only_before_its_explicit_expiry() {
        let active = VerificationTrustRoot::KmsKey {
            key_reference: "awskms:///alias/current".into(),
            signer_identity: "current-release".into(),
            require_transparency_bundle: true,
        };
        let retiring = VerificationTrustRoot::KmsKey {
            key_reference: "awskms:///alias/retiring".into(),
            signer_identity: "retiring-release".into(),
            require_transparency_bundle: true,
        };
        let mut policy = policy(active);
        policy.trust_root.retiring = Some(VerificationRetiringTrustRoot {
            root: retiring,
            retire_after_unix_seconds: 100,
        });

        assert!(policy.validate().is_ok());
        assert!(policy.signer_is_allowed_at("retiring-release", 99));
        assert!(!policy.signer_is_allowed_at("retiring-release", 100));
        assert!(policy.signer_is_allowed_at("current-release", 100));
    }
}
