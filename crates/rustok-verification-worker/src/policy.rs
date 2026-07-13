use serde::{Deserialize, Serialize};

/// Platform-owned trust inputs mounted into the isolated worker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationPolicy {
    pub trust_policy_revision: u64,
    pub capability_policy_revision: u64,
    pub trust_root: VerificationTrustRoot,
    pub allowed_builders: Vec<String>,
    pub allowed_source_repositories: Vec<String>,
    pub allowed_build_types: Vec<String>,
    pub allowed_licenses: Vec<String>,
    pub allowed_cyclonedx_spec_versions: Vec<String>,
    pub maximum_vulnerability_severity: String,
}

/// Exactly one trust-root mode is selected for a worker deployment. Keyless
/// Sigstore and first-party KMS keys never share an implicit fallback path.
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
            ("allowed_build_types", &self.allowed_build_types),
            ("allowed_licenses", &self.allowed_licenses),
            (
                "allowed_cyclonedx_spec_versions",
                &self.allowed_cyclonedx_spec_versions,
            ),
        ];
        match &self.trust_root {
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
        for (name, values) in required {
            validate_values(name, values)?;
        }
        if vulnerability_severity_rank(&self.maximum_vulnerability_severity).is_none() {
            return Err("verification policy has an invalid maximum_vulnerability_severity".into());
        }
        Ok(())
    }

    pub fn signer_is_allowed(&self, signer_identity: &str) -> bool {
        match &self.trust_root {
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
    use super::{VerificationPolicy, VerificationTrustRoot};

    fn policy(trust_root: VerificationTrustRoot) -> VerificationPolicy {
        VerificationPolicy {
            trust_policy_revision: 1,
            capability_policy_revision: 1,
            trust_root,
            allowed_builders: vec!["https://build.rustok.dev/worker".into()],
            allowed_source_repositories: vec!["https://github.com/rustok/".into()],
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
}
