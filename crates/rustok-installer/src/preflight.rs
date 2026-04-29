use serde::{Deserialize, Serialize};

use crate::plan::{DatabaseEngine, InstallEnvironment, InstallPlan};
use crate::secrets::SecretMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreflightSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreflightIssue {
    pub severity: PreflightSeverity,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreflightReport {
    pub issues: Vec<PreflightIssue>,
}

impl PreflightReport {
    pub fn passed(&self) -> bool {
        !self
            .issues
            .iter()
            .any(|issue| issue.severity == PreflightSeverity::Error)
    }
}

pub fn evaluate_preflight(plan: &InstallPlan) -> PreflightReport {
    let mut issues = Vec::new();

    if plan.environment == InstallEnvironment::Production
        && plan.database.engine != DatabaseEngine::Postgres
    {
        issues.push(error(
            "production_database_engine",
            "Production installer runs must use PostgreSQL explicitly.",
        ));
    }

    if plan.database.engine == DatabaseEngine::Sqlite && plan.environment.is_production() {
        issues.push(error(
            "sqlite_production",
            "SQLite is allowed only for local, demo, and test installer runs.",
        ));
    }

    if plan.environment.is_production() && plan.secrets_mode == SecretMode::DotenvFile {
        issues.push(error(
            "dotenv_production",
            "dotenv-file secret mode is local/dev only and cannot be used for production installs.",
        ));
    }

    for (name, secret) in [
        ("database.url", &plan.database.url),
        ("admin.password", &plan.admin.password),
    ] {
        if let Some(value) = secret.as_plaintext() {
            if plan.environment.is_production() {
                issues.push(error(
                    "plaintext_secret_production",
                    &format!(
                        "{name} is plaintext; production installs must use a secret reference."
                    ),
                ));
            }

            if let Some(sample) = known_sample_secret(value) {
                let message = format!("{name} contains known sample secret `{sample}`.");
                if plan.environment.is_production() {
                    issues.push(error("sample_secret", &message));
                } else {
                    issues.push(warning("sample_secret", &message));
                }
            }
        }
    }

    if !plan.modules.disable.is_empty() {
        issues.push(warning(
            "schema_disable_not_supported",
            "Disabled modules affect tenant enablement intent only; v1 does not remove module-owned schema from the global migrator.",
        ));
    }

    PreflightReport { issues }
}

fn error(code: &str, message: &str) -> PreflightIssue {
    PreflightIssue {
        severity: PreflightSeverity::Error,
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn warning(code: &str, message: &str) -> PreflightIssue {
    PreflightIssue {
        severity: PreflightSeverity::Warning,
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn known_sample_secret(value: &str) -> Option<&'static str> {
    const SAMPLES: &[&str] = &[
        "admin12345",
        "change-me-in-production",
        "dev-password-123",
        "rustok:rustok",
        "postgres:postgres",
        "dev_secret_change_in_production",
    ];

    SAMPLES
        .iter()
        .copied()
        .find(|sample| value.contains(sample))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AdminBootstrap, DatabaseConfig, InstallProfile, ModuleSelection, SecretRef, SecretValue,
        SeedProfile, TenantBootstrap,
    };

    fn production_plan(
        database_engine: DatabaseEngine,
        admin_password: SecretValue,
    ) -> InstallPlan {
        InstallPlan {
            environment: InstallEnvironment::Production,
            profile: InstallProfile::Monolith,
            database: DatabaseConfig {
                engine: database_engine,
                url: SecretValue::Reference {
                    reference: SecretRef {
                        backend: "vault".to_string(),
                        key: "db".to_string(),
                    },
                },
                create_if_missing: false,
            },
            tenant: TenantBootstrap {
                slug: "default".to_string(),
                name: "Default".to_string(),
            },
            admin: AdminBootstrap {
                email: "admin@example.com".to_string(),
                password: admin_password,
            },
            modules: ModuleSelection::default(),
            seed_profile: SeedProfile::Minimal,
            secrets_mode: SecretMode::ExternalSecret,
        }
    }

    #[test]
    fn production_sqlite_fails_preflight() {
        let report = evaluate_preflight(&production_plan(
            DatabaseEngine::Sqlite,
            SecretValue::Reference {
                reference: SecretRef {
                    backend: "vault".to_string(),
                    key: "admin".to_string(),
                },
            },
        ));

        assert!(!report.passed());
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "production_database_engine"));
    }

    #[test]
    fn production_plaintext_secret_fails_preflight() {
        let report = evaluate_preflight(&production_plan(
            DatabaseEngine::Postgres,
            SecretValue::Plaintext {
                value: "strong-but-plaintext".to_string(),
            },
        ));

        assert!(!report.passed());
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "plaintext_secret_production"));
    }

    #[test]
    fn module_disable_warns_without_failing() {
        let mut plan = production_plan(
            DatabaseEngine::Postgres,
            SecretValue::Reference {
                reference: SecretRef {
                    backend: "vault".to_string(),
                    key: "admin".to_string(),
                },
            },
        );
        plan.modules.disable.push("commerce".to_string());

        let report = evaluate_preflight(&plan);

        assert!(report.passed());
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "schema_disable_not_supported"
                && issue.severity == PreflightSeverity::Warning));
    }

    #[test]
    fn local_sample_secret_warns_without_failing() {
        let plan = InstallPlan {
            environment: InstallEnvironment::Local,
            profile: InstallProfile::DevLocal,
            database: DatabaseConfig {
                engine: DatabaseEngine::Postgres,
                url: SecretValue::Plaintext {
                    value: "postgres://rustok:rustok@localhost/rustok".to_string(),
                },
                create_if_missing: true,
            },
            tenant: TenantBootstrap {
                slug: "demo".to_string(),
                name: "Demo".to_string(),
            },
            admin: AdminBootstrap {
                email: "admin@local".to_string(),
                password: SecretValue::Plaintext {
                    value: "admin12345".to_string(),
                },
            },
            modules: ModuleSelection::default(),
            seed_profile: SeedProfile::Dev,
            secrets_mode: SecretMode::DotenvFile,
        };

        let report = evaluate_preflight(&plan);

        assert!(report.passed());
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "sample_secret"
                && issue.severity == PreflightSeverity::Warning));
    }
}
