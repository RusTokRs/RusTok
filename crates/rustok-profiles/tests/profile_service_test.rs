use rustok_profiles::dto::{ProfileVisibility, UpsertProfileInput};
use rustok_profiles::error::ProfileError;
use rustok_profiles::services::ProfileService;
use uuid::Uuid;

mod support;

async fn setup() -> ProfileService {
    let db = support::setup_profiles_test_db().await;
    ProfileService::new(db)
}

fn profile_input() -> UpsertProfileInput {
    UpsertProfileInput {
        handle: "Creator-One".to_string(),
        display_name: "Creator One".to_string(),
        bio: Some("Primary profile bio".to_string()),
        avatar_media_id: Some(Uuid::new_v4()),
        banner_media_id: Some(Uuid::new_v4()),
        preferred_locale: Some("ru".to_string()),
        visibility: ProfileVisibility::Public,
    }
}

#[tokio::test]
async fn upsert_and_get_profile_by_user() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    let created = service
        .upsert_profile(tenant_id, user_id, profile_input(), Some("en"))
        .await
        .unwrap();

    assert_eq!(created.user_id, user_id);
    assert_eq!(created.handle, "creator-one");
    assert_eq!(created.display_name, "Creator One");
    assert_eq!(created.bio.as_deref(), Some("Primary profile bio"));
    assert_eq!(created.preferred_locale.as_deref(), Some("ru"));

    let fetched = service
        .get_profile(tenant_id, user_id, Some("de"), Some("en"))
        .await
        .unwrap();
    assert_eq!(fetched.handle, "creator-one");
    assert_eq!(fetched.display_name, "Creator One");
    assert_eq!(fetched.bio.as_deref(), Some("Primary profile bio"));
}

#[tokio::test]
async fn get_profile_by_handle_normalizes_lookup() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    service
        .upsert_profile(tenant_id, user_id, profile_input(), Some("en"))
        .await
        .unwrap();

    let fetched = service
        .get_profile_by_handle(tenant_id, "  CREATOR-one ", None, Some("en"))
        .await
        .unwrap();

    assert_eq!(fetched.user_id, user_id);
    assert_eq!(fetched.handle, "creator-one");
}

#[tokio::test]
async fn duplicate_handle_is_rejected_per_tenant() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();

    service
        .upsert_profile(tenant_id, Uuid::new_v4(), profile_input(), Some("en"))
        .await
        .unwrap();

    let error = service
        .upsert_profile(
            tenant_id,
            Uuid::new_v4(),
            UpsertProfileInput {
                handle: "creator-one".to_string(),
                display_name: "Second User".to_string(),
                bio: None,
                avatar_media_id: None,
                banner_media_id: None,
                preferred_locale: Some("en".to_string()),
                visibility: ProfileVisibility::Authenticated,
            },
            Some("en"),
        )
        .await
        .unwrap_err();

    match error {
        ProfileError::DuplicateHandle(handle) => assert_eq!(handle, "creator-one"),
        other => panic!("expected duplicate handle error, got {other:?}"),
    }
}

#[tokio::test]
async fn summary_uses_profile_reader_path() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    service
        .upsert_profile(tenant_id, user_id, profile_input(), Some("en"))
        .await
        .unwrap();

    let summary = service
        .get_profile_summary(tenant_id, user_id, Some("ru"), Some("en"))
        .await
        .unwrap();

    assert_eq!(summary.user_id, user_id);
    assert_eq!(summary.handle, "creator-one");
    assert_eq!(summary.display_name, "Creator One");
    assert_eq!(summary.visibility, ProfileVisibility::Public);
}
