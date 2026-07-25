use std::time::Duration;

use chrono::Utc;
use rustok_api::{PortActor, PortContext};
use rustok_profiles::entities;
use rustok_profiles::{
    ProfilePrivacyDecision, ProfilePrivacyReadPort, ProfilePrivacyReadRequest,
    ProfilePrivacyService, ProfileStatus, ProfileVisibility,
};
use sea_orm::{ActiveModelTrait, Set};
use uuid::Uuid;

mod support;

#[tokio::test]
async fn privacy_read_uses_base_profile_without_localized_copy() {
    let db = support::setup_profiles_test_db().await;
    let tenant_id = Uuid::new_v4();
    let recipient_id = Uuid::new_v4();
    let now = Utc::now();

    entities::profile::ActiveModel {
        user_id: Set(recipient_id),
        tenant_id: Set(tenant_id),
        handle: Set("privacy-owner".to_string()),
        avatar_media_id: Set(None),
        banner_media_id: Set(None),
        preferred_locale: Set(None),
        visibility: Set(ProfileVisibility::Public),
        status: Set(ProfileStatus::Active),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await
    .unwrap();

    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("profile-privacy-test"),
        "und",
        "profile-privacy-base-row",
    )
    .with_deadline(Duration::from_secs(1));
    let decision = ProfilePrivacyService::new(db)
        .evaluate_profile_privacy(
            context,
            ProfilePrivacyReadRequest {
                recipient_id,
                actor_id: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(decision, ProfilePrivacyDecision::Allow);
}

#[tokio::test]
async fn privacy_read_keeps_tenant_scope_on_base_profile() {
    let db = support::setup_profiles_test_db().await;
    let owner_tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    let recipient_id = Uuid::new_v4();
    let now = Utc::now();

    entities::profile::ActiveModel {
        user_id: Set(recipient_id),
        tenant_id: Set(owner_tenant_id),
        handle: Set("tenant-owner".to_string()),
        avatar_media_id: Set(None),
        banner_media_id: Set(None),
        preferred_locale: Set(None),
        visibility: Set(ProfileVisibility::Public),
        status: Set(ProfileStatus::Active),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await
    .unwrap();

    let context = PortContext::new(
        other_tenant_id.to_string(),
        PortActor::service("profile-privacy-test"),
        "und",
        "profile-privacy-tenant-scope",
    )
    .with_deadline(Duration::from_secs(1));
    let decision = ProfilePrivacyService::new(db)
        .evaluate_profile_privacy(
            context,
            ProfilePrivacyReadRequest {
                recipient_id,
                actor_id: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(decision, ProfilePrivacyDecision::RecipientUnavailable);
}
