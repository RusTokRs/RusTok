use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseBackend,
    DatabaseConnection, EntityTrait, QueryFilter, Set, Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

use crate::MarketplaceSellerService;
use crate::dto::{
    MarketplaceSellerEventKind, MarketplaceSellerEventProvenance,
    MarketplaceSellerOnboardingStatus, MarketplaceSellerStatus,
    ReviewMarketplaceSellerOnboardingInput, SuspendMarketplaceSellerInput,
};
use crate::entities::{seller, seller_command_receipt, seller_event, seller_translation};
use crate::error::MarketplaceSellerError;

#[tokio::test]
async fn seller_event_timeline_is_bounded_newest_first_and_tenant_scoped() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    let seller_id = insert_seller(&db, tenant_id).await;
    let now = chrono::Utc::now().fixed_offset();

    insert_event(
        &db,
        tenant_id,
        seller_id,
        Some(Uuid::new_v4()),
        Some("en"),
        "created",
        "command",
        now - chrono::Duration::seconds(1),
    )
    .await;
    insert_event(
        &db,
        tenant_id,
        seller_id,
        Some(Uuid::new_v4()),
        Some("en"),
        "suspended",
        "command",
        now,
    )
    .await;

    let service = MarketplaceSellerService::new(db.clone());
    let events = service.list_events(tenant_id, seller_id, 1).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_kind, MarketplaceSellerEventKind::Suspended);
    assert_eq!(
        events[0].provenance,
        MarketplaceSellerEventProvenance::Command
    );

    assert!(matches!(
        service.list_events(other_tenant_id, seller_id, 10).await,
        Err(MarketplaceSellerError::SellerNotFound(id)) if id == seller_id
    ));
}

#[tokio::test]
async fn seller_event_attribution_constraint_accepts_truthful_provenance_only() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let seller_id = insert_seller(&db, tenant_id).await;
    let now = chrono::Utc::now().fixed_offset();

    insert_event(
        &db,
        tenant_id,
        seller_id,
        None,
        None,
        "legacy_suspension_snapshot",
        "legacy_snapshot",
        now,
    )
    .await;

    let invalid = seller_event::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        seller_id: Set(seller_id),
        actor_id: Set(None),
        event_kind: Set("suspended".to_string()),
        locale: Set(Some("en".to_string())),
        provenance: Set("command".to_string()),
        note: Set(None),
        metadata: Set(serde_json::json!({})),
        created_at: Set(now),
    }
    .insert(&db)
    .await;
    assert!(
        invalid.is_err(),
        "command event without actor must fail DB CHECK"
    );
}

#[tokio::test]
async fn lifecycle_commands_commit_one_event_with_state_and_receipt() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let seller_id = insert_seller_with_state(&db, tenant_id, "draft", "submitted").await;
    let service = MarketplaceSellerService::new(db.clone());

    let review = ReviewMarketplaceSellerOnboardingInput {
        approved: true,
        note: Some("verification passed".to_string()),
    };
    let approved = service
        .review_onboarding_with_receipt(
            tenant_id,
            actor_id,
            "review-approved",
            "en",
            seller_id,
            review.clone(),
        )
        .await
        .unwrap();
    assert_eq!(approved.status, MarketplaceSellerStatus::Active);
    assert_eq!(
        approved.onboarding_status,
        MarketplaceSellerOnboardingStatus::Approved
    );

    let replay = service
        .review_onboarding_with_receipt(
            tenant_id,
            actor_id,
            "review-approved",
            "en",
            seller_id,
            review,
        )
        .await
        .unwrap();
    assert_eq!(replay.id, approved.id);

    service
        .suspend_seller_with_receipt(
            tenant_id,
            actor_id,
            "suspend-active",
            "en",
            seller_id,
            SuspendMarketplaceSellerInput {
                reason: "risk hold".to_string(),
            },
        )
        .await
        .unwrap();
    service
        .suspend_seller_with_receipt(
            tenant_id,
            actor_id,
            "suspend-active",
            "en",
            seller_id,
            SuspendMarketplaceSellerInput {
                reason: "risk hold".to_string(),
            },
        )
        .await
        .unwrap();

    service
        .reactivate_seller_with_receipt(tenant_id, actor_id, "reactivate-seller", "en", seller_id)
        .await
        .unwrap();

    let events = service.list_events(tenant_id, seller_id, 20).await.unwrap();
    assert_eq!(events.len(), 3, "replay must not append duplicate events");
    for event in &events {
        assert_eq!(event.actor_id, Some(actor_id));
        assert_eq!(event.locale.as_deref(), Some("en"));
        assert_eq!(event.provenance, MarketplaceSellerEventProvenance::Command);
    }
    let kinds = events
        .iter()
        .map(|event| event.event_kind)
        .collect::<Vec<_>>();
    assert!(kinds.contains(&MarketplaceSellerEventKind::OnboardingApproved));
    assert!(kinds.contains(&MarketplaceSellerEventKind::Suspended));
    assert!(kinds.contains(&MarketplaceSellerEventKind::Reactivated));
    let review_event = events
        .iter()
        .find(|event| event.event_kind == MarketplaceSellerEventKind::OnboardingApproved)
        .unwrap();
    assert_eq!(review_event.note.as_deref(), Some("verification passed"));

    let receipts = seller_command_receipt::Entity::find()
        .filter(seller_command_receipt::Column::TenantId.eq(tenant_id))
        .all(&db)
        .await
        .unwrap();
    assert_eq!(receipts.len(), 3);
    assert!(receipts.iter().all(|receipt| receipt.status == "completed"));
}

