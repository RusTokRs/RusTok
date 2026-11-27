//! Host-only, per-attempt execution leases. No bearer evidence enters the
//! sandbox protocol. A capability admission is a fresh check, not a lock
//! held across an external side effect.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use async_trait::async_trait;
use rustok_modules::{
    ArtifactBindingDispatchError, ArtifactCapabilityExecution,
    ArtifactExecutionLeaseGuard, InstalledModuleArtifact,
    ModuleRuntimeBinding, ArtifactSqlEffectAuthorizer, ArtifactSqlEffectFence,
    ArtifactSqlEffectCommitGuard, lock_granted_artifact_capability_on,
    resolve_granted_artifact_capability, SeaOrmModuleArtifactSecurityResolver,
};
use rustok_sandbox::{
    CapabilityBroker, CapabilityCall, CapabilityCallContext, CapabilityGrant, CapabilityResponse,
    ExecutionPhase, SandboxContext, SandboxError, SandboxPolicy, SandboxResult, SandboxSubject,
};
use uuid::Uuid;

use super::ServerArtifactEffectivePolicyResolver;
use crate::services::{
    auth_admin_mutation_provider::ServerAuthAdminMutationProvider,
    rbac_request_scope::CapturedArtifactRequestScope,
};

const MAX_ACTIVE_EXECUTIONS: usize = 4096;

type ActiveLeases = Arc<Mutex<HashMap<Uuid, Arc<ExecutionLease>>>>;

#[derive(Clone)]
pub(super) struct ExecutionAuthority {
    active: ActiveLeases,
    provider: Option<Arc<ServerAuthAdminMutationProvider>>,
}

struct ExecutionLease {
    execution_id: Uuid,
    subject: SandboxSubject,
    context: CapabilityCallContext,
    artifact: InstalledModuleArtifact,
    binding: ModuleRuntimeBinding,
    effective_revision: Option<String>,
    security_revision: u64,
    serving_closure: Option<rustok_modules::ArtifactServingClosure>,
    policy: SandboxPolicy,
    deadline: Instant,
    direct_scope: Option<CapturedArtifactRequestScope>,
}

impl ExecutionLease {
    fn credential_live(&self) -> bool {
        self.direct_scope.as_ref().is_none_or(|scope| {
            scope.is_valid() && chrono::Utc::now() < scope.expires_at()
        })
    }

    fn live(&self) -> bool {
        Instant::now() < self.deadline && self.credential_live()
    }

    fn matches(&self, call: &CapabilityCall) -> bool {
        self.execution_id == call.execution_id
            && self.subject == call.subject
            && self.context == call.context
            && self.binding.capabilities.contains(&call.capability)
            && self.live()
    }

    fn tenant_id(&self) -> Uuid {
        // Construction rejects absent and nil tenants.
        self.context.tenant_id.expect("admitted execution tenant")
    }

    fn denial(&self) -> ArtifactBindingDispatchError {
        match self.effective_revision.as_ref() {
            Some(revision) => ArtifactBindingDispatchError::DeniedByPolicy {
                slug: self.artifact.release.slug.clone(),
                policy_revision: revision.clone(),
            },
            None => ArtifactBindingDispatchError::AdmissionMismatch,
        }
    }
}

struct LeaseGuard {
    active: ActiveLeases,
    id: Uuid,
    lease: Arc<ExecutionLease>,
}

impl ArtifactExecutionLeaseGuard for LeaseGuard {
    fn is_live(&self) -> bool {
        self.lease.live() && self.active.lock().is_ok_and(|active| {
            active.get(&self.id).is_some_and(|entry| Arc::ptr_eq(entry, &self.lease))
        })
    }
}

impl Drop for LeaseGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active.lock() {
            active.remove(&self.id);
        }
    }
}

impl ExecutionAuthority {
    pub(super) fn new(provider: Option<Arc<ServerAuthAdminMutationProvider>>) -> Self {
        Self { active: Arc::new(Mutex::new(HashMap::new())), provider }
    }

    async fn check_direct(&self, lease: &ExecutionLease) -> bool {
        let Some(scope) = lease.direct_scope.as_ref() else { return true; };
        let Some(provider) = self.provider.as_ref() else { return false; };
        let Some(actor_id) = lease.context.actor_id.as_deref()
            .and_then(|value| Uuid::parse_str(value).ok()) else { return false; };
        scope.run(provider.artifact_runtime_permission_keys(
            lease.tenant_id(), actor_id, &lease.artifact,
        )).await.is_ok_and(|keys| keys.contains(&lease.binding.permission))
            && lease.credential_live()
    }

