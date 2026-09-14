use async_trait::async_trait;
use rustok_events::DomainEvent;
use rustok_sandbox::{
    CapabilityBroker, CapabilityCall, CapabilityGrant, CapabilityName, CapabilityResponse,
    ExecutionPhase, SandboxError, SandboxResult, SandboxSubject,
};
use rustok_secrets::{SecretRef, SecretResolverRegistry, SecretString};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

use crate::data::{
    configure_tenant_scope, namespace_lock_clause, optional_revision_value, placeholder,
    revision_value, uuid_from_row, uuid_value,
};
use crate::{
    ArtifactCapabilityBrokerResolver, ArtifactCapabilityExecution, ArtifactCapabilityScope,
    ControlPlaneInfrastructure, InstalledModuleArtifact, ModuleCommandContext,
    resolve_granted_artifact_capability,
};

const MAX_REFERENCE_NAME_BYTES: usize = 96;
const MAX_RESOLVER_ALIAS_BYTES: usize = 96;
const MAX_RESOLVER_KEY_BYTES: usize = 512;
const MAX_REASON_BYTES: usize = 2_000;
const MAX_SECRET_USE_PURPOSE_BYTES: usize = 96;

/// Independent secret namespace with exact installation-backed capability authority.
/// Its opaque instance is persisted by installation continuity; data persistence
/// contracts and artifact-data instances do not define secret ownership.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactSecretScope {
    pub capability: ArtifactCapabilityScope,
    pub secret_instance_id: Uuid,
}

impl ArtifactSecretScope {
    fn validate(&self) -> Result<(), ArtifactSecretError> {
        let subject = SandboxSubject::ModuleArtifact {
            installation_id: self.capability.installation_id,
            slug: self.capability.release.slug.clone(),
            version: self.capability.release.version.clone(),
            digest: self.capability.release.digest.clone(),
        };
        if self.secret_instance_id.is_nil() || !self.capability.matches_subject(&subject) {
            return Err(ArtifactSecretError::InvalidScope);
        }
        Ok(())
    }
}

fn artifact_secret_scope_for_execution(
    installation: &InstalledModuleArtifact,
    execution: &ArtifactCapabilityExecution,
    capability: &CapabilityName,
) -> SandboxResult<ArtifactSecretScope> {
    let scope = ArtifactSecretScope {
        capability: crate::artifact_capability_router::artifact_capability_scope_for_execution(
            installation,
            execution,
            capability,
        )?,
        secret_instance_id: installation
            .secret_instance_id
            .ok_or_else(|| SandboxError::CapabilityDenied(capability.clone()))?,
    };
    scope
        .validate()
        .map_err(|_| SandboxError::CapabilityDenied(capability.clone()))?;
    Ok(scope)
}

/// Owner command that binds one admitted logical reference to a deployment
/// secret reference. The reference is validated by a host authorizer before it
/// becomes durable; no secret value is accepted or stored here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactSecretBindingRequest {
    pub scope: ArtifactSecretScope,
    pub reference: String,
    pub secret: SecretRef,
    pub expected_revision: Option<u64>,
    pub context: ModuleCommandContext,
    pub reason: String,
}

/// The only secret-binding shape returned to artifact-facing callers. Resolver
/// aliases, resolver keys, and secret values stay inside host-owned adapters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactSecretHandle {
    pub reference: String,
    pub secret_instance_id: Uuid,
    pub revision: u64,
}

/// Per-execution request to expose a previously bound logical secret handle.
/// It contains sandbox identity and scope only; it never carries a resolver
/// alias, resolver key, or resolved secret value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactSecretHandleRequest {
    pub scope: ArtifactSecretScope,
    pub reference: String,
    pub execution_id: Uuid,
    pub subject: SandboxSubject,
    pub phase: ExecutionPhase,
    pub actor_id: Option<String>,
    pub trace_id: Option<String>,
}

/// Host-only request to consume one exact logical handle revision. It carries
/// execution identity but never a resolver alias, resolver key, or secret
/// value. The selected consumer is fixed by host composition, not guest input.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactSecretUseRequest {
    pub scope: ArtifactSecretScope,
    pub handle: ArtifactSecretHandle,
    pub execution_id: Uuid,
    pub subject: SandboxSubject,
    pub phase: ExecutionPhase,
    pub actor_id: Option<String>,
    pub trace_id: Option<String>,
}

/// Non-secret context supplied to a trusted value consumer alongside the
/// short-lived `SecretString` borrow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactSecretUseContext {
    pub scope: ArtifactSecretScope,
    pub reference: String,
    pub revision: u64,
    pub execution_id: Uuid,
    pub subject: SandboxSubject,
    pub phase: ExecutionPhase,
    pub actor_id: Option<String>,
    pub trace_id: Option<String>,
    pub purpose: &'static str,
}

/// Redacted host receipt. It is the only output of secret use and cannot carry
/// consumer output or resolved material back into a sandbox payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSecretUseReceipt {
    pub reference: String,
    pub secret_instance_id: Uuid,
    pub revision: u64,
    pub purpose: String,
}

/// Content-free consumer failure. Resolver and secret values must never become
/// part of an error propagated across the owner boundary.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("artifact secret consumer failed")]
pub struct ArtifactSecretConsumerError;

/// Trusted host adapter that consumes resolved secret material without
/// returning arbitrary output. Implementations are composed for one fixed
/// purpose and must keep the borrowed value out of logs, errors, persistence,
/// and responses.
#[async_trait]
pub trait ArtifactSecretValueConsumer: Send + Sync {
    fn purpose(&self) -> &'static str;

    async fn consume_secret(
        &self,
        context: &ArtifactSecretUseContext,
        secret: &SecretString,
    ) -> Result<(), ArtifactSecretConsumerError>;
}

/// Host-owned authorization and reference-policy check. A production adapter
/// validates the `SecretRef` against its deployment `SecretResolverRegistry`,
/// actor RBAC, installation lifecycle, and admitted policy revision.
#[async_trait]
pub trait ArtifactSecretAuthorizer: Send + Sync {
    async fn authorize_secret_binding(
        &self,
        request: &ArtifactSecretBindingRequest,
    ) -> Result<(), ArtifactSecretError>;
}

/// Host-owned authorization before a sandbox execution can receive a logical
/// secret handle. Implementations bind the admitted artifact digest, active
/// installation lifecycle, effective policy revision, and actor grants to the
/// request. They do not resolve a secret value.
#[async_trait]
pub trait ArtifactSecretHandleAuthorizer: Send + Sync {
    async fn authorize_secret_handle(
        &self,
        request: &ArtifactSecretHandleRequest,
    ) -> Result<(), ArtifactSecretError>;
}

/// Stronger host policy for resolving and consuming a bound value. Handle
/// acquisition authority does not imply value-use authority.
#[async_trait]
pub trait ArtifactSecretUseAuthorizer: Send + Sync {
    async fn authorize_secret_use(
        &self,
        request: &ArtifactSecretUseRequest,
    ) -> Result<(), ArtifactSecretError>;
}

/// Deployment-owned lifecycle, actor, and admitted-policy checks for
/// management-time binding, handle acquisition, and value consumption.
#[async_trait]
pub trait ArtifactSecretPolicy: Send + Sync {
    async fn authorize_secret_binding(
        &self,
        request: &ArtifactSecretBindingRequest,
    ) -> Result<(), ArtifactSecretError>;

    async fn authorize_secret_handle(
        &self,
        request: &ArtifactSecretHandleRequest,
    ) -> Result<(), ArtifactSecretError>;

    async fn authorize_secret_use(
        &self,
        request: &ArtifactSecretUseRequest,
    ) -> Result<(), ArtifactSecretError>;
}

/// Concrete adapter that combines the deployment resolver registry with the
/// host's lifecycle/RBAC policy. Binding validation checks resolver alias and
/// tenant key policy without resolving a secret value.
#[derive(Clone)]
pub struct RegistryArtifactSecretAuthorizer<P> {
    resolvers: SecretResolverRegistry,
    policy: P,
}

impl<P> RegistryArtifactSecretAuthorizer<P>
where
    P: ArtifactSecretPolicy,
{
    pub fn new(resolvers: SecretResolverRegistry, policy: P) -> Self {
        Self { resolvers, policy }
    }
}

