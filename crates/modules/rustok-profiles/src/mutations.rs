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

/// Tenant, actor, subject, and locale context required by a user-initiated
/// profile mutation.
///
/// The actor is deliberately mandatory here: every self-service write must
/// produce an attributable `ProfileUpdated` event in the same transaction.
#[derive(Debug, Clone, Copy)]
pub struct ProfileMutationContext<'a> {
    pub tenant_id: Uuid,
    pub actor_id: Uuid,
    pub user_id: Uuid,
    pub tenant_default_locale: Option<&'a str>,
}

/// Owner-local request for provisioning one missing profile.
///
/// Backfill is system-initiated, so it intentionally has no human actor id.
#[derive(Debug, Clone, Copy)]
pub struct ProfileBackfillRequest<'a> {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub email: &'a str,
    pub display_name: Option<&'a str>,
    pub preferred_locale: Option<&'a str>,
    pub visibility: ProfileVisibility,
    pub tenant_default_locale: Option<&'a str>,
}

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

    pub async fn upsert_profile_with_event(
        &self,
        context: ProfileMutationContext<'_>,
        input: UpsertProfileInput,
    ) -> ProfileResult<ProfileRecord> {
        upsert_profile_with_event(self.db, self.event_bus, context, input).await
    }

    pub async fn update_profile_handle_with_event(
        &self,
        context: ProfileMutationContext<'_>,
        handle: &str,
    ) -> ProfileResult<ProfileRecord> {
        update_profile_handle_with_event(self.db, self.event_bus, context, handle).await
    }

    pub async fn update_profile_content_with_event(
        &self,
        context: ProfileMutationContext<'_>,
        display_name: &str,
        bio: Option<&str>,
    ) -> ProfileResult<ProfileRecord> {
        update_profile_content_with_event(self.db, self.event_bus, context, display_name, bio).await
    }

    pub async fn update_profile_locale_with_event(
        &self,
        context: ProfileMutationContext<'_>,
        preferred_locale: Option<&str>,
    ) -> ProfileResult<ProfileRecord> {
        update_profile_locale_with_event(self.db, self.event_bus, context, preferred_locale).await
    }

    pub async fn update_profile_visibility_with_event(
        &self,
        context: ProfileMutationContext<'_>,
        visibility: ProfileVisibility,
    ) -> ProfileResult<ProfileRecord> {
        update_profile_visibility_with_event(self.db, self.event_bus, context, visibility).await
    }

    pub async fn update_profile_media_with_event(
        &self,
        context: ProfileMutationContext<'_>,
        avatar_media_id: Option<Uuid>,
        banner_media_id: Option<Uuid>,
    ) -> ProfileResult<ProfileRecord> {
        update_profile_media_with_event(
            self.db,
            self.event_bus,
            context,
            avatar_media_id,
            banner_media_id,
        )
        .await
    }

    pub async fn backfill_profile_with_event(
        &self,
        request: ProfileBackfillRequest<'_>,
    ) -> ProfileResult<ProfileBackfillResult> {
        backfill_profile_with_event(self.db, self.event_bus, request).await
    }
}
