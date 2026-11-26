use ::rustok_channel::{ChannelError, ChannelResponse};
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

fn uuid_shape(value: &::uuid::Uuid) -> &'static str {
    if value.is_nil() {
        "nil"
    } else {
        "non_nil"
    }
}

/// Bounded owner detail projection: shape plus length only, never the payload itself.
fn owner_detail(error: &ChannelError) -> (&'static str, usize) {
    match error {
        ChannelError::Validation(_) => ("validation", 0),
        ChannelError::SlugAlreadyExists(slug) => ("slug_already_exists", slug.chars().count()),
        ChannelError::NotFound(value) => ("not_found", uuid_shape(value).len()),
        ChannelError::InactiveChannel(value) => ("inactive_channel", uuid_shape(value).len()),
        ChannelError::InvalidTargetType(value) => ("invalid_target_type", value.chars().count()),
        ChannelError::InvalidTargetValue(value) => ("invalid_target_value", value.chars().count()),
        ChannelError::InvalidPolicyDefinition(value) => {
            ("invalid_policy_definition", value.chars().count())
        }
        ChannelError::TargetAlreadyExists(target_type, value) => (
            "target_already_exists",
            target_type.chars().count().saturating_add(value.chars().count()),
        ),
        ChannelError::PolicySetSlugAlreadyExists(value) => {
            ("policy_set_slug_already_exists", value.chars().count())
        }
        ChannelError::InvalidPolicyOperation(value) => {
            ("invalid_policy_operation", value.chars().count())
        }
        ChannelError::Conflict(value) => ("concurrent_conflict", value.chars().count()),
        ChannelError::Database(_) => ("database_redacted", 0),
        ChannelError::Serialization(_) => ("serialization_redacted", 0),
    }
}

pub(crate) struct ChannelGraphqlMessage {
    error: ChannelError,
}

impl QueryGraphqlMessage for ChannelGraphqlMessage {
    fn into_query_boundary(self) -> BoundaryError {
        let (message, code, retryable, technical) = match &self.error {
            ChannelError::InvalidTargetType(_) => (
                "Channel query is invalid",
                "CHANNEL_REQUEST_INVALID",
                false,
                false,
            ),
            ChannelError::NotFound(_) => (
                "Channel data was not found",
                "CHANNEL_RESOURCE_NOT_FOUND",
                false,
                false,
            ),
            ChannelError::InactiveChannel(_) => (
                "Channel state conflicts with this query",
                "CHANNEL_STATE_CONFLICT",
                false,
                false,
            ),
            ChannelError::Database(_) => (
                "Channel data is temporarily unavailable",
                "CHANNEL_TEMPORARILY_UNAVAILABLE",
                true,
                true,
            ),
            ChannelError::Serialization(_) => (
                "Channel query could not be completed safely",
                "CHANNEL_OPERATION_FAILED",
                false,
                true,
            ),
            // A concurrent write won the owner's invariant race. It is not a bad
            // request: the same query is expected to succeed on retry, so the
            // public envelope must say so instead of reporting the query invalid.
            ChannelError::Conflict(_) => (
                "Channel query conflicted with a concurrent channel update",
                "CHANNEL_CONCURRENT_CONFLICT",
                true,
                false,
            ),
            ChannelError::Validation(_)
            | ChannelError::SlugAlreadyExists(_)
            | ChannelError::InvalidTargetValue(_)
            | ChannelError::InvalidPolicyDefinition(_)
            | ChannelError::TargetAlreadyExists(_, _)
            | ChannelError::PolicySetSlugAlreadyExists(_)
            | ChannelError::InvalidPolicyOperation(_) => (
                "Channel query is invalid",
                "CHANNEL_REQUEST_INVALID",
                false,
                false,
            ),
        };
        let (owner_detail_shape, owner_detail_length) = owner_detail(&self.error);
        let diagnostic_error = ChannelQueryDiagnosticError;
        if technical {
            tracing::error!(
                error = ?diagnostic_error,
                owner = "rustok_channel",
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
    #[allow(clippy::inherent_to_string, clippy::wrong_self_convention)]
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

    /// Mirrors exactly the owner calls `query.rs` makes through this facade.
    ///
    /// `query.rs` is compiled twice: directly against `::rustok_channel`, and again
    /// inside this boundary where `rustok_channel` resolves to this shim. Every
    /// owner method the resolver calls therefore has to exist here with the same
    /// arguments, or the boundary build breaks while the direct build still passes.
    pub(crate) async fn list_channel_options(
        &self,
        tenant_id: Uuid,
        include_inactive: bool,
    ) -> Result<Vec<ChannelResponse>, ChannelQueryError> {
        self.inner
            .list_channel_options(tenant_id, include_inactive)
            .await
            .map_err(Into::into)
    }
}
