use async_trait::async_trait;
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_sandbox::{
    CapabilityBroker, CapabilityCall, CapabilityGrant, CapabilityName, CapabilityResponse,
    ExecutionPhase, SandboxError, SandboxResult, SandboxSubject,
};
use rustok_secrets::{SecretRef, SecretResolverRegistry, SecretString};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

use crate::data::{
    artifact_data_scope_for_execution, configure_tenant_scope, namespace_lock_clause,
    optional_revision_value, placeholder, revision_value, uuid_from_row, uuid_value,
    ArtifactDataScope,
};
use crate::{
    resolve_granted_artifact_capability, ArtifactCapabilityBrokerResolver,
    ArtifactCapabilityExecution, ControlPlaneInfrastructure,
};

const MAX_REFERENCE_NAME_BYTES: usize = 96;
const MAX_RESOLVER_ALIAS_BYTES: usize = 96;
const MAX_RESOLVER_KEY_BYTES: usize = 512;
const MAX_REASON_BYTES: usize = 2_000;
const MAX_SECRET_USE_PURPOSE_BYTES: usize = 96;

/// Owner command that binds one admitted logical reference to a deployment
/// secret reference. The reference is validated by a host authorizer before it
/// becomes durable; no secret value is accepted or stored here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSecretBindingRequest {
    pub scope: ArtifactDataScope,
    pub reference: String,
    pub secret: SecretRef,
    pub expected_revision: Option<u64>,
    pub actor_id: Uuid,
    pub reason: String,
    pub idempotency_key: Uuid,
}

/// The only secret-binding shape returned to artifact-facing callers. Resolver
/// aliases, resolver keys, and secret values stay inside host-owned adapters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSecretHandle {
    pub reference: String,
    pub revision: u64,
}

