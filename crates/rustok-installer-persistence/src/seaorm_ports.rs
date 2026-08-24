use rustok_auth::{AuthUserBootstrapDbWriter, AuthUserBootstrapRequest};
use rustok_installer::{
    DatabaseEngine, InstallAdminOutcome, InstallAdminPort, InstallApplyOptions,
    InstallBootstrapPort, InstallDatabasePort, InstallDatabaseReady, InstallDeploymentPort,
    InstallDistributionDeployment, InstallDistributionDeploymentRequest, InstallExecutionError,
    InstallPersistencePort, InstallPlan, InstallReceipt, InstallReceiptRecord, InstallSchemaPort,
    InstallSeedOutcome, InstallSeedPort, InstallSessionRecord, InstallState,
    InstallVerificationOutcome, InstallVerificationPort, SeedExecutionError, SeedExecutionRequest,
    SeedIdentityPort, SeedModulePort, SeedPrincipalPort, SeedProfile, SeedRolePort, SeedTenant,
    SeedTenantPort, SeedTenantRequest, SeedUser, SeedUserRequest,
};
use rustok_modules::{ModuleControlPlane, ModuleLifecycleToggleCommand};
use rustok_rbac::RbacRoleAssignmentDbWriter;
use rustok_tenant::{
    CreateTenantInput, PortActor, PortContext, TenantReadPort, TenantReadRequest,
    TenantReadSelector, TenantService,
};
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, TransactionTrait,
};
use sha2::{Digest, Sha256};
use std::time::Duration;
use url::Url;
use uuid::Uuid;

use crate::{InstallerPersistenceService, entities::install_session};

const DEFAULT_PG_ADMIN_URL: &str = "postgres://postgres:postgres@localhost:5432/postgres";

async fn import_base_distribution(
    runtime: &DatabaseConnection,
    plan: &InstallPlan,
    public_key_base64: Option<&str>,
) -> Result<
    Option<rustok_modules::ModuleStaticDistributionBootstrapImportReceipt>,
    InstallExecutionError,
> {
    let Some(distribution) = plan.topology.distribution.as_ref() else {
        return Ok(None);
    };
    let Some(receipt) = distribution.bootstrap_receipt.as_deref() else {
        // An already admitted owner-ledger release needs no bootstrap import.
        return Ok(None);
    };
    let public_key_base64 = public_key_base64.ok_or_else(|| {
        InstallExecutionError::new(
            "signed base-distribution import requires a host-selected public key",
        )
    })?;
    let actor_id = plan.placement.instance_id;
    let idempotency_key =
        deterministic_bootstrap_idempotency_key(actor_id, receipt.payload.distribution_release_id);
    ModuleControlPlane::new(runtime.clone())
        .static_distribution_bootstrap(public_key_base64.to_owned())
        .import(
            rustok_modules::ModuleStaticDistributionBootstrapImportCommand {
                receipt: receipt.clone(),
                context: rustok_modules::ModuleCommandContext {
                    actor_id,
                    tenant_id: None,
                    trace_id: format!("installer:static-distribution-bootstrap:{idempotency_key}"),
                    correlation_id: idempotency_key,
                    idempotency_key,
                },
            },
        )
        .await
        .map(Some)
        .map_err(|error| {
            InstallExecutionError::new(format!(
                "failed to import signed base distribution into owner ledger: {error}"
            ))
        })
}

