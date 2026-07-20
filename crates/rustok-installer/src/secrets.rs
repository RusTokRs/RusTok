use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::plan::InstallPlan;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretMode {
    Env,
    DotenvFile,
    MountedFile,
    ExternalSecret,
}

impl SecretMode {
    pub fn parse_cli_value(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "env" => Ok(Self::Env),
            "dotenv_file" | "dotenv-file" => Ok(Self::DotenvFile),
            "mounted_file" | "mounted-file" => Ok(Self::MountedFile),
            "external_secret" | "external-secret" => Ok(Self::ExternalSecret),
            _ => Err(format!("unknown secret mode `{value}`")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretRef {
    pub backend: String,
    pub key: String,
}

impl SecretRef {
    pub fn parse_cli_value(value: &str) -> Result<Self, String> {
        let (backend, key) = value
            .split_once(':')
            .ok_or_else(|| "secret ref must use `<backend>:<key>` format".to_string())?;
        if backend.trim().is_empty() || key.trim().is_empty() {
            return Err("secret ref must include non-empty backend and key".to_string());
        }
        Ok(Self {
            backend: backend.trim().to_string(),
            key: key.trim().to_string(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SecretValue {
    Plaintext { value: String },
    Reference { reference: SecretRef },
}

/// Resolves setup secrets from local operator-managed sources.
///
/// External secret-manager references remain explicit contracts until their
/// provider-specific resolver is supplied by an executable adapter.
pub fn resolve_local_secret_value(
    secret: &SecretValue,
    label: &str,
) -> Result<String, SecretResolutionError> {
    match secret {
        SecretValue::Plaintext { value } => Ok(value.clone()),
        SecretValue::Reference { reference } => resolve_local_secret_ref(reference, label),
    }
}

fn resolve_local_secret_ref(
    reference: &SecretRef,
    label: &str,
) -> Result<String, SecretResolutionError> {
    match normalize(&reference.backend).as_str() {
        "env" => std::env::var(&reference.key).map_err(|_| {
            SecretResolutionError::new(format!("{label} env secret `{}` is not set", reference.key))
        }),
        "file" | "mounted_file" | "mounted-file" => read_secret_file(&reference.key, label),
        "dotenv" | "dotenv_file" | "dotenv-file" => read_dotenv_secret(&reference.key, label),
        "external_secret" | "external-secret" | "vault" | "kubernetes" | "k8s" | "aws" | "gcp"
        | "azure" => Err(SecretResolutionError::new(format!(
            "{label} secret backend `{}` requires an external secret resolver, which is not implemented yet",
            reference.backend
        ))),
        backend => Err(SecretResolutionError::new(format!(
            "{label} secret backend `{backend}` is not supported by install apply"
        ))),
    }
}

fn read_secret_file(path: &str, label: &str) -> Result<String, SecretResolutionError> {
    let value = std::fs::read_to_string(path).map_err(|error| {
        SecretResolutionError::new(format!(
            "failed to read {label} secret file `{path}`: {error}"
        ))
    })?;
    let value = strip_secret_newline(value);
    if value.is_empty() {
        return Err(SecretResolutionError::new(format!(
            "{label} secret file `{path}` is empty"
        )));
    }
    Ok(value)
}

fn read_dotenv_secret(reference_key: &str, label: &str) -> Result<String, SecretResolutionError> {
    let (path, key) = reference_key
        .split_once('#')
        .unwrap_or((".env", reference_key));
    if path.trim().is_empty() || key.trim().is_empty() {
        return Err(SecretResolutionError::new(format!(
            "{label} dotenv secret ref must use `dotenv:<path>#<KEY>` or `dotenv:<KEY>`"
        )));
    }

    let contents = std::fs::read_to_string(path).map_err(|error| {
        SecretResolutionError::new(format!(
            "failed to read {label} dotenv file `{path}`: {error}"
        ))
    })?;
    let Some(value) = parse_dotenv_value(&contents, key.trim()) else {
        return Err(SecretResolutionError::new(format!(
            "{label} dotenv key `{}` was not found in `{path}`",
            key.trim()
        )));
    };
    if value.is_empty() {
        return Err(SecretResolutionError::new(format!(
            "{label} dotenv key `{}` in `{path}` is empty",
            key.trim()
        )));
    }
    Ok(value)
}

fn parse_dotenv_value(contents: &str, key: &str) -> Option<String> {
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
        let Some((candidate, value)) = line.split_once('=') else {
            continue;
        };
        if candidate.trim() == key {
            return Some(unquote_dotenv_value(value.trim()));
        }
    }
    None
}

fn unquote_dotenv_value(value: &str) -> String {
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        value[1..value.len() - 1].to_string()
    } else {
        strip_secret_newline(value.to_string())
    }
}

fn strip_secret_newline(mut value: String) -> String {
    while value.ends_with('\n') || value.ends_with('\r') {
        value.pop();
    }
    value
}

fn normalize(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[derive(Debug, Error)]
#[error("secret resolution failed: {message}")]
pub struct SecretResolutionError {
    message: String,
}

impl SecretResolutionError {
    fn new(message: String) -> Self {
        Self { message }
    }
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
    fn secret_contract_parsers_normalize_values_and_validate_references() {
        assert_eq!(
            SecretMode::parse_cli_value("mounted-file"),
            Ok(SecretMode::MountedFile)
        );
        assert_eq!(
            SecretRef::parse_cli_value("vault: secret/rustok/db"),
            Ok(SecretRef {
                backend: "vault".to_string(),
                key: "secret/rustok/db".to_string(),
            })
        );
        assert!(SecretRef::parse_cli_value("vault").is_err());
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
            topology: crate::InstallTopology::for_mode(crate::InstallTopologyMode::Monolith)
                .bind_composition("test".to_string(), "a".repeat(64)),
            seed_profile: crate::SeedProfile::Dev,
            secrets_mode: SecretMode::DotenvFile,
        };

        let redacted = redact_install_plan(&plan).to_string();

        assert!(!redacted.contains("admin12345"));
        assert!(!redacted.contains("rustok:rustok"));
        assert!(redacted.contains("***"));
    }

    #[test]
    fn local_secret_resolver_rejects_external_backends_explicitly() {
        let error = resolve_local_secret_value(
            &SecretValue::Reference {
                reference: SecretRef {
                    backend: "vault".to_string(),
                    key: "secret/rustok/db".to_string(),
                },
            },
            "database URL",
        )
        .expect_err("external resolvers are not built into the foundation");

        assert!(error.to_string().contains("external secret resolver"));
    }
}
