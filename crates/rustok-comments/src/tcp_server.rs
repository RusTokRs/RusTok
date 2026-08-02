use std::{net::SocketAddr, sync::Arc, time::Duration};

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
use tokio::{net::TcpStream, time::timeout};

use crate::tcp_protocol::{
    DEFAULT_MAX_COMMENTS_FRAME_BYTES, read_frame, validate_frame_limit, write_frame,
};
use crate::{
    BoxCommentsTcpIo, COMMENTS_TCP_PROTOCOL_VERSION, CommentsTcpCredential,
    CommentsTcpIo, CommentsTcpRequestEnvelope, CommentsTcpServerChannelAcceptor,
    CommentsThreadPort, CommentsThreadRequest, CommentsThreadResponse,
    CommentsThreadTransportReply, PlaintextLoopbackCommentsTcpChannel,
};

/// Stable operation identity exposed to host-owned TCP authority resolvers.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CommentsTcpOperation {
    CreateComment,
    GetComment,
    ListCommentsForTarget,
    ListPublicCommentsForTarget,
    UpdateComment,
    SetCommentStatus,
    DeleteComment,
}

impl CommentsTcpOperation {
    pub const ALL: [Self; 7] = [
        Self::CreateComment,
        Self::GetComment,
        Self::ListCommentsForTarget,
        Self::ListPublicCommentsForTarget,
        Self::UpdateComment,
        Self::SetCommentStatus,
        Self::DeleteComment,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CreateComment => "create_comment",
            Self::GetComment => "get_comment",
            Self::ListCommentsForTarget => "list_comments_for_target",
            Self::ListPublicCommentsForTarget => "list_public_comments_for_target",
            Self::UpdateComment => "update_comment",
            Self::SetCommentStatus => "set_comment_status",
            Self::DeleteComment => "delete_comment",
        }
    }

    pub const fn is_write(self) -> bool {
        matches!(
            self,
            Self::CreateComment
                | Self::UpdateComment
                | Self::SetCommentStatus
                | Self::DeleteComment
        )
    }

    pub fn for_request(request: &CommentsThreadRequest) -> Self {
        match request {
            CommentsThreadRequest::CreateComment { .. } => Self::CreateComment,
            CommentsThreadRequest::GetComment { .. } => Self::GetComment,
            CommentsThreadRequest::ListCommentsForTarget { .. } => {
                Self::ListCommentsForTarget
            }
            CommentsThreadRequest::ListPublicCommentsForTarget { .. } => {
                Self::ListPublicCommentsForTarget
            }
            CommentsThreadRequest::UpdateComment { .. } => Self::UpdateComment,
            CommentsThreadRequest::SetCommentStatus { .. } => Self::SetCommentStatus,
            CommentsThreadRequest::DeleteComment { .. } => Self::DeleteComment,
        }
    }
}

/// Principal fields established by a host-owned authentication boundary.
///
/// Correlation, causation, trace, locale, channel, idempotency, and deadline
/// remain request metadata. Tenant and principal authority never come from the
/// untrusted TCP payload alone.
#[derive(Clone, Debug)]
pub struct TrustedCommentsTcpAuthority {
    pub tenant_id: String,
    pub actor: PortActor,
    pub claims: Vec<String>,
    pub roles: Vec<String>,
}

impl TrustedCommentsTcpAuthority {
    pub fn new(tenant_id: impl Into<String>, actor: PortActor) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            actor,
            claims: Vec::new(),
            roles: Vec::new(),
        }
    }

    pub fn with_claim(mut self, claim: impl Into<String>) -> Self {
        self.claims.push(claim.into());
        self
    }

    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.roles.push(role.into());
        self
    }
}

/// Host-owned authentication and operation-authorization seam.
///
/// Implementations may authenticate a service bearer, verify signed user
/// delegation against the complete typed request, resolve authority from mTLS
/// identity, or use another protected host policy. Returning an error produces
/// the stable typed error reply and prevents provider dispatch.
#[async_trait]
pub trait CommentsTcpAuthorityResolver: Send + Sync {
    async fn authorize(
        &self,
        peer_addr: SocketAddr,
        operation: CommentsTcpOperation,
        credential: Option<&CommentsTcpCredential>,
        request: &CommentsThreadRequest,
    ) -> Result<TrustedCommentsTcpAuthority, PortError>;
}