#[async_trait]
impl<P> ArtifactSecretAuthorizer for RegistryArtifactSecretAuthorizer<P>
where
    P: ArtifactSecretPolicy,
{
    async fn authorize_secret_binding(
        &self,
        request: &ArtifactSecretBindingRequest,
    ) -> Result<(), ArtifactSecretError> {
        self.resolvers
            .validate_reference_for_tenant(request.scope.capability.tenant_id, &request.secret)
            .map_err(|_| ArtifactSecretError::PolicyDenied)?;
        self.policy.authorize_secret_binding(request).await
    }
}

#[async_trait]
impl<P> ArtifactSecretHandleAuthorizer for RegistryArtifactSecretAuthorizer<P>
where
    P: ArtifactSecretPolicy,
{
    async fn authorize_secret_handle(
        &self,
        request: &ArtifactSecretHandleRequest,
    ) -> Result<(), ArtifactSecretError> {
        self.policy.authorize_secret_handle(request).await
    }
}

#[async_trait]
impl<P> ArtifactSecretUseAuthorizer for RegistryArtifactSecretAuthorizer<P>
where
    P: ArtifactSecretPolicy,
{
    async fn authorize_secret_use(
        &self,
        request: &ArtifactSecretUseRequest,
    ) -> Result<(), ArtifactSecretError> {
        self.policy.authorize_secret_use(request).await
    }
}

/// Standalone production policy for sandbox-visible logical handles. The
/// dynamic resolver already checks exact admission before broker construction;
/// this policy repeats that check immediately before the binding read so a
/// lifecycle or capability-policy change cannot leave a stale broker
/// authorized. Secret values and resolver identities are not involved.
#[derive(Clone)]
pub struct SeaOrmArtifactSecretHandlePolicy {
    db: DatabaseConnection,
}

impl SeaOrmArtifactSecretHandlePolicy {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ArtifactSecretHandleAuthorizer for SeaOrmArtifactSecretHandlePolicy {
    async fn authorize_secret_handle(
        &self,
        request: &ArtifactSecretHandleRequest,
    ) -> Result<(), ArtifactSecretError> {
        validate_handle_request(request)?;
        let (installation_id, slug, version, digest) = match &request.subject {
            SandboxSubject::ModuleArtifact {
                installation_id,
                slug,
                version,
                digest,
            } => (*installation_id, slug, version, digest),
            SandboxSubject::AlloyDraft { .. } => {
                return Err(ArtifactSecretError::PolicyDenied);
            }
        };
        let capability = CapabilityName::new("platform.secrets")
            .map_err(|_| ArtifactSecretError::PolicyDenied)?;
        let execution = ArtifactCapabilityExecution {
            installation_id,
            tenant_id: request.scope.capability.tenant_id,
            slug: slug.clone(),
            version: version.clone(),
            digest: digest.clone(),
        };
        let installation = resolve_granted_artifact_capability(&self.db, &execution, &capability)
            .await
            .map_err(|_| ArtifactSecretError::PolicyDenied)?;
        let resolved_scope =
            artifact_secret_scope_for_execution(&installation, &execution, &capability)
                .map_err(|_| ArtifactSecretError::PolicyDenied)?;
        if resolved_scope != request.scope {
            return Err(ArtifactSecretError::PolicyDenied);
        }
        Ok(())
    }
}

/// SeaORM owner service for logical artifact secret bindings. Its storage is a
/// reference catalog only; it has no secret resolver and never resolves a
/// secret value.
#[derive(Clone)]
pub struct SeaOrmArtifactSecretService<A> {
    db: DatabaseConnection,
    authorizer: A,
    infrastructure: ControlPlaneInfrastructure,
}

impl<A> SeaOrmArtifactSecretService<A>
where
    A: ArtifactSecretAuthorizer,
{
    pub fn new(db: DatabaseConnection, authorizer: A) -> Self {
        let infrastructure = ControlPlaneInfrastructure::for_database(db.clone());
        Self::with_infrastructure(db, authorizer, infrastructure)
    }

    pub fn with_infrastructure(
        db: DatabaseConnection,
        authorizer: A,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self {
            db,
            authorizer,
            infrastructure,
        }
    }

    pub async fn bind(
        &self,
        request: ArtifactSecretBindingRequest,
    ) -> Result<ArtifactSecretHandle, ArtifactSecretError> {
        validate_request(&request)?;
        let request_digest = crate::promotion::digest_json(&request).map_err(storage_error)?;
        self.authorizer.authorize_secret_binding(&request).await?;

        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.scope.capability.tenant_id)
            .await
            .map_err(|error| ArtifactSecretError::Storage(error.to_string()))?;
        let backend = transaction.get_database_backend();

        if let Some(row) = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT request_digest, reference_name, resolver_alias, resolver_key, expected_revision, actor_id, trace_id, correlation_id, idempotency_key, reason, revision
                     FROM module_artifact_secret_binding_operations
                     WHERE tenant_id = {} AND data_owner_id = {} AND secret_instance_id = {} AND idempotency_key = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                ),
                vec![
                    uuid_value(request.scope.capability.tenant_id, backend),
                    uuid_value(request.scope.capability.data_owner_id, backend),
                    uuid_value(request.scope.secret_instance_id, backend),
                    uuid_value(request.context.idempotency_key, backend),
                ],
            ))
            .await
            .map_err(storage_error)?
        {
            let reference: String = row.try_get("", "reference_name").map_err(storage_error)?;
            let resolver: String = row.try_get("", "resolver_alias").map_err(storage_error)?;
            let key: String = row.try_get("", "resolver_key").map_err(storage_error)?;
            let expected_revision: Option<i64> =
                row.try_get("", "expected_revision").map_err(storage_error)?;
            let context = command_context_from_receipt_row(&row, request.scope.capability.tenant_id, backend)?;
            let reason: String = row.try_get("", "reason").map_err(storage_error)?;
            let revision: i64 = row.try_get("", "revision").map_err(storage_error)?;
            let stored_digest: String = row.try_get("", "request_digest").map_err(storage_error)?;
            if stored_digest != request_digest
                || reference != request.reference
                || resolver != request.secret.resolver
                || key != request.secret.key
                || expected_revision
                    .map(u64::try_from)
                    .transpose()
                    .map_err(|_| ArtifactSecretError::IdempotencyConflict)?
                    != request.expected_revision
                || context != request.context
                || reason != request.reason
            {
                return Err(ArtifactSecretError::IdempotencyConflict);
            }
            transaction.commit().await.map_err(storage_error)?;
            return Ok(ArtifactSecretHandle {
                secret_instance_id: request.scope.secret_instance_id,
                reference,
                revision: positive_receipt_revision(revision)?,
            });
        }

        let current = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT revision FROM module_artifact_secret_bindings
                     WHERE tenant_id = {} AND data_owner_id = {} AND secret_instance_id = {}
                     AND reference_name = {}{}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    namespace_lock_clause(backend),
                ),
                binding_values(&request, backend)?,
            ))
            .await
            .map_err(storage_error)?;
        let revision = if let Some(row) = current {
            let current_revision: i64 = row.try_get("", "revision").map_err(storage_error)?;
            let current_revision = positive_receipt_revision(current_revision)?;
            if request.expected_revision != Some(current_revision) {
                return Err(ArtifactSecretError::RevisionConflict);
            }
            let revision = current_revision
                .checked_add(1)
                .ok_or(ArtifactSecretError::RevisionConflict)?;
            let updated = transaction
                .execute_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "UPDATE module_artifact_secret_bindings
                         SET resolver_alias = {}, resolver_key = {}, revision = {}, actor_id = {}, reason = {}, updated_at = {}
                         WHERE tenant_id = {} AND data_owner_id = {} AND secret_instance_id = {}
                         AND reference_name = {} AND revision = {}",
                        placeholder(backend, 1),
                        placeholder(backend, 2),
                        placeholder(backend, 3),
                        placeholder(backend, 4),
                        placeholder(backend, 5),
                        crate::data::now_expression(backend),
                        placeholder(backend, 6),
                        placeholder(backend, 7),
                        placeholder(backend, 8),
                        placeholder(backend, 9),
                        placeholder(backend, 10),
                    ),
                    vec![
                        request.secret.resolver.clone().into(),
                        request.secret.key.clone().into(),
                        revision_value(revision)
                            .map_err(|error| ArtifactSecretError::Storage(error.to_string()))?,
                        uuid_value(request.context.actor_id, backend),
                        request.reason.clone().into(),
                        uuid_value(request.scope.capability.tenant_id, backend),
                        uuid_value(request.scope.capability.data_owner_id, backend),
                        uuid_value(request.scope.secret_instance_id, backend),
                        request.reference.clone().into(),
                        revision_value(current_revision)
                            .map_err(|error| ArtifactSecretError::Storage(error.to_string()))?,
                    ],
                ))
                .await
                .map_err(storage_error)?;
            if updated.rows_affected() != 1 {
                return Err(ArtifactSecretError::RevisionConflict);
            }
            revision
        } else {
            if request.expected_revision.is_some() {
                return Err(ArtifactSecretError::RevisionConflict);
            }
            transaction
                .execute_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "INSERT INTO module_artifact_secret_bindings
                         (tenant_id, data_owner_id, secret_instance_id, reference_name, resolver_alias, resolver_key,
                          revision, actor_id, reason, created_at, updated_at)
                         VALUES ({}, {}, {}, {}, {}, {}, 1, {}, {}, {}, {})",
                        placeholder(backend, 1),
                        placeholder(backend, 2),
                        placeholder(backend, 3),
                        placeholder(backend, 4),
                        placeholder(backend, 5),
                        placeholder(backend, 6),
                        placeholder(backend, 7),
                        placeholder(backend, 8),
                        crate::data::now_expression(backend),
                        crate::data::now_expression(backend),
                    ),
                    vec![
                        uuid_value(request.scope.capability.tenant_id, backend),
                        uuid_value(request.scope.capability.data_owner_id, backend),
                        uuid_value(request.scope.secret_instance_id, backend),
                        request.reference.clone().into(),
                        request.secret.resolver.clone().into(),
                        request.secret.key.clone().into(),
                        uuid_value(request.context.actor_id, backend),
                        request.reason.clone().into(),
                    ],
                ))
                .await
                .map_err(storage_error)?;
            1
        };

        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_secret_binding_operations
                     (tenant_id, data_owner_id, secret_instance_id, idempotency_key, reference_name, resolver_alias,
                      resolver_key, expected_revision, actor_id, trace_id, correlation_id, reason, revision, request_digest, completed_at)
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                    placeholder(backend, 7),
                    placeholder(backend, 8),
                    placeholder(backend, 9),
                    placeholder(backend, 10),
                    placeholder(backend, 11),
                    placeholder(backend, 12),
                    placeholder(backend, 13),
                    placeholder(backend, 14),
                    crate::data::now_expression(backend),
                ),
                vec![
                    uuid_value(request.scope.capability.tenant_id, backend),
                    uuid_value(request.scope.capability.data_owner_id, backend),
                    uuid_value(request.scope.secret_instance_id, backend),
                    uuid_value(request.context.idempotency_key, backend),
                    request.reference.clone().into(),
                    request.secret.resolver.clone().into(),
                    request.secret.key.clone().into(),
                    optional_revision_value(request.expected_revision)
                        .map_err(|error| ArtifactSecretError::Storage(error.to_string()))?,
                    uuid_value(request.context.actor_id, backend),
                    request.context.trace_id.clone().into(),
                    uuid_value(request.context.correlation_id, backend),
                    request.reason.clone().into(),
                    revision_value(revision)
                        .map_err(|error| ArtifactSecretError::Storage(error.to_string()))?,
                    request_digest.into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope_for_command(
                    &request.context,
                    DomainEvent::ModuleArtifactSecretBound {
                        tenant_id: request.scope.capability.tenant_id,
                        module_slug: request.scope.capability.release.slug.clone(),
                        installation_id: request.scope.capability.installation_id,
                        data_owner_id: request.scope.capability.data_owner_id,
                        secret_instance_id: request.scope.secret_instance_id,
                        revision,
                    },
                ),
            )
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(ArtifactSecretHandle {
            secret_instance_id: request.scope.secret_instance_id,
            reference: request.reference,
            revision,
        })
    }
}

