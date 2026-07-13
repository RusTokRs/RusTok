use serde::{Deserialize, Serialize};

/// Platform-owned trust inputs mounted into the isolated worker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationPolicy {
    pub trust_policy_revision: u64,
    pub capability_policy_revision: u64,
    pub allowed_signer_identities: Vec<String>,
    pub allowed_oidc_issuers: Vec<String>,
    pub require_transparency_bundle: bool,
}
