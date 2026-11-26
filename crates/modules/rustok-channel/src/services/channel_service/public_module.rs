use std::time::Duration;

use async_trait::async_trait;
use rustok_api::{PortActorKind, PortContext, PortError};
use rustok_channel_ports::{
    CHANNEL_PUBLIC_MODULE_READ_CLAIM, ChannelPublicModuleReadPort, ReadChannelPublicModule,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect, sea_query::Query};
use uuid::Uuid;

use super::{CHANNEL_MODULE_SLUG_MAX_CHARS, CHANNEL_SLUG_MAX_CHARS, ChannelService};
use crate::entities::{channel, channel_module_binding};
use crate::ports::{map_channel_error, parse_tenant_id};

const MAX_PUBLIC_MODULE_READ_DEADLINE: Duration = Duration::from_secs(30);

#[async_trait]
impl ChannelPublicModuleReadPort for ChannelService {
    async fn is_public_module_available(
        &self,
        context: PortContext,
        request: ReadChannelPublicModule,
    ) -> Result<bool, PortError> {
        context.require_read_semantics()?;
        if context.actor.kind != PortActorKind::Service
            || context.actor.id.trim().is_empty()
            || !context.claims.iter().any(|claim| claim == CHANNEL_PUBLIC_MODULE_READ_CLAIM)
        {
            return Err(PortError::forbidden("channel.public_module_forbidden", "Channel availability access is denied"));
        }
        let tenant_id = parse_tenant_id(&context)?;
        let deadline = Duration::from_millis(context.deadline_ms.unwrap_or_default());
        let channel_slug = request.channel_slug.trim().to_ascii_lowercase();
        let module_slug = request.module_slug.trim().to_ascii_lowercase();
        if deadline > MAX_PUBLIC_MODULE_READ_DEADLINE
            || channel_slug.is_empty()
            || channel_slug.chars().count() > CHANNEL_SLUG_MAX_CHARS
            || module_slug.is_empty()
            || module_slug.chars().count() > CHANNEL_MODULE_SLUG_MAX_CHARS
        {
            return Err(PortError::validation("channel.public_module_request_invalid", "Channel availability request exceeds its bounds"));
        }

        // Resolve the identity, active state and binding in one database observation.
        // Channel IDs are unique and every binding has a non-null channel ID.
        let disabled = Query::select()
            .column(channel_module_binding::Column::ChannelId)
            .from(channel_module_binding::Entity)
            .and_where(channel_module_binding::Column::ModuleSlug.eq(module_slug))
            .and_where(channel_module_binding::Column::IsEnabled.eq(false))
            .to_owned();
        let query = channel::Entity::find()
            .filter(channel::Column::TenantId.eq(tenant_id))
            .filter(channel::Column::Slug.eq(channel_slug))
            .filter(channel::Column::IsActive.eq(true))
            .filter(channel::Column::Id.not_in_subquery(disabled))
            .select_only()
            .column(channel::Column::Id)
            .into_tuple::<Uuid>();
        let channel_id = tokio::time::timeout(deadline, query.one(&self.db))
            .await
            .map_err(|_| PortError::timeout("channel.public_module_timeout", "Channel availability read timed out"))?
            .map_err(|error| map_channel_error(error.into()))?;
        Ok(channel_id.is_some())
    }
}