/// SeaORM owner service for sandbox-visible logical handles. It reads the
/// binding catalog only after host authorization; resolver details remain in
/// the owner table and are never returned to the sandbox.
#[derive(Clone)]
pub struct SeaOrmArtifactSecretHandleService<A> {
    db: DatabaseConnection,
    authorizer: A,
}

impl<A> SeaOrmArtifactSecretHandleService<A>
where
    A: ArtifactSecretHandleAuthorizer,
{
    pub fn new(db: DatabaseConnection, authorizer: A) -> Self {
        Self { db, authorizer }
    }

    pub async fn acquire_handle(
        &self,
        request: ArtifactSecretHandleRequest,
    ) -> Result<ArtifactSecretHandle, ArtifactSecretError> {
        validate_handle_request(&request)?;
        self.authorizer.authorize_secret_handle(&request).await?;

        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.scope.capability.tenant_id)
            .await
            .map_err(|error| ArtifactSecretError::Storage(error.to_string()))?;
        let backend = transaction.get_database_backend();
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT reference_name, revision FROM module_artifact_secret_bindings
                     WHERE tenant_id = {} AND data_owner_id = {} AND secret_instance_id = {}
                     AND reference_name = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                ),
                binding_values_for_scope(&request.scope, &request.reference, backend)?,
            ))
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        let row = row.ok_or(ArtifactSecretError::HandleUnavailable)?;
        let reference: String = row.try_get("", "reference_name").map_err(storage_error)?;
        let revision: i64 = row.try_get("", "revision").map_err(storage_error)?;
        Ok(ArtifactSecretHandle {
            secret_instance_id: request.scope.secret_instance_id,
            reference,
            revision: positive_receipt_revision(revision)
                .map_err(|_| ArtifactSecretError::HandleUnavailable)?,
        })
    }
}

/// Host-only value-use boundary. It resolves one exact bound revision after a
/// stronger use authorization check, then lends the `SecretString` directly to
/// a fixed trusted consumer. No sandbox capability response can contain the
/// value or arbitrary consumer output.
#[derive(Clone)]
pub struct SeaOrmArtifactSecretUseService<A, C> {
    db: DatabaseConnection,
    resolvers: SecretResolverRegistry,
    authorizer: A,
    consumer: C,
}

impl<A, C> SeaOrmArtifactSecretUseService<A, C>
where
    A: ArtifactSecretUseAuthorizer,
    C: ArtifactSecretValueConsumer,
{
    pub fn new(
        db: DatabaseConnection,
        resolvers: SecretResolverRegistry,
        authorizer: A,
        consumer: C,
    ) -> Self {
        Self {
            db,
            resolvers,
            authorizer,
            consumer,
        }
    }

    pub async fn use_secret(
        &self,
        request: ArtifactSecretUseRequest,
    ) -> Result<ArtifactSecretUseReceipt, ArtifactSecretError> {
        let purpose = self.consumer.purpose();
        validate_use_request(&request, purpose)?;
        self.authorizer.authorize_secret_use(&request).await?;

        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.scope.capability.tenant_id)
            .await
            .map_err(|error| ArtifactSecretError::Storage(error.to_string()))?;
        let backend = transaction.get_database_backend();
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT resolver_alias, resolver_key, revision
                     FROM module_artifact_secret_bindings
                     WHERE tenant_id = {} AND data_owner_id = {} AND secret_instance_id = {}
                     AND reference_name = {} AND revision = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                ),
                binding_values_for_use(&request, backend)?,
            ))
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        let row = row.ok_or(ArtifactSecretError::HandleUnavailable)?;
        let resolver: String = row.try_get("", "resolver_alias").map_err(storage_error)?;
        let key: String = row.try_get("", "resolver_key").map_err(storage_error)?;
        let revision: i64 = row.try_get("", "revision").map_err(storage_error)?;
        let revision = positive_receipt_revision(revision)
            .map_err(|_| ArtifactSecretError::HandleUnavailable)?;
        let secret = self
            .resolvers
            .resolve_for_tenant(
                request.scope.capability.tenant_id,
                &SecretRef { resolver, key },
            )
            .await
            .map_err(|_| ArtifactSecretError::ResolutionUnavailable)?;
        let context = ArtifactSecretUseContext {
            scope: request.scope,
            reference: request.handle.reference,
            revision,
            execution_id: request.execution_id,
            subject: request.subject,
            phase: request.phase,
            actor_id: request.actor_id,
            trace_id: request.trace_id,
            purpose,
        };
        self.consumer
            .consume_secret(&context, &secret)
            .await
            .map_err(|_| ArtifactSecretError::ConsumerUnavailable)?;
        Ok(ArtifactSecretUseReceipt {
            reference: context.reference,
            secret_instance_id: context.scope.secret_instance_id,
            revision: context.revision,
            purpose: context.purpose.to_string(),
        })
    }
}

