use std::{fmt, net::SocketAddr, sync::Arc, time::Duration};

use async_trait::async_trait;
use rustok_api::{PortActorKind, PortError};
use tokio::time::timeout;

use crate::{
    CommentsTcpBearerToken, CommentsTcpChannelProtection, CommentsTcpClientChannelConnector,
    CommentsTcpOperation, CommentsTcpRequestEnvelope, CommentsThreadRequest,
    CommentsThreadResponse, CommentsThreadTransport, CommentsThreadTransportReply,
    ReloadableCommentsTcpDelegationSigner,
};
use crate::tcp_protocol::{
    DEFAULT_MAX_COMMENTS_FRAME_BYTES, ensure_frame_size, read_frame,
    validate_frame_limit, write_frame,
};

/// One-request-per-channel Comments TCP transport whose delegated user-write
/// signer obtains the current immutable keyring snapshot at each new operation.
#[derive(Clone)]
pub struct ReloadableTcpJsonCommentsTransport {
    endpoint: SocketAddr,
    channel_connector: Arc<dyn CommentsTcpClientChannelConnector>,
    bearer_token: CommentsTcpBearerToken,
    delegation_signer: ReloadableCommentsTcpDelegationSigner,
    max_frame_bytes: usize,
}

impl ReloadableTcpJsonCommentsTransport {
    pub fn with_channel_connector_bearer_and_delegation(
        endpoint: SocketAddr,
        channel_connector: Arc<dyn CommentsTcpClientChannelConnector>,
        bearer_token: CommentsTcpBearerToken,
        delegation_signer: ReloadableCommentsTcpDelegationSigner,
    ) -> Self {
        Self {
            endpoint,
            channel_connector,
            bearer_token,
            delegation_signer,
            max_frame_bytes: DEFAULT_MAX_COMMENTS_FRAME_BYTES,
        }
    }

    pub fn with_channel_connector_bearer_delegation_and_max_frame_bytes(
        endpoint: SocketAddr,
        channel_connector: Arc<dyn CommentsTcpClientChannelConnector>,
        bearer_token: CommentsTcpBearerToken,
        delegation_signer: ReloadableCommentsTcpDelegationSigner,
        max_frame_bytes: usize,
    ) -> Result<Self, PortError> {
        validate_frame_limit(max_frame_bytes)?;
        Ok(Self {
            endpoint,
            channel_connector,
            bearer_token,
            delegation_signer,
            max_frame_bytes,
        })
    }

    pub fn endpoint(&self) -> SocketAddr {
        self.endpoint
    }

    pub fn max_frame_bytes(&self) -> usize {
        self.max_frame_bytes
    }

    pub fn channel_protection(&self) -> CommentsTcpChannelProtection {
        self.channel_connector.protection()
    }

    pub fn is_authenticated(&self) -> bool {
        true
    }

    pub fn supports_delegated_writes(&self) -> bool {
        true
    }

    async fn exchange(
        &self,
        request_payload: &[u8],
    ) -> Result<CommentsThreadResponse, PortError> {
        let mut channel = self.channel_connector.connect(self.endpoint).await?;
        write_frame(&mut *channel, request_payload, self.max_frame_bytes).await?;
        let response_payload = read_frame(&mut *channel, self.max_frame_bytes).await?;
        decode_reloadable_reply(&response_payload)
    }

    async fn prepare_and_exchange(
        &self,
        request: CommentsThreadRequest,
    ) -> Result<CommentsThreadResponse, PortError> {
        let operation = CommentsTcpOperation::for_request(&request);
        let service_moderation = operation == CommentsTcpOperation::SetCommentStatus
            && request.context().actor.kind == PortActorKind::System;
        let envelope = if operation.is_write() && !service_moderation {
            let credential = self.delegation_signer.credential_for(&request)?;
            CommentsTcpRequestEnvelope::with_credential(request, credential)
        } else {
            CommentsTcpRequestEnvelope::with_bearer(request, &self.bearer_token)
        };
        let request_payload = serde_json::to_vec(&envelope).map_err(|_| {
            PortError::invariant_violation(
                "comments.tcp_encode",
                "comments TCP request envelope could not be encoded",
            )
        })?;
        ensure_frame_size(request_payload.len(), self.max_frame_bytes)?;
        self.exchange(&request_payload).await
    }
}

impl fmt::Debug for ReloadableTcpJsonCommentsTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReloadableTcpJsonCommentsTransport")
            .field("endpoint", &self.endpoint)
            .field("channel_protection", &self.channel_connector.protection())
            .field("bearer_token", &self.bearer_token)
            .field("delegation_signer", &self.delegation_signer)
            .field("max_frame_bytes", &self.max_frame_bytes)
            .finish()
    }
}

#[async_trait]
impl CommentsThreadTransport for ReloadableTcpJsonCommentsTransport {
    async fn execute(
        &self,
        request: CommentsThreadRequest,
    ) -> Result<CommentsThreadResponse, PortError> {
        request.context().require_deadline_semantics()?;
        let deadline_ms = request.context().deadline_ms.unwrap_or_default();
        timeout(
            Duration::from_millis(deadline_ms),
            self.prepare_and_exchange(request),
        )
        .await
        .map_err(|_| {
            PortError::timeout(
                "comments.tcp_timeout",
                "comments sidecar call exceeded the port deadline",
            )
        })?
    }
}

fn decode_reloadable_reply(payload: &[u8]) -> Result<CommentsThreadResponse, PortError> {
    let reply = serde_json::from_slice::<CommentsThreadTransportReply>(payload).map_err(|_| {
        PortError::invariant_violation(
            "comments.tcp_decode",
            "comments TCP reply is not a valid typed envelope",
        )
    })?;
    match reply {
        CommentsThreadTransportReply::Success(response) => Ok(response),
        CommentsThreadTransportReply::Error(error) => Err(error),
    }
}
