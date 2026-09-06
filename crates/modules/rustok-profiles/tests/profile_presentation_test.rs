use rustok_profiles::entities;
use rustok_profiles::{
    ProfileAccessAudience, ProfileError, ProfileMutationContext, ProfilePresentationService,
    ProfileService, ProfileStatus, ProfileVisibility, UpsertProfileInput,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

mod support;

fn profile_input(
    handle: &str,
    display_name: &str,
    visibility: ProfileVisibility,
) -> UpsertProfileInput {
    UpsertProfileInput {
        handle: handle.to_string(),
        display_name: display_name.to_string(),
        bio: Some(format!("{display_name} bio")),
        tags: Vec::new(),
        avatar_media_id: None,
        banner_media_id: None,
        preferred_locale: Some("en".to_string()),
        visibility,
    }
}

async fn create_profile(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    user_id: Uuid,
    handle: &str,
    display_name: &str,
    visibility: ProfileVisibility,
) {
    let event_bus = rustok_outbox::TransactionalEventBus::new(std::sync::Arc::new(
        rustok_outbox::OutboxTransport::new(db.clone()),
    ));
    let mutations = rustok_profiles::ProfileMutationService::new(db, &event_bus);
    mutations
        .upsert_profile_with_event(
            ProfileMutationContext {
                tenant_id,
                actor_id: user_id,
                user_id,
                tenant_default_locale: Some("en"),
            },
            profile_input(handle, display_name, visibility),
        )
        .await
        .expect("profile should be created");
}

async fn hide_profile(db: &sea_orm::DatabaseConnection, tenant_id: Uuid, user_id: Uuid) {
    let profile = entities::profile::Entity::find_by_id(user_id)
        .filter(entities::profile::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .expect("profile lookup should succeed")
        .expect("profile should exist");
    let mut active: entities::profile::ActiveModel = profile.into();
    active.status = Set(ProfileStatus::Hidden);
    active
        .update(db)
        .await
        .expect("profile status should update");
}

#[tokio::test]
async fn presentation_service_filters_summaries_for_every_audience_class() {
    let db = support::setup_profiles_test_db().await;
    let _service = ProfileService::new(db.clone());
    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    let public_id = Uuid::new_v4();
    let authenticated_id = Uuid::new_v4();
    let private_id = Uuid::new_v4();
    let hidden_id = Uuid::new_v4();
    let cross_tenant_id = Uuid::new_v4();

    create_profile(
        &db,
        tenant_id,
        public_id,
        "public-profile",
        "Public Profile",
        ProfileVisibility::Public,
    )
    .await;
    create_profile(
        &db,
        tenant_id,
        authenticated_id,
        "authenticated-profile",
        "Authenticated Profile",
        ProfileVisibility::Authenticated,
    )
    .await;
    create_profile(
        &db,
        tenant_id,
        private_id,
        "private-profile",
        "Private Profile",
        ProfileVisibility::Private,
    )
    .await;
    create_profile(
        &db,
        tenant_id,
        hidden_id,
        "hidden-profile",
        "Hidden Profile",
        ProfileVisibility::Public,
    )
    .await;
    hide_profile(&db, tenant_id, hidden_id).await;
    create_profile(
        &db,
        other_tenant_id,
        cross_tenant_id,
        "cross-tenant-profile",
        "Cross Tenant Profile",
        ProfileVisibility::Public,
    )
    .await;

    let requested = [
        public_id,
        authenticated_id,
        private_id,
        hidden_id,
        cross_tenant_id,
    ];

    let anonymous = ProfilePresentationService::new(db.clone())
        .find_profile_summaries(tenant_id, &requested, Some("en"), Some("en"))
        .await
        .expect("anonymous presentation should resolve");
    assert_eq!(anonymous.len(), 1);
    assert!(anonymous.contains_key(&public_id));

    let unrelated_actor_id = Uuid::new_v4();
    let authenticated = ProfilePresentationService::for_audience(
        db.clone(),
        ProfileAccessAudience::Authenticated {
            actor_id: unrelated_actor_id,
        },
    )
    .find_profile_summaries(tenant_id, &requested, Some("en"), Some("en"))
    .await
    .expect("authenticated presentation should resolve");
    assert_eq!(authenticated.len(), 2);
    assert!(authenticated.contains_key(&public_id));
    assert!(authenticated.contains_key(&authenticated_id));
    assert!(!authenticated.contains_key(&private_id));

    let owner = ProfilePresentationService::for_audience(
        db.clone(),
        ProfileAccessAudience::Authenticated {
            actor_id: private_id,
        },
    )
    .find_profile_summaries(tenant_id, &requested, Some("en"), Some("en"))
    .await
    .expect("owner presentation should resolve");
    assert_eq!(owner.len(), 3);
    assert!(owner.contains_key(&public_id));
    assert!(owner.contains_key(&authenticated_id));
    assert!(owner.contains_key(&private_id));
    assert!(!owner.contains_key(&hidden_id));
    assert!(!owner.contains_key(&cross_tenant_id));

    let trusted_service = ProfilePresentationService::for_audience(
        db,
        ProfileAccessAudience::TrustedService { actor_id: None },
    )
    .find_profile_summaries(tenant_id, &requested, Some("en"), Some("en"))
    .await
    .expect("trusted service presentation should resolve");
    assert_eq!(trusted_service.len(), 2);
    assert!(trusted_service.contains_key(&public_id));
    assert!(trusted_service.contains_key(&authenticated_id));
    assert!(!trusted_service.contains_key(&private_id));
    assert!(!trusted_service.contains_key(&hidden_id));
    assert!(!trusted_service.contains_key(&cross_tenant_id));
}

#[tokio::test]
async fn presentation_handle_lookup_hides_private_and_hidden_profiles() {
    let db = support::setup_profiles_test_db().await;
    let _service = ProfileService::new(db.clone());
    let tenant_id = Uuid::new_v4();
    let private_id = Uuid::new_v4();
    let hidden_id = Uuid::new_v4();

    create_profile(
        &db,
        tenant_id,
        private_id,
        "private-handle",
        "Private Handle",
        ProfileVisibility::Private,
    )
    .await;
    create_profile(
        &db,
        tenant_id,
        hidden_id,
        "hidden-handle",
        "Hidden Handle",
        ProfileVisibility::Public,
    )
    .await;
    hide_profile(&db, tenant_id, hidden_id).await;

    let unrelated = ProfilePresentationService::for_audience(
        db.clone(),
        ProfileAccessAudience::Authenticated {
            actor_id: Uuid::new_v4(),
        },
    );
    assert!(matches!(
        unrelated
            .get_profile_by_handle(tenant_id, "private-handle", Some("en"), Some("en"))
            .await,
        Err(ProfileError::ProfileByHandleNotFound(_))
    ));

    let owner = ProfilePresentationService::for_audience(
        db,
        ProfileAccessAudience::Authenticated {
            actor_id: private_id,
        },
    );
    let private_profile = owner
        .get_profile_by_handle(tenant_id, "private-handle", Some("en"), Some("en"))
        .await
        .expect("owner should see the private profile");
    assert_eq!(private_profile.user_id, private_id);
    assert!(matches!(
        owner
            .get_profile_by_handle(tenant_id, "hidden-handle", Some("en"), Some("en"))
            .await,
        Err(ProfileError::ProfileByHandleNotFound(_))
    ));
}

#[tokio::test]
async fn single_summary_uses_the_same_policy_as_the_batch_path() {
    let db = support::setup_profiles_test_db().await;
    let _service = ProfileService::new(db.clone());
    let tenant_id = Uuid::new_v4();
    let private_id = Uuid::new_v4();

    create_profile(
        &db,
        tenant_id,
        private_id,
        "single-private",
        "Single Private",
        ProfileVisibility::Private,
    )
    .await;

    let unrelated = ProfilePresentationService::for_audience(
        db.clone(),
        ProfileAccessAudience::Authenticated {
            actor_id: Uuid::new_v4(),
        },
    )
    .find_profile_summary(tenant_id, private_id, Some("en"), Some("en"))
    .await
    .expect("unrelated summary lookup should resolve");
    assert!(unrelated.is_none());

    let owner = ProfilePresentationService::for_audience(
        db,
        ProfileAccessAudience::Authenticated {
            actor_id: private_id,
        },
    )
    .find_profile_summary(tenant_id, private_id, Some("en"), Some("en"))
    .await
    .expect("owner summary lookup should resolve")
    .expect("owner should receive the private summary");
    assert_eq!(owner.user_id, private_id);
}
