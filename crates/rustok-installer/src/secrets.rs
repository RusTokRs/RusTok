use serde::{Deserialize, Serialize};

use crate::plan::InstallPlan;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretMode {
    Env,
    DotenvFile,
    MountedFile,
    ExternalSecret,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretRef {
    pub backend: String,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SecretValue {
    Plaintext { value: String },
    Reference { reference: SecretRef },
}

impl SecretValue {
    pub fn as_plaintext(&self) -> Option<&str> {
        match self {
            Self::Plaintext { value } => Some(value.as_str()),
            Self::Reference { .. } => None,
        }
    }
}

pub fn redact_secret(value: &SecretValue) -> serde_json::Value {
    match value {
        SecretValue::Plaintext { .. } => serde_json::json!({
            "kind": "redacted",
            "value": "***"
        }),
        SecretValue::Reference { reference } => serde_json::json!({
            "kind": "reference",
            "backend": reference.backend,
            "key": reference.key
        }),
    }
}

pub fn redact_install_plan(plan: &InstallPlan) -> serde_json::Value {
    serde_json::json!({
        "environment": plan.environment,
        "profile": plan.profile,
        "database": {
            "engine": plan.database.engine,
            "url": redact_secret(&plan.database.url),
            "create_if_missing": plan.database.create_if_missing,
        },
        "tenant": plan.tenant,
        "admin": {
            "email": plan.admin.email,
            "password": redact_secret(&plan.admin.password),
        },
        "modules": plan.modules,
        "seed_profile": plan.seed_profile,
        "secrets_mode": plan.secrets_mode,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plaintext_secret_is_redacted() {
        let redacted = redact_secret(&SecretValue::Plaintext {
            value: "admin12345".to_string(),
        });

        assert_eq!(redacted["value"], "***");
        assert!(!redacted.to_string().contains("admin12345"));
    }

    #[test]
    fn reference_secret_keeps_reference_identity() {
        let redacted = redact_secret(&SecretValue::Reference {
            reference: SecretRef {
                backend: "vault".to_string(),
                key: "secret/rustok/db".to_string(),
            },
        });

        assert_eq!(redacted["backend"], "vault");
        assert_eq!(redacted["key"], "secret/rustok/db");
    }

    #[test]
    fn install_plan_redaction_removes_plaintext_values() {
        let plan = InstallPlan {
            environment: crate::InstallEnvironment::Local,
            profile: crate::InstallProfile::DevLocal,
            database: crate::DatabaseConfig {
                engine: crate::DatabaseEngine::Postgres,
                url: SecretValue::Plaintext {
                    value: "postgres://rustok:rustok@localhost/rustok".to_string(),
                },
                create_if_missing: true,
            },
            tenant: crate::TenantBootstrap {
                slug: "demo".to_string(),
                name: "Demo".to_string(),
            },
            admin: crate::AdminBootstrap {
                email: "admin@local".to_string(),
                password: SecretValue::Plaintext {
                    value: "admin12345".to_string(),
                },
            },
            modules: crate::ModuleSelection::default(),
            seed_profile: crate::SeedProfile::Dev,
            secrets_mode: SecretMode::DotenvFile,
        };

        let redacted = redact_install_plan(&plan).to_string();

        assert!(!redacted.contains("admin12345"));
        assert!(!redacted.contains("rustok:rustok"));
        assert!(redacted.contains("***"));
    }
}