/// The `platform.secrets` adapter for one installed artifact scope. It is
/// injected into the neutral sandbox runtime by the deployment and exposes
/// only a redacted logical handle for `acquire_handle`.
#[derive(Clone)]
pub struct SeaOrmArtifactSecretCapabilityBroker<A> {
    handles: SeaOrmArtifactSecretHandleService<A>,
    scope: ArtifactSecretScope,
}

impl<A> SeaOrmArtifactSecretCapabilityBroker<A>
where
    A: ArtifactSecretHandleAuthorizer,
{
    pub fn new(db: DatabaseConnection, authorizer: A, scope: ArtifactSecretScope) -> Self {
        Self {
            handles: SeaOrmArtifactSecretHandleService::new(db, authorizer),
            scope,
        }
    }
}

#[async_trait]
impl<A> CapabilityBroker for SeaOrmArtifactSecretCapabilityBroker<A>
where
    A: ArtifactSecretHandleAuthorizer,
{
    async fn invoke(
        &self,
        call: &CapabilityCall,
        _grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse> {
        if call.capability.as_str() != "platform.secrets" || call.operation != "acquire_handle" {
            return Err(SandboxError::CapabilityDenied(call.capability.clone()));
        }
        if call.context.tenant_id != Some(self.scope.capability.tenant_id)
            || !self.scope.capability.matches_subject(&call.subject)
        {
            return Err(SandboxError::CapabilityDenied(call.capability.clone()));
        }
        let reference = capability_reference(call)?;
        let handle = self
            .handles
            .acquire_handle(ArtifactSecretHandleRequest {
                scope: self.scope.clone(),
                reference: reference.to_string(),
                execution_id: call.execution_id,
                subject: call.subject.clone(),
                phase: call.context.phase,
                actor_id: call.context.actor_id.clone(),
                trace_id: call.context.trace_id.clone(),
            })
            .await
            .map_err(|error| secret_capability_error(&call.capability, error))?;
        Ok(CapabilityResponse {
            output: serde_json::to_value(&handle)
                .map_err(|_| SandboxError::CapabilityDenied(call.capability.clone()))?,
        })
    }
}

/// Dynamic `platform.secrets` owner route. It derives the logical-secret
/// namespace from the exact admitted installation before exposing any handle;
/// artifact input cannot select another module, policy revision, or tenant.
#[derive(Clone)]
pub struct SeaOrmArtifactSecretCapabilityBrokerResolver<A> {
    db: DatabaseConnection,
    authorizer: A,
}

impl<A> SeaOrmArtifactSecretCapabilityBrokerResolver<A>
where
    A: ArtifactSecretHandleAuthorizer + Clone,
{
    pub fn new(db: DatabaseConnection, authorizer: A) -> Self {
        Self { db, authorizer }
    }
}

#[async_trait]
impl<A> ArtifactCapabilityBrokerResolver for SeaOrmArtifactSecretCapabilityBrokerResolver<A>
where
    A: ArtifactSecretHandleAuthorizer + Clone + Send + Sync + 'static,
{
    async fn resolve_broker(
        &self,
        execution: &ArtifactCapabilityExecution,
        capability: &CapabilityName,
    ) -> SandboxResult<Arc<dyn CapabilityBroker>> {
        if capability.as_str() != "platform.secrets" {
            return Err(SandboxError::CapabilityDenied(capability.clone()));
        }
        let installation =
            resolve_granted_artifact_capability(&self.db, execution, capability).await?;
        let scope = artifact_secret_scope_for_execution(&installation, execution, capability)?;
        Ok(Arc::new(SeaOrmArtifactSecretCapabilityBroker::new(
            self.db.clone(),
            self.authorizer.clone(),
            scope,
        )))
    }
}

fn validate_request(request: &ArtifactSecretBindingRequest) -> Result<(), ArtifactSecretError> {
    request
        .scope
        .validate()
        .map_err(|_| ArtifactSecretError::InvalidScope)?;
    if !valid_reference_name(&request.reference) {
        return Err(ArtifactSecretError::InvalidReference);
    }
    if !valid_resolver_alias(&request.secret.resolver)
        || request.secret.key.trim().is_empty()
        || request.secret.key.len() > MAX_RESOLVER_KEY_BYTES
        || request
            .secret
            .key
            .chars()
            .any(|character| matches!(character, '\r' | '\n'))
    {
        return Err(ArtifactSecretError::InvalidSecretReference);
    }
    if !valid_command_context(request.scope.capability.tenant_id, &request.context)
        || request.reason.trim().is_empty()
        || request.reason.len() > MAX_REASON_BYTES
        || request
            .expected_revision
            .is_some_and(|revision| revision == 0 || revision > i64::MAX as u64)
    {
        return Err(ArtifactSecretError::InvalidCommand);
    }
    Ok(())
}

fn positive_receipt_revision(revision: i64) -> Result<u64, ArtifactSecretError> {
    u64::try_from(revision)
        .ok()
        .filter(|value| *value > 0)
        .ok_or(ArtifactSecretError::RevisionConflict)
}

fn valid_command_context(tenant_id: Uuid, context: &ModuleCommandContext) -> bool {
    context.tenant_id == Some(tenant_id) && context.validate().is_ok()
}

fn command_context_from_receipt_row(
    row: &QueryResult,
    tenant_id: Uuid,
    backend: DbBackend,
) -> Result<ModuleCommandContext, ArtifactSecretError> {
    let context = ModuleCommandContext {
        actor_id: uuid_from_row(row, "actor_id", backend)
            .map_err(|error| ArtifactSecretError::Storage(error.to_string()))?,
        tenant_id: Some(tenant_id),
        trace_id: row.try_get("", "trace_id").map_err(storage_error)?,
        correlation_id: uuid_from_row(row, "correlation_id", backend)
            .map_err(|error| ArtifactSecretError::Storage(error.to_string()))?,
        idempotency_key: uuid_from_row(row, "idempotency_key", backend)
            .map_err(|error| ArtifactSecretError::Storage(error.to_string()))?,
    };
    context
        .validate()
        .map_err(|error| ArtifactSecretError::Storage(error.to_string()))?;
    Ok(context)
}

fn validate_handle_request(
    request: &ArtifactSecretHandleRequest,
) -> Result<(), ArtifactSecretError> {
    validate_execution_secret_identity(
        &request.scope,
        &request.reference,
        request.execution_id,
        &request.subject,
    )
}

fn validate_use_request(
    request: &ArtifactSecretUseRequest,
    purpose: &str,
) -> Result<(), ArtifactSecretError> {
    validate_execution_secret_identity(
        &request.scope,
        &request.handle.reference,
        request.execution_id,
        &request.subject,
    )?;
    if request.handle.secret_instance_id != request.scope.secret_instance_id
        || request.handle.revision == 0
        || request.handle.revision > i64::MAX as u64
        || purpose.is_empty()
        || purpose.len() > MAX_SECRET_USE_PURPOSE_BYTES
        || !purpose.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '-' | '.')
        })
    {
        return Err(ArtifactSecretError::InvalidCommand);
    }
    Ok(())
}

fn validate_execution_secret_identity(
    scope: &ArtifactSecretScope,
    reference: &str,
    execution_id: Uuid,
    subject: &SandboxSubject,
) -> Result<(), ArtifactSecretError> {
    scope
        .validate()
        .map_err(|_| ArtifactSecretError::InvalidScope)?;
    if !valid_reference_name(reference) || execution_id.is_nil() {
        return Err(ArtifactSecretError::InvalidCommand);
    }
    if !scope.capability.matches_subject(subject) {
        return Err(ArtifactSecretError::PolicyDenied);
    }
    Ok(())
}

