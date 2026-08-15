use ::rustok_channel::{ChannelError, ChannelResponse};
use ::sea_orm::DatabaseConnection;

use super::super::query_error_boundary::{BoundaryError, QueryGraphqlMessage};

const GRAPHQL_QUERY_CHANNEL_BOUNDARY: &str = "commerce_graphql_query_channel";

struct ChannelQueryDiagnosticError;

impl std::fmt::Debug for ChannelQueryDiagnosticError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("redacted")
    }
}

fn text_shape(value: &str) -> &'static str {
    if value.is_empty() { "empty" } else { "present" }
}

fn uuid_shape(value: &::uuid::Uuid) -> &'static str {
    if value.is_nil() {
        "uuid_nil"
    } else {
        "uuid_non_nil"
    }
}

fn owner_detail(error: &ChannelError) -> (&'static str, usize) {
    match error {
        ChannelError::SlugAlreadyExists(value)
        | ChannelError::InvalidTargetType(value)
        | ChannelError::InvalidTargetValue(value)
        | ChannelError::InvalidPolicyDefinition(value)
        | ChannelError::PolicySetSlugAlreadyExists(value)
        | ChannelError::InvalidPolicyOperation(value) => (text_shape(value), value.chars().count()),
        ChannelError::TargetAlreadyExists(target_type, value) => (
            "two_text_values",
            target_type
                .chars()
                .count()
                .saturating_add(value.chars().count()),
        ),
        ChannelError::NotFound(value) | ChannelError::InactiveChannel(value) => {
            (uuid_shape(value), 0)
        }
        ChannelError::Database(_) => ("database_redacted", 0),
        ChannelError::Serialization(_) => ("serialization_redacted", 0),
    }
}

pub(crate) struct ChannelGraphqlMessage {
    error: ChannelError,
}

impl QueryGraphqlMessage for ChannelGraphqlMessage {
    fn into_query_boundary(self) -> BoundaryError {
        let (message, code, retryable, error_kind, technical) = match &self.error {
            ChannelError::InvalidTargetType(_)
            | ChannelError::InvalidTargetValue(_)
            | ChannelError::InvalidPolicyDefinition(_)
            | ChannelError::InvalidPolicyOperation(_) => (
                "Channel query is invalid",
                "CHANNEL_REQUEST_INVALID",
                false,
                "validation",
                false,
            ),
            ChannelError::NotFound(_) => (
                "Channel data was not found",
                "CHANNEL_RESOURCE_NOT_FOUND",
                false,
                "not_found",
                false,
            ),
            ChannelError::InactiveChannel(_)
            | ChannelError::SlugAlreadyExists(_)
            | ChannelError::TargetAlreadyExists(_, _)
            | ChannelError::PolicySetSlugAlreadyExists(_) => (
                "Channel state conflicts with this query",
                "CHANNEL_STATE_CONFLICT",
                false,
                "conflict",
                false,
            ),
            ChannelError::Database(_) => (
                "Channel data is temporarily unavailable",
                "CHANNEL_TEMPORARILY_UNAVAILABLE",
                true,
                "database",
                true,
            ),
            ChannelError::Serialization(_) => (
                "Channel query could not be completed safely",
                "CHANNEL_OPERATION_FAILED",
                false,
                "serialization",
                true,
            ),
        };
        let (owner_detail_shape, owner_detail_length) = owner_detail(&self.error);
        let diagnostic_error = ChannelQueryDiagnosticError;
        if technical {
            tracing::error!(
                error = ?diagnostic_error,
                owner = "rustok_channel",
                error_kind,
                owner_detail_shape,
                owner_detail_length,
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
                owner_detail_shape,
                owner_detail_length,
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
    error: ChannelError,
}

impl From<ChannelError> for ChannelQueryError {
    fn from(error: ChannelError) -> Self {
        Self { error }
    }
}

impl ChannelQueryError {
    /// Preserve the unchanged resolver expression `err.to_string()` while retaining
    /// the typed Channel owner error until the transport-owned GraphQL mapper.
    #[allow(clippy::wrong_self_convention)]
    pub(crate) fn to_string(self) -> ChannelGraphqlMessage {
        ChannelGraphqlMessage { error: self.error }
    }
}

pub(crate) struct ChannelService {
    inner: ::rustok_channel::ChannelService,
}

impl ChannelService {
    pub(crate) fn new(db: DatabaseConnection) -> Self {
        Self {
            inner: ::rustok_channel::ChannelService::new(db),
        }
    }

    pub(crate) async fn list_channels(
        &self,
        tenant_id: ::uuid::Uuid,
        page: u64,
        per_page: u64,
    ) -> Result<(Vec<ChannelResponse>, u64), ChannelQueryError> {
        self.inner
            .list_channels(tenant_id, page, per_page)
            .await
            .map_err(Into::into)
    }
}
