use rustok_core::{SecurityContext, UserRole};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use rustok_profiles::dto::{ProfileVisibility, UpsertProfileInput};
use rustok_profiles::entities;
use rustok_profiles::{
    ProfileMutationContext, ProfileMutationService, ProfileService, ProfilesReader,
};
use rustok_taxonomy::{TaxonomyService, UpdateTaxonomyTermInput};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::sync::Arc;
use uuid::Uuid;

mod support;

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

fn profile_input(
    handle: &str,
    display_name: &str,
    preferred_locale: &str,
    tag: &str,
) -> UpsertProfileInput {
    UpsertProfileInput {
        handle: handle.to_string(),
        display_name: display_name.to_string(),
        bio: None,
        tags: vec![tag.to_string()],
        avatar_media_id: None,
        banner_media_id: None,
        preferred_locale: Some(preferred_locale.to_string()),
        visibility: ProfileVisibility::Public,
    }
}

async fn attached_term_id(db: &DatabaseConnection, tenant_id: Uuid, user_id: Uuid) -> Uuid {
    entities::profile_tag::Entity::find()
        .filter(entities::profile_tag::Column::TenantId.eq(tenant_id))
        .filter(entities::profile_tag::Column::ProfileUserId.eq(user_id))
        .one(db)
        .await
        .expect("profile tag relation lookup should succeed")
        .expect("profile should have one attached tag")
        .term_id
}

async fn add_term_translation(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    term_id: Uuid,
    locale: &str,
    name: &str,
    slug: &str,
) {
    TaxonomyService::new(db.clone())
        .update_term(
            tenant_id,
            term_id,
            SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4())),
            UpdateTaxonomyTermInput {
                locale: locale.to_string(),
                name: Some(name.to_string()),
                slug: Some(slug.to_string()),
                description: None,
                aliases: None,
            },
        )
        .await
        .expect("taxonomy translation fixture should apply through the owner service");
}

#[tokio::test]
async fn profile_tags_follow_requested_preferred_then_tenant_default_locale() {
    let db = support::setup_profiles_test_db().await;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let mutations = ProfileMutationService::new(&db, &event_bus);
    let service = ProfileService::new(db.clone());
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    mutations
        .upsert_profile_with_event(
            mutation_context(tenant_id, user_id, Some("en")),
            profile_input("locale-owner", "Locale Owner", "ru", "Preferred RU"),
        )
        .await
        .expect("profile should be created");

    let term_id = attached_term_id(&db, tenant_id, user_id).await;
    add_term_translation(
        &db,
        tenant_id,
        term_id,
        "en",
        "Tenant Default EN",
        "tenant-default-en",
    )
    .await;
    add_term_translation(
        &db,
        tenant_id,
        term_id,
        "fr",
        "Requested FR",
        "requested-fr",
    )
    .await;

    let preferred = service
        .get_profile(tenant_id, user_id, None, Some("en"))
        .await
        .expect("preferred-locale profile read should succeed");
    assert_eq!(preferred.tags, vec!["Preferred RU".to_string()]);

    let requested_missing = service
        .get_profile(tenant_id, user_id, Some("de"), Some("en"))
        .await
        .expect("missing requested locale should fall back through profile preference");
    assert_eq!(requested_missing.tags, vec!["Preferred RU".to_string()]);

    let requested_present = service
        .get_profile(tenant_id, user_id, Some("fr"), Some("en"))
        .await
        .expect("requested tag locale should win");
    assert_eq!(requested_present.tags, vec!["Requested FR".to_string()]);
}

#[tokio::test]
async fn batched_profile_tags_resolve_each_profiles_own_preferred_locale() {
    let db = support::setup_profiles_test_db().await;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let mutations = ProfileMutationService::new(&db, &event_bus);
    let service = ProfileService::new(db.clone());
    let tenant_id = Uuid::new_v4();
    let ru_user_id = Uuid::new_v4();
    let fr_user_id = Uuid::new_v4();

    mutations
        .upsert_profile_with_event(
            mutation_context(tenant_id, ru_user_id, Some("en")),
            profile_input("ru-owner", "RU Owner", "ru", "RU Preferred"),
        )
        .await
        .expect("RU profile should be created");
    mutations
        .upsert_profile_with_event(
            mutation_context(tenant_id, fr_user_id, Some("en")),
            profile_input("fr-owner", "FR Owner", "fr", "FR Preferred"),
        )
        .await
        .expect("FR profile should be created");

    let ru_term_id = attached_term_id(&db, tenant_id, ru_user_id).await;
    let fr_term_id = attached_term_id(&db, tenant_id, fr_user_id).await;
    add_term_translation(
        &db,
        tenant_id,
        ru_term_id,
        "en",
        "RU Tenant Default",
        "ru-tenant-default",
    )
    .await;
    add_term_translation(
        &db,
        tenant_id,
        fr_term_id,
        "en",
        "FR Tenant Default",
        "fr-tenant-default",
    )
    .await;

    let summaries = ProfilesReader::find_profile_summaries(
        &service,
        tenant_id,
        &[ru_user_id, fr_user_id],
        Some("de"),
        Some("en"),
    )
    .await
    .expect("batched profile summary read should succeed");

    assert_eq!(
        summaries.get(&ru_user_id).expect("RU summary").tags,
        vec!["RU Preferred".to_string()]
    );
    assert_eq!(
        summaries.get(&fr_user_id).expect("FR summary").tags,
        vec!["FR Preferred".to_string()]
    );
}