/// Per-execution request to expose a previously bound logical secret handle.
/// It contains sandbox identity and scope only; it never carries a resolver
/// alias, resolver key, or resolved secret value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSecretHandleRequest {
    pub scope: ArtifactDataScope,
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
pub struct ArtifactSecretUseRequest {
    pub scope: ArtifactDataScope,
    pub reference: String,
    pub expected_revision: u64,
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
    pub scope: ArtifactDataScope,
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

/// Deployment-owned lifecycle, actor, and admitted-policy checks for both
/// management-time binding and execution-time handle acquisition.
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
            .validate_reference_for_tenant(request.scope.tenant_id, &request.secret)
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
        self.authorizer.authorize_secret_binding(&request).await?;

        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.scope.tenant_id)
            .await
            .map_err(|error| ArtifactSecretError::Storage(error.to_string()))?;
        let backend = transaction.get_database_backend();

        if let Some(row) = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT reference_name, resolver_alias, resolver_key, expected_revision, actor_id, reason, revision
                     FROM module_artifact_secret_binding_operations
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {} AND idempotency_key = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                ),
                vec![
                    uuid_value(request.scope.tenant_id, backend),
                    request.scope.module_slug.clone().into(),
                    revision_value(request.scope.data_contract_revision)
                        .map_err(|error| ArtifactSecretError::Storage(error.to_string()))?,
                    uuid_value(request.idempotency_key, backend),
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
            let actor_id = uuid_from_row(&row, "actor_id", backend)
                .map_err(|error| ArtifactSecretError::Storage(error.to_string()))?;
            let reason: String = row.try_get("", "reason").map_err(storage_error)?;
            let revision: i64 = row.try_get("", "revision").map_err(storage_error)?;
            if reference != request.reference
                || resolver != request.secret.resolver
                || key != request.secret.key
                || expected_revision
                    .map(u64::try_from)
                    .transpose()
                    .map_err(|_| ArtifactSecretError::IdempotencyConflict)?
                    != request.expected_revision
                || actor_id != request.actor_id
                || reason != request.reason
            {
                return Err(ArtifactSecretError::IdempotencyConflict);
            }
            transaction.commit().await.map_err(storage_error)?;
            return Ok(ArtifactSecretHandle {
                reference,
                revision: u64::try_from(revision).map_err(|_| ArtifactSecretError::RevisionConflict)?,
            });
        }

        let current = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT revision FROM module_artifact_secret_bindings
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}
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
            let current_revision = u64::try_from(current_revision)
                .map_err(|_| ArtifactSecretError::RevisionConflict)?;
            if request.expected_revision != Some(current_revision) {
                return Err(ArtifactSecretError::RevisionConflict);
            }
            let revision = current_revision
                .checked_add(1)
                .ok_or(ArtifactSecretError::RevisionConflict)?;
            let updated = transaction
                .execute(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "UPDATE module_artifact_secret_bindings
                         SET resolver_alias = {}, resolver_key = {}, revision = {}, actor_id = {}, reason = {}, updated_at = {}
                         WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}
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
                        uuid_value(request.actor_id, backend),
                        request.reason.clone().into(),
                        uuid_value(request.scope.tenant_id, backend),
                        request.scope.module_slug.clone().into(),
                        revision_value(request.scope.data_contract_revision)
                            .map_err(|error| ArtifactSecretError::Storage(error.to_string()))?,
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
                .execute(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "INSERT INTO module_artifact_secret_bindings
                         (tenant_id, module_slug, data_contract_revision, reference_name, resolver_alias, resolver_key,
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
                        uuid_value(request.scope.tenant_id, backend),
                        request.scope.module_slug.clone().into(),
                        revision_value(request.scope.data_contract_revision)
                            .map_err(|error| ArtifactSecretError::Storage(error.to_string()))?,
                        request.reference.clone().into(),
                        request.secret.resolver.clone().into(),
                        request.secret.key.clone().into(),
                        uuid_value(request.actor_id, backend),
                        request.reason.clone().into(),
                    ],
                ))
                .await
                .map_err(storage_error)?;
            1
        };

        transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_secret_binding_operations
                     (tenant_id, module_slug, data_contract_revision, idempotency_key, reference_name, resolver_alias,
                      resolver_key, expected_revision, actor_id, reason, revision, completed_at)
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
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
                    crate::data::now_expression(backend),
                ),
                vec![
                    uuid_value(request.scope.tenant_id, backend),
                    request.scope.module_slug.clone().into(),
                    revision_value(request.scope.data_contract_revision)
                        .map_err(|error| ArtifactSecretError::Storage(error.to_string()))?,
                    uuid_value(request.idempotency_key, backend),
                    request.reference.clone().into(),
                    request.secret.resolver.clone().into(),
                    request.secret.key.clone().into(),
                    optional_revision_value(request.expected_revision)
                        .map_err(|error| ArtifactSecretError::Storage(error.to_string()))?,
                    uuid_value(request.actor_id, backend),
                    request.reason.clone().into(),
                    revision_value(revision)
                        .map_err(|error| ArtifactSecretError::Storage(error.to_string()))?,
                ],
            ))
            .await
            .map_err(storage_error)?;
        self.infrastructure
            .write_event(
                &transaction,
                EventEnvelope::new(
                    self.infrastructure.new_id(),
                    Some(request.scope.tenant_id),
                    DomainEvent::ModuleArtifactSecretBound {
                        tenant_id: request.scope.tenant_id,
                        module_slug: request.scope.module_slug.clone(),
                        data_contract_revision: request.scope.data_contract_revision,
                        revision,
                    },
                ),
            )
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(ArtifactSecretHandle {
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
        configure_tenant_scope(&transaction, request.scope.tenant_id)
            .await
            .map_err(|error| ArtifactSecretError::Storage(error.to_string()))?;
        let backend = transaction.get_database_backend();
        let row = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT reference_name, revision FROM module_artifact_secret_bindings
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}
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
            reference,
            revision: u64::try_from(revision)
                .map_err(|_| ArtifactSecretError::HandleUnavailable)?,
        })
    }
}

