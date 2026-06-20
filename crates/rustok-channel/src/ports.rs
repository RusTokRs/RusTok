use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dto::ChannelDetailResponse;

/// Transport-agnostic channel port context for host/runtime boundary calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortContext {
    pub tenant_id: String,
    pub correlation_id: String,
    pub deadline_ms: Option<u64>,
}

impl PortContext {
    pub fn require_deadline_semantics(&self) -> Result<(), PortError> {
        if self.deadline_ms.unwrap_or_default() == 0 {
            return Err(PortError::new(
                PortErrorKind::Timeout,
                "port.deadline_required",
                "channel read port calls require deadline semantics",
                true,
            ));
        }
        Ok(())
    }
}

/// Transport-neutral error returned by channel owner ports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortError {
    pub kind: PortErrorKind,
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl PortError {
    pub fn new(
        kind: PortErrorKind,
        code: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            kind,
            code: code.into(),
            message: message.into(),
            retryable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortErrorKind {
    Validation,
    NotFound,
    Unavailable,
    Timeout,
}

/// Transport-neutral selector for channel read-projection consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelReadSelector {
    Id(Uuid),
    Slug(String),
    HostTargetValue(String),
    Default,
}

/// Transport-neutral request for channel read-projection consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelReadRequest {
    pub selector: ChannelReadSelector,
    pub include_inactive: bool,
}

/// Transport-neutral request for tenant channel list consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelListRequest {
    pub include_inactive: bool,
}

/// Transport-neutral channel detail projection exposed by the channel owner module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelReadProjection {
    pub detail: ChannelDetailResponse,
}

/// Transport-neutral owner boundary for channel read projections.
#[async_trait]
pub trait ChannelReadPort: Send + Sync {
    async fn read_channel(
        &self,
        context: PortContext,
        request: ChannelReadRequest,
    ) -> Result<Option<ChannelReadProjection>, PortError>;

    async fn list_channels_for_tenant(
        &self,
        context: PortContext,
        request: ChannelListRequest,
    ) -> Result<Vec<ChannelReadProjection>, PortError>;
}

#[async_trait]
impl ChannelReadPort for crate::ChannelService {
    async fn read_channel(
        &self,
        context: PortContext,
        request: ChannelReadRequest,
    ) -> Result<Option<ChannelReadProjection>, PortError> {
        context.require_deadline_semantics()?;
        let tenant_id = parse_tenant_id(&context)?;
        validate_channel_read_request(&request)?;

        let detail = match request.selector {
            ChannelReadSelector::Id(channel_id) => {
                let detail = self
                    .get_channel_detail(channel_id)
                    .await
                    .map_err(map_channel_error)?;
                ensure_tenant_scope(tenant_id, &detail)?;
                Some(detail)
            }
            ChannelReadSelector::Slug(slug) => self
                .get_channel_detail_by_slug(tenant_id, &slug)
                .await
                .map_err(map_channel_error)?,
            ChannelReadSelector::HostTargetValue(target_value) => self
                .get_channel_by_host_target_value(tenant_id, &target_value)
                .await
                .map_err(map_channel_error)?,
            ChannelReadSelector::Default => self
                .get_default_channel(tenant_id)
                .await
                .map_err(map_channel_error)?,
        };

        let Some(detail) = detail else {
            return Ok(None);
        };
        if !request.include_inactive && !detail.channel.is_active {
            return Ok(None);
        }
        Ok(Some(ChannelReadProjection { detail }))
    }

    async fn list_channels_for_tenant(
        &self,
        context: PortContext,
        request: ChannelListRequest,
    ) -> Result<Vec<ChannelReadProjection>, PortError> {
        context.require_deadline_semantics()?;
        let tenant_id = parse_tenant_id(&context)?;
        self.list_channel_details(tenant_id)
            .await
            .map_err(map_channel_error)
            .map(|items| {
                items
                    .into_iter()
                    .filter(|detail| request.include_inactive || detail.channel.is_active)
                    .map(|detail| ChannelReadProjection { detail })
                    .collect()
            })
    }
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    context.tenant_id.parse::<Uuid>().map_err(|_| {
        PortError::new(
            PortErrorKind::Validation,
            "channel.tenant_id_invalid",
            "channel read port requires a UUID tenant_id in context",
            false,
        )
    })
}

fn validate_channel_read_request(request: &ChannelReadRequest) -> Result<(), PortError> {
    match &request.selector {
        ChannelReadSelector::Slug(slug) if slug.trim().is_empty() => Err(PortError::new(
            PortErrorKind::Validation,
            "channel.slug_empty",
            "channel read port requires a non-empty slug selector",
            false,
        )),
        ChannelReadSelector::HostTargetValue(value) if value.trim().is_empty() => {
            Err(PortError::new(
                PortErrorKind::Validation,
                "channel.host_target_empty",
                "channel read port requires a non-empty host target selector",
                false,
            ))
        }
        _ => Ok(()),
    }
}

fn ensure_tenant_scope(tenant_id: Uuid, detail: &ChannelDetailResponse) -> Result<(), PortError> {
    if detail.channel.tenant_id != tenant_id {
        return Err(PortError::new(
            PortErrorKind::NotFound,
            "channel.not_found",
            "channel read projection was not found for this tenant",
            false,
        ));
    }
    Ok(())
}

fn map_channel_error(error: crate::ChannelError) -> PortError {
    match error {
        crate::ChannelError::NotFound(_) => PortError::new(
            PortErrorKind::NotFound,
            "channel.not_found",
            "channel read projection was not found",
            false,
        ),
        crate::ChannelError::InactiveChannel(_) => PortError::new(
            PortErrorKind::NotFound,
            "channel.inactive",
            "channel read projection hides inactive channels unless explicitly requested",
            false,
        ),
        crate::ChannelError::InvalidTargetType(message)
        | crate::ChannelError::InvalidTargetValue(message)
        | crate::ChannelError::InvalidPolicyDefinition(message)
        | crate::ChannelError::InvalidPolicyOperation(message) => PortError::new(
            PortErrorKind::Validation,
            "channel.validation",
            message,
            false,
        ),
        crate::ChannelError::SlugAlreadyExists(message)
        | crate::ChannelError::TargetAlreadyExists(_, message)
        | crate::ChannelError::PolicySetSlugAlreadyExists(message) => PortError::new(
            PortErrorKind::Validation,
            "channel.conflict",
            message,
            false,
        ),
        crate::ChannelError::Database(error) => PortError::new(
            PortErrorKind::Unavailable,
            "channel.read_failed",
            error.to_string(),
            true,
        ),
        crate::ChannelError::Serialization(error) => PortError::new(
            PortErrorKind::Unavailable,
            "channel.serialization_failed",
            error.to_string(),
            true,
        ),
    }
}
