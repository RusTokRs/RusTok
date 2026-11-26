use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use rustok_api::{PortActorKind, PortContext, PortError};
use rustok_channel_ports::{
    CHANNEL_PUBLIC_MODULE_READ_CLAIM, ChannelPublicModuleReadPort, ChannelPublicModuleSelector,
    ReadChannelPublicModule,
};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect,
    sea_query::{Expr, ExprTrait, Query},
};

use super::{CHANNEL_MODULE_SLUG_MAX_CHARS, CHANNEL_SLUG_MAX_CHARS, ChannelService};
use crate::entities::{channel, channel_module_binding};
use crate::ports::{map_channel_error, parse_tenant_id};

const MAX_PUBLIC_MODULE_READ_DEADLINE: Duration = Duration::from_secs(30);

pub fn in_process_public_module_read_port(
    db: DatabaseConnection,
) -> Arc<dyn ChannelPublicModuleReadPort> {
    Arc::new(ChannelService::new(db))
}

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
            || !context
                .claims
                .iter()
                .any(|claim| claim == CHANNEL_PUBLIC_MODULE_READ_CLAIM)
        {
            return Err(PortError::forbidden(
                "channel.public_module_forbidden",
                "Channel availability access is denied",
            ));
        }
        let tenant_id = parse_tenant_id(&context)?;
        let deadline = Duration::from_millis(context.deadline_ms.unwrap_or_default());
        let module_slug = request.module_slug.trim().to_ascii_lowercase();
        if deadline > MAX_PUBLIC_MODULE_READ_DEADLINE
            || module_slug.is_empty()
            || module_slug.chars().count() > CHANNEL_MODULE_SLUG_MAX_CHARS
        {
            return Err(PortError::validation(
                "channel.public_module_request_invalid",
                "Channel availability request exceeds its bounds",
            ));
        }
        let (selector, required_identity) = match request.channel {
            ChannelPublicModuleSelector::Id(id) => (channel::Column::Id.eq(id), true),
            ChannelPublicModuleSelector::Slug(slug) => {
                let slug = slug.trim().to_ascii_lowercase();
                if slug.is_empty() || slug.chars().count() > CHANNEL_SLUG_MAX_CHARS {
                    return Err(PortError::validation(
                        "channel.public_module_request_invalid",
                        "Channel slug exceeds its bounds",
                    ));
                }
                (channel::Column::Slug.eq(slug), false)
            }
        };

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
            .filter(selector)
            .select_only()
            .column(channel::Column::IsActive)
            .expr_as(
                Expr::col((channel::Entity, channel::Column::Id)).not_in_subquery(disabled),
                "module_enabled",
            )
            .into_tuple::<(bool, bool)>();
        let state = tokio::time::timeout(deadline, query.one(&self.db))
            .await
            .map_err(|_| {
                PortError::timeout(
                    "channel.public_module_timeout",
                    "Channel availability read timed out",
                )
            })?
            .map_err(|error| map_channel_error(error.into()))?;
        match state {
            Some((active, enabled)) => Ok(active && enabled),
            None if required_identity => Err(PortError::not_found(
                "channel.public_module_channel_not_found",
                "Channel is not available in this tenant",
            )),
            None => Ok(false),
        }
    }
}
