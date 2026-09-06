use chrono::Utc;
use rustok_profiles::{
    ProfileStatus, ProfileVisibility, entities, redact_profile_for_account_deactivation_in_tx,
};
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set, TransactionTrait};
use uuid::Uuid;

mod support;

async fn insert_public_profile(db: &DatabaseConnection, tenant_id: Uuid, user_id: Uuid) {
    let now = Utc::now();
    entities::profile::ActiveModel {
        user_id: Set(user_id),
        tenant_id: Set(tenant_id),
        handle: Set(format!("author-{}", &user_id.simple().to_string()[..8])),
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
    .expect("profile fixture should insert");
}

#[tokio::test]
async fn account_redaction_hides_existing_tenant_profile() {
    let db = support::setup_profiles_test_db().await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    insert_public_profile(&db, tenant_id, user_id).await;

    let transaction = db.begin().await.expect("transaction should begin");
    let changed = redact_profile_for_account_deactivation_in_tx(&transaction, tenant_id, user_id)
        .await
        .expect("account redaction should succeed");
    assert!(changed);
    transaction
        .commit()
        .await
        .expect("transaction should commit");

    let profile = entities::profile::Entity::find_by_id(user_id)
        .one(&db)
        .await
        .expect("profile lookup should succeed")
        .expect("profile should remain for referential continuity");
    assert_eq!(profile.tenant_id, tenant_id);
    assert_eq!(profile.status, ProfileStatus::Hidden);
}

#[tokio::test]
async fn account_redaction_accepts_missing_profile_as_redacted_state() {
    let db = support::setup_profiles_test_db().await;
    let transaction = db.begin().await.expect("transaction should begin");

    let changed =
        redact_profile_for_account_deactivation_in_tx(&transaction, Uuid::new_v4(), Uuid::new_v4())
            .await
            .expect("missing profile should be a valid redacted state");

    assert!(!changed);
    transaction
        .commit()
        .await
        .expect("transaction should commit");
}

#[tokio::test]
async fn account_redaction_does_not_cross_tenant_scope() {
    let db = support::setup_profiles_test_db().await;
    let profile_tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    insert_public_profile(&db, profile_tenant_id, user_id).await;

    let transaction = db.begin().await.expect("transaction should begin");
    let changed =
        redact_profile_for_account_deactivation_in_tx(&transaction, other_tenant_id, user_id)
            .await
            .expect("cross-tenant lookup should remain a valid absent state");
    assert!(!changed);
    transaction
        .commit()
        .await
        .expect("transaction should commit");

    let profile = entities::profile::Entity::find_by_id(user_id)
        .one(&db)
        .await
        .expect("profile lookup should succeed")
        .expect("profile should remain present");
    assert_eq!(profile.tenant_id, profile_tenant_id);
    assert_eq!(profile.status, ProfileStatus::Active);
}