fn deterministic_bootstrap_idempotency_key(instance_id: Uuid, release_id: Uuid) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(b"rustok:installer:bootstrap:");
    hasher.update(instance_id.as_bytes());
    hasher.update(release_id.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    // Mark the deterministic identity as RFC 9562 custom version 8 rather
    // than allowing random-looking bytes to escape into the operation ledger.
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

fn deterministic_seed_identity(domain: &str, value: &str) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(b"rustok:installer:seed:");
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher.update(value.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

/// SeaORM implementation of database, schema and durable installer-state ports.
pub struct SeaOrmInstallerPorts;

/// Complete SeaORM adapter for the standalone installer executor. It is
/// intentionally independent of HTTP host state and verifies the same module
/// defaults used by the typed installer seed profile.
pub struct SeaOrmInstallerApplyPorts<'a> {
    registry: &'a rustok_core::ModuleRegistry,
}

impl<'a> SeaOrmInstallerApplyPorts<'a> {
    pub fn new(registry: &'a rustok_core::ModuleRegistry) -> Self {
        Self { registry }
    }
}

#[async_trait::async_trait]
impl InstallDatabasePort for SeaOrmInstallerApplyPorts<'_> {
    type Runtime = DatabaseConnection;

    async fn prepare_database(
        &self,
        plan: &InstallPlan,
        database_url: &str,
        options: &InstallApplyOptions,
    ) -> Result<InstallDatabaseReady<Self::Runtime>, InstallExecutionError> {
        InstallDatabasePort::prepare_database(&SeaOrmInstallerPorts, plan, database_url, options)
            .await
    }
}

#[async_trait::async_trait]
impl InstallSchemaPort<DatabaseConnection> for SeaOrmInstallerApplyPorts<'_> {
    async fn apply_owner_schema(
        &self,
        runtime: &DatabaseConnection,
    ) -> Result<(), InstallExecutionError> {
        InstallSchemaPort::apply_owner_schema(&SeaOrmInstallerPorts, runtime).await
    }

    async fn apply_remaining_schema(
        &self,
        runtime: &DatabaseConnection,
    ) -> Result<(), InstallExecutionError> {
        InstallSchemaPort::apply_remaining_schema(&SeaOrmInstallerPorts, runtime).await
    }
}

#[async_trait::async_trait]
impl InstallBootstrapPort<DatabaseConnection> for SeaOrmInstallerApplyPorts<'_> {
    async fn import_base_distribution(
        &self,
        runtime: &DatabaseConnection,
        plan: &InstallPlan,
        public_key_base64: Option<&str>,
    ) -> Result<
        Option<rustok_modules::ModuleStaticDistributionBootstrapImportReceipt>,
        InstallExecutionError,
    > {
        import_base_distribution(runtime, plan, public_key_base64).await
    }
}

#[async_trait::async_trait]
impl InstallPersistencePort<DatabaseConnection> for SeaOrmInstallerApplyPorts<'_> {
    async fn create_session(
        &self,
        runtime: &DatabaseConnection,
        plan: &InstallPlan,
    ) -> Result<InstallSessionRecord, InstallExecutionError> {
        InstallPersistencePort::create_session(&SeaOrmInstallerPorts, runtime, plan).await
    }

    async fn acquire_lock(
        &self,
        runtime: &DatabaseConnection,
        session: InstallSessionRecord,
        owner: &str,
        ttl_secs: i64,
    ) -> Result<InstallSessionRecord, InstallExecutionError> {
        InstallPersistencePort::acquire_lock(
            &SeaOrmInstallerPorts,
            runtime,
            session,
            owner,
            ttl_secs,
        )
        .await
    }

    async fn record_receipt(
        &self,
        runtime: &DatabaseConnection,
        receipt: &InstallReceipt,
    ) -> Result<InstallReceiptRecord, InstallExecutionError> {
        InstallPersistencePort::record_receipt(&SeaOrmInstallerPorts, runtime, receipt).await
    }

    async fn set_state(
        &self,
        runtime: &DatabaseConnection,
        session_id: Uuid,
        state: InstallState,
    ) -> Result<InstallSessionRecord, InstallExecutionError> {
        InstallPersistencePort::set_state(&SeaOrmInstallerPorts, runtime, session_id, state).await
    }

    async fn set_tenant_id(
        &self,
        runtime: &DatabaseConnection,
        session_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<InstallSessionRecord, InstallExecutionError> {
        InstallPersistencePort::set_tenant_id(&SeaOrmInstallerPorts, runtime, session_id, tenant_id)
            .await
    }
}

