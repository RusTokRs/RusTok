use rustok_modules::{
    ModuleArtifactNodeAssignmentHeartbeatReceipt, ModuleArtifactNodeAssignmentReport,
    ModuleArtifactNodeAssignmentReportReceipt, ModuleArtifactNodeAssignmentWorkItem,
};
use tokio::sync::Mutex;
use tonic::Request;
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};
use uuid::Uuid;

use crate::ARTIFACT_NODE_AGENT_PROTOCOL_REVISION;
use crate::proto::artifact_node_service_client::ArtifactNodeServiceClient;
use crate::proto::{
    ClaimAssignmentRequest, HeartbeatAssignmentRequest, ReportAssignmentRequest,
    claim_assignment_response,
};

/// Node-agent client. The only public connection constructor requires
/// deployment-provided mTLS material; there is no plaintext or in-process
/// fallback.
pub struct GrpcArtifactNodeAgent {
    client: Mutex<ArtifactNodeServiceClient<Channel>>,
}

impl GrpcArtifactNodeAgent {
    pub(crate) fn from_channel(channel: Channel) -> Self {
        Self {
            client: Mutex::new(
                ArtifactNodeServiceClient::new(channel)
                    .max_decoding_message_size(crate::ARTIFACT_NODE_AGENT_MAX_MESSAGE_SIZE)
                    .max_encoding_message_size(crate::ARTIFACT_NODE_AGENT_MAX_MESSAGE_SIZE),
            ),
        }
    }

    pub async fn connect_with_tls(
        endpoint: Endpoint,
        tls_config: ClientTlsConfig,
    ) -> Result<Self, String> {
        let channel = endpoint
            .tls_config(tls_config)
            .map_err(|error| error.to_string())?
            .connect()
            .await
            .map_err(|error| error.to_string())?;
        Ok(Self::from_channel(channel))
    }

    pub async fn claim_assignment(
        &self,
    ) -> Result<Option<ModuleArtifactNodeAssignmentWorkItem>, String> {
        let response = self
            .client
            .lock()
            .await
            .claim_assignment(Request::new(ClaimAssignmentRequest {
                protocol_revision: ARTIFACT_NODE_AGENT_PROTOCOL_REVISION,
            }))
            .await
            .map_err(|error| error.to_string())?
            .into_inner();
        validate_protocol_revision(response.protocol_revision)?;
        match response.result {
            Some(claim_assignment_response::Result::WorkItemJson(payload)) => {
                serde_json::from_slice(&payload)
                    .map(Some)
                    .map_err(|_| "artifact node assignment response is invalid".to_string())
            }
            Some(claim_assignment_response::Result::NoAssignment(_)) => Ok(None),
            None => Err("artifact node assignment response is missing".to_string()),
        }
    }

    pub async fn heartbeat_assignment(
        &self,
        claim_id: Uuid,
    ) -> Result<ModuleArtifactNodeAssignmentHeartbeatReceipt, String> {
        let response = self
            .client
            .lock()
            .await
            .heartbeat_assignment(Request::new(HeartbeatAssignmentRequest {
                protocol_revision: ARTIFACT_NODE_AGENT_PROTOCOL_REVISION,
                claim_id: claim_id.to_string(),
            }))
            .await
            .map_err(|error| error.to_string())?
            .into_inner();
        validate_protocol_revision(response.protocol_revision)?;
        serde_json::from_slice(&response.receipt_json)
            .map_err(|_| "artifact node heartbeat receipt is invalid".to_string())
    }

    pub async fn report_assignment(
        &self,
        report: &ModuleArtifactNodeAssignmentReport,
    ) -> Result<ModuleArtifactNodeAssignmentReportReceipt, String> {
        let report_json = serde_json::to_vec(report)
            .map_err(|_| "artifact node report cannot be encoded".to_string())?;
        if report_json.len() > crate::ARTIFACT_NODE_AGENT_MAX_MESSAGE_SIZE {
            return Err("artifact node report exceeds the message limit".to_string());
        }
        let response = self
            .client
            .lock()
            .await
            .report_assignment(Request::new(ReportAssignmentRequest {
                protocol_revision: ARTIFACT_NODE_AGENT_PROTOCOL_REVISION,
                report_json,
            }))
            .await
            .map_err(|error| error.to_string())?
            .into_inner();
        validate_protocol_revision(response.protocol_revision)?;
        serde_json::from_slice(&response.receipt_json)
            .map_err(|_| "artifact node report receipt is invalid".to_string())
    }
}

fn validate_protocol_revision(revision: u32) -> Result<(), String> {
    if revision == ARTIFACT_NODE_AGENT_PROTOCOL_REVISION {
        Ok(())
    } else {
        Err("artifact node agent protocol revision does not match".to_string())
    }
}
