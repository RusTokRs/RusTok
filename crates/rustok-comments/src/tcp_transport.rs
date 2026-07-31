use std::{net::SocketAddr, time::Duration};

use async_trait::async_trait;
use rustok_api::PortError;
use tokio::{net::TcpStream, time::timeout};

use crate::{
    CommentsThreadRequest, CommentsThreadResponse, CommentsThreadTransport,
    CommentsThreadTransportReply,
};

use crate::tcp_protocol::{
    ensure_frame_size, io_error, read_frame, validate_frame_limit, write_frame,
};

pub use crate::tcp_protocol::DEFAULT_MAX_COMMENTS_FRAME_BYTES;

/// Concrete sidecar transport using length-prefixed JSON over TCP.
///
/// Endpoint resolution and authentication remain host-owned. Each operation opens
/// one connection, carries the typed request unchanged, applies the port deadline
/// to the complete exchange, and closes the connection after one typed reply.
#[derive(Clone, Debug)]
pub struct TcpJsonCommentsTransport {
    endpoint: SocketAddr,
    max_frame_bytes: usize,
}

impl TcpJsonCommentsTransport {
    pub fn new(endpoint: SocketAddr) -> Self {
        Self {
            endpoint,
            max_frame_bytes: DEFAULT_MAX_COMMENTS_FRAME_BYTES,
        }
    }

    pub fn with_max_frame_bytes(
        endpoint: SocketAddr,
        max_frame_bytes: usize,
    ) -> Result<Self, PortError> {
        validate_frame_limit(max_frame_bytes)?;
        Ok(Self {
            endpoint,
            max_frame_bytes,
        })
    }

    pub fn endpoint(&self) -> SocketAddr {
        self.endpoint
    }

    pub fn max_frame_bytes(&self) -> usize {
        self.max_frame_bytes
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
}

#[async_trait]
impl CommentsThreadTransport for TcpJsonCommentsTransport {
    async fn execute(
        &self,
        request: CommentsThreadRequest,
    ) -> Result<CommentsThreadResponse, PortError> {
        request.context().require_deadline_semantics()?;
        let deadline_ms = request.context().deadline_ms.unwrap_or_default();
        let request_payload = serde_json::to_vec(&request).map_err(|error| {
            PortError::invariant_violation("comments.tcp_encode", error.to_string())
        })?;
        ensure_frame_size(request_payload.len(), self.max_frame_bytes)?;

        timeout(
            Duration::from_millis(deadline_ms),
            self.exchange(&request_payload),
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
    let reply = serde_json::from_slice::<CommentsThreadTransportReply>(payload).map_err(|error| {
        PortError::invariant_violation("comments.tcp_decode", error.to_string())
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
