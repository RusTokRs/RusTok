//! Host composition for artifact-originated MCP calls.
//!
//! Artifacts address only stable logical aliases. This adapter derives a
//! service identity from the admitted artifact subject, applies the MCP
//! owner's authorization contract, persists redacted audit evidence, and
//! invokes an owner-defined target without exposing transport or credentials.

use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use rustok_core::ModuleRegistry;
use rustok_mcp::{
    McpAccessContext, McpAccessPolicy, McpActorType, McpAuditSink, McpIdentity,
    McpToolCallAuditEvent, McpToolCallOutcome, REGISTRY_TOOL_NAMES, RegistryToolInvocationError,
    default_tool_requirement, invoke_registry_tool,
};
use rustok_modules::{ArtifactMcpCallRequest, ArtifactMcpError, ArtifactMcpInvoker};
use rustok_sandbox::SandboxSubject;
use serde_json::Value;

const RUSTOK_SERVER_ALIAS: &str = "rustok";
const ARTIFACT_MCP_TRANSPORT: &str = "artifact_capability";

#[async_trait]
trait ArtifactMcpServer: Send + Sync {
    fn supports(&self, tool_name: &str) -> bool;

    async fn invoke(
        &self,
        access_context: &McpAccessContext,
        tool_name: &str,
        arguments: Option<Value>,
    ) -> Result<Value, ArtifactMcpError>;
}

struct RegistryArtifactMcpServer {
    registry: ModuleRegistry,
}

#[async_trait]
impl ArtifactMcpServer for RegistryArtifactMcpServer {
    fn supports(&self, tool_name: &str) -> bool {
        REGISTRY_TOOL_NAMES.contains(&tool_name)
    }

    async fn invoke(
        &self,
        access_context: &McpAccessContext,
        tool_name: &str,
        arguments: Option<Value>,
    ) -> Result<Value, ArtifactMcpError> {
        invoke_registry_tool(&self.registry, access_context, tool_name, arguments)
            .await
            .map_err(map_registry_error)
    }
}

/// Deployment-owned alias registry and `ArtifactMcpInvoker` implementation.
/// The initial stable `rustok` alias exposes only the owner-defined, read-only
/// registry tool surface. Additional aliases require an explicit host adapter.
#[derive(Clone)]
pub struct ServerArtifactMcpInvoker {
    servers: Arc<BTreeMap<String, Arc<dyn ArtifactMcpServer>>>,
    audit: Arc<dyn McpAuditSink>,
}

impl ServerArtifactMcpInvoker {
    pub fn local_registry(registry: ModuleRegistry, audit: Arc<dyn McpAuditSink>) -> Self {
        let mut servers: BTreeMap<String, Arc<dyn ArtifactMcpServer>> = BTreeMap::new();
        servers.insert(
            RUSTOK_SERVER_ALIAS.to_string(),
            Arc::new(RegistryArtifactMcpServer { registry }),
        );
        Self {
            servers: Arc::new(servers),
            audit,
        }
    }

    async fn record(
        &self,
        request: &ArtifactMcpCallRequest,
        identity: Option<McpIdentity>,
        outcome: McpToolCallOutcome,
        reason: Option<&str>,
    ) -> Result<(), ArtifactMcpError> {
        let subject_metadata = match &request.subject {
            SandboxSubject::ModuleArtifact {
                installation_id,
                slug,
                version,
                digest,
            } => serde_json::json!({
                "installation_id": installation_id,
                "module_slug": slug,
                "version": version,
                "artifact_digest": digest,
            }),
            _ => serde_json::json!({ "subject": "invalid" }),
        };
        self.audit
            .record_tool_call(McpToolCallAuditEvent {
                transport: ARTIFACT_MCP_TRANSPORT.to_string(),
                correlation_id: request
                    .trace_id
                    .clone()
                    .or_else(|| Some(request.execution_id.to_string())),
                tenant_id: Some(request.scope.tenant_id.to_string()),
                client_id: None,
                token_id: None,
                identity,
                tool_name: request.tool.clone(),
                outcome,
                reason: reason.map(str::to_string),
                metadata: serde_json::json!({
                    "server_alias": request.server,
                    "execution_id": request.execution_id,
                    "phase": format!("{:?}", request.phase).to_lowercase(),
                    "data_contract_revision": request.scope.data_contract_revision,
                    "policy_revision": request.scope.policy_revision,
                    "subject": subject_metadata,
                }),
            })
            .await
            .map_err(|_| ArtifactMcpError::Unavailable)
    }
}

