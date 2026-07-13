use serde::{Deserialize, Serialize};

/// Platform-owned trust inputs mounted into the isolated worker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationPolicy {
    pub trust_policy_revision: u64,
    pub capability_policy_revision: u64,
    pub allowed_signer_identities: Vec<String>,
    pub allowed_oidc_issuers: Vec<String>,
    pub require_transparency_bundle: bool,
    pub allowed_builders: Vec<String>,
    pub allowed_source_repositories: Vec<String>,
    pub allowed_build_types: Vec<String>,
    pub allowed_licenses: Vec<String>,
    pub allowed_cyclonedx_spec_versions: Vec<String>,
    pub maximum_vulnerability_severity: String,
}

impl VerificationPolicy {
    /// Reject incomplete policy rather than silently broadening an admission
    /// decision. Every configured allow-list is part of the admission AND.
    pub fn validate(&self) -> Result<(), String> {
        let required = [
            ("allowed_signer_identities", &self.allowed_signer_identities),
            ("allowed_oidc_issuers", &self.allowed_oidc_issuers),
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
        for (name, values) in required {
            if values.is_empty() || values.iter().any(|value| value.trim().is_empty()) {
                return Err(format!("verification policy requires non-empty {name}"));
            }
        }
        if vulnerability_severity_rank(&self.maximum_vulnerability_severity).is_none() {
            return Err("verification policy has an invalid maximum_vulnerability_severity".into());
        }
        Ok(())
    }
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
