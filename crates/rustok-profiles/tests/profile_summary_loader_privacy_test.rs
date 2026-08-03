use async_graphql::dataloader::Loader;
use rustok_profiles::entities;
use rustok_profiles::{
    ProfileAccessAudience, ProfileMutationContext, ProfileService, ProfileStatus,
    ProfileSummaryLoader, ProfileSummaryLoaderKey, ProfileVisibility, UpsertProfileInput,
};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use uuid::Uuid;

mod support;

async fn create_profile(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    user_id: Uuid,
    label: &str,
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
            UpsertProfileInput {
                handle: format!("{label}-{}", &user_id.simple().to_string()[..8]),
                display_name: label.to_string(),
                bio: None,
                tags: Vec::new(),
                avatar_media_id: None,
                banner_media_id: None,
                preferred_locale: Some("en".to_string()),
                visibility,
            },
        )
        .await
        .expect("profile should be created");
}

async fn set_status(db: &sea_orm::DatabaseConnection, user_id: Uuid, status: ProfileStatus) {
    let model = entities::profile::Entity::find_by_id(user_id)
        .one(db)
        .await
        .expect("profile lookup should succeed")
        .expect("profile should exist");
    let mut active: entities::profile::ActiveModel = model.into();
    active.status = Set(status);
    active
        .update(db)
        .await
        .expect("profile status should update");
}

fn key(tenant_id: Uuid, user_id: Uuid) -> ProfileSummaryLoaderKey {
    ProfileSummaryLoaderKey {
        tenant_id,
        user_id,
        requested_locale: Some("en".to_string()),
        tenant_default_locale: Some("en".to_string()),
    }
}

#[tokio::test]
async fn default_summary_loader_is_anonymous_and_fail_closed() {
    let db = support::setup_profiles_test_db().await;
    let tenant_id = Uuid::new_v4();
    let _service = ProfileService::new(db.clone());
    let public_id = Uuid::new_v4();
    let authenticated_id = Uuid::new_v4();
    let private_id = Uuid::new_v4();
    let hidden_id = Uuid::new_v4();

    create_profile(
        &db,
        tenant_id,
        public_id,
        "public",
        ProfileVisibility::Public,
    )
    .await;
    create_profile(
        &db,
        tenant_id,
        authenticated_id,
        "authenticated",
        ProfileVisibility::Authenticated,
    )
    .await;
    create_profile(
        &db,
        tenant_id,
        private_id,
        "private",
        ProfileVisibility::Private,
    )
    .await;
    create_profile(
        &db,
        tenant_id,
        hidden_id,
        "hidden",
        ProfileVisibility::Public,
    )
    .await;
    set_status(&db, hidden_id, ProfileStatus::Hidden).await;

    let keys = vec![
        key(tenant_id, public_id),
        key(tenant_id, authenticated_id),
        key(tenant_id, private_id),
        key(tenant_id, hidden_id),
    ];
    let result = ProfileSummaryLoader::new(db)
        .load(&keys)
        .await
        .expect("summary batch should load");

    assert!(result.contains_key(&key(tenant_id, public_id)));
    assert!(!result.contains_key(&key(tenant_id, authenticated_id)));
    assert!(!result.contains_key(&key(tenant_id, private_id)));
    assert!(!result.contains_key(&key(tenant_id, hidden_id)));
}

#[tokio::test]
async fn authenticated_summary_loader_allows_authenticated_and_owner_private_profiles() {
    let db = support::setup_profiles_test_db().await;
    let tenant_id = Uuid::new_v4();
    let _service = ProfileService::new(db.clone());
    let actor_id = Uuid::new_v4();
    let public_id = Uuid::new_v4();
    let authenticated_id = Uuid::new_v4();
    let other_private_id = Uuid::new_v4();

    create_profile(
        &db,
        tenant_id,
        actor_id,
        "owner-private",
        ProfileVisibility::Private,
    )
    .await;
    create_profile(
        &db,
        tenant_id,
        public_id,
        "public",
        ProfileVisibility::Public,
    )
    .await;
    create_profile(
        &db,
        tenant_id,
        authenticated_id,
        "authenticated",
        ProfileVisibility::Authenticated,
    )
    .await;
    create_profile(
        &db,
        tenant_id,
        other_private_id,
        "other-private",
        ProfileVisibility::Private,
    )
    .await;

    let keys = vec![
        key(tenant_id, actor_id),
        key(tenant_id, public_id),
        key(tenant_id, authenticated_id),
        key(tenant_id, other_private_id),
    ];
    let result =
        ProfileSummaryLoader::for_audience(db, ProfileAccessAudience::Authenticated { actor_id })
            .load(&keys)
            .await
            .expect("summary batch should load");

    assert!(result.contains_key(&key(tenant_id, actor_id)));
    assert!(result.contains_key(&key(tenant_id, public_id)));
    assert!(result.contains_key(&key(tenant_id, authenticated_id)));
    assert!(!result.contains_key(&key(tenant_id, other_private_id)));
}
