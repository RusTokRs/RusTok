use std::{fmt, net::SocketAddr, sync::Arc, time::Duration};

use async_trait::async_trait;
use rustok_api::{PortActorKind, PortError};
use tokio::time::timeout;

use crate::{
    CommentsTcpAuthenticationConfigError, CommentsTcpBearerToken, CommentsTcpChannelProtection,
    CommentsTcpClientChannelConnector, CommentsTcpDelegationConfigError,
    CommentsTcpDelegationSecret, CommentsTcpDelegationSigner, CommentsTcpOperation,
    CommentsTcpRequestEnvelope, CommentsThreadRequest, CommentsThreadResponse,
    CommentsThreadTransport, CommentsThreadTransportReply, PlaintextLoopbackCommentsTcpChannel,
};

use crate::tcp_protocol::{ensure_frame_size, read_frame, validate_frame_limit, write_frame};

pub use crate::tcp_protocol::DEFAULT_MAX_COMMENTS_FRAME_BYTES;

/// Concrete sidecar transport using length-prefixed JSON over an injected byte channel.
///
/// Each operation opens one channel, wraps the typed request in a versioned
/// credential envelope, applies the port deadline to preparation and the complete
/// exchange, and closes the channel after one typed reply. Reads and the
/// host-owned system moderation path use the configured service bearer. User-
/// owned writes use a short-lived signed delegation when a signer is configured.
///
/// The default connector is the built-in plaintext loopback-only compatibility
/// channel. A host may inject an authenticated encrypted connector without
/// changing framing, typed requests, bearer authentication, or user delegation.
#[derive(Clone)]
pub struct TcpJsonCommentsTransport {
    endpoint: SocketAddr,
    channel_connector: Arc<dyn CommentsTcpClientChannelConnector>,
    bearer_token: Option<CommentsTcpBearerToken>,
    delegation_signer: Option<CommentsTcpDelegationSigner>,
    max_frame_bytes: usize,
}

impl TcpJsonCommentsTransport {
    pub fn new(endpoint: SocketAddr) -> Self {
        Self::with_channel_connector(endpoint, plaintext_channel_connector())
    }

    pub fn with_channel_connector(
        endpoint: SocketAddr,
        channel_connector: Arc<dyn CommentsTcpClientChannelConnector>,
    ) -> Self {
        Self {
            endpoint,
            channel_connector,
            bearer_token: None,
            delegation_signer: None,
            max_frame_bytes: DEFAULT_MAX_COMMENTS_FRAME_BYTES,
        }
    }

    pub fn with_bearer_token(endpoint: SocketAddr, bearer_token: CommentsTcpBearerToken) -> Self {
        Self::with_channel_connector_and_bearer_token(
            endpoint,
            plaintext_channel_connector(),
            bearer_token,
        )
    }

    pub fn with_channel_connector_and_bearer_token(
        endpoint: SocketAddr,
        channel_connector: Arc<dyn CommentsTcpClientChannelConnector>,
        bearer_token: CommentsTcpBearerToken,
    ) -> Self {
        Self {
            endpoint,
            channel_connector,
            bearer_token: Some(bearer_token),
            delegation_signer: None,
            max_frame_bytes: DEFAULT_MAX_COMMENTS_FRAME_BYTES,
        }
    }

    pub fn with_bearer_and_delegation(
        endpoint: SocketAddr,
        bearer_token: CommentsTcpBearerToken,
        delegation_signer: CommentsTcpDelegationSigner,
    ) -> Self {
        Self::with_channel_connector_bearer_and_delegation(
            endpoint,
            plaintext_channel_connector(),
            bearer_token,
            delegation_signer,
        )
    }

    pub fn with_channel_connector_bearer_and_delegation(
        endpoint: SocketAddr,
        channel_connector: Arc<dyn CommentsTcpClientChannelConnector>,
        bearer_token: CommentsTcpBearerToken,
        delegation_signer: CommentsTcpDelegationSigner,
    ) -> Self {
        Self {
            endpoint,
            channel_connector,
            bearer_token: Some(bearer_token),
            delegation_signer: Some(delegation_signer),
            max_frame_bytes: DEFAULT_MAX_COMMENTS_FRAME_BYTES,
        }
    }