/// The `platform.secrets` adapter for one installed artifact scope. It is
/// injected into the neutral sandbox runtime by the deployment and exposes
/// only a redacted logical handle for `acquire_handle`.
#[derive(Clone)]
pub struct SeaOrmArtifactSecretCapabilityBroker<A> {
    handles: SeaOrmArtifactSecretHandleService<A>,
    scope: ArtifactDataScope,
}

impl<A> SeaOrmArtifactSecretCapabilityBroker<A>
where
    A: ArtifactSecretHandleAuthorizer,
{
    pub fn new(db: DatabaseConnection, authorizer: A, scope: ArtifactDataScope) -> Self {
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
        if call.context.tenant_id != Some(self.scope.tenant_id)
            || !matches!(
                &call.subject,
                SandboxSubject::ModuleArtifact { slug, .. } if slug == &self.scope.module_slug
            )
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
            output: json!({
                "reference": handle.reference,
                "revision": handle.revision,
            }),
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
        let scope = artifact_data_scope_for_execution(&installation, execution, capability)?;
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
    if request.actor_id.is_nil()
        || request.idempotency_key.is_nil()
        || request.reason.trim().is_empty()
        || request.reason.len() > MAX_REASON_BYTES
        || request.expected_revision == Some(0)
    {
        return Err(ArtifactSecretError::InvalidCommand);
    }
    Ok(())
}

fn validate_handle_request(
    request: &ArtifactSecretHandleRequest,
) -> Result<(), ArtifactSecretError> {
    request
        .scope
        .validate()
        .map_err(|_| ArtifactSecretError::InvalidScope)?;
    if !valid_reference_name(&request.reference) || request.execution_id.is_nil() {
        return Err(ArtifactSecretError::InvalidCommand);
    }
    if !matches!(
        &request.subject,
        SandboxSubject::ModuleArtifact { slug, .. } if slug == &request.scope.module_slug
    ) {
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
        uuid_value(request.scope.tenant_id, backend),
        request.scope.module_slug.clone().into(),
        revision_value(request.scope.data_contract_revision)
            .map_err(|error| ArtifactSecretError::Storage(error.to_string()))?,
        request.reference.clone().into(),
    ])
}

fn binding_values_for_scope(
    scope: &ArtifactDataScope,
    reference: &str,
    backend: DbBackend,
) -> Result<Vec<sea_orm::Value>, ArtifactSecretError> {
    Ok(vec![
        uuid_value(scope.tenant_id, backend),
        scope.module_slug.clone().into(),
        revision_value(scope.data_contract_revision)
            .map_err(|error| ArtifactSecretError::Storage(error.to_string()))?,
        reference.to_owned().into(),
    ])
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
    #[error("artifact secret storage failed: {0}")]
    Storage(String),
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use rustok_sandbox::{
        CapabilityCall, CapabilityCallContext, CapabilityName, ExecutionPhase, SandboxSubject,
    };
    use rustok_secrets::SecretRef;
    use rustok_secrets::{EnvResolver, SecretAccessPolicy, SecretResolverRegistry};
    use serde_json::json;
    use uuid::Uuid;

    use super::{
        capability_reference, validate_handle_request, validate_request, ArtifactSecretAuthorizer,
        ArtifactSecretBindingRequest, ArtifactSecretError, ArtifactSecretHandleRequest,
        ArtifactSecretPolicy, RegistryArtifactSecretAuthorizer,
    };
    use crate::ArtifactDataScope;

    fn request() -> ArtifactSecretBindingRequest {
        ArtifactSecretBindingRequest {
            scope: ArtifactDataScope {
                tenant_id: Uuid::new_v4(),
                module_slug: "sample_module".to_string(),
                data_contract_revision: 1,
                policy_revision: 1,
            },
            reference: "payment_api".to_string(),
            secret: SecretRef {
                resolver: "vault".to_string(),
                key: "tenant/payment-api".to_string(),
            },
            expected_revision: None,
            actor_id: Uuid::new_v4(),
            reason: "initial configuration".to_string(),
            idempotency_key: Uuid::new_v4(),
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
}
