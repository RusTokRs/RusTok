use async_graphql::dataloader::DataLoader;
use chrono::Utc;
use rustok_profiles::dto::{ProfileVisibility, UpsertProfileInput};
use rustok_profiles::entities;
use rustok_profiles::error::ProfileError;
use rustok_profiles::services::ProfileService;
use rustok_profiles::ProfilesReader;
use rustok_profiles::{ProfileSummaryLoader, ProfileSummaryLoaderKey};
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use uuid::Uuid;

mod support;

async fn setup() -> ProfileService {
    setup_with_db().await.1
}

async fn setup_with_db() -> (DatabaseConnection, ProfileService) {
    let db = support::setup_profiles_test_db().await;
    let service = ProfileService::new(db.clone());
    (db, service)
}

fn profile_input() -> UpsertProfileInput {
    UpsertProfileInput {
        handle: "Creator-One".to_string(),
        display_name: "Creator One".to_string(),
        bio: Some("Primary profile bio".to_string()),
        tags: vec!["rust".to_string(), "creator".to_string()],
        avatar_media_id: Some(Uuid::new_v4()),
        banner_media_id: Some(Uuid::new_v4()),
        preferred_locale: Some("ru".to_string()),
        visibility: ProfileVisibility::Public,
    }
}

async fn insert_translation(
    db: &DatabaseConnection,
    user_id: Uuid,
    locale: &str,
    display_name: &str,
    bio: Option<&str>,
) {
    let now = Utc::now();
    entities::profile_translation::ActiveModel {
        id: Set(Uuid::new_v4()),
        profile_user_id: Set(user_id),
        locale: Set(locale.to_string()),
        display_name: Set(display_name.to_string()),
        bio: Set(bio.map(str::to_string)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await
    .unwrap();
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
    assert_eq!(
        created.tags,
        vec!["rust".to_string(), "creator".to_string()]
    );
    assert_eq!(created.preferred_locale.as_deref(), Some("ru"));

    let fetched = service
        .get_profile(tenant_id, user_id, Some("de"), Some("en"))
        .await
        .unwrap();
    assert_eq!(fetched.handle, "creator-one");
    assert_eq!(fetched.display_name, "Creator One");
    assert_eq!(fetched.bio.as_deref(), Some("Primary profile bio"));
    assert_eq!(
        fetched.tags,
        vec!["rust".to_string(), "creator".to_string()]
    );
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
                tags: vec![],
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
    assert_eq!(
        summary.tags,
        vec!["rust".to_string(), "creator".to_string()]
    );
    assert_eq!(summary.visibility, ProfileVisibility::Public);
}

#[tokio::test]
async fn batched_reader_uses_locale_fallback_and_skips_missing_profiles() {
    let (db, service) = setup_with_db().await;
    let tenant_id = Uuid::new_v4();
    let first_user_id = Uuid::new_v4();
    let second_user_id = Uuid::new_v4();
    let missing_user_id = Uuid::new_v4();

    service
        .upsert_profile(
            tenant_id,
            first_user_id,
            UpsertProfileInput {
                handle: "creator-one".to_string(),
                display_name: "Creator One".to_string(),
                bio: Some("Primary profile bio".to_string()),
                tags: vec!["rust".to_string()],
                avatar_media_id: None,
                banner_media_id: None,
                preferred_locale: Some("en".to_string()),
                visibility: ProfileVisibility::Public,
            },
            Some("en"),
        )
        .await
        .unwrap();
    service
        .upsert_profile(
            tenant_id,
            second_user_id,
            UpsertProfileInput {
                handle: "creator-two".to_string(),
                display_name: "Creator Two".to_string(),
                bio: None,
                tags: vec!["design".to_string()],
                avatar_media_id: None,
                banner_media_id: None,
                preferred_locale: Some("en".to_string()),
                visibility: ProfileVisibility::Authenticated,
            },
            Some("en"),
        )
        .await
        .unwrap();

    insert_translation(
        &db,
        first_user_id,
        "ru",
        "Создатель Один",
        Some("Русская биография"),
    )
    .await;

    let profiles = service
        .find_profile_summaries(
            tenant_id,
            &[first_user_id, second_user_id, missing_user_id],
            Some("ru"),
            Some("en"),
        )
        .await
        .unwrap();

    assert_eq!(profiles.len(), 2);
    assert_eq!(
        profiles.get(&first_user_id).unwrap().display_name,
        "Создатель Один"
    );
    assert_eq!(
        profiles.get(&second_user_id).unwrap().display_name,
        "Creator Two"
    );
    assert_eq!(
        profiles.get(&first_user_id).unwrap().tags,
        vec!["rust".to_string()]
    );
    assert_eq!(
        profiles.get(&second_user_id).unwrap().tags,
        vec!["design".to_string()]
    );
    assert!(!profiles.contains_key(&missing_user_id));
}

#[tokio::test]
async fn dataloader_batches_profile_summary_requests() {
    let (db, service) = setup_with_db().await;
    let tenant_id = Uuid::new_v4();
    let first_user_id = Uuid::new_v4();
    let second_user_id = Uuid::new_v4();
    let missing_user_id = Uuid::new_v4();

    service
        .upsert_profile(
            tenant_id,
            first_user_id,
            UpsertProfileInput {
                handle: "loader-one".to_string(),
                display_name: "Loader One".to_string(),
                bio: None,
                tags: vec!["rust".to_string()],
                avatar_media_id: None,
                banner_media_id: None,
                preferred_locale: Some("en".to_string()),
                visibility: ProfileVisibility::Public,
            },
            Some("en"),
        )
        .await
        .unwrap();
    service
        .upsert_profile(
            tenant_id,
            second_user_id,
            UpsertProfileInput {
                handle: "loader-two".to_string(),
                display_name: "Loader Two".to_string(),
                bio: None,
                tags: vec!["design".to_string()],
                avatar_media_id: None,
                banner_media_id: None,
                preferred_locale: Some("en".to_string()),
                visibility: ProfileVisibility::Authenticated,
            },
            Some("en"),
        )
        .await
        .unwrap();

    let loader = DataLoader::new(ProfileSummaryLoader::new(db), tokio::spawn);
    let loaded = loader
        .load_many(vec![
            ProfileSummaryLoaderKey {
                tenant_id,
                user_id: first_user_id,
                requested_locale: Some("en".to_string()),
                tenant_default_locale: Some("en".to_string()),
            },
            ProfileSummaryLoaderKey {
                tenant_id,
                user_id: second_user_id,
                requested_locale: Some("en".to_string()),
                tenant_default_locale: Some("en".to_string()),
            },
            ProfileSummaryLoaderKey {
                tenant_id,
                user_id: missing_user_id,
                requested_locale: Some("en".to_string()),
                tenant_default_locale: Some("en".to_string()),
            },
        ])
        .await
        .unwrap();

    assert_eq!(loaded.len(), 2);
    assert_eq!(
        loaded
            .get(&ProfileSummaryLoaderKey {
                tenant_id,
                user_id: first_user_id,
                requested_locale: Some("en".to_string()),
                tenant_default_locale: Some("en".to_string()),
            })
            .unwrap()
            .display_name,
        "Loader One"
    );
    assert_eq!(
        loaded
            .get(&ProfileSummaryLoaderKey {
                tenant_id,
                user_id: second_user_id,
                requested_locale: Some("en".to_string()),
                tenant_default_locale: Some("en".to_string()),
            })
            .unwrap()
            .display_name,
        "Loader Two"
    );
    assert_eq!(
        loaded
            .get(&ProfileSummaryLoaderKey {
                tenant_id,
                user_id: first_user_id,
                requested_locale: Some("en".to_string()),
                tenant_default_locale: Some("en".to_string()),
            })
            .unwrap()
            .tags,
        vec!["rust".to_string()]
    );
    assert_eq!(
        loaded
            .get(&ProfileSummaryLoaderKey {
                tenant_id,
                user_id: second_user_id,
                requested_locale: Some("en".to_string()),
                tenant_default_locale: Some("en".to_string()),
            })
            .unwrap()
            .tags,
        vec!["design".to_string()]
    );
    assert!(!loaded.contains_key(&ProfileSummaryLoaderKey {
        tenant_id,
        user_id: missing_user_id,
        requested_locale: Some("en".to_string()),
        tenant_default_locale: Some("en".to_string()),
    }));
}

#[tokio::test]
async fn targeted_updates_modify_existing_profile() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let avatar_media_id = Uuid::new_v4();
    let banner_media_id = Uuid::new_v4();

    service
        .upsert_profile(tenant_id, user_id, profile_input(), Some("en"))
        .await
        .unwrap();

    let updated = service
        .update_profile_handle(tenant_id, user_id, "updated-handle", Some("en"))
        .await
        .unwrap();
    assert_eq!(updated.handle, "updated-handle");

    let updated = service
        .update_profile_content(
            tenant_id,
            user_id,
            "Updated Name",
            Some("Updated bio"),
            Some("en"),
        )
        .await
        .unwrap();
    assert_eq!(updated.display_name, "Updated Name");
    assert_eq!(updated.bio.as_deref(), Some("Updated bio"));

    let updated = service
        .update_profile_locale(tenant_id, user_id, Some("fr"), Some("en"))
        .await
        .unwrap();
    assert_eq!(updated.preferred_locale.as_deref(), Some("fr"));

    let updated = service
        .update_profile_visibility(tenant_id, user_id, ProfileVisibility::Private, Some("en"))
        .await
        .unwrap();
    assert_eq!(updated.visibility, ProfileVisibility::Private);

    let updated = service
        .update_profile_media(
            tenant_id,
            user_id,
            Some(avatar_media_id),
            Some(banner_media_id),
            Some("en"),
        )
        .await
        .unwrap();
    assert_eq!(updated.avatar_media_id, Some(avatar_media_id));
    assert_eq!(updated.banner_media_id, Some(banner_media_id));
}

#[tokio::test]
async fn targeted_updates_require_existing_profile() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    let error = service
        .update_profile_handle(tenant_id, user_id, "missing-user", Some("en"))
        .await
        .unwrap_err();

    assert!(matches!(error, ProfileError::ProfileNotFound(id) if id == user_id));
}