#[async_trait]
impl ArtifactMcpInvoker for ServerArtifactMcpInvoker {
    async fn invoke_artifact_mcp(
        &self,
        request: ArtifactMcpCallRequest,
    ) -> Result<Value, ArtifactMcpError> {
        let identity = match artifact_identity(&request) {
            Ok(identity) => identity,
            Err(error) => {
                self.record(
                    &request,
                    None,
                    McpToolCallOutcome::Denied,
                    Some("invalid_artifact_scope"),
                )
                .await?;
                return Err(error);
            }
        };
        let Some(server) = self.servers.get(&request.server) else {
            self.record(
                &request,
                Some(identity),
                McpToolCallOutcome::Denied,
                Some("unknown_server_alias"),
            )
            .await?;
            return Err(ArtifactMcpError::InvalidTarget);
        };
        if !server.supports(&request.tool) {
            self.record(
                &request,
                Some(identity),
                McpToolCallOutcome::Denied,
                Some("unsupported_tool"),
            )
            .await?;
            return Err(ArtifactMcpError::InvalidTarget);
        }

        let access_context = artifact_access_context(identity.clone(), &request.tool);
        let decision = access_context.authorize_tool(&default_tool_requirement(&request.tool));
        if !decision.allowed {
            self.record(
                &request,
                Some(identity),
                McpToolCallOutcome::Denied,
                Some("access_denied"),
            )
            .await?;
            return Err(ArtifactMcpError::Denied);
        }

        self.record(&request, Some(identity), McpToolCallOutcome::Allowed, None)
            .await?;
        server
            .invoke(&access_context, &request.tool, request.arguments)
            .await
    }
}

fn artifact_identity(request: &ArtifactMcpCallRequest) -> Result<McpIdentity, ArtifactMcpError> {
    let SandboxSubject::ModuleArtifact {
        installation_id,
        slug,
        version,
        digest,
    } = &request.subject
    else {
        return Err(ArtifactMcpError::InvalidScope);
    };
    if slug != &request.scope.module_slug
        || version.trim().is_empty()
        || digest.trim().is_empty()
        || request.scope.data_contract_revision == 0
        || request.scope.policy_revision == 0
    {
        return Err(ArtifactMcpError::InvalidScope);
    }

    Ok(McpIdentity {
        actor_id: format!("artifact:{installation_id}"),
        actor_type: McpActorType::ServiceClient,
        tenant_id: Some(request.scope.tenant_id.to_string()),
        delegated_user_id: request.actor_id.clone(),
        display_name: Some(format!("{} artifact", request.scope.module_slug)),
        scopes: vec![
            format!("tenant:{}", request.scope.tenant_id),
            format!("module:{}", request.scope.module_slug),
        ],
    })
}

fn artifact_access_context(identity: McpIdentity, tool_name: &str) -> McpAccessContext {
    let requirement = default_tool_requirement(tool_name);
    McpAccessContext {
        identity: Some(identity),
        granted_permissions: requirement.required_permissions,
        policy: McpAccessPolicy {
            allowed_tools: Some(vec![tool_name.to_string()]),
            denied_tools: Vec::new(),
        },
    }
}