    pub fn with_bearer_secret(
        endpoint: SocketAddr,
        secret: impl AsRef<str>,
    ) -> Result<Self, CommentsTcpAuthenticationConfigError> {
        Ok(Self::with_bearer_token(
            endpoint,
            CommentsTcpBearerToken::new(secret)?,
        ))
    }

    pub fn with_bearer_and_delegation_secrets(
        endpoint: SocketAddr,
        bearer_secret: impl AsRef<str>,
        delegation_secret: impl AsRef<str>,
    ) -> Result<Self, CommentsTcpTransportConfigError> {
        let bearer = CommentsTcpBearerToken::new(bearer_secret)
            .map_err(CommentsTcpTransportConfigError::Authentication)?;
        let delegation = CommentsTcpDelegationSecret::new(delegation_secret)
            .map_err(CommentsTcpTransportConfigError::Delegation)?;
        Ok(Self::with_bearer_and_delegation(
            endpoint,
            bearer,
            CommentsTcpDelegationSigner::new(delegation),
        ))
    }

    pub fn with_max_frame_bytes(
        endpoint: SocketAddr,
        max_frame_bytes: usize,
    ) -> Result<Self, PortError> {
        Self::with_channel_connector_and_max_frame_bytes(
            endpoint,
            plaintext_channel_connector(),
            max_frame_bytes,
        )
    }

    pub fn with_channel_connector_and_max_frame_bytes(
        endpoint: SocketAddr,
        channel_connector: Arc<dyn CommentsTcpClientChannelConnector>,
        max_frame_bytes: usize,
    ) -> Result<Self, PortError> {
        validate_frame_limit(max_frame_bytes)?;
        Ok(Self {
            endpoint,
            channel_connector,
            bearer_token: None,
            delegation_signer: None,
            max_frame_bytes,
        })
    }

    pub fn with_bearer_token_and_max_frame_bytes(
        endpoint: SocketAddr,
        bearer_token: CommentsTcpBearerToken,
        max_frame_bytes: usize,
    ) -> Result<Self, PortError> {
        validate_frame_limit(max_frame_bytes)?;
        Ok(Self {
            endpoint,
            channel_connector: plaintext_channel_connector(),
            bearer_token: Some(bearer_token),
            delegation_signer: None,
            max_frame_bytes,
        })
    }