#[tokio::test]
async fn backfill_profile_creates_missing_profile_with_generated_handle() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    let result = service
        .backfill_profile(
            tenant_id,
            user_id,
            "jane.doe@example.com",
            None,
            Some("de"),
            ProfileVisibility::Authenticated,
            Some("en"),
        )
        .await
        .unwrap();

    assert!(result.created);
    assert_eq!(result.profile.user_id, user_id);
    assert_eq!(result.profile.handle, "jane-doe");
    assert_eq!(result.profile.display_name, "Jane Doe");
    assert_eq!(result.profile.preferred_locale.as_deref(), Some("de"));
    assert_eq!(result.profile.visibility, ProfileVisibility::Authenticated);
}

#[tokio::test]
async fn backfill_profile_uses_suffix_and_skips_existing_profile() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();
    let first_user_id = Uuid::new_v4();
    let second_user_id = Uuid::new_v4();

    let first = service
        .backfill_profile(
            tenant_id,
            first_user_id,
            "same@example.com",
            Some("Same Name"),
            Some("en"),
            ProfileVisibility::Public,
            Some("en"),
        )
        .await
        .unwrap();
    let second = service
        .backfill_profile(
            tenant_id,
            second_user_id,
            "same@example.com",
            Some("Same Name"),
            Some("en"),
            ProfileVisibility::Public,
            Some("en"),
        )
        .await
        .unwrap();
    let repeat = service
        .backfill_profile(
            tenant_id,
            second_user_id,
            "changed@example.com",
            Some("Changed Name"),
            Some("fr"),
            ProfileVisibility::Private,
            Some("en"),
        )
        .await
        .unwrap();

    assert!(first.created);
    assert!(second.created);
    assert_eq!(first.profile.handle, "same-name");
    assert_eq!(second.profile.handle, "same-name-2");

    assert!(!repeat.created);
    assert_eq!(repeat.profile.handle, "same-name-2");
    assert_eq!(repeat.profile.display_name, "Same Name");
    assert_eq!(repeat.profile.preferred_locale.as_deref(), Some("en"));
    assert_eq!(repeat.profile.visibility, ProfileVisibility::Public);
}