/// Provider-side adapter for one length-prefixed JSON request per accepted channel.
///
/// The host owns listener lifecycle and passes each accepted raw socket through a
/// channel acceptor. The acceptor may retain the built-in plaintext loopback
/// profile or complete a bounded authenticated encrypted handshake. This adapter
/// then decodes one versioned envelope, requires trusted authority, invokes the
/// host-selected Comments provider, writes one typed reply, and returns.
#[derive(Clone)]
pub struct TcpJsonCommentsServerAdapter {
    provider: Arc<dyn CommentsThreadPort>,
    authority: Arc<dyn CommentsTcpAuthorityResolver>,
    max_frame_bytes: usize,
}

impl TcpJsonCommentsServerAdapter {
    pub fn new(
        provider: Arc<dyn CommentsThreadPort>,
        authority: Arc<dyn CommentsTcpAuthorityResolver>,
    ) -> Self {
        Self {
            provider,
            authority,
            max_frame_bytes: DEFAULT_MAX_COMMENTS_FRAME_BYTES,
        }
    }

    pub fn with_max_frame_bytes(
        provider: Arc<dyn CommentsThreadPort>,
        authority: Arc<dyn CommentsTcpAuthorityResolver>,
        max_frame_bytes: usize,
    ) -> Result<Self, PortError> {
        validate_frame_limit(max_frame_bytes)?;
        Ok(Self {
            provider,
            authority,
            max_frame_bytes,
        })
    }

    pub fn max_frame_bytes(&self) -> usize {
        self.max_frame_bytes
    }

    /// Handles exactly one request/reply exchange using the built-in plaintext
    /// loopback-only compatibility acceptor.
    pub async fn handle_connection(
        &self,
        stream: TcpStream,
        peer_addr: SocketAddr,
    ) -> Result<(), PortError> {
        let acceptor = PlaintextLoopbackCommentsTcpChannel;
        self.handle_connection_with_acceptor(stream, peer_addr, &acceptor)
            .await
    }

    /// Handles one plaintext loopback exchange while bounding how long an
    /// accepted peer may remain idle before sending the complete first frame.
    pub async fn handle_connection_with_pre_request_timeout(
        &self,
        stream: TcpStream,
        peer_addr: SocketAddr,
        pre_request_timeout: Duration,
    ) -> Result<(), PortError> {
        let acceptor = PlaintextLoopbackCommentsTcpChannel;
        self.handle_connection_with_acceptor_and_pre_request_timeout(
            stream,
            peer_addr,
            &acceptor,
            pre_request_timeout,
        )
        .await
    }

    /// Handles one request/reply exchange through a host-provided channel
    /// acceptor. A TLS/mTLS acceptor owns and bounds its handshake before this
    /// method starts reading the typed request frame.
    pub async fn handle_connection_with_acceptor(
        &self,
        stream: TcpStream,
        peer_addr: SocketAddr,
        acceptor: &dyn CommentsTcpServerChannelAcceptor,
    ) -> Result<(), PortError> {
        self.accept_and_handle(stream, peer_addr, acceptor, None)
            .await
    }

    /// Handles one exchange through a host-provided channel acceptor and bounds
    /// receipt of the first complete request frame after channel establishment.
    ///
    /// The request's own `PortContext` deadline continues to bound trusted
    /// authority resolution and provider dispatch after the frame is decoded.
    pub async fn handle_connection_with_acceptor_and_pre_request_timeout(
        &self,
        stream: TcpStream,
        peer_addr: SocketAddr,
        acceptor: &dyn CommentsTcpServerChannelAcceptor,
        pre_request_timeout: Duration,
    ) -> Result<(), PortError> {
        if pre_request_timeout.is_zero() {
            return Err(PortError::validation(
                "comments.tcp_server_invalid_idle_timeout",
                "comments TCP pre-request timeout must be greater than zero",
            ));
        }
        self.accept_and_handle(
            stream,
            peer_addr,
            acceptor,
            Some(pre_request_timeout),
        )
        .await
    }

    async fn accept_and_handle(
        &self,
        stream: TcpStream,
        peer_addr: SocketAddr,
        acceptor: &dyn CommentsTcpServerChannelAcceptor,
        pre_request_timeout: Option<Duration>,
    ) -> Result<(), PortError> {
        let channel = acceptor.accept(stream, peer_addr).await?;
        self.handle_channel_inner(channel, peer_addr, pre_request_timeout)
            .await
    }

