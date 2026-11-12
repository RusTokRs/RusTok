use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use rustok_api::{PortActor, PortCallPolicy, PortContext, PortError, PortErrorKind};

use crate::dto::ChannelDetailResponse;

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
    pub page: u64,
    pub per_page: u64,
    pub include_inactive: bool,
}

/// A paginated tenant channel-read projection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelListProjectionPage {
    pub items: Vec<ChannelReadProjection>,
    pub total: u64,
}

/// Transport-neutral channel detail projection exposed by the channel owner module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelReadProjection {
    pub detail: ChannelDetailResponse,
}

/// Build the owner-controlled in-process channel read adapter for direct module composition.
pub fn in_process_channel_read_port(
    db: sea_orm::DatabaseConnection,
) -> std::sync::Arc<dyn ChannelReadPort> {
    std::sync::Arc::new(crate::ChannelService::new(db))
}

/// Transport-neutral owner boundary for channel read projections.
#[async_trait]
pub trait ChannelReadPort: Send + Sync {
    async fn read_channel(
        &self,
        context: PortContext,
        request: ChannelReadRequest,
    ) -> Result<Option<ChannelReadProjection>, PortError>;

    /// Resolve an explicitly saved ID/slug pair through the selected owner provider.
    /// No selectors means global scope, not `Default` or `context.channel`. A supplied
    /// selector must identify an active channel; omission never follows lookup failure.
    async fn read_selected_channel(
        &self,
        context: PortContext,
        channel_id: Option<Uuid>,
        channel_slug: Option<String>,
    ) -> Result<Option<ChannelReadProjection>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context)?;
        if tenant_id.is_nil() || channel_id.is_some_and(|id| id.is_nil()) {
            return Err(PortError::validation(
                "channel.selector_invalid",
                "Channel selection requires non-nil identities",
            ));
        }
        let channel_slug = channel_slug.map(|slug| slug.trim().to_ascii_lowercase());
        if channel_slug.as_deref().is_some_and(str::is_empty) {
            return Err(PortError::validation(
                "channel.slug_empty",
                "Channel selection requires a non-empty slug",
            ));
        }
        let selector = channel_id
            .map(ChannelReadSelector::Id)
            .or_else(|| channel_slug.clone().map(ChannelReadSelector::Slug));
        let Some(selector) = selector else {
            return Ok(None);
        };
        let mut projection = self
            .read_channel(
                context,
                ChannelReadRequest {
                    selector,
                    include_inactive: false,
                },
            )
            .await?
            .ok_or_else(|| {
                PortError::not_found(
                    "channel.not_found",
                    "The selected channel is not available in this tenant",
                )
            })?;
        ensure_tenant_scope(tenant_id, &projection.detail)?;
        let channel = &mut projection.detail.channel;
        let slug = channel.slug.trim().to_ascii_lowercase();
        if channel.id.is_nil() || slug.is_empty() || !channel.is_active {
            return Err(PortError::invariant_violation(
                "channel.projection_invalid",
                "The Channel owner returned an inconsistent active identity",
            ));
        }
        if channel_id.is_some_and(|id| id != channel.id)
            || channel_slug
                .as_ref()
                .is_some_and(|requested| requested != &slug)
        {
            return Err(PortError::validation(
                "channel.selector_mismatch",
                "Channel ID and slug must identify the same channel",
            ));
        }
        channel.slug = slug;
        Ok(Some(projection))
    }

    async fn list_channels_for_tenant(
        &self,
        context: PortContext,
        request: ChannelListRequest,
    ) -> Result<ChannelListProjectionPage, PortError>;
}

#[async_trait]
impl ChannelReadPort for crate::ChannelService {
    async fn read_channel(
        &self,
        context: PortContext,
        request: ChannelReadRequest,
    ) -> Result<Option<ChannelReadProjection>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context)?;
        validate_channel_read_request(&request)?;

        let detail = match request.selector {
            ChannelReadSelector::Id(channel_id) => {
                match self.get_channel_detail(channel_id).await {
                    Ok(detail) if detail.channel.tenant_id == tenant_id => {
                        ensure_tenant_scope(tenant_id, &detail)?;
                        Some(detail)
                    }
                    Ok(_) => None,
                    Err(crate::ChannelError::NotFound(_)) => None,
                    Err(error) => return Err(map_channel_error(error)),
                }
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
    ) -> Result<ChannelListProjectionPage, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context)?;
        if request.page == 0 || request.per_page == 0 {
            return Err(PortError::validation(
                "channel.pagination_invalid",
                "channel read port requires non-zero page and per_page",
            ));
        }
        let (items, total) = self
            .list_channel_details_page(
                tenant_id,
                request.page,
                request.per_page,
                request.include_inactive,
            )
            .await
            .map_err(map_channel_error)?;
        Ok(ChannelListProjectionPage {
            items: items
                .into_iter()
                .map(|detail| ChannelReadProjection { detail })
                .collect(),
            total,
        })
    }
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    let id = context.tenant_id.parse::<Uuid>().map_err(|_| {
        PortError::new(
            PortErrorKind::Validation,
            "channel.tenant_id_invalid",
            "channel read port requires a UUID tenant_id in context",
            false,
        )
    })?;
    if id.is_nil() {
        return Err(PortError::new(
            PortErrorKind::Validation,
            "channel.tenant_id_nil",
            "channel read port requires a non-nil UUID tenant_id in context",
            false,
        ));
    }
    Ok(id)
}

fn validate_channel_read_request(request: &ChannelReadRequest) -> Result<(), PortError> {
    match &request.selector {
        ChannelReadSelector::Id(id) if id.is_nil() => Err(PortError::new(
            PortErrorKind::Validation,
            "channel.id_nil",
            "channel read port requires a non-nil ID selector",
            false,
        )),
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
        crate::ChannelError::Validation(message)
        | crate::ChannelError::InvalidTargetType(message)
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
        crate::ChannelError::Database(error) => {
            tracing::error!(error = ?error, "channel port storage operation failed");
            PortError::new(
                PortErrorKind::Unavailable,
                "channel.read_failed",
                "channel storage is temporarily unavailable",
                true,
            )
        }
        crate::ChannelError::Serialization(error) => {
            tracing::error!(error = ?error, "channel port projection encoding failed");
            PortError::new(
                PortErrorKind::InvariantViolation,
                "channel.serialization_failed",
                "channel projection could not be encoded",
                false,
            )
        }
    }
}

#[cfg(test)]
#[path = "ports_tests.rs"]
mod tests;
