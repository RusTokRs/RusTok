use sea_orm::{ColumnTrait, ConnectOptions, Database, EntityTrait, QueryFilter};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

use crate::MarketplaceSellerService;
use crate::dto::{
    CreateMarketplaceSellerInput, MarketplaceSellerEventKind, MarketplaceSellerEventProvenance,
    MarketplaceSellerOnboardingStatus, SubmitMarketplaceSellerOnboardingInput,
    UpdateMarketplaceSellerProfileInput,
};
use crate::entities::seller_command_receipt;

#[tokio::test]
async fn create_profile_and_submit_commit_one_event_per_receipt() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let owner_user_id = Uuid::new_v4();
    let service = MarketplaceSellerService::new(db.clone());

    let create = CreateMarketplaceSellerInput {
        handle: "seller-event-coverage".to_string(),
        display_name: "Seller event coverage".to_string(),
        legal_name: Some("Seller Event Coverage LLC".to_string()),
        owner_user_id,
        metadata: serde_json::json!({"tier": "test"}),
    };
    let created = service
        .create_seller_with_receipt(
            tenant_id,
            actor_id,
            "create-event-coverage",
            "en",
            create.clone(),
        )
        .await
        .unwrap();
    let replayed = service
        .create_seller_with_receipt(tenant_id, actor_id, "create-event-coverage", "en", create)
        .await
        .unwrap();
    assert_eq!(replayed.id, created.id);

    let update = UpdateMarketplaceSellerProfileInput {
        display_name: Some("Updated event coverage".to_string()),
        legal_name: Some("Updated Seller Event Coverage LLC".to_string()),
        metadata: Some(serde_json::json!({"tier": "updated"})),
    };
    service
        .update_profile_with_receipt(
            tenant_id,
            actor_id,
            "profile-event-coverage",
            "en",
            created.id,
            update.clone(),
        )
        .await
        .unwrap();
    service
        .update_profile_with_receipt(
            tenant_id,
            actor_id,
            "profile-event-coverage",
            "en",
            created.id,
            update,
        )
        .await
        .unwrap();

    let submit = SubmitMarketplaceSellerOnboardingInput {
        note: Some("ready for verification".to_string()),
    };
    let submitted = service
        .submit_onboarding_with_receipt(
            tenant_id,
            actor_id,
            "submit-event-coverage",
            "en",
            created.id,
            submit.clone(),
        )
        .await
        .unwrap();
    assert_eq!(
        submitted.onboarding_status,
        MarketplaceSellerOnboardingStatus::Submitted
    );
    service
        .submit_onboarding_with_receipt(
            tenant_id,
            actor_id,
            "submit-event-coverage",
            "en",
            created.id,
            submit,
        )
        .await
        .unwrap();

    let events = service
        .list_events(tenant_id, created.id, 20)
        .await
        .unwrap();
    assert_eq!(
        events.len(),
        3,
        "completed replay must not duplicate events"
    );
    assert!(events.iter().all(|event| event.actor_id == Some(actor_id)
        && event.locale.as_deref() == Some("en")
        && event.provenance == MarketplaceSellerEventProvenance::Command));
    let kinds = events
        .iter()
        .map(|event| event.event_kind)
        .collect::<Vec<_>>();
    assert!(kinds.contains(&MarketplaceSellerEventKind::Created));
    assert!(kinds.contains(&MarketplaceSellerEventKind::ProfileUpdated));
    assert!(kinds.contains(&MarketplaceSellerEventKind::OnboardingSubmitted));
    let submitted_event = events
        .iter()
        .find(|event| event.event_kind == MarketplaceSellerEventKind::OnboardingSubmitted)
        .unwrap();
    assert_eq!(
        submitted_event.note.as_deref(),
        Some("ready for verification")
    );

    let receipts = seller_command_receipt::Entity::find()
        .filter(seller_command_receipt::Column::TenantId.eq(tenant_id))
        .all(&db)
        .await
        .unwrap();
    assert_eq!(receipts.len(), 3);
    assert!(receipts.iter().all(|receipt| receipt.status == "completed"));
}

async fn setup_database() -> sea_orm::DatabaseConnection {
    let url = format!(
        "sqlite:file:marketplace_seller_response_events_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await.unwrap();
    let manager = SchemaManager::new(&db);
    for migration in crate::migrations::migrations() {
        migration.up(&manager).await.unwrap();
    }
    db
}
