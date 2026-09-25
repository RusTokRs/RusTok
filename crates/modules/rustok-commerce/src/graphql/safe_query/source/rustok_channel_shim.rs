use ::rustok_channel::{
    in_process_channel_read_port, ChannelListRequest, ChannelReadPort, ChannelResponse,
};
use ::rustok_api::{PortActor, PortCallPolicy, PortContext, PortError, PortErrorKind};
use ::sea_orm::DatabaseConnection;
use ::uuid::Uuid;

use super::super::query_error_boundary::{BoundaryError, QueryGraphqlMessage};

const GRAPHQL_QUERY_CHANNEL_BOUNDARY: &str = "commerce_graphql_query_channel";

struct ChannelQueryDiagnosticError;

impl std::fmt::Debug for ChannelQueryDiagnosticError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("redacted")
    }
}

fn port_error_kind(kind: &PortErrorKind) -> &'static str {
    match kind {
        PortErrorKind::Validation => "validation",
        PortErrorKind::NotFound => "not_found",
        PortErrorKind::Conflict => "conflict",
        PortErrorKind::Forbidden => "forbidden",
        PortErrorKind::Unavailable => "unavailable",
        PortErrorKind::Timeout => "timeout",
        PortErrorKind::InvariantViolation => "invariant_violation",
    }
}

pub(crate) struct ChannelGraphqlMessage {
    error: PortError,
}

impl QueryGraphqlMessage for ChannelGraphqlMessage {
    fn into_query_boundary(self) -> BoundaryError {
        let (message, code, retryable, technical) = match &self.error.kind {
            PortErrorKind::Validation => (
                "Channel query is invalid",
                "CHANNEL_REQUEST_INVALID",
                false,
                false,
            ),
            PortErrorKind::NotFound => (
                "Channel data was not found",
                "CHANNEL_RESOURCE_NOT_FOUND",
                false,
                false,
            ),
            PortErrorKind::Conflict => (
                "Channel state conflicts with this query",
                "CHANNEL_STATE_CONFLICT",
                false,
                false,
            ),
            PortErrorKind::Forbidden => (
                "Channel query is not permitted",
                "CHANNEL_ACCESS_DENIED",
                false,
                false,
            ),
            PortErrorKind::Unavailable | PortErrorKind::Timeout => (
                "Channel data is temporarily unavailable",
                "CHANNEL_TEMPORARILY_UNAVAILABLE",
                true,
                true,
            ),
            PortErrorKind::InvariantViolation => (
                "Channel query could not be completed safely",
                "CHANNEL_OPERATION_FAILED",
                false,
                true,
            ),
        };
        let error_kind = port_error_kind(&self.error.kind);
        let diagnostic_error = ChannelQueryDiagnosticError;
        if technical {
            tracing::error!(
                error = ?diagnostic_error,
                owner = "rustok_channel",
                error_kind,
                owner_code = %self.error.code,
                owner_retryable = self.error.retryable,
                public_code = code,
                retryable,
                boundary = GRAPHQL_QUERY_CHANNEL_BOUNDARY,
                "commerce GraphQL channel query failed"
            );
        } else {
            tracing::warn!(
                error = ?diagnostic_error,
                owner = "rustok_channel",
                error_kind,
                owner_code = %self.error.code,
                owner_retryable = self.error.retryable,
                public_code = code,
                retryable,
                boundary = GRAPHQL_QUERY_CHANNEL_BOUNDARY,
                "commerce GraphQL channel query was rejected"
            );
        }
        BoundaryError::Public {
            message,
            code,
            retryable,
        }
    }
}

pub(crate) struct ChannelQueryError {
    error: PortError,
}

impl From<PortError> for ChannelQueryError {
    fn from(error: PortError) -> Self {
        Self { error }
    }
}

impl ChannelQueryError {
    pub(crate) fn to_string(self) -> ChannelGraphqlMessage {
        ChannelGraphqlMessage { error: self.error }
    }
}

pub(crate) struct ChannelService {
    reads: std::sync::Arc<dyn ChannelReadPort>,
}

impl ChannelService {
    pub(crate) fn new(db: DatabaseConnection) -> Self {
        Self {
            reads: in_process_channel_read_port(db),
        }
    }

    pub(crate) async fn list_channels(
        &self,
        tenant_id: Uuid,
        page: u64,
        per_page: u64,
    ) -> Result<(Vec<ChannelResponse>, u64), ChannelQueryError> {
        if page == 0 || per_page == 0 {
            return Err(PortError::validation(
                "channel.pagination_invalid",
                "channel query pagination requires non-zero page and per_page",
            )
            .into());
        }

        let context = PortContext::new(
            tenant_id.to_string(),
            PortActor::service("rustok-commerce.graphql-query-channels"),
            "en",
            format!("commerce-graphql-channels:list:{tenant_id}:{page}:{per_page}"),
        )
        .with_deadline(std::time::Duration::from_secs(2));
        context.require_policy(PortCallPolicy::read())?;

        let result = self
            .reads
            .list_channels_for_tenant(
                context,
                ChannelListRequest {
                    page,
                    per_page,
                    include_inactive: false,
                },
            )
            .await?;

        Ok((
            result
                .items
                .into_iter()
                .map(|projection| projection.detail.channel)
                .collect(),
            result.total,
        ))
    }
}