    async fn handle_channel_inner(
        &self,
        mut channel: BoxCommentsTcpIo,
        peer_addr: SocketAddr,
        pre_request_timeout: Option<Duration>,
    ) -> Result<(), PortError> {
        let reply = match self
            .read_authorize_and_dispatch(&mut *channel, peer_addr, pre_request_timeout)
            .await
        {
            Ok(response) => CommentsThreadTransportReply::Success(response),
            Err(error) => CommentsThreadTransportReply::Error(error),
        };
        let reply_payload = serde_json::to_vec(&reply).map_err(|_| {
            PortError::invariant_violation(
                "comments.tcp_server_encode",
                "comments TCP server reply could not be encoded",
            )
        })?;
        write_frame(&mut *channel, &reply_payload, self.max_frame_bytes).await
    }

    async fn read_authorize_and_dispatch(
        &self,
        channel: &mut dyn CommentsTcpIo,
        peer_addr: SocketAddr,
        pre_request_timeout: Option<Duration>,
    ) -> Result<CommentsThreadResponse, PortError> {
        let request_payload = match pre_request_timeout {
            Some(duration) => timeout(duration, read_frame(channel, self.max_frame_bytes))
                .await
                .map_err(|_| {
                    PortError::timeout(
                        "comments.tcp_server_idle_timeout",
                        "comments TCP peer did not send a complete request frame before the idle timeout",
                    )
                })??,
            None => read_frame(channel, self.max_frame_bytes).await?,
        };
        let envelope = serde_json::from_slice::<CommentsTcpRequestEnvelope>(&request_payload)
            .map_err(|_| {
                PortError::validation(
                    "comments.tcp_server_invalid_request",
                    "comments TCP request is not a valid typed envelope",
                )
            })?;
        let (protocol_version, credential, mut request) = envelope.into_parts();
        if protocol_version != COMMENTS_TCP_PROTOCOL_VERSION {
            return Err(PortError::validation(
                "comments.tcp_server_unsupported_protocol",
                "comments TCP request protocol version is not supported",
            ));
        }
        request.context().require_deadline_semantics()?;

        let operation = CommentsTcpOperation::for_request(&request);
        let deadline_ms = request.context().deadline_ms.unwrap_or_default();
        timeout(Duration::from_millis(deadline_ms), async {
            let authority = self
                .authority
                .authorize(peer_addr, operation, credential.as_ref(), &request)
                .await?;
            let trusted_context = apply_authority(request.context(), authority)?;
            replace_request_context(&mut request, trusted_context);
            dispatch_request(self.provider.as_ref(), request).await
        })
        .await
        .map_err(|_| {
            PortError::timeout(
                "comments.tcp_server_timeout",
                "comments TCP authority or provider dispatch exceeded the port deadline",
            )
        })?
    }
}

fn apply_authority(
    claimed: &PortContext,
    authority: TrustedCommentsTcpAuthority,
) -> Result<PortContext, PortError> {
    if claimed.tenant_id != authority.tenant_id {
        return Err(PortError::new(
            PortErrorKind::Forbidden,
            "comments.tcp_authority_tenant_mismatch",
            "comments TCP tenant does not match trusted authority",
            false,
        ));
    }

    let mut trusted = claimed.clone();
    trusted.tenant_id = authority.tenant_id;
    trusted.actor = authority.actor;
    trusted.claims = authority.claims;
    trusted.roles = authority.roles;
    Ok(trusted)
}

fn replace_request_context(request: &mut CommentsThreadRequest, trusted: PortContext) {
    match request {
        CommentsThreadRequest::CreateComment { context, .. }
        | CommentsThreadRequest::GetComment { context, .. }
        | CommentsThreadRequest::ListCommentsForTarget { context, .. }
        | CommentsThreadRequest::ListPublicCommentsForTarget { context, .. }
        | CommentsThreadRequest::UpdateComment { context, .. }
        | CommentsThreadRequest::SetCommentStatus { context, .. }
        | CommentsThreadRequest::DeleteComment { context, .. } => *context = trusted,
    }
}