#[async_trait::async_trait]
impl InstallSeedPort<DatabaseConnection> for SeaOrmInstallerApplyPorts<'_> {
    async fn apply_seed(
        &self,
        runtime: &DatabaseConnection,
        plan: &InstallPlan,
    ) -> Result<InstallSeedOutcome, InstallExecutionError> {
        SeaOrmInstallerBootstrapPorts::new(
            runtime.clone(),
            self.registry,
            plan.seed_profile.default_enabled_modules(),
        )
        .apply_seed_profile(plan, "rustok-cli install apply")
        .await
    }
}

#[async_trait::async_trait]
impl InstallAdminPort<DatabaseConnection> for SeaOrmInstallerApplyPorts<'_> {
    async fn provision_admin(
        &self,
        runtime: &DatabaseConnection,
        plan: &InstallPlan,
        tenant_id: Uuid,
        password: &str,
    ) -> Result<InstallAdminOutcome, InstallExecutionError> {
        SeaOrmInstallerBootstrapPorts::new(
            runtime.clone(),
            self.registry,
            plan.seed_profile.default_enabled_modules(),
        )
        .provision_admin(plan, tenant_id, password)
        .await
    }
}

#[async_trait::async_trait]
impl InstallVerificationPort<DatabaseConnection> for SeaOrmInstallerApplyPorts<'_> {
    async fn verify_installation(
        &self,
        runtime: &DatabaseConnection,
        plan: &InstallPlan,
        tenant_id: Uuid,
    ) -> Result<InstallVerificationOutcome, InstallExecutionError> {
        verify_standalone_installation(runtime, plan, tenant_id, self.registry).await
    }
}

#[async_trait::async_trait]
impl InstallDeploymentPort<DatabaseConnection> for SeaOrmInstallerApplyPorts<'_> {
    fn supports_distribution_deployment(&self) -> bool {
        false
    }

    async fn deploy_distribution(
        &self,
        _runtime: &DatabaseConnection,
        _request: InstallDistributionDeploymentRequest,
    ) -> Result<InstallDistributionDeployment, InstallExecutionError> {
        Err(InstallExecutionError::new(
            "standalone installer apply has no configured distribution deployment adapter",
        ))
    }
}

/// Shared SeaORM-backed seed and bootstrap adapter. Hosts supply the composed
/// module registry; auth, tenant, RBAC and module persistence stay with their
/// owning crates rather than being reimplemented by each host.
pub struct SeaOrmInstallerBootstrapPorts<'a> {
    db: DatabaseConnection,
    registry: &'a rustok_core::ModuleRegistry,
    defaults: Vec<String>,
}

impl<'a> SeaOrmInstallerBootstrapPorts<'a> {
    pub fn new(
        db: DatabaseConnection,
        registry: &'a rustok_core::ModuleRegistry,
        defaults: Vec<String>,
    ) -> Self {
        Self {
            db,
            registry,
            defaults,
        }
    }

    pub async fn apply_seed_profile(
        &self,
        plan: &InstallPlan,
        actor: &str,
    ) -> Result<InstallSeedOutcome, InstallExecutionError> {
        let mut enabled_modules = plan.seed_profile.default_enabled_modules();
        enabled_modules.extend(plan.modules.enable.iter().cloned());
        let outcome = rustok_installer::execute_seed_profile(
            SeedExecutionRequest {
                profile: plan.seed_profile,
                tenant: SeedTenantRequest {
                    name: plan.tenant.name.clone(),
                    slug: plan.tenant.slug.clone(),
                    domain: None,
                },
                enabled_modules,
                disabled_modules: plan.modules.disable.clone(),
                admin: None,
                demo_customer_password: (plan.seed_profile == SeedProfile::Dev)
                    .then(|| "dev-password-123".to_string()),
                actor: actor.to_string(),
            },
            self,
            self,
            self,
        )
        .await
        .map_err(execution_error)?;
        Ok(InstallSeedOutcome {
            tenant_id: outcome.tenant.id,
            tenant_slug: outcome.tenant.slug,
            tenant_created: outcome.tenant.created,
            enabled_modules: outcome.enabled_modules,
            disabled_modules: outcome.disabled_modules,
            demo_customer_created: outcome
                .demo_customer
                .map(|user| user.created)
                .unwrap_or(false),
        })
    }