    pub(super) async fn begin(
        &self,
        resolver: &ServerArtifactEffectivePolicyResolver,
        artifact: &InstalledModuleArtifact,
        binding: &ModuleRuntimeBinding,
        context: &SandboxContext,
        policy: &SandboxPolicy,
        effective_revision: Option<&str>,
    ) -> Result<Box<dyn ArtifactExecutionLeaseGuard>, ArtifactBindingDispatchError> {
        let invalid = || ArtifactBindingDispatchError::InvalidContext;
        let tenant_id = context.tenant_id.filter(|id| !id.is_nil()).ok_or_else(invalid)?;
        if context.execution_id.is_nil()
            || (context.phase != ExecutionPhase::Lifecycle && effective_revision.is_none())
        {
            return Err(invalid());
        }
        // Only user-facing dispatch phases derive authority from the actor's
        // credential. Event/hook/schedule actors are audit attribution, not
        // permission to impersonate a user. Lifecycle has its own owner gate.
        let direct_scope = if context.phase == ExecutionPhase::Http
            || (context.phase == ExecutionPhase::Manual
                && binding.kind == rustok_modules::ModuleRuntimeBindingKind::Command)
        {
            let actor_id = context.actor_id.as_deref()
                .and_then(|value| Uuid::parse_str(value).ok())
                .ok_or_else(invalid)?;
            Some(CapturedArtifactRequestScope::capture(tenant_id, actor_id)
                .ok_or_else(|| ArtifactBindingDispatchError::AdmissionMismatch)?)
        } else {
            None
        };
        let security_revision = SeaOrmModuleArtifactSecurityResolver::new(resolver.db.clone())
            .resolve(&artifact.release).await
            .map_err(|error| ArtifactEffectivePolicyResolveError::Store(error.to_string()))?
            .execution_revision()
            .ok_or(ArtifactBindingDispatchError::AdmissionMismatch)?;
        let serving_closure = if let Some(revision) = effective_revision {
            let (current, closure) = resolver.resolve_with_closure(tenant_id, artifact).await?;
            if current.policy_revision() != revision {
                return Err(ArtifactBindingDispatchError::AdmissionMismatch);
            }
            let closure = closure.ok_or(ArtifactBindingDispatchError::AdmissionMismatch)?;
            if closure.root_security_revision(artifact) != Some(security_revision) {
                return Err(ArtifactBindingDispatchError::AdmissionMismatch);
            }
            Some(closure)
        } else {
            // Owner lifecycle is not ordinary serving admission; its root
            // security and exact installation remain mandatory.
            None
        };
        let mut lease = ExecutionLease {
            execution_id: context.execution_id,
            subject: SandboxSubject::ModuleArtifact {
                installation_id: artifact.installation_id,
                slug: artifact.release.slug.clone(),
                version: artifact.release.version.clone(),
                digest: artifact.release.digest.clone(),
            },
            context: CapabilityCallContext::from(context),
            artifact: artifact.clone(),
            binding: binding.clone(),
            effective_revision: effective_revision.map(str::to_owned),
            security_revision,
            serving_closure,
            policy: policy.clone(),
            // The lease has not yet been registered. Set its non-renewable
            // monotonic deadline only after the last durable preflight.
            deadline: Instant::now(),
            direct_scope,
        };
        // Last pre-execution policy and credential check, after resolving the
        // sandbox grant snapshot. No SQL transaction spans guest execution.
        if !lease.credential_live()
            || !self.check_direct(&lease).await
            || !check_security(resolver, &lease).await
        {
            return Err(lease.denial());
        }
        let mut duration = Duration::from_millis(policy.limits.wall_clock_ms);
        if let Some(scope) = lease.direct_scope.as_ref() {
            let remaining = (scope.expires_at() - chrono::Utc::now()).to_std()
                .map_err(|_| lease.denial())?;
            duration = duration.min(remaining);
        }
        lease.deadline = Instant::now().checked_add(duration).ok_or_else(invalid)?;
        if !lease.live() {
            return Err(lease.denial());
        }
        self.insert(Arc::new(lease))
    }