fn valid_reference_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_REFERENCE_NAME_BYTES
        && !value.starts_with('_')
        && !value.ends_with('_')
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '-')
        })
}

fn valid_resolver_alias(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_RESOLVER_ALIAS_BYTES
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '-')
        })
}

fn binding_values(
    request: &ArtifactSecretBindingRequest,
    backend: DbBackend,
) -> Result<Vec<sea_orm::Value>, ArtifactSecretError> {
    Ok(vec![
        uuid_value(request.scope.capability.tenant_id, backend),
        uuid_value(request.scope.capability.data_owner_id, backend),
        uuid_value(request.scope.secret_instance_id, backend),
        request.reference.clone().into(),
    ])
}

fn binding_values_for_scope(
    scope: &ArtifactSecretScope,
    reference: &str,
    backend: DbBackend,
) -> Result<Vec<sea_orm::Value>, ArtifactSecretError> {
    Ok(vec![
        uuid_value(scope.capability.tenant_id, backend),
        uuid_value(scope.capability.data_owner_id, backend),
        uuid_value(scope.secret_instance_id, backend),
        reference.to_owned().into(),
    ])
}

fn binding_values_for_use(
    request: &ArtifactSecretUseRequest,
    backend: DbBackend,
) -> Result<Vec<sea_orm::Value>, ArtifactSecretError> {
    let mut values = binding_values_for_scope(&request.scope, &request.handle.reference, backend)?;
    values.push(
        revision_value(request.handle.revision)
            .map_err(|error| ArtifactSecretError::Storage(error.to_string()))?,
    );
    Ok(values)
}

fn capability_reference(call: &CapabilityCall) -> SandboxResult<&str> {
    let input = call
        .input
        .as_object()
        .ok_or_else(|| SandboxError::CapabilityConstraintDenied {
            capability: call.capability.clone(),
            reason: "secret input must be an object".to_string(),
        })?;
    if input.len() != 1 {
        return Err(SandboxError::CapabilityConstraintDenied {
            capability: call.capability.clone(),
            reason: "secret input must contain only reference".to_string(),
        });
    }
    input
        .get("reference")
        .and_then(serde_json::Value::as_str)
        .filter(|reference| valid_reference_name(reference))
        .ok_or_else(|| SandboxError::CapabilityConstraintDenied {
            capability: call.capability.clone(),
            reason: "secret reference is invalid".to_string(),
        })
}

fn secret_capability_error(
    capability: &CapabilityName,
    error: ArtifactSecretError,
) -> SandboxError {
    match error {
        ArtifactSecretError::InvalidScope
        | ArtifactSecretError::InvalidReference
        | ArtifactSecretError::InvalidSecretReference
        | ArtifactSecretError::InvalidCommand
        | ArtifactSecretError::PolicyDenied
        | ArtifactSecretError::HandleUnavailable => {
            SandboxError::CapabilityDenied(capability.clone())
        }
        ArtifactSecretError::RevisionConflict | ArtifactSecretError::IdempotencyConflict => {
            SandboxError::HostCapability {
                capability: capability.clone(),
                message: "artifact secret handle is unavailable".to_string(),
            }
        }
        ArtifactSecretError::Storage(_) => SandboxError::HostCapability {
            capability: capability.clone(),
            message: "artifact secret handle is unavailable".to_string(),
        },
        ArtifactSecretError::ResolutionUnavailable | ArtifactSecretError::ConsumerUnavailable => {
            SandboxError::HostCapability {
                capability: capability.clone(),
                message: "artifact secret handle is unavailable".to_string(),
            }
        }
    }
}