    pub async fn provision_admin(
        &self,
        plan: &InstallPlan,
        tenant_id: Uuid,
        password: &str,
    ) -> Result<rustok_installer::InstallAdminOutcome, InstallExecutionError> {
        let tx = self.db.begin().await.map_err(execution_error)?;
        let result: Result<rustok_installer::InstallAdminOutcome, InstallExecutionError> = async {
            let user = AuthUserBootstrapDbWriter::ensure_user_on(
                &tx,
                AuthUserBootstrapRequest {
                    tenant_id,
                    email: plan.admin.email.clone(),
                    name: "Super Admin".to_string(),
                    password: password.to_string(),
                },
            )
            .await
            .map_err(execution_error)?;
            RbacRoleAssignmentDbWriter::assign_role_permissions_on(
                &tx,
                tenant_id,
                user.id,
                rustok_core::UserRole::SuperAdmin,
            )
            .await
            .map_err(execution_error)?;
            Ok(rustok_installer::InstallAdminOutcome {
                user_id: user.id,
                email: user.email,
                created: user.created,
            })
        }
        .await;

        match result {
            Ok(outcome) => {
                tx.commit().await.map_err(execution_error)?;
                Ok(outcome)
            }
            Err(error) => {
                tx.rollback().await.map_err(|rollback_error| {
                    InstallExecutionError::new(format!(
                        "installer admin provisioning failed: {error}; rollback failed: {rollback_error}"
                    ))
                })?;
                Err(error)
            }
        }
    }
}

#[async_trait::async_trait]
impl SeedTenantPort for SeaOrmInstallerBootstrapPorts<'_> {
    async fn ensure_seed_tenant(
        &self,
        request: SeedTenantRequest,
    ) -> Result<SeedTenant, SeedExecutionError> {
        let (tenant, created) = TenantService::new(self.db.clone())
            .ensure_tenant(CreateTenantInput {
                name: request.name,
                slug: request.slug,
                domain: request.domain,
            })
            .await
            .map_err(seed_error)?;
        Ok(SeedTenant {
            id: tenant.id,
            slug: tenant.slug,
            created,
        })
    }
}

#[async_trait::async_trait]
impl SeedPrincipalPort for SeaOrmInstallerBootstrapPorts<'_> {
    async fn ensure_seed_principal(
        &self,
        request: SeedUserRequest,
        role: rustok_core::UserRole,
    ) -> Result<SeedUser, SeedExecutionError> {
        let tx = self.db.begin().await.map_err(seed_error)?;
        let result: Result<SeedUser, SeedExecutionError> = async {
            let user = AuthUserBootstrapDbWriter::ensure_user_on(
                &tx,
                AuthUserBootstrapRequest {
                    tenant_id: request.tenant_id,
                    email: request.email,
                    name: request.name,
                    password: request.password,
                },
            )
            .await
            .map_err(seed_error)?;
            RbacRoleAssignmentDbWriter::assign_role_permissions_on(
                &tx,
                request.tenant_id,
                user.id,
                role,
            )
            .await
            .map_err(seed_error)?;
            Ok(SeedUser {
                id: user.id,
                email: user.email,
                created: user.created,
            })
        }
        .await;

        match result {
            Ok(user) => {
                tx.commit().await.map_err(seed_error)?;
                Ok(user)
            }
            Err(error) => {
                tx.rollback().await.map_err(|rollback_error| {
                    SeedExecutionError::Dependency(format!(
                        "seed principal provisioning failed: {error}; rollback failed: {rollback_error}"
                    ))
                })?;
                Err(error)
            }
        }
    }
}

#[async_trait::async_trait]
impl SeedIdentityPort for SeaOrmInstallerBootstrapPorts<'_> {
    async fn ensure_seed_user(
        &self,
        request: SeedUserRequest,
    ) -> Result<SeedUser, SeedExecutionError> {
        ensure_user(&self.db, request).await.map_err(seed_error)
    }
}

