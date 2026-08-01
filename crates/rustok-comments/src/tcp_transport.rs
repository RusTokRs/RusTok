use std::{net::SocketAddr, time::Duration};

use async_trait::async_trait;
use rustok_api::PortError;
use tokio::{net::TcpStream, time::timeout};

use crate::{
    CommentsTcpAuthenticationConfigError, CommentsTcpBearerToken,
    CommentsTcpDelegationConfigError, CommentsTcpDelegationSecret, CommentsTcpDelegationSigner,
    CommentsTcpOperation, CommentsTcpRequestEnvelope, CommentsThreadRequest,
    CommentsThreadResponse, CommentsThreadTransport, CommentsThreadTransportReply,
};

use crate::tcp_protocol::{
    ensure_frame_size, io_error, read_frame, validate_frame_limit, write_frame,
};

pub use crate::tcp_protocol::DEFAULT_MAX_COMMENTS_FRAME_BYTES;

/// Concrete sidecar transport using length-prefixed JSON over TCP.
///
/// Each operation opens one connection, wraps the typed request in a versioned
/// credential envelope, applies the port deadline to preparation and the complete
/// exchange, and closes the connection after one typed reply. Reads use the
/// configured service bearer. Owner writes use a short-lived signed user
/// delegation when a signer is configured.
#[derive(Clone, Debug)]
pub struct TcpJsonCommentsTransport {
    endpoint: SocketAddr,
    bearer_token: Option<CommentsTcpBearerToken>,
    delegation_signer: Option<CommentsTcpDelegationSigner>,
    max_frame_bytes: usize,
}

impl TcpJsonCommentsTransport {
    pub fn new(endpoint: SocketAddr) -> Self {
        Self {
            endpoint,
            bearer_token: None,
            delegation_signer: None,
            max_frame_bytes: DEFAULT_MAX_COMMENTS_FRAME_BYTES,
        }
    }

    pub fn with_bearer_token(endpoint: SocketAddr, bearer_token: CommentsTcpBearerToken) -> Self {
        Self {
            endpoint,
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
        Self {
            endpoint,
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
        validate_frame_limit(max_frame_bytes)?;
        Ok(Self {
            endpoint,
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

    pub fn is_authenticated(&self) -> bool {
        self.bearer_token.is_some()
    }

    pub fn supports_delegated_writes(&self) -> bool {
        self.delegation_signer.is_some()
    }

    async fn exchange(
        &self,
        request_payload: &[u8],
    ) -> Result<CommentsThreadResponse, PortError> {
        let mut stream = TcpStream::connect(self.endpoint)
            .await
            .map_err(|error| io_error("connect", error))?;
        stream
            .set_nodelay(true)
            .map_err(|error| io_error("set_nodelay", error))?;

        write_frame(&mut stream, request_payload, self.max_frame_bytes).await?;
        let response_payload = read_frame(&mut stream, self.max_frame_bytes).await?;
        decode_reply(&response_payload)
    }

    async fn prepare_and_exchange(
        &self,
        request: CommentsThreadRequest,
    ) -> Result<CommentsThreadResponse, PortError> {
        let operation = CommentsTcpOperation::for_request(&request);
        let envelope = if operation.is_write() {
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
        CommentsThreadTransportReply::Success(response) => Ok(response),
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
        let payload = serde_json::to_vec(&CommentsThreadTransportReply::Error(expected.clone()))
            .unwrap();

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
