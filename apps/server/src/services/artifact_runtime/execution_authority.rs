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
    ArtifactBindingDispatchError, ArtifactCapabilityExecution, ArtifactEffectivePolicyResolver,
    ArtifactExecutionLeaseGuard, ArtifactInstallationTarget, InstalledModuleArtifact,
    ModuleRuntimeBinding, resolve_granted_artifact_capability,
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
            // The lease has not yet been registered. Set its non-renewable
            // monotonic deadline only after the last durable preflight.
            deadline: Instant::now(),
            direct_scope,
        };
        // Last pre-execution policy and credential check, after resolving the
        // sandbox grant snapshot. No SQL transaction spans guest execution.
        if !lease.credential_live() || !check_policy(resolver, &lease).await?
            || !self.check_direct(&lease).await
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

async fn check_policy(
    resolver: &ServerArtifactEffectivePolicyResolver,
    lease: &ExecutionLease,
) -> Result<bool, rustok_modules::ArtifactEffectivePolicyResolveError> {
    let Some(revision) = lease.effective_revision.as_deref() else {
        return Ok(lease.context.phase == ExecutionPhase::Lifecycle);
    };
    let policy = resolver.resolve(lease.tenant_id(), &lease.artifact).await?;
    Ok(policy.policy_revision() == revision
        && policy.admit_artifact_execution(
            lease.tenant_id(), &lease.artifact.release,
            ArtifactInstallationTarget::ExactInstallation { installation_id: lease.artifact.installation_id },
        ).is_some())
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
        if grant.name != call.capability {
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