fn map_registry_error(error: RegistryToolInvocationError) -> ArtifactMcpError {
    match error {
        RegistryToolInvocationError::Denied => ArtifactMcpError::Denied,
        RegistryToolInvocationError::InvalidArguments
        | RegistryToolInvocationError::UnsupportedTool => ArtifactMcpError::InvalidTarget,
        RegistryToolInvocationError::Serialization => ArtifactMcpError::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use anyhow::anyhow;
    use rustok_mcp::McpToolCallAuditEvent;
    use rustok_modules::{ArtifactDataScope, ArtifactMcpInvoker};
    use rustok_sandbox::{ExecutionPhase, SandboxSubject};
    use uuid::Uuid;

    use super::*;

    #[derive(Default)]
    struct RecordingAudit {
        events: Mutex<Vec<McpToolCallAuditEvent>>,
        fail: bool,
    }

    #[async_trait]
    impl McpAuditSink for RecordingAudit {
        async fn record_tool_call(&self, event: McpToolCallAuditEvent) -> anyhow::Result<()> {
            if self.fail {
                return Err(anyhow!("audit unavailable"));
            }
            self.events.lock().expect("audit lock").push(event);
            Ok(())
        }
    }

    fn request(server: &str, tool: &str) -> ArtifactMcpCallRequest {
        let tenant_id = Uuid::new_v4();
        let installation_id = Uuid::new_v4();
        ArtifactMcpCallRequest {
            scope: ArtifactDataScope {
                tenant_id,
                module_slug: "external_sample".to_string(),
                data_contract_revision: 1,
                policy_revision: 2,
            },
            execution_id: Uuid::new_v4(),
            subject: SandboxSubject::ModuleArtifact {
                installation_id,
                slug: "external_sample".to_string(),
                version: "1.0.0".to_string(),
                digest: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
            },
            phase: ExecutionPhase::Manual,
            actor_id: Some(Uuid::new_v4().to_string()),
            trace_id: None,
            server: server.to_string(),
            tool: tool.to_string(),
            arguments: None,
        }
    }

    #[tokio::test]
    async fn invokes_the_owner_registry_surface_and_redacts_arguments_from_audit() {
        let audit = Arc::new(RecordingAudit::default());
        let invoker =
            ServerArtifactMcpInvoker::local_registry(ModuleRegistry::new(), audit.clone());

        let output = invoker
            .invoke_artifact_mcp(request(RUSTOK_SERVER_ALIAS, rustok_mcp::TOOL_LIST_MODULES))
            .await
            .expect("artifact MCP invocation");

        assert_eq!(output["ok"], serde_json::json!(true));
        let events = audit.events.lock().expect("audit lock");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].outcome, McpToolCallOutcome::Allowed);
        assert!(events[0].metadata.get("arguments").is_none());
    }

    #[tokio::test]
    async fn rejects_and_audits_unknown_aliases() {
        let audit = Arc::new(RecordingAudit::default());
        let invoker =
            ServerArtifactMcpInvoker::local_registry(ModuleRegistry::new(), audit.clone());

        let result = invoker
            .invoke_artifact_mcp(request("external", rustok_mcp::TOOL_LIST_MODULES))
            .await;

        assert_eq!(result, Err(ArtifactMcpError::InvalidTarget));
        let events = audit.events.lock().expect("audit lock");
        assert_eq!(events[0].outcome, McpToolCallOutcome::Denied);
        assert_eq!(events[0].reason.as_deref(), Some("unknown_server_alias"));
    }

    #[tokio::test]
    async fn fails_closed_when_durable_audit_is_unavailable() {
        let audit = Arc::new(RecordingAudit {
            events: Mutex::new(Vec::new()),
            fail: true,
        });
        let invoker = ServerArtifactMcpInvoker::local_registry(ModuleRegistry::new(), audit);

        let result = invoker
            .invoke_artifact_mcp(request(RUSTOK_SERVER_ALIAS, rustok_mcp::TOOL_LIST_MODULES))
            .await;

        assert_eq!(result, Err(ArtifactMcpError::Unavailable));
    }
}
