use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait,
    QueryFilter, Set,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

use crate::MarketplaceSellerService;
use crate::dto::{
    AddMarketplaceSellerMemberInput, MarketplaceSellerEventKind, MarketplaceSellerEventProvenance,
    MarketplaceSellerMemberRole, MarketplaceSellerMemberStatus, UpdateMarketplaceSellerMemberInput,
};
use crate::entities::{seller, seller_command_receipt};
use crate::error::MarketplaceSellerError;

#[tokio::test]
async fn member_commands_commit_one_event_per_receipt_and_bind_locale() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let seller_id = insert_seller(&db, tenant_id).await;
    let user_id = Uuid::new_v4();
    let service = MarketplaceSellerService::new(db.clone());

    let add = AddMarketplaceSellerMemberInput {
        user_id,
        role: MarketplaceSellerMemberRole::Operations,
        metadata: serde_json::json!({"source": "test"}),
    };
    let added = service
        .add_member_with_receipt(
            tenant_id,
            actor_id,
            "add-member-event",
            "en-US",
            seller_id,
            add.clone(),
        )
        .await
        .unwrap();
    let replayed = service
        .add_member_with_receipt(
            tenant_id,
            actor_id,
            "add-member-event",
            "en-US",
            seller_id,
            add,
        )
        .await
        .unwrap();
    assert_eq!(replayed.id, added.id);

    let update = UpdateMarketplaceSellerMemberInput {
        role: Some(MarketplaceSellerMemberRole::Finance),
        status: Some(MarketplaceSellerMemberStatus::Active),
        metadata: Some(serde_json::json!({"source": "activated"})),
    };
    let updated = service
        .update_member_with_receipt(
            tenant_id,
            actor_id,
            "update-member-event",
            "en-US",
            seller_id,
            added.id,
            update.clone(),
        )
        .await
        .unwrap();
    assert_eq!(updated.role, MarketplaceSellerMemberRole::Finance);
    assert_eq!(updated.status, MarketplaceSellerMemberStatus::Active);

    let replayed = service
        .update_member_with_receipt(
            tenant_id,
            actor_id,
            "update-member-event",
            "en-US",
            seller_id,
            added.id,
            update.clone(),
        )
        .await
        .unwrap();
    assert_eq!(replayed.id, updated.id);

    let conflict = service
        .update_member_with_receipt(
            tenant_id,
            actor_id,
            "update-member-event",
            "ru",
            seller_id,
            added.id,
            update,
        )
        .await;
    assert!(matches!(
        conflict,
        Err(MarketplaceSellerError::IdempotencyConflict(key)) if key == "update-member-event"
    ));

    let events = service.list_events(tenant_id, seller_id, 20).await.unwrap();
    let member_events = events
        .iter()
        .filter(|event| {
            matches!(
                event.event_kind,
                MarketplaceSellerEventKind::MemberAdded | MarketplaceSellerEventKind::MemberUpdated
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        member_events.len(),
        2,
        "replay must not duplicate member events"
    );
    assert!(member_events.iter().all(|event| {
        event.actor_id == Some(actor_id)
            && event.locale.as_deref() == Some("en-US")
            && event.provenance == MarketplaceSellerEventProvenance::Command
    }));

    let added_event = member_events
        .iter()
        .find(|event| event.event_kind == MarketplaceSellerEventKind::MemberAdded)
        .unwrap();
    assert_eq!(added_event.metadata["member_id"], added.id.to_string());
    assert_eq!(added_event.metadata["user_id"], user_id.to_string());
    assert_eq!(added_event.metadata["role"], "operations");
    assert_eq!(added_event.metadata["status"], "invited");

    let updated_event = member_events
        .iter()
        .find(|event| event.event_kind == MarketplaceSellerEventKind::MemberUpdated)
        .unwrap();
    assert_eq!(updated_event.metadata["member_id"], added.id.to_string());
    assert_eq!(updated_event.metadata["role"], "finance");
    assert_eq!(updated_event.metadata["status"], "active");

    let receipts = seller_command_receipt::Entity::find()
        .filter(seller_command_receipt::Column::TenantId.eq(tenant_id))
        .all(&db)
        .await
        .unwrap();
    assert_eq!(receipts.len(), 2);
    assert!(receipts.iter().all(|receipt| receipt.status == "completed"));
}

async fn setup_database() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:marketplace_seller_member_events_{}?mode=memory&cache=shared",
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

async fn insert_seller(db: &DatabaseConnection, tenant_id: Uuid) -> Uuid {
    let seller_id = Uuid::new_v4();
    let now = chrono::Utc::now().fixed_offset();
    seller::ActiveModel {
        id: Set(seller_id),
        tenant_id: Set(tenant_id),
        handle: Set(format!("member-events-{seller_id}")),
        legal_name: Set(None),
        status: Set("active".to_string()),
        onboarding_status: Set("approved".to_string()),
        metadata: Set(serde_json::json!({})),
        created_at: Set(now),
        updated_at: Set(now),
        activated_at: Set(Some(now)),
        suspended_at: Set(None),
    }
    .insert(db)
    .await
    .unwrap();
    seller_id
}
