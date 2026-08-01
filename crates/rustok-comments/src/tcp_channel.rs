use std::{fmt, net::SocketAddr};

use async_trait::async_trait;
use rustok_api::PortError;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

use crate::tcp_protocol::io_error;

/// Async byte channel consumed by the Comments framing and typed transport layers.
///
/// Implementations may be a loopback `TcpStream`, a TLS stream, or another
/// authenticated encrypted channel. Framing and Comments authorization do not
/// depend on the concrete channel type.
pub trait CommentsTcpIo: AsyncRead + AsyncWrite + Unpin + Send {}

impl<T> CommentsTcpIo for T where T: AsyncRead + AsyncWrite + Unpin + Send {}

pub type BoxCommentsTcpIo = Box<dyn CommentsTcpIo>;

/// Closed transport-protection classification exposed by channel adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommentsTcpChannelProtection {
    /// Unencrypted transport that is valid only for loopback endpoints and peers.
    PlaintextLoopback,
    /// Host-provided channel that authenticates both peers and encrypts bytes.
    AuthenticatedEncrypted,
}

/// Client-side channel establishment seam.
///
/// A connector owns DNS/SNI, certificate verification, handshake bounds, or any
/// equivalent protected-channel policy. It returns a byte channel only after its
/// policy succeeds.
#[async_trait]
pub trait CommentsTcpClientChannelConnector: Send + Sync {
    async fn connect(&self, endpoint: SocketAddr) -> Result<BoxCommentsTcpIo, PortError>;

    fn protection(&self) -> CommentsTcpChannelProtection;
}

/// Server-side accepted-stream protection seam.
///
/// An acceptor receives the raw socket and peer address from the host listener.
/// TLS/mTLS implementations must complete and bound the handshake before
/// returning a channel. Provider dispatch remains owned by the server adapter.
#[async_trait]
pub trait CommentsTcpServerChannelAcceptor: Send + Sync {
    async fn accept(
        &self,
        stream: TcpStream,
        peer_addr: SocketAddr,
    ) -> Result<BoxCommentsTcpIo, PortError>;

    fn protection(&self) -> CommentsTcpChannelProtection;
}

/// Built-in compatibility channel for unencrypted loopback-only deployments.
#[derive(Clone, Copy, Default)]
pub struct PlaintextLoopbackCommentsTcpChannel;

impl fmt::Debug for PlaintextLoopbackCommentsTcpChannel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PlaintextLoopbackCommentsTcpChannel")
            .field("protection", &CommentsTcpChannelProtection::PlaintextLoopback)
            .finish()
    }
}

#[async_trait]
impl CommentsTcpClientChannelConnector for PlaintextLoopbackCommentsTcpChannel {
    async fn connect(&self, endpoint: SocketAddr) -> Result<BoxCommentsTcpIo, PortError> {
        ensure_loopback(endpoint, "endpoint")?;
        let stream = TcpStream::connect(endpoint)
            .await
            .map_err(|error| io_error("connect", error))?;
        stream.set_nodelay(true)
            .map_err(|error| io_error("set_nodelay", error))?;
        Ok(Box::new(stream))
    }

    fn protection(&self) -> CommentsTcpChannelProtection {
        CommentsTcpChannelProtection::PlaintextLoopback
    }
}

#[async_trait]
impl CommentsTcpServerChannelAcceptor for PlaintextLoopbackCommentsTcpChannel {
    async fn accept(
        &self,
        stream: TcpStream,
        peer_addr: SocketAddr,
    ) -> Result<BoxCommentsTcpIo, PortError> {
        ensure_loopback(peer_addr, "peer")?;
        stream.set_nodelay(true)
            .map_err(|error| io_error("server_set_nodelay", error))?;
        Ok(Box::new(stream))
    }

    fn protection(&self) -> CommentsTcpChannelProtection {
        CommentsTcpChannelProtection::PlaintextLoopback
    }
}

fn ensure_loopback(address: SocketAddr, subject: &'static str) -> Result<(), PortError> {
    if address.ip().is_loopback() {
        return Ok(());
    }
    Err(PortError::forbidden(
        "comments.tcp_plaintext_non_loopback",
        format!("comments plaintext TCP {subject} must be loopback"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plaintext_channel_is_explicitly_loopback_only() {
        assert_eq!(
            CommentsTcpClientChannelConnector::protection(
                &PlaintextLoopbackCommentsTcpChannel,
            ),
            CommentsTcpChannelProtection::PlaintextLoopback
        );
        assert!(ensure_loopback("127.0.0.1:9000".parse().unwrap(), "endpoint").is_ok());
        let error = ensure_loopback("192.0.2.10:9000".parse().unwrap(), "endpoint")
            .unwrap_err();
        assert_eq!(error.code, "comments.tcp_plaintext_non_loopback");
    }
}
