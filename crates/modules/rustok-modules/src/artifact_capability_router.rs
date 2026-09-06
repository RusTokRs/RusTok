use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use rustok_sandbox::{
    CapabilityBroker, CapabilityCall, CapabilityGrant, CapabilityName, CapabilityResponse,
    SandboxError, SandboxResult, SandboxSubject,
};

use crate::{
    ArtifactInstallationResolver, ArtifactReleaseRef, ArtifactSandboxPolicyResolver,
    InstalledModuleArtifact, SeaOrmArtifactInstallationStore, SeaOrmArtifactSandboxPolicyResolver,
};

/// Host-controlled identity used to resolve one installed artifact's capability
/// scope for a single sandbox call. It is extracted only from the admitted
/// sandbox subject and context; artifact input cannot select it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactCapabilityExecution {
    pub installation_id: Uuid,
    pub tenant_id: Uuid,
    pub slug: String,
    pub version: String,
    pub digest: String,
}

impl ArtifactCapabilityExecution {
    fn from_call(call: &CapabilityCall) -> SandboxResult<Self> {
        let (installation_id, slug, version, digest) = match &call.subject {
            SandboxSubject::ModuleArtifact {
                installation_id,
                slug,
                version,
                digest,
            } => (*installation_id, slug, version, digest),
            SandboxSubject::AlloyDraft { .. } => {
                return Err(SandboxError::CapabilityDenied(call.capability.clone()));
            }
        };
        let tenant_id = call
            .context
            .tenant_id
            .ok_or_else(|| SandboxError::CapabilityDenied(call.capability.clone()))?;
        if installation_id.is_nil()
            || slug.trim().is_empty()
            || version.trim().is_empty()
            || digest.trim().is_empty()
        {
            return Err(SandboxError::CapabilityDenied(call.capability.clone()));
        }
        Ok(Self {
            installation_id,
            tenant_id,
            slug: slug.clone(),
            version: version.clone(),
            digest: digest.clone(),
        })
    }
}

/// Deployment-owned resolver for one exact artifact execution. Implementations
/// must load only the admitted installation named by `execution`, verify its
/// tenant/lifecycle/policy eligibility, and return a broker for the requested
/// capability. Returning a global or latest-release broker is not permitted.
#[async_trait]
pub trait ArtifactCapabilityBrokerResolver: Send + Sync {
    async fn resolve_broker(
        &self,
        execution: &ArtifactCapabilityExecution,
        capability: &CapabilityName,
    ) -> SandboxResult<Arc<dyn CapabilityBroker>>;
}

/// Resolves one exact artifact installation and proves that its durable policy
/// currently grants `capability`. Dynamic owner routes must use this helper
/// instead of reimplementing admission, tenant-lifecycle, uninstall, and
/// policy-revision checks.
pub async fn resolve_granted_artifact_capability(
    db: &DatabaseConnection,
    execution: &ArtifactCapabilityExecution,
    capability: &CapabilityName,
) -> SandboxResult<InstalledModuleArtifact> {
    if execution.installation_id.is_nil()
        || execution.tenant_id.is_nil()
        || execution.slug.trim().is_empty()
        || execution.version.trim().is_empty()
        || execution.digest.trim().is_empty()
    {
        return Err(SandboxError::CapabilityDenied(capability.clone()));
    }
    let release = ArtifactReleaseRef {
        slug: execution.slug.clone(),
        version: execution.version.clone(),
        digest: execution.digest.clone(),
    };
    let installation = SeaOrmArtifactInstallationStore::new(db.clone())
        .resolve_exact(execution.installation_id, &release, execution.tenant_id)
        .await
        .map_err(|_| SandboxError::CapabilityDenied(capability.clone()))?;
    let policy = SeaOrmArtifactSandboxPolicyResolver::new(db.clone())
        .resolve(&installation, execution.tenant_id)
        .await
        .map_err(|_| SandboxError::CapabilityDenied(capability.clone()))?;
    if policy.grant(capability).is_none() {
        return Err(SandboxError::CapabilityDenied(capability.clone()));
    }
    Ok(installation)
}

/// Capability-name router for independently owned artifact capability
/// resolvers. A deployment explicitly registers each owner; no resolver is
/// selected by module slug, release, or a global fallback.
#[derive(Clone, Default)]
pub struct ArtifactCapabilityBrokerResolverRouter {
    routes: Arc<HashMap<CapabilityName, Arc<dyn ArtifactCapabilityBrokerResolver>>>,
}

