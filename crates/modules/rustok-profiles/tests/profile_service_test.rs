use async_graphql::dataloader::DataLoader;
use chrono::Utc;
use rustok_outbox::TransactionalEventBus;
use rustok_profiles::dto::{ProfileVisibility, UpsertProfileInput};
use rustok_profiles::entities;
use rustok_profiles::error::ProfileError;
use rustok_profiles::services::ProfileService;
use rustok_profiles::{
    ProfileAccessAudience, ProfileBackfillRequest, ProfileMutationContext, ProfileMutationService,
    ProfileSummaryLoader, ProfileSummaryLoaderKey, ProfilesReader,
};
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use uuid::Uuid;

mod support;

async fn setup_context() -> (DatabaseConnection, ProfileService, TransactionalEventBus) {
    let db = support::setup_profiles_test_db().await;
    let service = ProfileService::new(db.clone());
    let event_bus = TransactionalEventBus::new(std::sync::Arc::new(
        rustok_outbox::OutboxTransport::new(db.clone()),
    ));
    (db, service, event_bus)
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

fn mutation_context<'a>(
    tenant_id: Uuid,
    user_id: Uuid,
    tenant_default_locale: Option<&'a str>,
) -> ProfileMutationContext<'a> {
    ProfileMutationContext {
        tenant_id,
        actor_id: user_id,
        user_id,
        tenant_default_locale,
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
    let (db, service, event_bus) = setup_context().await;
    let mutations = ProfileMutationService::new(&db, &event_bus);
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    let created = mutations
        .upsert_profile_with_event(
            mutation_context(tenant_id, user_id, Some("en")),
            profile_input(),
        )
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
    let (db, service, event_bus) = setup_context().await;
    let mutations = ProfileMutationService::new(&db, &event_bus);
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    mutations
        .upsert_profile_with_event(
            mutation_context(tenant_id, user_id, Some("en")),
            profile_input(),
        )
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
    let (db, _service, event_bus) = setup_context().await;
    let mutations = ProfileMutationService::new(&db, &event_bus);
    let tenant_id = Uuid::new_v4();
    let first_user_id = Uuid::new_v4();
    let second_user_id = Uuid::new_v4();

    mutations
        .upsert_profile_with_event(
            mutation_context(tenant_id, first_user_id, Some("en")),
            profile_input(),
        )
        .await
        .unwrap();

    let error = mutations
        .upsert_profile_with_event(
            mutation_context(tenant_id, second_user_id, Some("en")),
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
    let (db, service, event_bus) = setup_context().await;
    let mutations = ProfileMutationService::new(&db, &event_bus);
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    mutations
        .upsert_profile_with_event(
            mutation_context(tenant_id, user_id, Some("en")),
            profile_input(),
        )
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
    let (db, service, event_bus) = setup_context().await;
    let mutations = ProfileMutationService::new(&db, &event_bus);
    let tenant_id = Uuid::new_v4();
    let first_user_id = Uuid::new_v4();
    let second_user_id = Uuid::new_v4();
    let missing_user_id = Uuid::new_v4();

    mutations
        .upsert_profile_with_event(
            mutation_context(tenant_id, first_user_id, Some("en")),
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
        )
        .await
        .unwrap();
    mutations
        .upsert_profile_with_event(
            mutation_context(tenant_id, second_user_id, Some("en")),
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
    let (db, _service, event_bus) = setup_context().await;
    let mutations = ProfileMutationService::new(&db, &event_bus);
    let tenant_id = Uuid::new_v4();
    let first_user_id = Uuid::new_v4();
    let second_user_id = Uuid::new_v4();
    let missing_user_id = Uuid::new_v4();

    mutations
        .upsert_profile_with_event(
            mutation_context(tenant_id, first_user_id, Some("en")),
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
        )
        .await
        .unwrap();
    mutations
        .upsert_profile_with_event(
            mutation_context(tenant_id, second_user_id, Some("en")),
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
        )
        .await
        .unwrap();

    let loader = DataLoader::new(
        ProfileSummaryLoader::for_audience(
            db,
            ProfileAccessAudience::Authenticated {
                actor_id: first_user_id,
            },
        ),
        tokio::spawn,
    );
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
    let (db, _service, event_bus) = setup_context().await;
    let mutations = ProfileMutationService::new(&db, &event_bus);
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let avatar_media_id = Uuid::new_v4();
    let banner_media_id = Uuid::new_v4();

    let mut initial_input = profile_input();
    initial_input.preferred_locale = Some("en".to_string());
    mutations
        .upsert_profile_with_event(
            mutation_context(tenant_id, user_id, Some("en")),
            initial_input,
        )
        .await
        .unwrap();

    let updated = mutations
        .update_profile_handle_with_event(
            mutation_context(tenant_id, user_id, Some("en")),
            "updated-handle",
        )
        .await
        .unwrap();
    assert_eq!(updated.handle, "updated-handle");

    let updated = mutations
        .update_profile_content_with_event(
            mutation_context(tenant_id, user_id, Some("en")),
            "Updated Name",
            Some("Updated bio"),
        )
        .await
        .unwrap();
    assert_eq!(updated.display_name, "Updated Name");
    assert_eq!(updated.bio.as_deref(), Some("Updated bio"));

    let updated = mutations
        .update_profile_locale_with_event(
            mutation_context(tenant_id, user_id, Some("en")),
            Some("fr"),
        )
        .await
        .unwrap();
    assert_eq!(updated.preferred_locale.as_deref(), Some("fr"));

    let updated = mutations
        .update_profile_visibility_with_event(
            mutation_context(tenant_id, user_id, Some("en")),
            ProfileVisibility::Private,
        )
        .await
        .unwrap();
    assert_eq!(updated.visibility, ProfileVisibility::Private);

    let updated = mutations
        .update_profile_media_with_event(
            mutation_context(tenant_id, user_id, Some("en")),
            Some(avatar_media_id),
            Some(banner_media_id),
        )
        .await
        .unwrap();
    assert_eq!(updated.avatar_media_id, Some(avatar_media_id));
    assert_eq!(updated.banner_media_id, Some(banner_media_id));
}

#[tokio::test]
async fn targeted_updates_require_existing_profile() {
    let (db, _service, event_bus) = setup_context().await;
    let mutations = ProfileMutationService::new(&db, &event_bus);
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    let error = mutations
        .update_profile_handle_with_event(
            mutation_context(tenant_id, user_id, Some("en")),
            "missing-user",
        )
        .await
        .unwrap_err();

    assert!(matches!(error, ProfileError::ProfileNotFound(id) if id == user_id));
}

#[tokio::test]
async fn backfill_profile_creates_missing_profile_with_generated_handle() {
    let (db, _service, event_bus) = setup_context().await;
    let mutations = ProfileMutationService::new(&db, &event_bus);
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    let result = mutations
        .backfill_profile_with_event(ProfileBackfillRequest {
            tenant_id,
            user_id,
            email: "jane.doe@example.com",
            display_name: None,
            preferred_locale: Some("de"),
            visibility: ProfileVisibility::Authenticated,
            tenant_default_locale: Some("en"),
        })
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
    let (db, _service, event_bus) = setup_context().await;
    let mutations = ProfileMutationService::new(&db, &event_bus);
    let tenant_id = Uuid::new_v4();
    let first_user_id = Uuid::new_v4();
    let second_user_id = Uuid::new_v4();

    let first = mutations
        .backfill_profile_with_event(ProfileBackfillRequest {
            tenant_id,
            user_id: first_user_id,
            email: "same@example.com",
            display_name: Some("Same Name"),
            preferred_locale: Some("en"),
            visibility: ProfileVisibility::Public,
            tenant_default_locale: Some("en"),
        })
        .await
        .unwrap();
    let second = mutations
        .backfill_profile_with_event(ProfileBackfillRequest {
            tenant_id,
            user_id: second_user_id,
            email: "same@example.com",
            display_name: Some("Same Name"),
            preferred_locale: Some("en"),
            visibility: ProfileVisibility::Public,
            tenant_default_locale: Some("en"),
        })
        .await
        .unwrap();
    let repeat = mutations
        .backfill_profile_with_event(ProfileBackfillRequest {
            tenant_id,
            user_id: second_user_id,
            email: "changed@example.com",
            display_name: Some("Changed Name"),
            preferred_locale: Some("fr"),
            visibility: ProfileVisibility::Private,
            tenant_default_locale: Some("en"),
        })
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