#[async_trait::async_trait]
impl SeedRolePort for SeaOrmInstallerBootstrapPorts<'_> {
    async fn assign_seed_role(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        role: rustok_core::UserRole,
    ) -> Result<(), SeedExecutionError> {
        RbacRoleAssignmentDbWriter::new(self.db.clone())
            .assign_role_permissions(tenant_id, user_id, role)
            .await
            .map_err(seed_error)
    }
}

#[async_trait::async_trait]
impl SeedModulePort for SeaOrmInstallerBootstrapPorts<'_> {
    async fn set_seed_module_enabled(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
        enabled: bool,
        actor: &str,
    ) -> Result<(), SeedExecutionError> {
        let actor_id = deterministic_seed_identity("actor", actor);
        let idempotency_key = deterministic_seed_identity(
            "module-toggle",
            &format!("{tenant_id}/{module_slug}/{enabled}/{actor}"),
        );
        let lifecycle = ModuleControlPlane::new(self.db.clone())
            .lifecycle(self.registry, self.defaults.clone());
        let expected_revision = lifecycle
            .static_lifecycle_snapshot(tenant_id, module_slug)
            .await
            .map_err(|error| SeedExecutionError::Dependency(error.to_string()))?
            .revision;
        lifecycle
            .toggle(ModuleLifecycleToggleCommand {
                tenant_id,
                module_slug: module_slug.to_string(),
                enabled,
                actor_id,
                idempotency_key,
                expected_revision,
            })
            .await
            .map(|_| ())
            .map_err(seed_error)
    }
}

async fn ensure_user(
    db: &DatabaseConnection,
    request: SeedUserRequest,
) -> Result<SeedUser, rustok_auth::AuthLifecycleMutationError> {
    let user = AuthUserBootstrapDbWriter::new(db.clone())
        .ensure_user(AuthUserBootstrapRequest {
            tenant_id: request.tenant_id,
            email: request.email,
            name: request.name,
            password: request.password,
        })
        .await?;
    Ok(SeedUser {
        id: user.id,
        email: user.email,
        created: user.created,
    })
}

fn seed_error(error: impl std::fmt::Display) -> SeedExecutionError {
    SeedExecutionError::Dependency(error.to_string())
}

fn execution_error(error: impl std::fmt::Debug) -> InstallExecutionError {
    InstallExecutionError::new(format!("{error:?}"))
}

async fn verify_standalone_installation(
    db: &DatabaseConnection,
    plan: &InstallPlan,
    tenant_id: Uuid,
    registry: &rustok_core::ModuleRegistry,
) -> Result<InstallVerificationOutcome, InstallExecutionError> {
    let tenant_service = TenantService::new(db.clone());
    let tenant = tenant_service
        .read_tenant(
            PortContext::new(
                plan.tenant.slug.clone(),
                PortActor::service("rustok-installer.cli"),
                "und",
                format!("installer:verify:{}", plan.tenant.slug),
            )
            .with_deadline(Duration::from_secs(15)),
            TenantReadRequest {
                selector: TenantReadSelector::Slug(plan.tenant.slug.clone()),
                include_inactive: true,
            },
        )
        .await
        .map_err(execution_error)?;
    if tenant.id != tenant_id {
        return Err(InstallExecutionError::new(format!(
            "installer tenant slug `{}` resolved to unexpected tenant {}",
            plan.tenant.slug, tenant.id
        )));
    }

    let admin = AuthUserBootstrapDbWriter::new(db.clone())
        .find_user(tenant.id, &plan.admin.email)
        .await
        .map_err(execution_error)?
        .ok_or_else(|| {
            InstallExecutionError::new(format!(
                "installer admin `{}` was not created",
                plan.admin.email
            ))
        })?;
    let mut enabled_modules = ModuleControlPlane::new(db.clone())
        .effective_policy(registry, plan.seed_profile.default_enabled_modules())
        .resolve_enabled(tenant.id)
        .await
        .map_err(execution_error)?
        .into_iter()
        .collect::<Vec<_>>();
    enabled_modules.sort();

    Ok(InstallVerificationOutcome {
        tenant_id: tenant.id,
        tenant_slug: tenant.slug,
        admin_user_id: admin.id,
        enabled_modules,
    })
}

