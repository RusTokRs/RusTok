use sea_orm::{EntityTrait, Set, sea_query::OnConflict};
use uuid::Uuid;

use crate::common::settings::EventDeliveryProfile;
use crate::models::_entities::event_delivery_settings::{self, Entity};
use crate::services::server_runtime_context::ServerRuntimeContext;

const SINGLETON_ID: i32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventDeliveryConfiguration {
    pub active_profile: EventDeliveryProfile,
    pub desired_profile: EventDeliveryProfile,
    pub iggy_configured: bool,
}

#[derive(Debug)]
pub enum EventDeliverySettingsError {
    InvalidProfile(String),
    IggyNotConfigured(String),
    Database(sea_orm::DbErr),
}

impl std::fmt::Display for EventDeliverySettingsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidProfile(value) => write!(
                formatter,
                "invalid event delivery profile `{value}`; expected outbox or outbox_iggy"
            ),
            Self::IggyNotConfigured(reason) => write!(
                formatter,
                "Configure Iggy before selecting outbox_iggy: {reason}"
            ),
            Self::Database(error) => {
                write!(formatter, "event delivery settings database error: {error}")
            }
        }
    }
}

impl From<sea_orm::DbErr> for EventDeliverySettingsError {
    fn from(value: sea_orm::DbErr) -> Self {
        Self::Database(value)
    }
}

pub struct EventDeliverySettingsService;

impl EventDeliverySettingsService {
    pub async fn configuration(
        ctx: &ServerRuntimeContext,
    ) -> Result<EventDeliveryConfiguration, EventDeliverySettingsError> {
        let active_profile = Self::active_profile(ctx);
        let desired_profile = Self::desired_profile(ctx).await?;
        Ok(EventDeliveryConfiguration {
            active_profile,
            desired_profile,
            iggy_configured:
                crate::services::iggy_connector_settings_service::IggyConnectorSettingsService::readiness_error(ctx)
                    .await
                    .is_none(),
        })
    }

    pub fn active_profile(ctx: &ServerRuntimeContext) -> EventDeliveryProfile {
        ctx.shared_get::<std::sync::Arc<crate::services::event_transport_factory::EventRuntime>>()
            .map(|runtime| runtime.delivery_profile)
            .unwrap_or(ctx.settings().events.delivery_profile)
    }

    pub async fn desired_profile(
        ctx: &ServerRuntimeContext,
    ) -> Result<EventDeliveryProfile, EventDeliverySettingsError> {
        match Entity::find_by_id(SINGLETON_ID).one(ctx.db()).await? {
            Some(row) => parse_profile(&row.profile),
            None => Ok(ctx.settings().events.delivery_profile),
        }
    }

    pub async fn save_profile(
        ctx: &ServerRuntimeContext,
        profile: EventDeliveryProfile,
        actor_id: Uuid,
    ) -> Result<(), EventDeliverySettingsError> {
        if profile.requires_iggy()
            && let Some(reason) =
                crate::services::iggy_connector_settings_service::IggyConnectorSettingsService::readiness_error(ctx)
                    .await
        {
            return Err(EventDeliverySettingsError::IggyNotConfigured(reason));
        }

        let now: chrono::DateTime<chrono::FixedOffset> = chrono::Utc::now().into();
        Entity::insert(event_delivery_settings::ActiveModel {
            id: Set(SINGLETON_ID),
            profile: Set(profile.as_str().to_string()),
            updated_by: Set(Some(actor_id)),
            created_at: Set(now),
            updated_at: Set(now),
        })
        .on_conflict(
            OnConflict::column(event_delivery_settings::Column::Id)
                .update_columns([
                    event_delivery_settings::Column::Profile,
                    event_delivery_settings::Column::UpdatedBy,
                    event_delivery_settings::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .exec_without_returning(ctx.db())
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::settings::RustokSettings;
    use crate::services::event_transport_factory::EventRuntime;
    use rustok_core::events::MemoryTransport;
    use std::sync::Arc;

    #[tokio::test]
    async fn singleton_profile_saves_preserve_creation_and_replace_actor() {
        let db = rustok_test_utils::db::setup_test_db_with_migrations::<
            rustok_migrations::SqliteTestMigrator,
        >()
        .await;
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        let first_actor = Uuid::new_v4();
        let second_actor = Uuid::new_v4();
        let final_actor = Uuid::new_v4();

        let (first_save, second_save) = tokio::join!(
            EventDeliverySettingsService::save_profile(
                &ctx,
                EventDeliveryProfile::Outbox,
                first_actor,
            ),
            EventDeliverySettingsService::save_profile(
                &ctx,
                EventDeliveryProfile::Outbox,
                second_actor,
            ),
        );
        first_save.expect("first save");
        second_save.expect("second save");
        let first = Entity::find_by_id(SINGLETON_ID)
            .one(ctx.db())
            .await
            .expect("read")
            .expect("singleton");
        assert!(
            matches!(first.updated_by, Some(actor) if actor == first_actor || actor == second_actor)
        );

        EventDeliverySettingsService::save_profile(&ctx, EventDeliveryProfile::Outbox, final_actor)
            .await
            .expect("final save");
        let current = Entity::find_by_id(SINGLETON_ID)
            .one(ctx.db())
            .await
            .expect("read")
            .expect("singleton");

        assert_eq!(current.created_at, first.created_at);
        assert_eq!(current.updated_by, Some(final_actor));
        assert_eq!(current.profile, EventDeliveryProfile::Outbox.as_str());
    }

    #[test]
    fn active_profile_comes_from_published_runtime_not_bootstrap_default() {
        let mut settings = RustokSettings::default();
        settings.events.delivery_profile = EventDeliveryProfile::Outbox;
        let ctx = ServerRuntimeContext::new(sea_orm::DatabaseConnection::default(), settings);
        assert_eq!(EventDeliverySettingsService::active_profile(&ctx), EventDeliveryProfile::Outbox);

        ctx.shared_insert(Arc::new(EventRuntime {
            delivery_profile: EventDeliveryProfile::OutboxIggy,
            iggy_mode: None,
            transport: crate::services::event_bus::shared_event_transport(&ctx),
            listener_bus: MemoryTransport::with_capacity(8).event_bus(),
            relay_config: None,
            outbox_retention: None,
            channel_capacity: 8,
            relay_fallback_active: false,
        }));
        assert_eq!(
            EventDeliverySettingsService::active_profile(&ctx),
            EventDeliveryProfile::OutboxIggy,
        );
    }
}

fn parse_profile(value: &str) -> Result<EventDeliveryProfile, EventDeliverySettingsError> {
    EventDeliveryProfile::parse(value)
        .ok_or_else(|| EventDeliverySettingsError::InvalidProfile(value.to_string()))
}
