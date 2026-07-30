use chrono::Utc;
use rustok_profiles::{
    ProfileStatus, ProfileVisibility, entities, redact_profile_for_account_deactivation_in_tx,
};
use sea_orm::{ActiveModelTrait, EntityTrait, Set, TransactionTrait};
use uuid::Uuid;

mod support;

#[tokio::test]
async fn account_redaction_hides_existing_tenant_profile() {
    let db = support::setup_profiles_test_db().await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let now = Utc::now();

    entities::profile::ActiveModel {
        user_id: Set(user_id),
        tenant_id: Set(tenant_id),
        handle: Set("redacted-author".to_string()),
        avatar_media_id: Set(None),
        banner_media_id: Set(None),
        preferred_locale: Set(Some("en".to_string())),
        visibility: Set(ProfileVisibility::Public),
        status: Set(ProfileStatus::Active),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await
    .expect("profile fixture should insert");

    let transaction = db.begin().await.expect("transaction should begin");
    let changed = redact_profile_for_account_deactivation_in_tx(
        &transaction,
        tenant_id,
        user_id,
    )
    .await
    .expect("account redaction should succeed");
    assert!(changed);
    transaction.commit().await.expect("transaction should commit");

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

    let changed = redact_profile_for_account_deactivation_in_tx(
        &transaction,
        Uuid::new_v4(),
        Uuid::new_v4(),
    )
    .await
    .expect("missing profile should be a valid redacted state");

    assert!(!changed);
    transaction.commit().await.expect("transaction should commit");
}
