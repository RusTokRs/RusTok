use rustok_cli_core::{
    CliCoreError, CliCoreResult, CommandDescriptor, CommandOutcome, CommandProvider, CommandRequest,
};
use rustok_installer::{
    AdminBootstrap, DatabaseConfig, DatabaseEngine, InstallApplyOptions, InstallEnvironment,
    InstallPlan, InstallProfile, InstallTopology, InstallTopologyMode, ModuleSelection, SecretMode,
    SecretRef, SecretValue, SeedExecutionRequest, SeedProfile, SeedTenantRequest, SeedUserRequest,
    TenantBootstrap, evaluate_preflight, execute_install_apply, execute_seed_profile,
    redact_install_plan,
};
use rustok_installer_persistence::{
    InstallerPersistenceService, SeaOrmInstallerApplyPorts, SeaOrmInstallerBootstrapPorts,
};
use rustok_runtime::{RuntimeComposition, db_clone};

pub struct InstallerCommandProvider {
    runtime: RuntimeComposition,
}

pub fn command_provider(runtime: &RuntimeComposition) -> Box<dyn CommandProvider> {
    Box::new(InstallerCommandProvider {
        runtime: runtime.clone(),
    })
}

#[async_trait::async_trait]
impl CommandProvider for InstallerCommandProvider {
    fn commands(&self) -> Vec<CommandDescriptor> {
        vec![
            CommandDescriptor::new("seed", "apply", "Apply a typed tenant seed profile"),
            CommandDescriptor::new(
                "install",
                "plan",
                "Validate and render a redacted installer plan without database access",
            ),
            CommandDescriptor::new(
                "install",
                "preflight",
                "Validate installer policy without database access or mutation",
            ),
            CommandDescriptor::new(
                "install",
                "apply",
                "Apply the typed installer plan through the shared executor",
            ),
            CommandDescriptor::new(
                "install",
                "status",
                "Read the latest durable installer session",
            ),
        ]
    }

    async fn execute(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {
        match (request.namespace.as_str(), request.name.as_str()) {
            ("install", "plan") => return install_plan_command(&request.args),
            ("install", "preflight") => return install_preflight_command(&request.args),
            ("install", "apply") => {
                return self
                    .install_apply_command(&request.args, request.dry_run)
                    .await;
            }
            ("install", "status") => return self.install_status_command().await,
            ("seed", "apply") => {}
            _ => {
                return Err(CliCoreError::UnknownCommand {
                    namespace: request.namespace,
                    name: request.name,
                });
            }
        }
        if request.dry_run {
            return Ok(CommandOutcome::success(
                "Seed profile validated; dry run does not mutate state.",
            ));
        }
        let db = db_clone(
            self.runtime
                .require_host()
                .map_err(|error| failed(error.to_string()))?,
        );
        let options = &request.args["options"];
        let profile = option(options, "profile")
            .as_deref()
            .map(SeedProfile::parse_cli_value)
            .transpose()
            .map_err(input)?
            .unwrap_or(SeedProfile::Dev);
        let password = option(options, "password")
            .or_else(|| environment("SEED_ADMIN_PASSWORD"))
            .or_else(|| environment("SUPERADMIN_PASSWORD"))
            .ok_or_else(|| {
                input("seed apply requires --password, SEED_ADMIN_PASSWORD, or SUPERADMIN_PASSWORD")
            })?;
        let tenant = SeedTenantRequest {
            name: option(options, "tenant_name").unwrap_or_else(|| "Demo Workspace".to_string()),
            slug: option(options, "tenant_slug").unwrap_or_else(|| "demo".to_string()),
            domain: option(options, "tenant_domain"),
        };
        let admin = Some(SeedUserRequest {
            tenant_id: uuid::Uuid::nil(),
            email: option(options, "email").unwrap_or_else(|| "admin@demo.local".to_string()),
            name: option(options, "name").unwrap_or_else(|| "Super Admin".to_string()),
            password: password.clone(),
        });
        let registry = rustok_distribution::build_registry();
        let ports =
            SeaOrmInstallerBootstrapPorts::new(db, &registry, profile.default_enabled_modules());
        let result = execute_seed_profile(
            SeedExecutionRequest {
                profile,
                tenant,
                enabled_modules: profile.default_enabled_modules(),
                disabled_modules: Vec::new(),
                admin,
                demo_customer_password: Some(password),
                actor: "rustok-cli seed apply".to_string(),
            },
            &ports,
            &ports,
            &ports,
        )
        .await
        .map_err(failed)?;
        Ok(CommandOutcome::success("Seed profile applied").with_data(serde_json::json!({ "tenant_id": result.tenant.id, "tenant_slug": result.tenant.slug, "enabled_modules": result.enabled_modules })))
    }
}

impl InstallerCommandProvider {
    async fn install_apply_command(
        &self,
        args: &serde_json::Value,
        dry_run: bool,
    ) -> CliCoreResult<CommandOutcome> {
        let plan = parse_install_plan(args)?;
        let report = evaluate_preflight(&plan);
        if dry_run {
            return Ok(
                CommandOutcome::success("Installer apply dry run completed").with_data(
                    serde_json::json!({
                        "passed": report.passed(),
                        "report": report,
                        "redacted_plan": redact_install_plan(&plan),
                    }),
                ),
            );
        }
        if !report.passed() {
            return Err(failed("installer preflight failed"));
        }

        let registry = rustok_distribution::build_registry();
        let ports = SeaOrmInstallerApplyPorts::new(&registry);
        let output = execute_install_apply(&ports, plan, parse_apply_options(args)?)
            .await
            .map_err(failed)?;
        Ok(CommandOutcome::success("Installer apply completed")
            .with_data(serde_json::to_value(output).map_err(failed)?))
    }

