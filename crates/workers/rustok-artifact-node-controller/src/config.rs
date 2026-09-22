use std::env;

use rustok_artifact_node_transport::{
    ModuleArtifactNodeAgentIdentity, StaticModuleArtifactNodeAgentAuthenticator,
};
use rustok_worker_transport::PeerCertificateFingerprint;
use serde::Deserialize;
use uuid::Uuid;

const DATABASE_URL_ENV: &str = "RUSTOK_ARTIFACT_NODE_CONTROLLER_DATABASE_URL";
const AGENT_IDENTITIES_JSON_ENV: &str = "RUSTOK_ARTIFACT_NODE_CONTROLLER_AGENT_IDENTITIES_JSON";

/// All deployment-owned controller configuration that is independent from the
/// shared mTLS listener settings. The controller holds only the agent port;
/// it does not configure topology, artifact identity, or policy selection.
pub struct ArtifactNodeControllerConfig {
    pub database_url: String,
    pub agent_authenticator: StaticModuleArtifactNodeAgentAuthenticator,
}

impl ArtifactNodeControllerConfig {
    pub fn from_environment() -> Result<Self, String> {
        let database_url = required_nonempty_env(DATABASE_URL_ENV)?;
        let agent_authenticator =
            parse_agent_authenticator(&required_nonempty_env(AGENT_IDENTITIES_JSON_ENV)?)?;
        Ok(Self {
            database_url,
            agent_authenticator,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentIdentityConfig {
    certificate_fingerprint: String,
    node_id: Uuid,
    agent_id: String,
}

fn parse_agent_authenticator(
    value: &str,
) -> Result<StaticModuleArtifactNodeAgentAuthenticator, String> {
    let entries: Vec<AgentIdentityConfig> = serde_json::from_str(value).map_err(|_| {
        "artifact node controller agent identity configuration is invalid".to_string()
    })?;
    let identities = entries
        .into_iter()
        .map(|entry| {
            Ok((
                PeerCertificateFingerprint::parse(entry.certificate_fingerprint)?,
                ModuleArtifactNodeAgentIdentity::new(entry.node_id, entry.agent_id)?,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    StaticModuleArtifactNodeAgentAuthenticator::new(identities)
}

fn required_nonempty_env(name: &str) -> Result<String, String> {
    let value = env::var(name).map_err(|_| format!("{name} must be configured"))?;
    if value.is_empty() || value.trim() != value {
        return Err(format!("{name} must be a non-empty trimmed value"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::parse_agent_authenticator;

    #[test]
    fn agent_identity_map_requires_canonical_fingerprints_and_principals() {
        assert!(parse_agent_authenticator("[]").is_err());
        assert!(parse_agent_authenticator(
            r#"[{"certificate_fingerprint":"sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","node_id":"d7d5eaef-47f1-49d8-96c4-60f25b714d40","agent_id":"node-a"}]"#
        )
        .is_err());
        assert!(parse_agent_authenticator(
            r#"[{"certificate_fingerprint":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","node_id":"00000000-0000-0000-0000-000000000000","agent_id":"node-a"}]"#
        )
        .is_err());
    }

    #[test]
    fn agent_identity_map_rejects_duplicates_and_unknown_fields() {
        let duplicate = r#"[
            {"certificate_fingerprint":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","node_id":"d7d5eaef-47f1-49d8-96c4-60f25b714d40","agent_id":"node-a"},
            {"certificate_fingerprint":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","node_id":"466c597d-2112-433a-b341-fd43125f4675","agent_id":"node-b"}
        ]"#;
        assert!(parse_agent_authenticator(duplicate).is_err());
        assert!(parse_agent_authenticator(
            r#"[{"certificate_fingerprint":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","node_id":"d7d5eaef-47f1-49d8-96c4-60f25b714d40","agent_id":"node-a","unexpected":true}]"#
        )
        .is_err());
    }
}