impl ArtifactCapabilityBrokerResolverRouter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn route(
        mut self,
        capability: CapabilityName,
        resolver: Arc<dyn ArtifactCapabilityBrokerResolver>,
    ) -> SandboxResult<Self> {
        let routes = Arc::make_mut(&mut self.routes);
        if routes.insert(capability.clone(), resolver).is_some() {
            return Err(SandboxError::InvalidRequest(format!(
                "artifact capability resolver route `{capability}` is already registered"
            )));
        }
        Ok(self)
    }
}

#[async_trait]
impl ArtifactCapabilityBrokerResolver for ArtifactCapabilityBrokerResolverRouter {
    async fn resolve_broker(
        &self,
        execution: &ArtifactCapabilityExecution,
        capability: &CapabilityName,
    ) -> SandboxResult<Arc<dyn CapabilityBroker>> {
        self.routes
            .get(capability)
            .ok_or_else(|| SandboxError::CapabilityDenied(capability.clone()))?
            .resolve_broker(execution, capability)
            .await
    }
}

/// Capability bridge for all installed artifacts in one neutral sandbox
/// runtime. It has no default routes: absent, ineligible, or unresolved owner
/// brokers remain denied.
#[derive(Clone)]
pub struct ResolvingArtifactCapabilityBroker<R> {
    resolver: R,
}

impl<R> ResolvingArtifactCapabilityBroker<R>
where
    R: ArtifactCapabilityBrokerResolver,
{
    pub fn new(resolver: R) -> Self {
        Self { resolver }
    }
}

#[async_trait]
impl<R> CapabilityBroker for ResolvingArtifactCapabilityBroker<R>
where
    R: ArtifactCapabilityBrokerResolver,
{
    async fn invoke(
        &self,
        call: &CapabilityCall,
        grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse> {
        if grant.name != call.capability {
            return Err(SandboxError::CapabilityDenied(call.capability.clone()));
        }
        let execution = ArtifactCapabilityExecution::from_call(call)?;
        let broker = self
            .resolver
            .resolve_broker(&execution, &call.capability)
            .await?;
        broker.invoke(call, grant).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use serde_json::json;
    use uuid::Uuid;

    use rustok_sandbox::{
        CapabilityBroker, CapabilityCall, CapabilityCallContext, CapabilityGrant, CapabilityName,
        CapabilityResponse, ExecutionPhase, SandboxError, SandboxResult, SandboxSubject,
    };

    use super::{
        ArtifactCapabilityBrokerResolver, ArtifactCapabilityExecution,
        ResolvingArtifactCapabilityBroker,
    };

    #[derive(Clone)]
    struct FixtureResolver {
        expected_installation_id: Uuid,
    }

    struct FixtureBroker;

    #[async_trait]
    impl CapabilityBroker for FixtureBroker {
        async fn invoke(
            &self,
            _call: &CapabilityCall,
            _grant: &CapabilityGrant,
        ) -> SandboxResult<CapabilityResponse> {
            Ok(CapabilityResponse {
                output: json!({ "owner": "fixture" }),
            })
        }
    }

    #[async_trait]
    impl ArtifactCapabilityBrokerResolver for FixtureResolver {
        async fn resolve_broker(
            &self,
            execution: &ArtifactCapabilityExecution,
            capability: &CapabilityName,
        ) -> SandboxResult<Arc<dyn CapabilityBroker>> {
            if execution.installation_id != self.expected_installation_id
                || capability.as_str() != "platform.fixture"
            {
                return Err(SandboxError::CapabilityDenied(capability.clone()));
            }
            Ok(Arc::new(FixtureBroker))
        }
    }

    fn call(installation_id: Uuid) -> CapabilityCall {
        CapabilityCall {
            execution_id: Uuid::new_v4(),
            subject: SandboxSubject::ModuleArtifact {
                installation_id,
                slug: "sample_module".to_string(),
                version: "1.0.0".to_string(),
                digest: "sha256:sample".to_string(),
            },
            context: CapabilityCallContext {
                phase: ExecutionPhase::Manual,
                tenant_id: Some(Uuid::new_v4()),
                actor_id: None,
                trace_id: None,
            },
            capability: CapabilityName::new("platform.fixture").expect("capability"),
            operation: "invoke".to_string(),
            input: json!({}),
        }
    }

    #[tokio::test]
    async fn resolving_broker_uses_the_exact_installation_identity() {
        let installation_id = Uuid::new_v4();
        let broker = ResolvingArtifactCapabilityBroker::new(FixtureResolver {
            expected_installation_id: installation_id,
        });
        let request = call(installation_id);
        let grant = CapabilityGrant {
            name: request.capability.clone(),
            constraints: json!({}),
        };
        let response = broker.invoke(&request, &grant).await.expect("response");
        assert_eq!(response.output, json!({ "owner": "fixture" }));

        let rejected_call = call(Uuid::new_v4());
        assert!(broker.invoke(&rejected_call, &grant).await.is_err());
    }
}
