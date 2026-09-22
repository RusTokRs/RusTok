use std::env;

use rustok_artifact_node_transport::{
    ModuleArtifactNodeReconciliationIdentity, StaticModuleArtifactNodeReconciliationAuthenticator,
};
use rustok_worker_transport::PeerCertificateFingerprint;
use serde::Deserialize;
use uuid::Uuid;

const DATABASE_URL_ENV: &str = "RUSTOK_ARTIFACT_NODE_RECONCILER_DATABASE_URL";
const OPERATOR_IDENTITIES_JSON_ENV: &str =
    "RUSTOK_ARTIFACT_NODE_RECONCILER_OPERATOR_IDENTITIES_JSON";

/// Deployment-owned reconciler configuration that is independent of the
/// common mTLS listener. It deliberately contains certificate-bound operator
/// scope only; topology, release, policy, and installation values arrive in
/// the authenticated request and are revalidated by the durable owner.
pub struct ArtifactNodeReconcilerConfig {
    pub database_url: String,
    pub operator_authenticator: StaticModuleArtifactNodeReconciliationAuthenticator,
}

impl ArtifactNodeReconcilerConfig {
    pub fn from_environment() -> Result<Self, String> {
        let database_url = required_nonempty_env(DATABASE_URL_ENV)?;
        let operator_authenticator =
            parse_operator_authenticator(&required_nonempty_env(OPERATOR_IDENTITIES_JSON_ENV)?)?;
        Ok(Self {
            database_url,
            operator_authenticator,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OperatorIdentityConfig {
    certificate_fingerprint: String,
    actor_id: Uuid,
    allowed_node_ids: Vec<Uuid>,
}

fn parse_operator_authenticator(
    value: &str,
) -> Result<StaticModuleArtifactNodeReconciliationAuthenticator, String> {
    let entries: Vec<OperatorIdentityConfig> = serde_json::from_str(value).map_err(|_| {
        "artifact node reconciler operator identity configuration is invalid".to_string()
    })?;
    let identities = entries
        .into_iter()
        .map(|entry| {
            Ok((
                PeerCertificateFingerprint::parse(entry.certificate_fingerprint)?,
                ModuleArtifactNodeReconciliationIdentity::new(
                    entry.actor_id,
                    entry.allowed_node_ids,
                )?,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    StaticModuleArtifactNodeReconciliationAuthenticator::new(identities)
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
    use super::parse_operator_authenticator;

    #[test]
    fn operator_identity_map_requires_canonical_scoped_principals() {
        assert!(parse_operator_authenticator("[]").is_err());
        assert!(parse_operator_authenticator(
            r#"[{"certificate_fingerprint":"sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","actor_id":"d7d5eaef-47f1-49d8-96c4-60f25b714d40","allowed_node_ids":["466c597d-2112-433a-b341-fd43125f4675"]}]"#
        )
        .is_err());
        assert!(parse_operator_authenticator(
            r#"[{"certificate_fingerprint":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","actor_id":"00000000-0000-0000-0000-000000000000","allowed_node_ids":["466c597d-2112-433a-b341-fd43125f4675"]}]"#
        )
        .is_err());
        assert!(parse_operator_authenticator(
            r#"[{"certificate_fingerprint":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","actor_id":"d7d5eaef-47f1-49d8-96c4-60f25b714d40","allowed_node_ids":[]}]"#
        )
        .is_err());
        assert!(parse_operator_authenticator(
            r#"[{"certificate_fingerprint":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","actor_id":"d7d5eaef-47f1-49d8-96c4-60f25b714d40","allowed_node_ids":["466c597d-2112-433a-b341-fd43125f4675","466c597d-2112-433a-b341-fd43125f4675"]}]"#
        )
        .is_err());
    }

    #[test]
    fn operator_identity_map_rejects_duplicate_fingerprints_and_unknown_fields() {
        let duplicate = r#"[
            {"certificate_fingerprint":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","actor_id":"d7d5eaef-47f1-49d8-96c4-60f25b714d40","allowed_node_ids":["466c597d-2112-433a-b341-fd43125f4675"]},
            {"certificate_fingerprint":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","actor_id":"466c597d-2112-433a-b341-fd43125f4675","allowed_node_ids":["d7d5eaef-47f1-49d8-96c4-60f25b714d40"]}
        ]"#;
        assert!(parse_operator_authenticator(duplicate).is_err());
        assert!(parse_operator_authenticator(
            r#"[{"certificate_fingerprint":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","actor_id":"d7d5eaef-47f1-49d8-96c4-60f25b714d40","allowed_node_ids":["466c597d-2112-433a-b341-fd43125f4675"],"unexpected":true}]"#
        )
        .is_err());
    }
}
