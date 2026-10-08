use rustok_cli_core::{
    CliCoreError, CliCoreResult, CommandDescriptor, CommandOutcome, CommandProvider, CommandRequest,
};
use rustok_installer::{
    AdminBootstrap, DatabaseConfig, DatabaseEngine, InstallApplyOptions, InstallEnvironment,
    InstallPlan, InstallProfile, InstallTopology, InstallTopologyMode, ModuleSelection, SecretMode,
    SecretRef, SecretValue, SeedExecutionRequest, SeedProfile, SeedTenantRequest, SeedUserRequest,
    TenantBootstrap, bind_instance_placement, execute_install_apply, execute_seed_profile,
    load_base_distribution_receipt, redact_install_plan,
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
            CommandDescriptor::new("seed", "apply", "Apply a typed tenant seed profile")
                .with_dry_run(),
            CommandDescriptor::new(
                "starter",
                "import",
                "Import a starter blueprint package into a tenant",
            )
            .with_dry_run(),
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
            )
            .with_dry_run(),
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
            ("starter", "import") => {
                return self
                    .starter_import_command(&request.args, request.dry_run)
                    .await;
            }
            ("seed", "apply") => {}
            _ => {
                return Err(CliCoreError::UnknownCommand {
                    namespace: request.namespace,
                    name: request.name,
                });
            }
        }
        let options = &request.args["options"];
        let seed_environment = option(options, "environment")
            .as_deref()
            .map(InstallEnvironment::parse_cli_value)
            .transpose()
            .map_err(input)?
            .ok_or_else(|| {
                input("seed apply requires an explicit --environment (local, demo, or test)")
            })?;
        if seed_environment.is_production() {
            return Err(input(
                "seed apply is not allowed for production installations; use install apply",
            ));
        }

        if request.dry_run {
            let profile = option(options, "profile")
                .as_deref()
                .map(SeedProfile::parse_cli_value)
                .transpose()
                .map_err(input)?
                .unwrap_or(SeedProfile::Dev);
            return Ok(CommandOutcome::success(
                "Seed profile validated; dry run does not mutate state.",
            )
            .with_data(serde_json::json!({
                "environment": seed_environment.as_str(),
                "profile": profile,
            })));
        }

        let db = db_clone(
            self.runtime
                .require_host()
                .map_err(|error| failed(error.to_string()))?,
        );
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
        let demo_customer_password = if profile == SeedProfile::Dev {
            Some(
                option(options, "demo_customer_password")
                    .or_else(|| environment("SEED_DEMO_CUSTOMER_PASSWORD"))
                    .or_else(|| environment("DEMO_CUSTOMER_PASSWORD"))
                    .ok_or_else(|| {
                        input(
                            "dev seed apply requires --demo-customer-password, SEED_DEMO_CUSTOMER_PASSWORD, or DEMO_CUSTOMER_PASSWORD",
                        )
                    })?,
            )
        } else {
            None
        };
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
        let ports = SeaOrmInstallerBootstrapPorts::new(
            db.clone(),
            &registry,
            profile.default_enabled_modules(),
        );
        let result = execute_seed_profile(
            SeedExecutionRequest {
                profile,
                tenant,
                enabled_modules: profile.default_enabled_modules(),
                disabled_modules: Vec::new(),
                admin,
                demo_customer_password,
                actor: "rustok-cli seed apply".to_string(),
            },
            &ports,
            &ports,
            &ports,
        )
        .await
        .map_err(failed)?;

        let starter_name = option(options, "starter");
        let starter_report = if let Some(starter_name) = starter_name {
            let blueprint = if starter_name == "default" {
                rustok_starter::default_starter()
            } else {
                return Err(input(format!(
                    "Unknown starter blueprint: {}",
                    starter_name
                )));
            };
            let event_bus = rustok_outbox::TransactionalEventBus::new(std::sync::Arc::new(
                rustok_outbox::OutboxTransport::new(db.clone()),
            ));
            let engine = rustok_starter::StarterEngine::new(db, event_bus);
            let security = rustok_core::SecurityContext::system();
            let report = engine
                .import_blueprint(result.tenant.id, &security, &blueprint)
                .await
                .map_err(failed)?;
            Some(report)
        } else {
            None
        };

        Ok(
            CommandOutcome::success("Seed profile applied").with_data(serde_json::json!({
                "tenant_id": result.tenant.id,
                "tenant_slug": result.tenant.slug,
                "enabled_modules": result.enabled_modules,
                "starter": starter_report,
            })),
        )
    }
}

