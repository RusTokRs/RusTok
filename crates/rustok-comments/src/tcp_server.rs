use std::{net::SocketAddr, sync::Arc, time::Duration};

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
use tokio::{net::TcpStream, time::timeout};

use crate::tcp_protocol::{
    DEFAULT_MAX_COMMENTS_FRAME_BYTES, io_error, read_frame, validate_frame_limit, write_frame,
};
use crate::{
    CommentsThreadPort, CommentsThreadRequest, CommentsThreadResponse, CommentsThreadTransportReply,
};

/// Stable operation identity exposed to the host-owned TCP authority resolver.
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
/// Implementations may resolve authority from an authenticated sidecar channel,
/// mTLS identity, a pre-authorized local peer, or another host policy. Returning
/// an error produces the stable typed error reply and prevents provider dispatch.
#[async_trait]
pub trait CommentsTcpAuthorityResolver: Send + Sync {
    async fn authorize(
        &self,
        peer_addr: SocketAddr,
        operation: CommentsTcpOperation,
        claimed_context: &PortContext,
    ) -> Result<TrustedCommentsTcpAuthority, PortError>;
}

/// Provider-side adapter for one length-prefixed JSON request per accepted TCP stream.
///
/// The host owns listener lifecycle and passes each accepted stream with its peer
/// address. This adapter decodes one request, requires trusted authority, invokes
/// the host-selected Comments provider, writes one typed reply, and returns.
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

    /// Handles exactly one request/reply exchange on an accepted stream.
    pub async fn handle_connection(
        &self,
        mut stream: TcpStream,
        peer_addr: SocketAddr,
    ) -> Result<(), PortError> {
        stream
            .set_nodelay(true)
            .map_err(|error| io_error("server_set_nodelay", error))?;

        let reply = match self.read_authorize_and_dispatch(&mut stream, peer_addr).await {
            Ok(response) => CommentsThreadTransportReply::Success(response),
            Err(error) => CommentsThreadTransportReply::Error(error),
        };
        let reply_payload = serde_json::to_vec(&reply).map_err(|error| {
            PortError::invariant_violation("comments.tcp_server_encode", error.to_string())
        })?;
        write_frame(&mut stream, &reply_payload, self.max_frame_bytes).await
    }

    async fn read_authorize_and_dispatch(
        &self,
        stream: &mut TcpStream,
        peer_addr: SocketAddr,
    ) -> Result<CommentsThreadResponse, PortError> {
        let request_payload = read_frame(stream, self.max_frame_bytes).await?;
        let mut request = serde_json::from_slice::<CommentsThreadRequest>(&request_payload)
            .map_err(|error| {
                PortError::validation("comments.tcp_server_invalid_request", error.to_string())
            })?;
        request.context().require_deadline_semantics()?;

        let operation = request_operation(&request);
        let deadline_ms = request.context().deadline_ms.unwrap_or_default();
        timeout(Duration::from_millis(deadline_ms), async {
            let authority = self
                .authority
                .authorize(peer_addr, operation, request.context())
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

fn request_operation(request: &CommentsThreadRequest) -> CommentsTcpOperation {
    match request {
        CommentsThreadRequest::CreateComment { .. } => CommentsTcpOperation::CreateComment,
        CommentsThreadRequest::GetComment { .. } => CommentsTcpOperation::GetComment,
        CommentsThreadRequest::ListCommentsForTarget { .. } => {
            CommentsTcpOperation::ListCommentsForTarget
        }
        CommentsThreadRequest::ListPublicCommentsForTarget { .. } => {
            CommentsTcpOperation::ListPublicCommentsForTarget
        }
        CommentsThreadRequest::UpdateComment { .. } => CommentsTcpOperation::UpdateComment,
        CommentsThreadRequest::SetCommentStatus { .. } => CommentsTcpOperation::SetCommentStatus,
        CommentsThreadRequest::DeleteComment { .. } => CommentsTcpOperation::DeleteComment,
    }
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
}
