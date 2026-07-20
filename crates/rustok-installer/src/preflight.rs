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
    evaluate_preflight_with_deployment(plan, false)
}

/// Evaluates installer policy with the deployment capability selected by a host.
///
/// A plan alone cannot prove whether a host can build and activate distributed
/// roles, so CLI and HTTP adapters must supply that capability explicitly.
pub fn evaluate_preflight_with_deployment(
    plan: &InstallPlan,
    distributed_deployment_available: bool,
) -> PreflightReport {
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

    if let Err(message) = plan.topology.validate() {
        issues.push(error("invalid_topology", &message));
    }
    if plan.topology.mode == crate::InstallTopologyMode::Distributed
        && !distributed_deployment_available
    {
        issues.push(error(
            "distributed_topology_unavailable",
            "Distributed topology requires a deployment adapter and is not available for apply yet.",
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
        AdminBootstrap, DatabaseConfig, InstallProfile, InstallTopology, InstallTopologyMode,
        ModuleSelection, SecretRef, SecretValue, SeedProfile, TenantBootstrap,
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
            topology: bound_topology(),
            seed_profile: SeedProfile::Minimal,
            secrets_mode: SecretMode::ExternalSecret,
        }
    }

    fn bound_topology() -> InstallTopology {
        InstallTopology::for_mode(InstallTopologyMode::Monolith)
            .bind_composition("test".to_string(), "a".repeat(64))
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
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "production_database_engine")
        );
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
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "plaintext_secret_production")
        );
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
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "schema_disable_not_supported"
                    && issue.severity == PreflightSeverity::Warning)
        );
    }

    #[test]
    fn distributed_topology_requires_a_deployment_adapter() {
        let mut plan = production_plan(
            DatabaseEngine::Postgres,
            SecretValue::Reference {
                reference: SecretRef {
                    backend: "vault".to_string(),
                    key: "admin".to_string(),
                },
            },
        );
        plan.topology = InstallTopology::for_mode(InstallTopologyMode::Distributed)
            .bind_composition("distribution@1".to_string(), "a".repeat(64));

        let report = evaluate_preflight(&plan);

        assert!(!report.passed());
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "distributed_topology_unavailable")
        );

        let adapter_report = evaluate_preflight_with_deployment(&plan, true);
        assert!(adapter_report.passed());
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
            topology: bound_topology(),
            seed_profile: SeedProfile::Dev,
            secrets_mode: SecretMode::DotenvFile,
        };

        let report = evaluate_preflight(&plan);

        assert!(report.passed());
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "sample_secret"
                    && issue.severity == PreflightSeverity::Warning)
        );
    }
}