impl InstallerCommandProvider {
    async fn install_apply_command(
        &self,
        args: &serde_json::Value,
        dry_run: bool,
    ) -> CliCoreResult<CommandOutcome> {
        let plan = parse_install_plan(args)?;
        let report = rustok_installer::evaluate_preflight_with_deployment(&plan, false);
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

    async fn starter_import_command(
        &self,
        args: &serde_json::Value,
        dry_run: bool,
    ) -> CliCoreResult<CommandOutcome> {
        let options = &args["options"];
        let tenant_slug = option(options, "tenant_slug")
            .or_else(|| option(options, "tenant"))
            .unwrap_or_else(|| "demo".to_string());
        let starter_name = option(options, "blueprint")
            .or_else(|| option(options, "starter"))
            .or_else(|| option(options, "name"))
            .unwrap_or_else(|| "default".to_string());
        let file_path = option(options, "file");

        let blueprint = if let Some(path) = file_path {
            let content = std::fs::read_to_string(&path).map_err(|err| {
                failed(format!("Failed to read blueprint file `{}`: {}", path, err))
            })?;
            serde_json::from_str::<rustok_starter::StarterBlueprint>(&content).map_err(|err| {
                failed(format!(
                    "Failed to parse blueprint JSON `{}`: {}",
                    path, err
                ))
            })?
        } else if starter_name == "default" {
            rustok_starter::default_starter()
        } else {
            return Err(input(format!(
                "Unknown starter blueprint: {}",
                starter_name
            )));
        };

        if dry_run {
            return Ok(CommandOutcome::success(
                "Starter blueprint validated; dry run does not mutate state.",
            )
            .with_data(serde_json::json!({
                "tenant_slug": tenant_slug,
                "blueprint_id": blueprint.id,
                "blueprint_name": blueprint.name,
                "locale": blueprint.locale,
                "dry_run": true,
            })));
        }

        let db = db_clone(
            self.runtime
                .require_host()
                .map_err(|error| failed(error.to_string()))?,
        );

        let tenant_service = rustok_tenant::services::TenantService::new(db.clone());
        let tenant = tenant_service
            .get_tenant_by_slug(&tenant_slug)
            .await
            .map_err(|err| failed(format!("Tenant `{}` not found: {}", tenant_slug, err)))?;

        let event_bus = rustok_outbox::TransactionalEventBus::new(std::sync::Arc::new(
            rustok_outbox::OutboxTransport::new(db.clone()),
        ));
        let engine = rustok_starter::StarterEngine::new(db, event_bus);
        let security = rustok_core::SecurityContext::system();

        let report = engine
            .import_blueprint(tenant.id, &security, &blueprint)
            .await
            .map_err(failed)?;

        Ok(
            CommandOutcome::success("Starter blueprint imported successfully")
                .with_data(serde_json::to_value(&report).map_err(failed)?),
        )
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
    let report = rustok_installer::evaluate_preflight_with_deployment(&plan, false);
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
        bootstrap_public_key_base64: option(options, "base_distribution_public_key")
            .or_else(|| environment("RUSTOK_INSTALL_BASE_DISTRIBUTION_PUBLIC_KEY")),
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
    let invocation_dir = std::env::current_dir()
        .map_err(|error| failed(format!("failed to resolve invocation directory: {error}")))?;
    let placement = bind_instance_placement(
        option(options, "root").ok_or_else(|| input("install command requires --root"))?,
        invocation_dir,
    )
    .map_err(|error| input(error.to_string()))?;
    let mut topology = InstallTopology::for_mode(topology_mode)
        .bind_composition(composition.revision.clone(), composition.hash.clone());
    let receipt_path = option(options, "base_distribution_receipt")
        .or_else(|| environment("RUSTOK_INSTALL_BASE_DISTRIBUTION_RECEIPT"));
    let receipt_public_key = option(options, "base_distribution_public_key")
        .or_else(|| environment("RUSTOK_INSTALL_BASE_DISTRIBUTION_PUBLIC_KEY"));
    let (Some(receipt_path), Some(receipt_public_key)) = (receipt_path, receipt_public_key) else {
        return Err(input(
            "install requires --base-distribution-receipt and --base-distribution-public-key",
        ));
    };
    let receipt =
        load_base_distribution_receipt(receipt_path, &receipt_public_key, chrono::Utc::now())
            .map_err(|_| input("base-distribution receipt could not be verified"))?;
    if receipt.payload().host_composition_revision != composition.revision
        || receipt.payload().host_composition_hash != composition.hash
    {
        return Err(input(
            "signed base-distribution receipt is not compatible with this installer executable",
        ));
    }
    topology = topology.bind_distribution(
        receipt
            .into_binding()
            .map_err(|_| input("base-distribution receipt could not be bound"))?,
    );
    Ok(InstallPlan {
        placement,
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
        topology,
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

    fn signed_base_distribution_receipt(
        now: chrono::DateTime<chrono::Utc>,
    ) -> (
        rustok_modules::ModuleStaticDistributionBootstrapReceipt,
        String,
    ) {
        use base64::Engine;
        use base64::engine::general_purpose::STANDARD;
        use ed25519_dalek::{Signer, SigningKey};
        use sha2::{Digest, Sha256};

        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let public_key = signing_key.verifying_key().to_bytes();
        let digest_fn = |c: char| format!("sha256:{}", c.to_string().repeat(64));
        let platform_source_digest = digest_fn('1');
        let items = Vec::new();
        let roles = vec![rustok_modules::ModuleStaticDistributionRoleArtifact {
            role: rustok_modules::ModuleStaticDistributionRole::Monolith,
            artifact_digest: digest_fn('2'),
        }];
        let role_set_digest =
            rustok_modules::ModuleStaticDistributionBuildEvidence::role_set_digest(&roles).unwrap();
        let preparation = rustok_modules::ModuleStaticDistributionBootstrapPreparation {
            composition_revision: 1,
            composition_digest: rustok_modules::module_static_distribution_composition_digest(
                &format!("cas://{platform_source_digest}"),
                &platform_source_digest,
                &digest_fn('3'),
                "x86_64-unknown-linux-gnu",
                &items,
            )
            .unwrap(),
            platform_source_reference: format!("cas://{platform_source_digest}"),
            platform_source_digest,
            toolchain_digest: digest_fn('3'),
            build_target: "x86_64-unknown-linux-gnu".to_string(),
            items,
            evidence: rustok_modules::ModuleStaticDistributionBuildEvidence {
                bundle_reference: format!("registry.example/rustok/base@{}", digest_fn('b')),
                bundle_root_digest: digest_fn('b'),
                role_set_digest,
                roles,
                sbom_reference: "oci://base/sbom".to_string(),
                sbom_digest: digest_fn('4'),
                provenance_reference: "oci://base/provenance".to_string(),
                provenance_digest: digest_fn('5'),
                signature_reference: "oci://base/signature".to_string(),
                signature_digest: digest_fn('6'),
                test_evidence_reference: "oci://base/tests".to_string(),
                test_evidence_digest: digest_fn('7'),
            },
            admission: rustok_modules::ModuleStaticDistributionReleaseAdmission {
                verifier_identity: "platform-bootstrap-signer".to_string(),
                policy_revision: "policy@1".to_string(),
                evidence_reference: "oci://base/admission".to_string(),
                evidence_digest: digest_fn('8'),
                signature_verified: true,
                provenance_verified: true,
                sbom_verified: true,
                test_evidence_verified: true,
                dependency_policy_verified: true,
            },
        };
        let composition = rustok_distribution::composition_identity();
        let mut hasher = Sha256::new();
        hasher.update(&public_key);
        let signer_key_digest = format!("sha256:{}", hex::encode(hasher.finalize()));

        let payload = rustok_modules::ModuleStaticDistributionBootstrapReceiptPayload {
            contract: rustok_modules::MODULE_STATIC_DISTRIBUTION_BOOTSTRAP_RECEIPT_CONTRACT
                .to_string(),
            preparation_id: uuid::Uuid::from_u128(1),
            distribution_release_id: uuid::Uuid::from_u128(2),
            host_composition_revision: composition.revision,
            host_composition_hash: composition.hash,
            preparation,
            migration_plan_digest: digest_fn('d'),
            data_contract_digest: digest_fn('e'),
            signer_key_digest,
            issued_at: now - chrono::Duration::minutes(1),
            expires_at: now + chrono::Duration::hours(1),
        };
        let signature =
            signing_key.sign(&rustok_api::manifest_hash::canonical_json_bytes(&payload).unwrap());
        (
            rustok_modules::ModuleStaticDistributionBootstrapReceipt {
                payload,
                signature: STANDARD.encode(signature.to_bytes()),
            },
            STANDARD.encode(public_key),
        )
    }

    fn write_test_receipt() -> (std::path::PathBuf, String) {
        let now = chrono::Utc::now();
        let (receipt, public_key) = signed_base_distribution_receipt(now);
        let path =
            std::env::temp_dir().join(format!("rustok-test-receipt-{}.json", uuid::Uuid::new_v4()));
        std::fs::write(&path, serde_json::to_vec(&receipt).unwrap()).unwrap();
        (path, public_key)
    }

    #[tokio::test]
    async fn seed_apply_requires_explicit_non_production_environment_in_dry_run() {
        let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
        let provider = command_provider(&runtime);

        let missing_environment = provider
            .execute(CommandRequest {
                namespace: "seed".to_string(),
                name: "apply".to_string(),
                args: serde_json::json!({
                    "options": {
                        "profile": "dev"
                    }
                }),
                dry_run: true,
            })
            .await
            .expect_err("seed apply must require explicit environment");
        assert!(
            missing_environment
                .to_string()
                .contains("explicit --environment")
        );

        let production = provider
            .execute(CommandRequest {
                namespace: "seed".to_string(),
                name: "apply".to_string(),
                args: serde_json::json!({
                    "options": {
                        "environment": "production",
                        "profile": "dev"
                    }
                }),
                dry_run: true,
            })
            .await
            .expect_err("seed apply must reject production");
        assert!(
            production
                .to_string()
                .contains("not allowed for production installations")
        );

        let local = provider
            .execute(CommandRequest {
                namespace: "seed".to_string(),
                name: "apply".to_string(),
                args: serde_json::json!({
                    "options": {
                        "environment": "local",
                        "profile": "dev"
                    }
                }),
                dry_run: true,
            })
            .await
            .expect("non-production seed dry run should validate");

        assert_eq!(local.exit_code, 0);
        assert_eq!(local.data["environment"], "local");
        assert_eq!(local.data["profile"], "dev");
    }

    #[tokio::test]
    async fn plan_command_redacts_plaintext_secrets_without_runtime_database() {
        let (receipt_path, receipt_public_key) = write_test_receipt();
        let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
        let provider = command_provider(&runtime);
        let outcome = provider
            .execute(CommandRequest {
                namespace: "install".to_string(),
                name: "plan".to_string(),
                args: serde_json::json!({
                    "options": {
                        "root": std::env::temp_dir().join(format!("rustok-cli-plan-{}", uuid::Uuid::new_v4())).display().to_string(),
                        "database_url": "postgres://rustok:secret@localhost/rustok",
                        "admin_password": "admin12345",
                        "base_distribution_receipt": receipt_path.display().to_string(),
                        "base_distribution_public_key": receipt_public_key
                    }
                }),
                dry_run: false,
            })
            .await
            .expect("plan command should not require a database runtime");

        let _ = std::fs::remove_file(&receipt_path);

        assert_eq!(outcome.exit_code, 0);
        assert!(!outcome.data.to_string().contains("admin12345"));
        assert!(!outcome.data.to_string().contains("rustok:secret"));
    }

    #[tokio::test]
    async fn apply_dry_run_uses_shared_preflight_without_runtime_database() {
        let (receipt_path, receipt_public_key) = write_test_receipt();
        let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
        let provider = command_provider(&runtime);
        let outcome = provider
            .execute(CommandRequest {
                namespace: "install".to_string(),
                name: "apply".to_string(),
                args: serde_json::json!({
                    "options": {
                        "root": std::env::temp_dir().join(format!("rustok-cli-dry-run-{}", uuid::Uuid::new_v4())).display().to_string(),
                        "database_url": "postgres://rustok:secret@localhost/rustok",
                        "admin_password": "admin12345",
                        "base_distribution_receipt": receipt_path.display().to_string(),
                        "base_distribution_public_key": receipt_public_key
                    }
                }),
                dry_run: true,
            })
            .await
            .expect("apply dry run should not require a database runtime");

        let _ = std::fs::remove_file(&receipt_path);

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.data["passed"], serde_json::json!(false));
        assert!(
            outcome.data["report"]["issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|issue| issue["code"] == "distribution_deployment_unavailable")
        );
        assert!(!outcome.data.to_string().contains("admin12345"));
        assert!(!outcome.data.to_string().contains("rustok:secret"));
    }

    #[tokio::test]
    async fn starter_import_dry_run_validates_default_blueprint_without_runtime_database() {
        let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
        let provider = command_provider(&runtime);
        let outcome = provider
            .execute(CommandRequest {
                namespace: "starter".to_string(),
                name: "import".to_string(),
                args: serde_json::json!({
                    "options": {
                        "tenant_slug": "demo",
                        "starter": "default"
                    }
                }),
                dry_run: true,
            })
            .await
            .expect("starter import dry run should not require a database runtime");

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.data["blueprint_id"], "default-starter");
        assert_eq!(outcome.data["dry_run"], true);
    }
}
