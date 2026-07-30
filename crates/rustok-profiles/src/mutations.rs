use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    ProfileBackfillResult, ProfileRecord, ProfileResult, ProfileVisibility, UpsertProfileInput,
    content_write::update_profile_content_with_event,
    handle_write::update_profile_handle_with_event,
    locale_write::update_profile_locale_with_event,
    media_write::update_profile_media_with_event,
    upsert_write::{backfill_profile_with_event, upsert_profile_with_event},
    visibility_write::update_profile_visibility_with_event,
};

/// Public Profiles mutation facade whose construction requires the durable event bus.
///
/// Every method delegates to a Profiles-owned transaction that commits the owner write only after
/// the corresponding `ProfileUpdated` outbox envelope has been persisted.
#[derive(Clone, Copy)]
pub struct ProfileMutationService<'a> {
    db: &'a DatabaseConnection,
    event_bus: &'a TransactionalEventBus,
}

impl<'a> ProfileMutationService<'a> {
    pub fn new(db: &'a DatabaseConnection, event_bus: &'a TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_profile_with_event(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        user_id: Uuid,
        input: UpsertProfileInput,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileRecord> {
        upsert_profile_with_event(
            self.db,
            self.event_bus,
            tenant_id,
            actor_id,
            user_id,
            input,
            tenant_default_locale,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_profile_handle_with_event(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        user_id: Uuid,
        handle: &str,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileRecord> {
        update_profile_handle_with_event(
            self.db,
            self.event_bus,
            tenant_id,
            actor_id,
            user_id,
            handle,
            tenant_default_locale,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_profile_content_with_event(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        user_id: Uuid,
        display_name: &str,
        bio: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileRecord> {
        update_profile_content_with_event(
            self.db,
            self.event_bus,
            tenant_id,
            actor_id,
            user_id,
            display_name,
            bio,
            tenant_default_locale,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_profile_locale_with_event(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        user_id: Uuid,
        preferred_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileRecord> {
        update_profile_locale_with_event(
            self.db,
            self.event_bus,
            tenant_id,
            actor_id,
            user_id,
            preferred_locale,
            tenant_default_locale,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_profile_visibility_with_event(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        user_id: Uuid,
        visibility: ProfileVisibility,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileRecord> {
        update_profile_visibility_with_event(
            self.db,
            self.event_bus,
            tenant_id,
            actor_id,
            user_id,
            visibility,
            tenant_default_locale,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_profile_media_with_event(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        user_id: Uuid,
        avatar_media_id: Option<Uuid>,
        banner_media_id: Option<Uuid>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileRecord> {
        update_profile_media_with_event(
            self.db,
            self.event_bus,
            tenant_id,
            actor_id,
            user_id,
            avatar_media_id,
            banner_media_id,
            tenant_default_locale,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn backfill_profile_with_event(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        email: &str,
        display_name: Option<&str>,
        preferred_locale: Option<&str>,
        visibility: ProfileVisibility,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileBackfillResult> {
        backfill_profile_with_event(
            self.db,
            self.event_bus,
            tenant_id,
            user_id,
            email,
            display_name,
            preferred_locale,
            visibility,
            tenant_default_locale,
        )
        .await
    }
}
