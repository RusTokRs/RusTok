use chrono::Utc;
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_profiles::{ProfileStatus, ProfileVisibility, ProfilesModule, entities};
use rustok_taxonomy::{
    CreateTaxonomyTermInput, TaxonomyModule, TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
};
use rustok_test_utils::db::setup_test_db;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection};
use sea_orm_migration::prelude::SchemaManager;
use uuid::Uuid;

async fn setup() -> (DatabaseConnection, TaxonomyService) {
    let db = setup_test_db().await;
    let schema_manager = SchemaManager::new(&db);
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&schema_manager)
            .await
            .expect("taxonomy migration should succeed");
    }
    for migration in ProfilesModule.migrations() {
        migration
            .up(&schema_manager)
            .await
            .expect("profiles migration should succeed");
    }
    let taxonomy = TaxonomyService::new(db.clone());
    (db, taxonomy)
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

async fn create_profile(db: &DatabaseConnection, tenant_id: Uuid, user_id: Uuid) {
    let now = Utc::now();
    entities::profile::ActiveModel {
        user_id: Set(user_id),
        tenant_id: Set(tenant_id),
        handle: Set(format!("user-{}", &user_id.to_string()[..8])),
        avatar_media_id: Set(None),
        banner_media_id: Set(None),
        preferred_locale: Set(Some("en".to_string())),
        visibility: Set(ProfileVisibility::Public),
        status: Set(ProfileStatus::Active),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await
    .expect("profile should be inserted");
}

async fn create_tag(taxonomy: &TaxonomyService, tenant_id: Uuid, name: &str) -> Uuid {
    let route_key = name.to_ascii_lowercase();
    taxonomy
        .create_term(
            tenant_id,
            admin(),
            CreateTaxonomyTermInput {
                kind: TaxonomyTermKind::Tag,
                scope_type: TaxonomyScopeType::Module,
                scope_value: Some("profiles".to_string()),
                locale: "en".to_string(),
                name: name.to_string(),
                slug: Some(route_key.clone()),
                canonical_key: Some(route_key),
                description: None,
                aliases: Vec::new(),
            },
        )
        .await
        .expect("profile tag should be created")
}

#[tokio::test]
async fn storage_rejects_cross_tenant_profile_tag_attachment() {
    let (db, taxonomy) = setup().await;
    let profile_tenant = Uuid::new_v4();
    let foreign_tenant = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    create_profile(&db, profile_tenant, user_id).await;

    let local_term = create_tag(&taxonomy, profile_tenant, "Local").await;
    let foreign_term = create_tag(&taxonomy, foreign_tenant, "Foreign").await;
    let now = Utc::now();

    entities::profile_tag::ActiveModel {
        profile_user_id: Set(user_id),
        term_id: Set(local_term),
        tenant_id: Set(profile_tenant),
        created_at: Set(now.into()),
    }
    .insert(&db)
    .await
    .expect("same-tenant tag attachment should succeed");

    let error = entities::profile_tag::ActiveModel {
        profile_user_id: Set(user_id),
        term_id: Set(foreign_term),
        tenant_id: Set(profile_tenant),
        created_at: Set((now + chrono::Duration::microseconds(1)).into()),
    }
    .insert(&db)
    .await
    .expect_err("storage must reject a cross-tenant taxonomy tag attachment");

    assert!(error.to_string().contains("profile tag tenant mismatch"));
}