async fn dispatch_request(
    provider: &dyn CommentsThreadPort,
    request: CommentsThreadRequest,
) -> Result<CommentsThreadResponse, PortError> {
    match request {
        CommentsThreadRequest::CreateComment { context, request } => provider
            .create_comment(context, request)
            .await
            .map(CommentsThreadResponse::Comment),
        CommentsThreadRequest::GetComment {
            context,
            comment_id,
            fallback_locale,
        } => provider
            .get_comment(context, comment_id, fallback_locale)
            .await
            .map(CommentsThreadResponse::Comment),
        CommentsThreadRequest::ListCommentsForTarget {
            context,
            target_type,
            target_id,
            filter,
            fallback_locale,
        } => provider
            .list_comments_for_target(
                context,
                target_type,
                target_id,
                filter,
                fallback_locale,
            )
            .await
            .map(|(items, total)| CommentsThreadResponse::CommentsPage { items, total }),
        CommentsThreadRequest::ListPublicCommentsForTarget {
            context,
            target_type,
            target_id,
            filter,
            fallback_locale,
        } => provider
            .list_public_comments_for_target(
                context,
                target_type,
                target_id,
                filter,
                fallback_locale,
            )
            .await
            .map(|(items, total)| CommentsThreadResponse::CommentsPage { items, total }),
        CommentsThreadRequest::UpdateComment {
            context,
            comment_id,
            request,
        } => provider
            .update_comment(context, comment_id, request)
            .await
            .map(CommentsThreadResponse::Comment),
        CommentsThreadRequest::SetCommentStatus {
            context,
            comment_id,
            request,
        } => provider
            .set_comment_status(context, comment_id, request)
            .await
            .map(CommentsThreadResponse::Comment),
        CommentsThreadRequest::DeleteComment {
            context,
            comment_id,
        } => provider
            .delete_comment(context, comment_id)
            .await
            .map(|()| CommentsThreadResponse::Deleted),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trusted_authority_replaces_only_principal_fields() {
        let claimed = PortContext::new(
            "tenant-a",
            PortActor::user("forged-user"),
            "en",
            "corr-1",
        )
        .with_claim("forged-claim")
        .with_role("forged-role")
        .with_channel("storefront")
        .with_causation_id("cause-1")
        .with_traceparent("trace-1")
        .with_idempotency_key("idem-1")
        .with_deadline(Duration::from_millis(500));
        let trusted = apply_authority(
            &claimed,
            TrustedCommentsTcpAuthority::new(
                "tenant-a",
                PortActor::service("comments-sidecar"),
            )
            .with_claim("comments:read")
            .with_role("comments-worker"),
        )
        .unwrap();

        assert_eq!(trusted.actor.id, "comments-sidecar");
        assert_eq!(trusted.claims, ["comments:read"]);
        assert_eq!(trusted.roles, ["comments-worker"]);
        assert_eq!(trusted.channel, claimed.channel);
        assert_eq!(trusted.correlation_id, claimed.correlation_id);
        assert_eq!(trusted.causation_id, claimed.causation_id);
        assert_eq!(trusted.traceparent, claimed.traceparent);
        assert_eq!(trusted.idempotency_key, claimed.idempotency_key);
        assert_eq!(trusted.deadline_ms, claimed.deadline_ms);
    }

    #[test]
    fn trusted_authority_rejects_tenant_mismatch() {
        let claimed = PortContext::new(
            "tenant-a",
            PortActor::user("forged-user"),
            "en",
            "corr-1",
        );
        let error = apply_authority(
            &claimed,
            TrustedCommentsTcpAuthority::new(
                "tenant-b",
                PortActor::service("comments-sidecar"),
            ),
        )
        .unwrap_err();

        assert_eq!(error.kind, PortErrorKind::Forbidden);
        assert_eq!(error.code, "comments.tcp_authority_tenant_mismatch");
    }

    #[test]
    fn operation_classifies_owner_writes() {
        assert!(CommentsTcpOperation::CreateComment.is_write());
        assert!(CommentsTcpOperation::UpdateComment.is_write());
        assert!(CommentsTcpOperation::SetCommentStatus.is_write());
        assert!(CommentsTcpOperation::DeleteComment.is_write());
        assert!(!CommentsTcpOperation::GetComment.is_write());
        assert!(!CommentsTcpOperation::ListCommentsForTarget.is_write());
        assert!(!CommentsTcpOperation::ListPublicCommentsForTarget.is_write());
    }

    #[test]
    fn channel_acceptor_api_is_source_visible() {
        let handler =
            TcpJsonCommentsServerAdapter::handle_connection_with_acceptor_and_pre_request_timeout;
        let _ = handler;
    }
}
