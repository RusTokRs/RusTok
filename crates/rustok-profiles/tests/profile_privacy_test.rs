use std::time::Duration;

use chrono::Utc;
use rustok_api::{PortActor, PortContext};
use rustok_profiles::entities;
use rustok_profiles::{
    ProfileAccessAudience, ProfilePrivacyDecision, ProfilePrivacyReadPort,
    ProfilePrivacyReadRequest, ProfilePrivacyService, ProfileStatus, ProfileVisibility,
};
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use uuid::Uuid;

mod support;

async fn insert_profile(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    user_id: Uuid,
    visibility: ProfileVisibility,
    status: ProfileStatus,
) {
    let now = Utc::now();
    entities::profile::ActiveModel {
        user_id: Set(user_id),
        tenant_id: Set(tenant_id),
        handle: Set(format!("profile-{}", &user_id.simple().to_string()[..8])),
        avatar_media_id: Set(None),
        banner_media_id: Set(None),
        preferred_locale: Set(None),
        visibility: Set(visibility),
        status: Set(status),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await
    .unwrap();
}

fn service_context(tenant_id: Uuid, correlation_id: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("profile-privacy-test"),
        "und",
        correlation_id,
    )
    .with_deadline(Duration::from_secs(1))
}

#[tokio::test]
async fn privacy_read_uses_base_profile_without_localized_copy() {
    let db = support::setup_profiles_test_db().await;
    let tenant_id = Uuid::new_v4();
    let recipient_id = Uuid::new_v4();
    insert_profile(
        &db,
        tenant_id,
        recipient_id,
        ProfileVisibility::Public,
        ProfileStatus::Active,
    )
    .await;

    let decision = ProfilePrivacyService::new(db)
        .evaluate_profile_privacy(
            service_context(tenant_id, "profile-privacy-base-row"),
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
    insert_profile(
        &db,
        owner_tenant_id,
        recipient_id,
        ProfileVisibility::Public,
        ProfileStatus::Active,
    )
    .await;

    let decision = ProfilePrivacyService::new(db)
        .evaluate_profile_privacy(
            service_context(other_tenant_id, "profile-privacy-tenant-scope"),
            ProfilePrivacyReadRequest {
                recipient_id,
                actor_id: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(decision, ProfilePrivacyDecision::RecipientUnavailable);
}

#[tokio::test]
async fn authenticated_visibility_requires_a_non_anonymous_audience() {
    let db = support::setup_profiles_test_db().await;
    let tenant_id = Uuid::new_v4();
    let recipient_id = Uuid::new_v4();
    insert_profile(
        &db,
        tenant_id,
        recipient_id,
        ProfileVisibility::Authenticated,
        ProfileStatus::Active,
    )
    .await;

    let service = ProfilePrivacyService::new(db);
    assert_eq!(
        service
            .evaluate_access(
                tenant_id,
                recipient_id,
                ProfileAccessAudience::Anonymous,
            )
            .await
            .unwrap(),
        ProfilePrivacyDecision::Restricted
    );
    assert_eq!(
        service
            .evaluate_access(
                tenant_id,
                recipient_id,
                ProfileAccessAudience::Authenticated {
                    actor_id: Uuid::new_v4(),
                },
            )
            .await
            .unwrap(),
        ProfilePrivacyDecision::Allow
    );
    assert_eq!(
        service
            .evaluate_access(
                tenant_id,
                recipient_id,
                ProfileAccessAudience::TrustedService { actor_id: None },
            )
            .await
            .unwrap(),
        ProfilePrivacyDecision::Allow
    );
}

#[tokio::test]
async fn active_private_profile_is_owner_only() {
    let db = support::setup_profiles_test_db().await;
    let tenant_id = Uuid::new_v4();
    let recipient_id = Uuid::new_v4();
    insert_profile(
        &db,
        tenant_id,
        recipient_id,
        ProfileVisibility::Private,
        ProfileStatus::Active,
    )
    .await;

    let service = ProfilePrivacyService::new(db);
    assert_eq!(
        service
            .evaluate_access(
                tenant_id,
                recipient_id,
                ProfileAccessAudience::Authenticated {
                    actor_id: recipient_id,
                },
            )
            .await
            .unwrap(),
        ProfilePrivacyDecision::Allow
    );
    assert_eq!(
        service
            .evaluate_access(
                tenant_id,
                recipient_id,
                ProfileAccessAudience::Authenticated {
                    actor_id: Uuid::new_v4(),
                },
            )
            .await
            .unwrap(),
        ProfilePrivacyDecision::Restricted
    );
    assert_eq!(
        service
            .evaluate_access(
                tenant_id,
                recipient_id,
                ProfileAccessAudience::TrustedService { actor_id: None },
            )
            .await
            .unwrap(),
        ProfilePrivacyDecision::Restricted
    );
}

#[tokio::test]
async fn hidden_profile_is_unavailable_even_to_owner_audience() {
    let db = support::setup_profiles_test_db().await;
    let tenant_id = Uuid::new_v4();
    let recipient_id = Uuid::new_v4();
    insert_profile(
        &db,
        tenant_id,
        recipient_id,
        ProfileVisibility::Public,
        ProfileStatus::Hidden,
    )
    .await;

    let decision = ProfilePrivacyService::new(db)
        .evaluate_access(
            tenant_id,
            recipient_id,
            ProfileAccessAudience::Authenticated {
                actor_id: recipient_id,
            },
        )
        .await
        .unwrap();

    assert_eq!(decision, ProfilePrivacyDecision::RecipientUnavailable);
}

#[tokio::test]
async fn user_port_actor_cannot_claim_another_profile_actor() {
    let db = support::setup_profiles_test_db().await;
    let tenant_id = Uuid::new_v4();
    let context_actor_id = Uuid::new_v4();
    let request_actor_id = Uuid::new_v4();
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(context_actor_id.to_string()),
        "und",
        "profile-privacy-actor-mismatch",
    )
    .with_deadline(Duration::from_secs(1));

    let error = ProfilePrivacyService::new(db)
        .evaluate_profile_privacy(
            context,
            ProfilePrivacyReadRequest {
                recipient_id: Uuid::new_v4(),
                actor_id: Some(request_actor_id),
            },
        )
        .await
        .unwrap_err();

    assert_eq!(error.code, "profiles.actor_id_mismatch");
}

#[tokio::test]
async fn privacy_batch_returns_one_decision_per_distinct_requested_profile() {
    let db = support::setup_profiles_test_db().await;
    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    let public_id = Uuid::new_v4();
    let authenticated_id = Uuid::new_v4();
    let private_id = Uuid::new_v4();
    let hidden_id = Uuid::new_v4();
    let cross_tenant_id = Uuid::new_v4();
    let missing_id = Uuid::new_v4();

    insert_profile(
        &db,
        tenant_id,
        public_id,
        ProfileVisibility::Public,
        ProfileStatus::Active,
    )
    .await;
    insert_profile(
        &db,
        tenant_id,
        authenticated_id,
        ProfileVisibility::Authenticated,
        ProfileStatus::Active,
    )
    .await;
    insert_profile(
        &db,
        tenant_id,
        private_id,
        ProfileVisibility::Private,
        ProfileStatus::Active,
    )
    .await;
    insert_profile(
        &db,
        tenant_id,
        hidden_id,
        ProfileVisibility::Public,
        ProfileStatus::Hidden,
    )
    .await;
    insert_profile(
        &db,
        other_tenant_id,
        cross_tenant_id,
        ProfileVisibility::Public,
        ProfileStatus::Active,
    )
    .await;

    let service = ProfilePrivacyService::new(db);
    let requested = [
        public_id,
        authenticated_id,
        private_id,
        hidden_id,
        cross_tenant_id,
        missing_id,
        public_id,
    ];
    let anonymous = service
        .evaluate_access_batch(tenant_id, &requested, ProfileAccessAudience::Anonymous)
        .await
        .unwrap();

    assert_eq!(anonymous.len(), 6);
    assert_eq!(anonymous[&public_id], ProfilePrivacyDecision::Allow);
    assert_eq!(
        anonymous[&authenticated_id],
        ProfilePrivacyDecision::Restricted
    );
    assert_eq!(anonymous[&private_id], ProfilePrivacyDecision::Restricted);
    assert_eq!(
        anonymous[&hidden_id],
        ProfilePrivacyDecision::RecipientUnavailable
    );
    assert_eq!(
        anonymous[&cross_tenant_id],
        ProfilePrivacyDecision::RecipientUnavailable
    );
    assert_eq!(
        anonymous[&missing_id],
        ProfilePrivacyDecision::RecipientUnavailable
    );

    let owner = service
        .evaluate_access_batch(
            tenant_id,
            &requested,
            ProfileAccessAudience::Authenticated {
                actor_id: private_id,
            },
        )
        .await
        .unwrap();
    assert_eq!(owner[&public_id], ProfilePrivacyDecision::Allow);
    assert_eq!(owner[&authenticated_id], ProfilePrivacyDecision::Allow);
    assert_eq!(owner[&private_id], ProfilePrivacyDecision::Allow);
    assert_eq!(
        owner[&hidden_id],
        ProfilePrivacyDecision::RecipientUnavailable
    );
}