    async fn install_status_command(&self) -> CliCoreResult<CommandOutcome> {
        let db = db_clone(
            self.runtime
                .require_host()
                .map_err(|error| failed(error.to_string()))?,
        );
        let session = InstallerPersistenceService::new(db)
            .latest_session()
            .await
            .map_err(failed)?;
        match session {
            Some(session) => Ok(
                CommandOutcome::success("Installer status collected").with_data(
                    serde_json::json!({
                        "initialized": true,
                        "session": session,
                    }),
                ),
            ),
            None => Ok(
                CommandOutcome::success("Installer has not started").with_data(serde_json::json!({
                    "initialized": false,
                    "session": null,
                })),
            ),
        }
    }
}

fn option(options: &serde_json::Value, name: &str) -> Option<String> {
    options
        .get(name)
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
        .filter(|value| !value.trim().is_empty())
}

fn install_plan_command(args: &serde_json::Value) -> CliCoreResult<CommandOutcome> {
    let plan = parse_install_plan(args)?;
    Ok(CommandOutcome::success("Installer plan validated")
        .with_data(serde_json::json!({ "redacted_plan": redact_install_plan(&plan) })))
}

fn install_preflight_command(args: &serde_json::Value) -> CliCoreResult<CommandOutcome> {
    let plan = parse_install_plan(args)?;
    let report = evaluate_preflight(&plan);
    let message = if report.passed() {
        "Installer preflight passed"
    } else {
        "Installer preflight failed"
    };
    Ok(
        CommandOutcome::success(message).with_data(serde_json::json!({
            "passed": report.passed(),
            "report": report,
            "redacted_plan": redact_install_plan(&plan),
        })),
    )
}

fn parse_apply_options(args: &serde_json::Value) -> Result<InstallApplyOptions, CliCoreError> {
    let options = &args["options"];
    let lock_ttl_secs = option(options, "lock_ttl_secs")
        .map(|value| {
            value
                .parse::<i64>()
                .map_err(|_| input("--lock-ttl-secs must be a positive integer number of seconds"))
        })
        .transpose()?
        .unwrap_or(InstallApplyOptions::default().lock_ttl_secs);
    if lock_ttl_secs < 1 {
        return Err(input(
            "--lock-ttl-secs must be a positive integer number of seconds",
        ));
    }
    Ok(InstallApplyOptions {
        lock_owner: option(options, "lock_owner")
            .unwrap_or_else(|| "rustok-cli install apply".to_string()),
        lock_ttl_secs,
        pg_admin_url: option(options, "pg_admin_url"),
    })
}

fn parse_install_plan(args: &serde_json::Value) -> Result<InstallPlan, CliCoreError> {
    let options = &args["options"];
    let database_url = secret_option(options, "database_url", "database_secret_ref")?;
    let admin_password = secret_option(options, "admin_password", "admin_password_ref")?;
    let topology_mode = option(options, "topology")
        .as_deref()
        .map(InstallTopologyMode::parse_cli_value)
        .transpose()
        .map_err(input)?
        .unwrap_or(InstallTopologyMode::Monolith);
    let composition = rustok_distribution::composition_identity();
    Ok(InstallPlan {
        environment: option(options, "environment")
            .as_deref()
            .map(InstallEnvironment::parse_cli_value)
            .transpose()
            .map_err(input)?
            .unwrap_or(InstallEnvironment::Local),
        profile: option(options, "profile")
            .as_deref()
            .map(InstallProfile::parse_cli_value)
            .transpose()
            .map_err(input)?
            .unwrap_or(InstallProfile::DevLocal),
        database: DatabaseConfig {
            engine: option(options, "database_engine")
                .as_deref()
                .map(DatabaseEngine::parse_cli_value)
                .transpose()
                .map_err(input)?
                .unwrap_or(DatabaseEngine::Postgres),
            url: database_url,
            create_if_missing: options["create_database"].as_bool().unwrap_or(false),
        },
        tenant: TenantBootstrap {
            slug: option(options, "tenant_slug").unwrap_or_else(|| "demo".to_string()),
            name: option(options, "tenant_name").unwrap_or_else(|| "Demo Workspace".to_string()),
        },
        admin: AdminBootstrap {
            email: option(options, "admin_email").unwrap_or_else(|| "admin@local".to_string()),
            password: admin_password,
        },
        modules: ModuleSelection {
            enable: csv_option(options, "enable_modules"),
            disable: csv_option(options, "disable_modules"),
        },
        topology: InstallTopology::for_mode(topology_mode)
            .bind_composition(composition.revision, composition.hash),
        seed_profile: option(options, "seed_profile")
            .as_deref()
            .map(SeedProfile::parse_cli_value)
            .transpose()
            .map_err(input)?
            .unwrap_or(SeedProfile::Dev),
        secrets_mode: option(options, "secrets_mode")
            .as_deref()
            .map(SecretMode::parse_cli_value)
            .transpose()
            .map_err(input)?
            .unwrap_or(SecretMode::Env),
    })
}

fn secret_option(
    options: &serde_json::Value,
    plaintext_name: &str,
    reference_name: &str,
) -> Result<SecretValue, CliCoreError> {
    if let Some(value) = option(options, plaintext_name) {
        return Ok(SecretValue::Plaintext { value });
    }
    if let Some(value) = option(options, reference_name) {
        return SecretRef::parse_cli_value(&value)
            .map(|reference| SecretValue::Reference { reference })
            .map_err(input);
    }
    Err(input(format!(
        "install command requires --{plaintext_name} or --{reference_name}"
    )))
}

fn csv_option(options: &serde_json::Value, name: &str) -> Vec<String> {
    option(options, name)
        .into_iter()
        .flat_map(|value| {
            value
                .split(',')
                .map(str::trim)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|value| !value.is_empty())
        .collect()
}
fn environment(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}
fn input(message: impl Into<String>) -> CliCoreError {
    CliCoreError::InvalidInput {
        message: message.into(),
    }
}
fn failed(error: impl std::fmt::Display) -> CliCoreError {
    CliCoreError::CommandFailed {
        message: error.to_string(),
    }
}
#[cfg(test)]
mod tests {
    use super::{RuntimeComposition, command_provider};
    use rustok_cli_core::CommandRequest;

    #[tokio::test]
    async fn plan_command_redacts_plaintext_secrets_without_runtime_database() {
        let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
        let provider = command_provider(&runtime);
        let outcome = provider
            .execute(CommandRequest {
                namespace: "install".to_string(),
                name: "plan".to_string(),
                args: serde_json::json!({
                    "options": {
                        "database_url": "postgres://rustok:secret@localhost/rustok",
                        "admin_password": "admin12345"
                    }
                }),
                dry_run: false,
            })
            .await
            .expect("plan command should not require a database runtime");

        assert_eq!(outcome.exit_code, 0);
        assert!(!outcome.data.to_string().contains("admin12345"));
        assert!(!outcome.data.to_string().contains("rustok:secret"));
    }

    #[tokio::test]
    async fn apply_dry_run_uses_shared_preflight_without_runtime_database() {
        let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
        let provider = command_provider(&runtime);
        let outcome = provider
            .execute(CommandRequest {
                namespace: "install".to_string(),
                name: "apply".to_string(),
                args: serde_json::json!({
                    "options": {
                        "database_url": "postgres://rustok:secret@localhost/rustok",
                        "admin_password": "admin12345"
                    }
                }),
                dry_run: true,
            })
            .await
            .expect("apply dry run should not require a database runtime");

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.data["passed"], serde_json::json!(true));
        assert!(!outcome.data.to_string().contains("admin12345"));
        assert!(!outcome.data.to_string().contains("rustok:secret"));
    }
}