    fn insert(&self, lease: Arc<ExecutionLease>) -> Result<Box<dyn ArtifactExecutionLeaseGuard>, ArtifactBindingDispatchError> {
        let mut active = self.active.lock()
            .map_err(|_| ArtifactBindingDispatchError::Execution("execution authority unavailable".into()))?;
        if active.len() >= MAX_ACTIVE_EXECUTIONS || active.contains_key(&lease.execution_id) {
            return Err(ArtifactBindingDispatchError::InvalidContext);
        }
        let id = lease.execution_id;
        active.insert(id, Arc::clone(&lease));
        Ok(Box::new(LeaseGuard { active: Arc::clone(&self.active), id, lease }))
    }

    fn lookup(&self, call: &CapabilityCall) -> SandboxResult<Arc<ExecutionLease>> {
        self.active.lock().ok()
            .and_then(|active| active.get(&call.execution_id).cloned())
            .filter(|lease| lease.matches(call))
            .ok_or_else(|| SandboxError::CapabilityDenied(call.capability.clone()))
    }
}

async fn check_security(
    resolver: &ServerArtifactEffectivePolicyResolver,
    lease: &ExecutionLease,
) -> bool {
    SeaOrmModuleArtifactSecurityResolver::new(resolver.db.clone())
        .resolve(&lease.artifact.release)
        .await
        .is_ok_and(|snapshot| snapshot.execution_revision() == Some(lease.security_revision))
}

async fn check_policy(
    resolver: &ServerArtifactEffectivePolicyResolver,
    lease: &ExecutionLease,
) -> Result<bool, rustok_modules::ArtifactEffectivePolicyResolveError> {
    let Some(revision) = lease.effective_revision.as_deref() else {
        return Ok(lease.context.phase == ExecutionPhase::Lifecycle);
    };
    let (policy, closure) = resolver.resolve_with_closure(lease.tenant_id(), &lease.artifact).await?;
    Ok(policy.policy_revision() == revision
        && closure.is_some()
        && closure.as_ref() == lease.serving_closure.as_ref())
}

/// A bound call is produced from the live host registry, not the guest.
struct SqlEffectFence {
    authority: ExecutionAuthority,
    lease: Arc<ExecutionLease>,
    call: CapabilityCall,
}

struct SqlEffectCommitGuard {
    fence: SqlEffectFence,
    credential_deadline: Option<chrono::DateTime<chrono::Utc>>,
}

impl SqlEffectFence {
    fn ensure_live(&self) -> SandboxResult<()> {
        let current = self.authority.lookup(&self.call)?;
        if !Arc::ptr_eq(&current, &self.lease) {
            return Err(SandboxError::CapabilityDenied(self.call.capability.clone()));
        }
        Ok(())
    }

    fn deny(&self) -> SandboxError {
        SandboxError::CapabilityDenied(self.call.capability.clone())
    }
}

#[async_trait]
impl ArtifactSqlEffectAuthorizer for ExecutionAuthority {
    async fn prepare(&self, call: &CapabilityCall) -> SandboxResult<Box<dyn ArtifactSqlEffectFence>> {
        if !matches!(call.capability.as_str(), "platform.data" | "platform.events") {
            return Err(SandboxError::CapabilityDenied(call.capability.clone()));
        }
        let lease = self.lookup(call)?;
        if lease.direct_scope.is_some() {
            let provider = self.provider.as_ref()
                .ok_or_else(|| SandboxError::CapabilityDenied(call.capability.clone()))?;
            provider.prepare_artifact_effect_authority().await
                .map_err(|_| SandboxError::CapabilityDenied(call.capability.clone()))?;
        }
        let fence = SqlEffectFence { authority: self.clone(), lease, call: call.clone() };
        fence.ensure_live()?;
        Ok(Box::new(fence))
    }
}