#[async_trait::async_trait]
impl InstallDatabasePort for SeaOrmInstallerPorts {
    type Runtime = DatabaseConnection;

    async fn prepare_database(
        &self,
        plan: &InstallPlan,
        database_url: &str,
        options: &InstallApplyOptions,
    ) -> Result<InstallDatabaseReady<Self::Runtime>, InstallExecutionError> {
        let target = parse_database_target(&plan.database.engine, database_url)?;
        let mut created_database = false;
        if plan.database.create_if_missing {
            if plan.database.engine != DatabaseEngine::Postgres {
                return Err(InstallExecutionError::new(
                    "--create-database is only supported for postgres install plans",
                ));
            }
            created_database = ensure_postgres_database(
                options
                    .pg_admin_url
                    .as_deref()
                    .unwrap_or(DEFAULT_PG_ADMIN_URL),
                &target,
            )
            .await?;
        }
        let runtime = Database::connect(database_url)
            .await
            .map_err(database_error)?;
        runtime
            .query_one(Statement::from_string(
                runtime.get_database_backend(),
                "SELECT 1".to_string(),
            ))
            .await
            .map_err(database_error)?;
        Ok(InstallDatabaseReady {
            runtime,
            database_name: target.database_name,
            created_database,
        })
    }
}

#[async_trait::async_trait]
impl InstallSchemaPort<DatabaseConnection> for SeaOrmInstallerPorts {
    async fn apply_owner_schema(
        &self,
        runtime: &DatabaseConnection,
    ) -> Result<(), InstallExecutionError> {
        rustok_migrations::apply_installer_owner_schema(runtime)
            .await
            .map_err(database_error)
    }

    async fn apply_remaining_schema(
        &self,
        runtime: &DatabaseConnection,
    ) -> Result<(), InstallExecutionError> {
        rustok_migrations::apply_installer_remaining_schema(runtime)
            .await
            .map_err(database_error)
    }
}

#[async_trait::async_trait]
impl InstallBootstrapPort<DatabaseConnection> for SeaOrmInstallerPorts {
    async fn import_base_distribution(
        &self,
        runtime: &DatabaseConnection,
        plan: &InstallPlan,
        public_key_base64: Option<&str>,
    ) -> Result<
        Option<rustok_modules::ModuleStaticDistributionBootstrapImportReceipt>,
        InstallExecutionError,
    > {
        import_base_distribution(runtime, plan, public_key_base64).await
    }
}

