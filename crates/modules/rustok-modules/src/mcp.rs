use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

use rustok_sandbox::{
    CapabilityBroker, CapabilityCall, CapabilityGrant, CapabilityName, CapabilityResponse,
    ExecutionPhase, SandboxError, SandboxResult, SandboxSubject,
};

use crate::data::artifact_data_scope_for_execution;
use crate::{
    ArtifactCapabilityBrokerResolver, ArtifactCapabilityExecution, ArtifactDataScope,
    resolve_granted_artifact_capability,
};

const MAX_ARTIFACT_MCP_OUTPUT_BYTES: usize = 64 * 1024;

/// A host-owned MCP invocation after sandbox admission. The artifact can supply
/// only a pre-authorized server alias, tool name, and JSON arguments. The host
/// receives the immutable artifact scope and execution identity needed to bind
/// this call to its own MCP authorization and audit policies.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactMcpCallRequest {
    pub scope: ArtifactDataScope,
    pub execution_id: Uuid,
    pub subject: SandboxSubject,
    pub phase: ExecutionPhase,
    pub actor_id: Option<String>,
    pub trace_id: Option<String>,
    pub server: String,
    pub tool: String,
    pub arguments: Option<Value>,
}

/// Deployment-owned bridge to an MCP tool owner. Implementations must apply
/// their own server-alias resolution, per-tool authorization, tenant policy,
/// audit, and response-redaction rules; they must not route to an arbitrary
/// network endpoint supplied by an artifact.
#[async_trait]
pub trait ArtifactMcpInvoker: Send + Sync {
    async fn invoke_artifact_mcp(
        &self,
        request: ArtifactMcpCallRequest,
    ) -> Result<Value, ArtifactMcpError>;
}

/// The `platform.mcp` adapter for one admitted artifact scope. It is injected
/// into the neutral sandbox runtime by the deployment and delegates all MCP
/// server resolution, access policy, and audit ownership to its host invoker.
#[derive(Clone)]
pub struct ArtifactMcpCapabilityBroker<I> {
    invoker: I,
    scope: ArtifactDataScope,
}

impl<I> ArtifactMcpCapabilityBroker<I>
where
    I: ArtifactMcpInvoker,
{
    pub fn new(invoker: I, scope: ArtifactDataScope) -> Self {
        Self { invoker, scope }
    }
}

#[async_trait]
impl<I> CapabilityBroker for ArtifactMcpCapabilityBroker<I>
where
    I: ArtifactMcpInvoker,
{
    async fn invoke(
        &self,
        call: &CapabilityCall,
        _grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse> {
        if call.capability.as_str() != "platform.mcp"
            || call.context.tenant_id != Some(self.scope.tenant_id)
            || !matches!(
                &call.subject,
                SandboxSubject::ModuleArtifact { slug, .. } if slug == &self.scope.module_slug
            )
        {
            return Err(SandboxError::CapabilityDenied(call.capability.clone()));
        }

        let target = decode_mcp_capability_call(call)?;
        let output = self
            .invoker
            .invoke_artifact_mcp(ArtifactMcpCallRequest {
                scope: self.scope.clone(),
                execution_id: call.execution_id,
                subject: call.subject.clone(),
                phase: call.context.phase,
                actor_id: call.context.actor_id.clone(),
                trace_id: call.context.trace_id.clone(),
                server: target.server,
                tool: target.tool,
                arguments: target.arguments,
            })
            .await
            .map_err(|error| mcp_capability_error(&call.capability, error))?;

        let output_size = serde_json::to_vec(&output)
            .map_err(|_| mcp_capability_unavailable(&call.capability))?
            .len();
        if output_size > MAX_ARTIFACT_MCP_OUTPUT_BYTES {
            return Err(SandboxError::LimitExceeded {
                resource: "artifact_mcp_output_bytes".to_string(),
                limit: MAX_ARTIFACT_MCP_OUTPUT_BYTES as u64,
            });
        }
        Ok(CapabilityResponse { output })
    }
}

/// Dynamic `platform.mcp` owner route. It derives the MCP authorization scope
/// from the exact admitted installation and delegates only to the deployment's
/// explicit MCP invoker; no artifact-controlled endpoint is ever resolved.
#[derive(Clone)]
pub struct ArtifactMcpCapabilityBrokerResolver<I> {
    db: sea_orm::DatabaseConnection,
    invoker: I,
}

impl<I> ArtifactMcpCapabilityBrokerResolver<I>
where
    I: ArtifactMcpInvoker + Clone,
{
    pub fn new(db: sea_orm::DatabaseConnection, invoker: I) -> Self {
        Self { db, invoker }
    }
}

#[async_trait]
impl<I> ArtifactCapabilityBrokerResolver for ArtifactMcpCapabilityBrokerResolver<I>
where
    I: ArtifactMcpInvoker + Clone + Send + Sync + 'static,
{
    async fn resolve_broker(
        &self,
        execution: &ArtifactCapabilityExecution,
        capability: &CapabilityName,
    ) -> SandboxResult<Arc<dyn CapabilityBroker>> {
        if capability.as_str() != "platform.mcp" {
            return Err(SandboxError::CapabilityDenied(capability.clone()));
        }
        let installation =
            resolve_granted_artifact_capability(&self.db, execution, capability).await?;
        let scope = artifact_data_scope_for_execution(&installation, execution, capability)?;
        Ok(Arc::new(ArtifactMcpCapabilityBroker::new(
            self.invoker.clone(),
            scope,
        )))
    }
}

struct McpCapabilityCall {
    server: String,
    tool: String,
    arguments: Option<Value>,
}

fn decode_mcp_capability_call(call: &CapabilityCall) -> SandboxResult<McpCapabilityCall> {
    if call.operation != "call" {
        return Err(mcp_capability_constraint(
            call,
            "MCP operation is unsupported",
        ));
    }
    let input = call
        .input
        .as_object()
        .ok_or_else(|| mcp_capability_constraint(call, "MCP input must be an object"))?;
    if input
        .keys()
        .any(|field| !matches!(field.as_str(), "server" | "tool" | "arguments"))
    {
        return Err(mcp_capability_constraint(
            call,
            "MCP input contains an unsupported field",
        ));
    }
    Ok(McpCapabilityCall {
        server: required_mcp_name(call, input, "server")?.to_string(),
        tool: required_mcp_name(call, input, "tool")?.to_string(),
        arguments: input.get("arguments").cloned(),
    })
}

fn required_mcp_name<'a>(
    call: &CapabilityCall,
    input: &'a serde_json::Map<String, Value>,
    field: &str,
) -> SandboxResult<&'a str> {
    input
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| valid_mcp_name(value))
        .ok_or_else(|| mcp_capability_constraint(call, &format!("MCP {field} is invalid")))
}

