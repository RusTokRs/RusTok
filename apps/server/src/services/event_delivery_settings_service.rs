use sea_orm::{ActiveModelTrait, EntityTrait, Set};
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
                "invalid event delivery profile `{value}`; expected memory, outbox_local, or outbox_iggy"
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
        let active_profile = ctx.settings().events.delivery_profile;
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
        if profile.requires_iggy() {
            if let Some(reason) =
                crate::services::iggy_connector_settings_service::IggyConnectorSettingsService::readiness_error(ctx)
                    .await
            {
                return Err(EventDeliverySettingsError::IggyNotConfigured(reason));
            }
        }

        let now: chrono::DateTime<chrono::FixedOffset> = chrono::Utc::now().into();
        match Entity::find_by_id(SINGLETON_ID).one(ctx.db()).await? {
            Some(row) => {
                let mut active: event_delivery_settings::ActiveModel = row.into();
                active.profile = Set(profile.as_str().to_string());
                active.updated_by = Set(Some(actor_id));
                active.updated_at = Set(now);
                active.update(ctx.db()).await?;
            }
            None => {
                event_delivery_settings::ActiveModel {
                    id: Set(SINGLETON_ID),
                    profile: Set(profile.as_str().to_string()),
                    updated_by: Set(Some(actor_id)),
                    created_at: Set(now),
                    updated_at: Set(chrono::Utc::now().into()),
                }
                .insert(ctx.db())
                .await?;
            }
        }
        Ok(())
    }
}

fn parse_profile(value: &str) -> Result<EventDeliveryProfile, EventDeliverySettingsError> {
    EventDeliveryProfile::parse(value)
        .ok_or_else(|| EventDeliverySettingsError::InvalidProfile(value.to_string()))
}