#[tokio::test]
async fn event_insert_failure_rolls_back_state_and_pending_receipt() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let seller_id = insert_seller_with_state(&db, tenant_id, "draft", "submitted").await;
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "DROP TABLE marketplace_seller_events".to_string(),
    ))
    .await
    .unwrap();

    let service = MarketplaceSellerService::new(db.clone());
    let result = service
        .review_onboarding_with_receipt(
            tenant_id,
            actor_id,
            "review-without-events-table",
            "en",
            seller_id,
            ReviewMarketplaceSellerOnboardingInput {
                approved: true,
                note: Some("must roll back".to_string()),
            },
        )
        .await;
    assert!(matches!(result, Err(MarketplaceSellerError::Database(_))));

    let persisted = seller::Entity::find_by_id(seller_id)
        .filter(seller::Column::TenantId.eq(tenant_id))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(persisted.status, "draft");
    assert_eq!(persisted.onboarding_status, "submitted");
    assert!(persisted.activated_at.is_none());

    let receipt = seller_command_receipt::Entity::find()
        .filter(seller_command_receipt::Column::TenantId.eq(tenant_id))
        .filter(seller_command_receipt::Column::IdempotencyKey.eq("review-without-events-table"))
        .one(&db)
        .await
        .unwrap();
    assert!(
        receipt.is_none(),
        "pending receipt must roll back with state"
    );
}

async fn setup_database() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:marketplace_seller_events_{}?mode=memory&cache=shared",
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
    insert_seller_with_state(db, tenant_id, "draft", "draft").await
}

async fn insert_seller_with_state(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    status: &str,
    onboarding_status: &str,
) -> Uuid {
    let seller_id = Uuid::new_v4();
    let now = chrono::Utc::now().fixed_offset();
    seller::ActiveModel {
        id: Set(seller_id),
        tenant_id: Set(tenant_id),
        handle: Set(format!("seller-{seller_id}")),
        legal_name: Set(None),
        status: Set(status.to_string()),
        onboarding_status: Set(onboarding_status.to_string()),
        onboarding_note: Set(None),
        suspension_reason: Set(None),
        metadata: Set(serde_json::json!({})),
        created_at: Set(now),
        updated_at: Set(now),
        activated_at: Set(None),
        suspended_at: Set(None),
    }
    .insert(db)
    .await
    .unwrap();
    seller_translation::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        seller_id: Set(seller_id),
        locale: Set("en".to_string()),
        display_name: Set("Test seller".to_string()),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(db)
    .await
    .unwrap();
    seller_id
}

#[allow(clippy::too_many_arguments)]
async fn insert_event(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    seller_id: Uuid,
    actor_id: Option<Uuid>,
    locale: Option<&str>,
    event_kind: &str,
    provenance: &str,
    created_at: chrono::DateTime<chrono::FixedOffset>,
) {
    seller_event::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        seller_id: Set(seller_id),
        actor_id: Set(actor_id),
        event_kind: Set(event_kind.to_string()),
        locale: Set(locale.map(str::to_string)),
        provenance: Set(provenance.to_string()),
        note: Set(None),
        metadata: Set(serde_json::json!({})),
        created_at: Set(created_at),
    }
    .insert(db)
    .await
    .unwrap();
}