    pub fn with_bearer_and_delegation_and_max_frame_bytes(
        endpoint: SocketAddr,
        bearer_token: CommentsTcpBearerToken,
        delegation_signer: CommentsTcpDelegationSigner,
        max_frame_bytes: usize,
    ) -> Result<Self, PortError> {
        validate_frame_limit(max_frame_bytes)?;
        Ok(Self {
            endpoint,
            channel_connector: plaintext_channel_connector(),
            bearer_token: Some(bearer_token),
            delegation_signer: Some(delegation_signer),
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
        self.bearer_token.is_some()
    }

    pub fn supports_delegated_writes(&self) -> bool {
        self.delegation_signer.is_some()
    }

    async fn exchange(&self, request_payload: &[u8]) -> Result<CommentsThreadResponse, PortError> {
        let mut channel = self.channel_connector.connect(self.endpoint).await?;
        write_frame(&mut *channel, request_payload, self.max_frame_bytes).await?;
        let response_payload = read_frame(&mut *channel, self.max_frame_bytes).await?;
        decode_reply(&response_payload)
    }

    async fn prepare_and_exchange(
        &self,
        request: CommentsThreadRequest,
    ) -> Result<CommentsThreadResponse, PortError> {
        let operation = CommentsTcpOperation::for_request(&request);
        let service_moderation = operation == CommentsTcpOperation::SetCommentStatus
            && request.context().actor.kind == PortActorKind::System;
        let envelope = if operation.is_write() && !service_moderation {
            match self.delegation_signer.as_ref() {
                Some(signer) => {
                    let credential = signer.credential_for(&request)?;
                    CommentsTcpRequestEnvelope::with_credential(request, credential)
                }
                None => match self.bearer_token.as_ref() {
                    Some(token) => CommentsTcpRequestEnvelope::with_bearer(request, token),
                    None => CommentsTcpRequestEnvelope::unauthenticated(request),
                },
            }
        } else {
            match self.bearer_token.as_ref() {
                Some(token) => CommentsTcpRequestEnvelope::with_bearer(request, token),
                None => CommentsTcpRequestEnvelope::unauthenticated(request),
            }
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

impl fmt::Debug for TcpJsonCommentsTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TcpJsonCommentsTransport")
            .field("endpoint", &self.endpoint)
            .field("channel_protection", &self.channel_connector.protection())
            .field("bearer_token", &self.bearer_token)
            .field("delegation_signer", &self.delegation_signer)
            .field("max_frame_bytes", &self.max_frame_bytes)
            .finish()
    }
}

fn plaintext_channel_connector() -> Arc<dyn CommentsTcpClientChannelConnector> {
    Arc::new(PlaintextLoopbackCommentsTcpChannel)
}

#[derive(Debug, thiserror::Error)]
pub enum CommentsTcpTransportConfigError {
    #[error(transparent)]
    Authentication(CommentsTcpAuthenticationConfigError),
    #[error(transparent)]
    Delegation(CommentsTcpDelegationConfigError),
}

#[async_trait]
impl CommentsThreadTransport for TcpJsonCommentsTransport {
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

fn decode_reply(payload: &[u8]) -> Result<CommentsThreadResponse, PortError> {
    let reply = serde_json::from_slice::<CommentsThreadTransportReply>(payload).map_err(|_| {
        PortError::invariant_violation(
            "comments.tcp_decode",
            "comments TCP reply is not a valid typed envelope",
        )
    })?;
    match reply {
        CommentsThreadTransportReply::Success(response) => Ok(*response),
        CommentsThreadTransportReply::Error(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rustok_api::PortErrorKind;

    use super::*;

    #[test]
    fn tcp_transport_is_injectable_without_connecting() {
        let endpoint = "127.0.0.1:1".parse().unwrap();
        let transport: Arc<dyn CommentsThreadTransport> =
            Arc::new(TcpJsonCommentsTransport::new(endpoint));
        let _ = transport;
    }

    #[test]
    fn default_transport_is_plaintext_loopback() {
        let endpoint = "127.0.0.1:1".parse().unwrap();
        let transport = TcpJsonCommentsTransport::new(endpoint);
        assert_eq!(
            transport.channel_protection(),
            CommentsTcpChannelProtection::PlaintextLoopback
        );
    }

    #[test]
    fn bearer_transport_debug_is_redacted() {
        let endpoint = "127.0.0.1:1".parse().unwrap();
        let transport =
            TcpJsonCommentsTransport::with_bearer_secret(endpoint, "comments-secret").unwrap();
        let debug = format!("{transport:?}");

        assert!(transport.is_authenticated());
        assert!(!transport.supports_delegated_writes());
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("comments-secret"));
    }

    #[test]
    fn delegated_transport_debug_is_redacted() {
        let endpoint = "127.0.0.1:1".parse().unwrap();
        let transport = TcpJsonCommentsTransport::with_bearer_and_delegation_secrets(
            endpoint,
            "comments-read-secret",
            "0123456789abcdef0123456789abcdef",
        )
        .unwrap();
        let debug = format!("{transport:?}");

        assert!(transport.supports_delegated_writes());
        assert!(!debug.contains("comments-read-secret"));
        assert!(!debug.contains("0123456789abcdef"));
    }

    #[test]
    fn provider_error_reply_is_preserved() {
        let expected = PortError::validation("comments.exact", "exact provider error");
        let payload =
            serde_json::to_vec(&CommentsThreadTransportReply::Error(expected.clone())).unwrap();

        assert_eq!(decode_reply(&payload).unwrap_err(), expected);
    }

    #[test]
    fn invalid_frame_limit_fails_closed() {
        let endpoint = "127.0.0.1:1".parse().unwrap();
        let error = TcpJsonCommentsTransport::with_max_frame_bytes(endpoint, 0).unwrap_err();

        assert_eq!(error.kind, PortErrorKind::Validation);
        assert_eq!(error.code, "comments.tcp_invalid_frame_limit");
    }
}
