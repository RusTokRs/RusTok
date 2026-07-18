use sea_orm::{
    ActiveModelTrait, ConnectOptions, Database, DatabaseConnection, Set,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

use crate::dto::{MarketplaceSellerEventKind, MarketplaceSellerEventProvenance};
use crate::entities::{seller, seller_event};
use crate::error::MarketplaceSellerError;
use crate::MarketplaceSellerService;

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
    assert_eq!(events[0].provenance, MarketplaceSellerEventProvenance::Command);

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
    assert!(invalid.is_err(), "command event without actor must fail DB CHECK");
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
    let seller_id = Uuid::new_v4();
    let now = chrono::Utc::now().fixed_offset();
    seller::ActiveModel {
        id: Set(seller_id),
        tenant_id: Set(tenant_id),
        handle: Set(format!("seller-{seller_id}")),
        legal_name: Set(None),
        status: Set("draft".to_string()),
        onboarding_status: Set("draft".to_string()),
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