#[async_trait]
impl ArtifactSqlEffectFence for SqlEffectFence {
    async fn lock_on(&self, tx: &sea_orm::DatabaseTransaction) -> SandboxResult<Box<dyn ArtifactSqlEffectCommitGuard>> {
        self.ensure_live()?;
        let credential_deadline = if let Some(scope) = self.lease.direct_scope.as_ref() {
            let provider = self.authority.provider.as_ref().ok_or_else(|| self.deny())?;
            let actor_id = self.lease.context.actor_id.as_deref()
                .and_then(|actor| Uuid::parse_str(actor).ok()).ok_or_else(|| self.deny())?;
            // Same transaction as the effect. No independently committed
            // credential or membership check and no second pool checkout.
            let (keys, deadline) = scope.run(provider.lock_artifact_runtime_permission_keys_on(
                tx, self.lease.tenant_id(), actor_id, &self.lease.artifact,
            )).await.map_err(|_| self.deny())?;
            if !keys.contains(&self.lease.binding.permission) {
                return Err(self.deny());
            }
            Some(deadline)
        } else {
            // Durable work and lifecycle have distinct owner authority. This
            // port does not manufacture a credential for an attributed actor.
            None
        };
        // Acquire the complete artifact closure before taking any individual
        // root lock. Overlapping serving closures use the same slug order.
        if let Some(closure) = &self.lease.serving_closure {
            closure.require_current_on(tx).await.map_err(|_| self.deny())?;
        }
        // Keep exact selection and the owner grant in the effect transaction
        // after the direct credential/RBAC fences, including background work.
        let current_policy = lock_granted_artifact_capability_on(
            tx, &self.lease.artifact, self.lease.tenant_id(), &self.call.capability,
        ).await?;
        SeaOrmModuleArtifactSecurityResolver::require_execution_revision_on(
            tx, &self.lease.artifact.release, self.lease.security_revision,
        ).await.map_err(|_| self.deny())?;
        let original = self.lease.policy.grant(&self.call.capability).ok_or_else(|| self.deny())?;
        let current = current_policy.grant(&self.call.capability).ok_or_else(|| self.deny())?;
        if current.constraints != original.constraints {
            return Err(self.deny());
        }
        self.ensure_live()?;
        Ok(Box::new(SqlEffectCommitGuard {
            fence: SqlEffectFence {
                authority: self.authority.clone(),
                lease: Arc::clone(&self.lease),
                call: self.call.clone(),
            },
            credential_deadline,
        }))
    }
}

#[async_trait]
impl ArtifactSqlEffectCommitGuard for SqlEffectCommitGuard {
    async fn check_before_commit(&self, tx: &sea_orm::DatabaseTransaction) -> SandboxResult<()> {
        self.fence.ensure_live()?;
        if let Some(deadline) = self.credential_deadline {
            let provider = self.fence.authority.provider.as_ref()
                .ok_or_else(|| self.fence.deny())?;
            provider.check_artifact_effect_deadline_on(tx, deadline).await
                .map_err(|_| self.fence.deny())?;
        }
        self.fence.ensure_live()
    }
}

pub(super) struct FencedCapabilityBroker<B> {
    resolver: ServerArtifactEffectivePolicyResolver,
    inner: B,
}

impl<B> FencedCapabilityBroker<B> {
    pub(super) fn new(resolver: ServerArtifactEffectivePolicyResolver, inner: B) -> Self {
        Self { resolver, inner }
    }
}

#[async_trait]
impl<B: CapabilityBroker> CapabilityBroker for FencedCapabilityBroker<B> {
    async fn invoke(&self, call: &CapabilityCall, grant: &CapabilityGrant) -> SandboxResult<CapabilityResponse> {
        let denied = || SandboxError::CapabilityDenied(call.capability.clone());
        let authority = &self.resolver.execution_authority;
        let lease = authority.lookup(call)?;
        if grant.name != call.capability
            || lease.policy.grant(&call.capability)
                .is_none_or(|original| original.constraints != grant.constraints)
        {
            return Err(denied());
        }
        // Resolve the live capability revision even for lifecycle hooks; a
        // revoked or replaced installation/grant cannot inherit an old lease.
        let current = resolve_granted_artifact_capability(&self.resolver.db,
            &ArtifactCapabilityExecution {
                installation_id: lease.artifact.installation_id,
                tenant_id: lease.tenant_id(),
                slug: lease.artifact.release.slug.clone(),
                version: lease.artifact.release.version.clone(),
                digest: lease.artifact.release.digest.clone(),
            }, &call.capability).await?;
        if current != lease.artifact
            || !check_policy(&self.resolver, &lease).await.unwrap_or(false)
            || !authority.check_direct(&lease).await
            || !check_security(&self.resolver, &lease).await
        {
            return Err(denied());
        }
        // The independent owner broker still performs its own capability
        // grant and transactional effect checks. Recheck lease membership
        // after all awaits: cancellation may have removed the execution.
        let live = authority.lookup(call)?;
        if !Arc::ptr_eq(&live, &lease) {
            return Err(denied());
        }
        self.inner.invoke(call, grant).await
    }
}