#[async_trait::async_trait]
impl InstallPersistencePort<DatabaseConnection> for SeaOrmInstallerPorts {
    async fn create_session(
        &self,
        runtime: &DatabaseConnection,
        plan: &InstallPlan,
    ) -> Result<InstallSessionRecord, InstallExecutionError> {
        InstallerPersistenceService::new(runtime.clone())
            .create_session(plan, None, None)
            .await
            .map(session_record)
            .map_err(database_error)
    }
    async fn acquire_lock(
        &self,
        runtime: &DatabaseConnection,
        session: InstallSessionRecord,
        owner: &str,
        ttl_secs: i64,
    ) -> Result<InstallSessionRecord, InstallExecutionError> {
        let persistence = InstallerPersistenceService::new(runtime.clone());
        let model = persistence
            .get_session(session.id)
            .await
            .map_err(database_error)?
            .ok_or_else(|| {
                InstallExecutionError::new(format!("install session {} not found", session.id))
            })?;
        persistence
            .acquire_lock(model, owner, chrono::Duration::seconds(ttl_secs.max(1)))
            .await
            .map(session_record)
            .map_err(database_error)
    }
    async fn record_receipt(
        &self,
        runtime: &DatabaseConnection,
        receipt: &InstallReceipt,
    ) -> Result<InstallReceiptRecord, InstallExecutionError> {
        InstallerPersistenceService::new(runtime.clone())
            .record_receipt(receipt)
            .await
            .map(|model| InstallReceiptRecord {
                id: model.id,
                input_checksum: model.input_checksum,
            })
            .map_err(database_error)
    }
    async fn set_state(
        &self,
        runtime: &DatabaseConnection,
        session_id: Uuid,
        state: InstallState,
    ) -> Result<InstallSessionRecord, InstallExecutionError> {
        InstallerPersistenceService::new(runtime.clone())
            .set_state(session_id, state)
            .await
            .map(session_record)
            .map_err(database_error)
    }
    async fn set_tenant_id(
        &self,
        runtime: &DatabaseConnection,
        session_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<InstallSessionRecord, InstallExecutionError> {
        InstallerPersistenceService::new(runtime.clone())
            .set_tenant_id(session_id, tenant_id)
            .await
            .map(session_record)
            .map_err(database_error)
    }
}

fn session_record(model: install_session::Model) -> InstallSessionRecord {
    InstallSessionRecord {
        id: model.id,
        tenant_id: model.tenant_id,
        lock_owner: model.lock_owner,
        lock_expires_at: model.lock_expires_at,
    }
}
fn database_error(error: impl std::fmt::Display) -> InstallExecutionError {
    InstallExecutionError::new(error.to_string())
}

struct DatabaseTarget {
    database_name: Option<String>,
    username: Option<String>,
    password: Option<String>,
}
fn parse_database_target(
    engine: &DatabaseEngine,
    database_url: &str,
) -> Result<DatabaseTarget, InstallExecutionError> {
    if *engine == DatabaseEngine::Sqlite {
        return Ok(DatabaseTarget {
            database_name: None,
            username: None,
            password: None,
        });
    }
    let parsed = Url::parse(database_url).map_err(database_error)?;
    if !matches!(parsed.scheme(), "postgres" | "postgresql") {
        return Err(InstallExecutionError::new(format!(
            "postgres install plan requires postgres URL, got `{}`",
            parsed.scheme()
        )));
    }
    let database_name = parsed
        .path_segments()
        .and_then(|mut s| s.next_back())
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| {
            InstallExecutionError::new("postgres database URL must include a database name")
        })?
        .to_string();
    if parsed.username().trim().is_empty() {
        return Err(InstallExecutionError::new(
            "postgres database URL must include a username",
        ));
    }
    Ok(DatabaseTarget {
        database_name: Some(database_name),
        username: Some(parsed.username().to_string()),
        password: parsed.password().map(ToString::to_string),
    })
}
async fn ensure_postgres_database(
    admin_url: &str,
    target: &DatabaseTarget,
) -> Result<bool, InstallExecutionError> {
    let database_name = target
        .database_name
        .as_deref()
        .ok_or_else(|| InstallExecutionError::new("postgres database name is required"))?;
    let username = target
        .username
        .as_deref()
        .ok_or_else(|| InstallExecutionError::new("postgres username is required"))?;
    let password = target.password.as_deref().unwrap_or_default();
    let admin = Database::connect(admin_url).await.map_err(database_error)?;
    let role_exists = admin
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            format!(
                "SELECT 1 FROM pg_roles WHERE rolname = {}",
                quote_literal(username)
            ),
        ))
        .await
        .map_err(database_error)?
        .is_some();
    if !role_exists {
        admin
            .execute(Statement::from_string(
                DbBackend::Postgres,
                format!(
                    "CREATE ROLE {} LOGIN PASSWORD {}",
                    quote_ident(username),
                    quote_literal(password)
                ),
            ))
            .await
            .map_err(database_error)?;
    }
    let database_exists = admin
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            format!(
                "SELECT 1 FROM pg_database WHERE datname = {}",
                quote_literal(database_name)
            ),
        ))
        .await
        .map_err(database_error)?
        .is_some();
    if database_exists {
        return Ok(false);
    }
    admin
        .execute(Statement::from_string(
            DbBackend::Postgres,
            format!(
                "CREATE DATABASE {} OWNER {}",
                quote_ident(database_name),
                quote_ident(username)
            ),
        ))
        .await
        .map_err(database_error)?;
    Ok(true)
}
fn quote_ident(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
fn quote_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