fn valid_mcp_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && !matches!(value.chars().next(), Some('.' | '-' | '_'))
        && !matches!(value.chars().next_back(), Some('.' | '-' | '_'))
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '-' | '.')
        })
}

fn mcp_capability_constraint(call: &CapabilityCall, reason: &str) -> SandboxError {
    SandboxError::CapabilityConstraintDenied {
        capability: call.capability.clone(),
        reason: reason.to_string(),
    }
}

fn mcp_capability_error(capability: &CapabilityName, error: ArtifactMcpError) -> SandboxError {
    match error {
        ArtifactMcpError::InvalidScope
        | ArtifactMcpError::InvalidTarget
        | ArtifactMcpError::Denied => SandboxError::CapabilityDenied(capability.clone()),
        ArtifactMcpError::Unavailable => mcp_capability_unavailable(capability),
    }
}

fn mcp_capability_unavailable(capability: &CapabilityName) -> SandboxError {
    SandboxError::HostCapability {
        capability: capability.clone(),
        message: "artifact MCP capability is unavailable".to_string(),
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArtifactMcpError {
    #[error("artifact MCP scope is invalid")]
    InvalidScope,
    #[error("artifact MCP target is invalid")]
    InvalidTarget,
    #[error("artifact MCP access is denied")]
    Denied,
    #[error("artifact MCP invocation is unavailable")]
    Unavailable,
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use uuid::Uuid;

    use rustok_sandbox::{
        CapabilityCall, CapabilityCallContext, CapabilityName, ExecutionPhase, SandboxSubject,
    };

    use super::decode_mcp_capability_call;

    fn call(input: serde_json::Value) -> CapabilityCall {
        CapabilityCall {
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
            capability: CapabilityName::new("platform.mcp").expect("capability name"),
            operation: "call".to_string(),
            input,
        }
    }

    #[test]
    fn mcp_adapter_accepts_only_a_logical_target_and_arguments() {
        let decoded = decode_mcp_capability_call(&call(json!({
            "server": "rustok",
            "tool": "module_details",
            "arguments": { "slug": "content" }
        })))
        .expect("valid logical MCP target");
        assert_eq!(decoded.server, "rustok");
        assert_eq!(decoded.tool, "module_details");

        assert!(
            decode_mcp_capability_call(&call(json!({
                "server": "rustok",
                "tool": "module_details",
                "endpoint": "https://attacker.invalid"
            })))
            .is_err()
        );
    }
}