fn storage_error(error: impl std::fmt::Display) -> ArtifactSecretError {
    ArtifactSecretError::Storage(error.to_string())
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArtifactSecretError {
    #[error("artifact secret scope is invalid")]
    InvalidScope,
    #[error("artifact secret logical reference is invalid")]
    InvalidReference,
    #[error("artifact secret resolver reference is invalid")]
    InvalidSecretReference,
    #[error("artifact secret binding command is invalid")]
    InvalidCommand,
    #[error("artifact secret binding revision conflict")]
    RevisionConflict,
    #[error("artifact secret idempotency key was reused for a different command")]
    IdempotencyConflict,
    #[error("artifact secret policy denied the operation")]
    PolicyDenied,
    #[error("artifact secret handle is unavailable")]
    HandleUnavailable,
    #[error("artifact secret value could not be resolved")]
    ResolutionUnavailable,
    #[error("artifact secret consumer is unavailable")]
    ConsumerUnavailable,
    #[error("artifact secret storage failed: {0}")]
    Storage(String),
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use async_trait::async_trait;
    use rustok_core::MigrationSource;
    use rustok_sandbox::{
        CapabilityCall, CapabilityCallContext, CapabilityGrant, CapabilityName, ExecutionPhase,
        SandboxPolicy, SandboxSubject,
    };
    use rustok_secrets::{
        EnvResolver, SecretAccessPolicy, SecretError, SecretRef, SecretResolver,
        SecretResolverRegistry, SecretString,
    };
    use sea_orm::{
        ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, Value as SqlValue,
    };
    use sea_orm_migration::{MigrationTrait, SchemaManager};
    use serde_json::json;
    use uuid::Uuid;

    use super::{
        ArtifactSecretAuthorizer, ArtifactSecretBindingRequest, ArtifactSecretError,
        ArtifactSecretHandle, ArtifactSecretHandleAuthorizer, ArtifactSecretHandleRequest,
        ArtifactSecretPolicy, ArtifactSecretScope, ArtifactSecretUseContext,
        ArtifactSecretUseRequest, ArtifactSecretValueConsumer, RegistryArtifactSecretAuthorizer,
        SeaOrmArtifactSecretHandlePolicy, SeaOrmArtifactSecretService,
        SeaOrmArtifactSecretUseService, capability_reference, validate_handle_request,
        validate_request, validate_use_request,
    };
    use crate::{
        ArtifactCapabilityScope, ArtifactModuleKind, ArtifactPayloadKind, ArtifactReleaseRef,
        ArtifactSecretConsumerError, MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
        MODULE_ARTIFACT_RHAI_SOURCE_MEDIA_TYPE, ModuleArtifactDescriptor, ModuleCommandContext,
        ModuleDependencyLockGraph, ModulesModule,
    };

    fn request() -> ArtifactSecretBindingRequest {
        let scope = ArtifactSecretScope {
            capability: ArtifactCapabilityScope {
                tenant_id: Uuid::new_v4(),
                installation_id: Uuid::new_v4(),
                data_owner_id: Uuid::new_v4(),
                release: ArtifactReleaseRef {
                    slug: "sample_module".to_string(),
                    version: "1.0.0".to_string(),
                    digest: crate::promotion::digest_json(&"fixture payload")
                        .expect("payload digest"),
                },
                policy_revision: 1,
            },
            secret_instance_id: Uuid::new_v4(),
        };
        ArtifactSecretBindingRequest {
            context: ModuleCommandContext {
                actor_id: Uuid::new_v4(),
                tenant_id: Some(scope.capability.tenant_id),
                trace_id: "test:artifact-secret-binding".to_string(),
                correlation_id: Uuid::new_v4(),
                idempotency_key: Uuid::new_v4(),
            },
            scope,
            reference: "payment_api".to_string(),
            secret: SecretRef {
                resolver: "vault".to_string(),
                key: "tenant/payment-api".to_string(),
            },
            expected_revision: None,
            reason: "initial configuration".to_string(),
        }
    }

    async fn active_secret_policy_fixture(
        database: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> ArtifactSecretHandleRequest {
        let installation_id = Uuid::new_v4();
        let capability = CapabilityName::new("platform.secrets").expect("capability name");
        let payload_digest =
            crate::promotion::digest_json(&("fixture Rhai payload", installation_id))
                .expect("distinct immutable fixture payload digest");
        let descriptor = ModuleArtifactDescriptor {
            schema_version: MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
            slug: "sample_module".to_string(),
            version: "1.0.0".to_string(),
            payload_kind: ArtifactPayloadKind::Rhai,
            module_kind: ArtifactModuleKind::Optional,
            runtime_abi: "rustok:module/runtime@1".to_string(),
            platform_compatibility: "^0.1".to_string(),
            required_features: Vec::new(),
            artifact_digest: payload_digest.clone(),
            entrypoint: "main".to_string(),
            capabilities: vec![capability.clone()],
            bindings: Vec::new(),
            dependencies: Vec::new(),
            permissions: Vec::new(),
            schema_documents: Vec::new(),
            settings_schema_digest: None,
            data_schema_digest: None,
            localization_catalogs: Vec::new(),
            ui_contributions: Vec::new(),
            persistence_contract: None,
        };
        descriptor.validate().expect("valid fixture descriptor");
        let dependency_lock =
            ModuleDependencyLockGraph::create(0, Vec::new()).expect("empty dependency lock");
        let sandbox_policy = SandboxPolicy {
            grants: vec![CapabilityGrant {
                name: capability,
                constraints: json!({
                    "references": ["payment_api"],
                    "operations": ["acquire_handle"]
                }),
            }],
            ..SandboxPolicy::default()
        };
        let installed_at = "2026-07-26T12:00:00+00:00";
        let data_owner_id = Uuid::new_v4();
        let settings_instance_id = Uuid::new_v4();
        let secret_instance_id = Uuid::new_v4();

        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO module_artifact_installations (
                    installation_id, scope_kind, tenant_id, registry, repository, manifest_digest,
                    slug, version, payload_kind, runtime_abi, payload_digest, entrypoint, descriptor,
                    data_owner_id, settings_instance_id, secret_instance_id, dependency_graph_revision, dependency_graph_digest, dependency_lock, installed_at,
                    previous_installation_id, capability_grant_revision
                 ) VALUES (
                    ?1, 'tenant', ?2, 'registry.example', 'modules/sample_module', ?3,
                    'sample_module', '1.0.0', 'rhai', 'rustok:module/runtime@1', ?4, 'main', ?5,
                    ?6, ?7, ?12, ?8, ?9, ?10, ?11, NULL, 1
                 )",
                vec![
                    installation_id.to_string().into(),
                    tenant_id.to_string().into(),
                    crate::promotion::digest_json(&installation_id).expect("fixture manifest digest").into(),
                    payload_digest.clone().into(),
                    SqlValue::Json(Some(Box::new(
                        serde_json::to_value(&descriptor).expect("descriptor JSON"),
                    ))),
                    data_owner_id.to_string().into(),
                    settings_instance_id.to_string().into(),
                    i64::try_from(dependency_lock.graph_revision)
                        .expect("graph revision")
                        .into(),
                    dependency_lock.graph_digest.clone().into(),
                    SqlValue::Json(Some(Box::new(
                        serde_json::to_value(&dependency_lock).expect("dependency lock JSON"),
                    ))),
                    installed_at.into(),
                    secret_instance_id.to_string().into(),
                ],
            ))
            .await
            .expect("installation fixture");
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO module_artifact_admissions (
                    stage_id, installation_id, payload_digest, media_type, size_bytes,
                    verification_evidence, status, revision, committed_at
                 ) VALUES (?1, ?2, ?3, ?4, 1, ?5, 'active', 1, ?6)",
                vec![
                    Uuid::new_v4().to_string().into(),
                    installation_id.to_string().into(),
                    payload_digest.clone().into(),
                    MODULE_ARTIFACT_RHAI_SOURCE_MEDIA_TYPE.into(),
                    SqlValue::Json(Some(Box::new(json!({})))),
                    installed_at.into(),
                ],
            ))
            .await
            .expect("active admission fixture");
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO module_artifact_sandbox_policies (
                    installation_id, tenant_id, capability_grant_revision, policy, created_at
                 ) VALUES (?1, ?2, 1, ?3, ?4)",
                vec![
                    installation_id.to_string().into(),
                    tenant_id.to_string().into(),
                    SqlValue::Json(Some(Box::new(
                        serde_json::to_value(&sandbox_policy).expect("sandbox policy JSON"),
                    ))),
                    installed_at.into(),
                ],
            ))
            .await
            .expect("sandbox policy fixture");

        ArtifactSecretHandleRequest {
            scope: ArtifactSecretScope {
                capability: ArtifactCapabilityScope {
                    tenant_id,
                    installation_id,
                    data_owner_id,
                    release: descriptor.release_ref(),
                    policy_revision: 1,
                },
                secret_instance_id,
            },
            reference: "payment_api".to_string(),
            execution_id: Uuid::new_v4(),
            subject: SandboxSubject::ModuleArtifact {
                installation_id,
                slug: descriptor.slug,
                version: descriptor.version,
                digest: payload_digest,
            },
            phase: ExecutionPhase::Manual,
            actor_id: Some("artifact-actor".to_string()),
            trace_id: Some("trace-1".to_string()),
        }
    }

    #[test]
    fn binding_rejects_non_logical_reference_and_multiline_resolver_key() {
        let mut invalid_name = request();
        invalid_name.reference = "Payment API".to_string();
        assert!(matches!(
            validate_request(&invalid_name),
            Err(ArtifactSecretError::InvalidReference)
        ));

        let mut multiline_key = request();
        multiline_key.secret.key = "tenant/payment\napi".to_string();
        assert!(matches!(
            validate_request(&multiline_key),
            Err(ArtifactSecretError::InvalidSecretReference)
        ));
    }

    #[test]
    fn binding_rejects_a_command_context_for_another_tenant() {
        let mut binding = request();
        binding.context.tenant_id = Some(Uuid::new_v4());

        assert!(matches!(
            validate_request(&binding),
            Err(ArtifactSecretError::InvalidCommand)
        ));
    }

    #[test]
    fn sandbox_handle_request_never_accepts_a_resolver_input_or_foreign_subject() {
        let call = CapabilityCall {
            execution_id: Uuid::new_v4(),
            subject: SandboxSubject::ModuleArtifact {
                installation_id: Uuid::new_v4(),
                slug: "sample_module".to_string(),
                version: "1.0.0".to_string(),
                digest: "sha256:sample".to_string(),
            },
            context: CapabilityCallContext {
                phase: ExecutionPhase::Lifecycle,
                tenant_id: Some(Uuid::new_v4()),
                actor_id: None,
                trace_id: None,
            },
            capability: CapabilityName::new("platform.secrets").expect("capability name"),
            operation: "acquire_handle".to_string(),
            input: json!({ "reference": "payment_api", "resolver": "vault" }),
        };
        assert!(capability_reference(&call).is_err());

        let request = ArtifactSecretHandleRequest {
            scope: request().scope,
            reference: "payment_api".to_string(),
            execution_id: Uuid::new_v4(),
            subject: SandboxSubject::AlloyDraft {
                draft_id: Uuid::new_v4(),
                revision: 1,
            },
            phase: ExecutionPhase::Lifecycle,
            actor_id: None,
            trace_id: None,
        };
        assert!(matches!(
            validate_handle_request(&request),
            Err(ArtifactSecretError::PolicyDenied)
        ));
    }

    #[test]
    fn secret_use_requires_exact_nonzero_handle_revision_and_fixed_purpose() {
        let binding = request();
        let mut use_request = ArtifactSecretUseRequest {
            scope: binding.scope.clone(),
            handle: ArtifactSecretHandle {
                reference: binding.reference,
                secret_instance_id: binding.scope.secret_instance_id,
                revision: 1,
            },
            execution_id: Uuid::new_v4(),
            subject: SandboxSubject::ModuleArtifact {
                installation_id: binding.scope.capability.installation_id,
                slug: binding.scope.capability.release.slug,
                version: binding.scope.capability.release.version,
                digest: binding.scope.capability.release.digest,
            },
            phase: ExecutionPhase::Manual,
            actor_id: Some("artifact-actor".to_string()),
            trace_id: Some("trace-1".to_string()),
        };

        assert!(validate_use_request(&use_request, "http.authorization").is_ok());
        use_request.handle.secret_instance_id = Uuid::new_v4();
        assert_eq!(
            validate_use_request(&use_request, "http.authorization"),
            Err(ArtifactSecretError::InvalidCommand)
        );
        use_request.handle.secret_instance_id = use_request.scope.secret_instance_id;
        use_request.handle.revision = u64::MAX;
        assert_eq!(
            validate_use_request(&use_request, "http.authorization"),
            Err(ArtifactSecretError::InvalidCommand)
        );
        use_request.handle.revision = 0;
        assert!(matches!(
            validate_use_request(&use_request, "http.authorization"),
            Err(ArtifactSecretError::InvalidCommand)
        ));
        use_request.handle.revision = 1;
        assert!(matches!(
            validate_use_request(&use_request, "HTTP Authorization"),
            Err(ArtifactSecretError::InvalidCommand)
        ));
    }

    struct AllowSecretPolicy;

    #[async_trait]
    impl ArtifactSecretPolicy for AllowSecretPolicy {
        async fn authorize_secret_binding(
            &self,
            _request: &ArtifactSecretBindingRequest,
        ) -> Result<(), ArtifactSecretError> {
            Ok(())
        }

        async fn authorize_secret_handle(
            &self,
            _request: &ArtifactSecretHandleRequest,
        ) -> Result<(), ArtifactSecretError> {
            Ok(())
        }

        async fn authorize_secret_use(
            &self,
            _request: &ArtifactSecretUseRequest,
        ) -> Result<(), ArtifactSecretError> {
            Ok(())
        }
    }

    struct AllowSecretBindingAuthorizer;

    #[async_trait]
    impl ArtifactSecretAuthorizer for AllowSecretBindingAuthorizer {
        async fn authorize_secret_binding(
            &self,
            _request: &ArtifactSecretBindingRequest,
        ) -> Result<(), ArtifactSecretError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn secret_binding_persists_and_replays_the_complete_command_context() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("sqlite database");
        let manager = SchemaManager::new(&database);
        rustok_outbox::SysEventsMigration
            .up(&manager)
            .await
            .expect("outbox migration");
        for migration in ModulesModule.migrations() {
            migration.up(&manager).await.expect("module migration");
        }
        let service =
            SeaOrmArtifactSecretService::new(database.clone(), AllowSecretBindingAuthorizer);
        let binding = request();

        let first = service
            .bind(binding.clone())
            .await
            .expect("initial secret binding");
        let replay = service
            .bind(binding.clone())
            .await
            .expect("exact secret binding replay");
        assert_eq!(replay, first);
        assert_eq!(first.secret_instance_id, binding.scope.secret_instance_id);
        assert!(database.execute_unprepared("UPDATE module_artifact_secret_binding_operations SET request_digest = 'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'").await.is_err());
        assert!(
            database
                .execute_unprepared("DELETE FROM module_artifact_secret_binding_operations")
                .await
                .is_err()
        );

        let receipt = database
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT actor_id, trace_id, correlation_id, idempotency_key
                 FROM module_artifact_secret_binding_operations
                 WHERE tenant_id = ?1 AND data_owner_id = ?2 AND secret_instance_id = ?3
                 AND idempotency_key = ?4",
                vec![
                    binding.scope.capability.tenant_id.to_string().into(),
                    binding.scope.capability.data_owner_id.to_string().into(),
                    binding.scope.secret_instance_id.to_string().into(),
                    binding.context.idempotency_key.to_string().into(),
                ],
            ))
            .await
            .expect("secret binding receipt query")
            .expect("secret binding receipt");
        assert_eq!(
            receipt
                .try_get::<String>("", "actor_id")
                .expect("receipt actor"),
            binding.context.actor_id.to_string()
        );
        assert_eq!(
            receipt
                .try_get::<String>("", "trace_id")
                .expect("receipt trace"),
            binding.context.trace_id
        );
        assert_eq!(
            receipt
                .try_get::<String>("", "correlation_id")
                .expect("receipt correlation"),
            binding.context.correlation_id.to_string()
        );
        assert_eq!(
            receipt
                .try_get::<String>("", "idempotency_key")
                .expect("receipt idempotency"),
            binding.context.idempotency_key.to_string()
        );

        let event = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT payload FROM sys_events WHERE event_type = 'module.artifact.secret_bound'"
                    .to_string(),
            ))
            .await
            .expect("secret binding event query")
            .expect("secret binding event");
        let payload: serde_json::Value = event
            .try_get("", "payload")
            .expect("secret binding event payload");
        let envelope: rustok_events::EventEnvelope =
            serde_json::from_value(payload).expect("secret binding event envelope");
        assert_eq!(envelope.tenant_id, binding.scope.capability.tenant_id);
        assert_eq!(envelope.actor_id, Some(binding.context.actor_id));
        assert_eq!(envelope.correlation_id, binding.context.correlation_id);
        assert_eq!(
            envelope.trace_id.as_deref(),
            Some(binding.context.trace_id.as_str())
        );

        for field in ["policy", "installation", "version", "digest"] {
            let mut altered = binding.clone();
            match field {
                "policy" => altered.scope.capability.policy_revision += 1,
                "installation" => altered.scope.capability.installation_id = Uuid::new_v4(),
                "version" => altered.scope.capability.release.version = "1.1.0".to_string(),
                "digest" => {
                    altered.scope.capability.release.digest =
                        crate::promotion::digest_json(&field).expect("changed digest")
                }
                _ => unreachable!("bounded fixture field"),
            }
            assert_eq!(
                service.bind(altered).await,
                Err(ArtifactSecretError::IdempotencyConflict),
                "changed {field}"
            );
        }
        let mut conflicting_replay = binding;
        conflicting_replay.context.trace_id = "test:changed-trace".to_string();
        assert!(matches!(
            service.bind(conflicting_replay).await,
            Err(ArtifactSecretError::IdempotencyConflict)
        ));
    }

    #[tokio::test]
    async fn registry_authorizer_checks_secret_ref_without_resolving_it() {
        let registry = SecretResolverRegistry::builder()
            .resolver(
                "env",
                EnvResolver,
                SecretAccessPolicy::Exact(vec!["allowed-key".to_string()]),
            )
            .build();
        let authorizer = RegistryArtifactSecretAuthorizer::new(registry, AllowSecretPolicy);
        let mut allowed = request();
        allowed.secret.resolver = "env".to_string();
        allowed.secret.key = "allowed-key".to_string();
        assert!(authorizer.authorize_secret_binding(&allowed).await.is_ok());

        allowed.secret.key = "forbidden-key".to_string();
        assert!(matches!(
            authorizer.authorize_secret_binding(&allowed).await,
            Err(ArtifactSecretError::PolicyDenied)
        ));
    }

    #[tokio::test]
    async fn stateless_secret_route_isolates_owners_and_instances_with_the_same_slug() {
        use crate::{ArtifactCapabilityBrokerResolver, ModuleControlPlane};

        let database = Database::connect("sqlite::memory:").await.expect("SQLite");
        let manager = SchemaManager::new(&database);
        rustok_outbox::SysEventsMigration
            .up(&manager)
            .await
            .expect("outbox");
        for migration in ModulesModule.migrations() {
            migration.up(&manager).await.expect("module migration");
        }
        let tenant_id = Uuid::new_v4();
        let first = active_secret_policy_fixture(&database, tenant_id).await;
        let second = active_secret_policy_fixture(&database, tenant_id).await;
        assert_eq!(
            first.scope.capability.release.slug,
            second.scope.capability.release.slug
        );
        assert_ne!(
            first.scope.capability.data_owner_id,
            second.scope.capability.data_owner_id
        );
        assert_ne!(
            first.scope.secret_instance_id,
            second.scope.secret_instance_id
        );

        let owner = ModuleControlPlane::new(database.clone());
        // Only management authorization is bounded fixture authority here.
        // Handle acquisition and dynamic routing use the production owner policy.
        let bindings = owner.artifact_secret_bindings(AllowSecretBindingAuthorizer);
        let mut first_binding = request();
        first_binding.scope = first.scope.clone();
        first_binding.context.tenant_id = Some(tenant_id);
        let bound = bindings
            .bind(first_binding.clone())
            .await
            .expect("first binding");
        let handles = super::SeaOrmArtifactSecretHandleService::new(
            database.clone(),
            owner.artifact_secret_handle_policy(),
        );
        assert_eq!(
            handles.acquire_handle(second.clone()).await,
            Err(ArtifactSecretError::HandleUnavailable)
        );
        assert_eq!(
            handles
                .acquire_handle(first.clone())
                .await
                .expect("first handle"),
            bound
        );

        let mut second_binding = first_binding.clone();
        second_binding.scope = second.scope.clone();
        let second_handle = bindings
            .bind(second_binding)
            .await
            .expect("independent owner binding");
        assert_eq!(
            second_handle.secret_instance_id,
            second.scope.secret_instance_id
        );
        assert_ne!(second_handle, bound);
        assert_eq!(
            handles
                .acquire_handle(second.clone())
                .await
                .expect("second handle"),
            second_handle
        );

        let mut transplanted = second.clone();
        transplanted.scope = first.scope.clone();
        assert_eq!(
            handles.acquire_handle(transplanted).await,
            Err(ArtifactSecretError::PolicyDenied)
        );
        let mut wrong_instance = first.clone();
        wrong_instance.scope.secret_instance_id = second.scope.secret_instance_id;
        assert_eq!(
            handles.acquire_handle(wrong_instance).await,
            Err(ArtifactSecretError::PolicyDenied)
        );

        let capability = CapabilityName::new("platform.secrets").expect("capability");
        let execution = crate::ArtifactCapabilityExecution {
            installation_id: first.scope.capability.installation_id,
            tenant_id,
            slug: first.scope.capability.release.slug.clone(),
            version: first.scope.capability.release.version.clone(),
            digest: first.scope.capability.release.digest.clone(),
        };
        let resolver = owner.artifact_secret_capability(owner.artifact_secret_handle_policy());
        let broker = resolver
            .resolve_broker(&execution, &capability)
            .await
            .expect("stateless secret broker");
        let call = CapabilityCall {
            execution_id: first.execution_id,
            subject: first.subject,
            context: CapabilityCallContext {
                phase: first.phase,
                tenant_id: Some(tenant_id),
                actor_id: first.actor_id,
                trace_id: first.trace_id,
            },
            capability: capability.clone(),
            operation: "acquire_handle".to_string(),
            input: json!({"reference": first.reference}),
        };
        let grant = CapabilityGrant {
            name: capability,
            constraints: json!({"references": ["payment_api"], "operations": ["acquire_handle"]}),
        };
        let response = broker
            .invoke(&call, &grant)
            .await
            .expect("stateless handle call");
        let returned: ArtifactSecretHandle =
            serde_json::from_value(response.output).expect("opaque handle");
        assert_eq!(returned, bound);
        let event_count = database.query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS count FROM sys_events WHERE event_type='module.artifact.secret_bound'",
        )).await.expect("events").expect("count");
        assert_eq!(
            event_count
                .try_get::<i64>("", "count")
                .expect("event count"),
            2
        );
    }

    #[tokio::test]
    async fn production_handle_policy_rechecks_exact_active_installation_and_grant() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("sqlite database");
        let manager = SchemaManager::new(&database);
        for migration in ModulesModule.migrations() {
            migration.up(&manager).await.expect("module migration");
        }
        let request = active_secret_policy_fixture(&database, Uuid::new_v4()).await;
        let policy = SeaOrmArtifactSecretHandlePolicy::new(database.clone());

        policy
            .authorize_secret_handle(&request)
            .await
            .expect("exact active installation with current grant");

        let mut stale_scope = request.clone();
        stale_scope.scope.capability.policy_revision += 1;
        assert!(matches!(
            policy.authorize_secret_handle(&stale_scope).await,
            Err(ArtifactSecretError::PolicyDenied)
        ));

        let mut foreign_installation = request.clone();
        let SandboxSubject::ModuleArtifact {
            installation_id, ..
        } = &mut foreign_installation.subject
        else {
            unreachable!("fixture uses an artifact subject")
        };
        *installation_id = Uuid::new_v4();
        assert!(matches!(
            policy.authorize_secret_handle(&foreign_installation).await,
            Err(ArtifactSecretError::PolicyDenied)
        ));

        let SandboxSubject::ModuleArtifact {
            installation_id, ..
        } = &request.subject
        else {
            unreachable!("fixture uses an artifact subject")
        };
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "UPDATE module_artifact_admissions SET status = 'inactive' WHERE installation_id = ?1",
                vec![installation_id.to_string().into()],
            ))
            .await
            .expect("deactivate fixture");
        assert!(matches!(
            policy.authorize_secret_handle(&request).await,
            Err(ArtifactSecretError::PolicyDenied)
        ));

        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "UPDATE module_artifact_admissions SET status = 'active' WHERE installation_id = ?1",
                vec![installation_id.to_string().into()],
            ))
            .await
            .expect("reactivate fixture");
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "UPDATE module_artifact_sandbox_policies SET policy = ?1 WHERE installation_id = ?2",
                vec![
                    SqlValue::Json(Some(Box::new(
                        serde_json::to_value(SandboxPolicy::default())
                            .expect("default sandbox policy JSON"),
                    ))),
                    installation_id.to_string().into(),
                ],
            ))
            .await
            .expect("remove fixture grant");
        assert!(matches!(
            policy.authorize_secret_handle(&request).await,
            Err(ArtifactSecretError::PolicyDenied)
        ));
    }

    #[derive(Clone)]
    struct FixedSecretResolver;

    #[async_trait]
    impl SecretResolver for FixedSecretResolver {
        async fn resolve(&self, _key: &str) -> Result<SecretString, SecretError> {
            Ok(SecretString::from("top-secret-value".to_string()))
        }
    }

    #[derive(Clone)]
    struct RecordingSecretConsumer {
        calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl ArtifactSecretValueConsumer for RecordingSecretConsumer {
        fn purpose(&self) -> &'static str {
            "http.authorization"
        }

        async fn consume_secret(
            &self,
            _context: &ArtifactSecretUseContext,
            _secret: &SecretString,
        ) -> Result<(), ArtifactSecretConsumerError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn secret_use_returns_only_a_redacted_receipt() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("sqlite database");
        let manager = SchemaManager::new(&database);
        for migration in ModulesModule.migrations() {
            migration.up(&manager).await.expect("module migration");
        }
        let binding = request();
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO module_artifact_secret_bindings
                 (tenant_id, data_owner_id, secret_instance_id, reference_name, resolver_alias,
                  resolver_key, revision, actor_id, reason, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8, datetime('now'), datetime('now'))",
                vec![
                    binding.scope.capability.tenant_id.to_string().into(),
                    binding.scope.capability.data_owner_id.to_string().into(),
                    binding.scope.secret_instance_id.to_string().into(),
                    binding.reference.clone().into(),
                    "fixed".into(),
                    "allowed-key".into(),
                    binding.context.actor_id.to_string().into(),
                    binding.reason.clone().into(),
                ],
            ))
            .await
            .expect("secret binding fixture");
        let resolvers = SecretResolverRegistry::builder()
            .resolver(
                "fixed",
                FixedSecretResolver,
                SecretAccessPolicy::Exact(vec!["allowed-key".to_string()]),
            )
            .build();
        let authorizer =
            RegistryArtifactSecretAuthorizer::new(resolvers.clone(), AllowSecretPolicy);
        let calls = Arc::new(AtomicUsize::new(0));
        let service = SeaOrmArtifactSecretUseService::new(
            database,
            resolvers,
            authorizer,
            RecordingSecretConsumer {
                calls: Arc::clone(&calls),
            },
        );
        let receipt = service
            .use_secret(ArtifactSecretUseRequest {
                scope: binding.scope.clone(),
                handle: ArtifactSecretHandle {
                    reference: binding.reference.clone(),
                    secret_instance_id: binding.scope.secret_instance_id,
                    revision: 1,
                },
                execution_id: Uuid::new_v4(),
                subject: SandboxSubject::ModuleArtifact {
                    installation_id: binding.scope.capability.installation_id,
                    slug: binding.scope.capability.release.slug,
                    version: binding.scope.capability.release.version,
                    digest: binding.scope.capability.release.digest,
                },
                phase: ExecutionPhase::Manual,
                actor_id: Some("artifact-actor".to_string()),
                trace_id: Some("trace-1".to_string()),
            })
            .await
            .expect("secret use receipt");

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(receipt.reference, binding.reference);
        assert_eq!(receipt.revision, 1);
        assert_eq!(receipt.purpose, "http.authorization");
        assert!(
            !serde_json::to_string(&receipt)
                .expect("receipt JSON")
                .contains("top-secret-value")
        );
    }
}
