use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use async_trait::async_trait;
use chrono::{Duration, Utc};
use rustok_api::{PortActor, PortContext, PortError};
use rustok_marketplace_ledger::{
    ListMarketplaceSellerLedgerEntriesRequest, MarketplaceLedgerAccountCode,
    MarketplaceLedgerEntryDirection, MarketplaceLedgerEntryListResponse,
    MarketplaceLedgerEntryResponse, MarketplaceLedgerReadPort,
    MarketplaceLedgerTransactionResponse, ReadMarketplaceOrderLedgerRequest,
};
use sea_orm::{
    ConnectOptions, Database, DatabaseConnection, EntityTrait, PaginatorTrait,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

use crate::dto::ScheduleMarketplacePayoutInput;
use crate::entities::{item, payout, schedule_receipt};
use crate::error::MarketplacePayoutError;
use crate::MarketplacePayoutService;

#[tokio::test]
async fn payout_schedule_is_atomic_replay_safe_and_entry_unique() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let seller_id = Uuid::new_v4();
    let entries = vec![
        seller_payable_entry(tenant_id, seller_id, 1_000),
        seller_payable_entry(tenant_id, seller_id, 2_000),
    ];
    let provider = Arc::new(FakeLedgerReader::new(entries.clone()));
    let service = MarketplacePayoutService::new(db.clone(), provider.clone());
    let input = ScheduleMarketplacePayoutInput {
        seller_id,
        currency_code: "usd".to_string(),
        ledger_entry_ids: vec![entries[1].id, entries[0].id],
        scheduled_for: Utc::now().fixed_offset() + Duration::hours(1),
        destination_reference: Some("seller-bank-account".to_string()),
        metadata: serde_json::json!({"source": "operator"}),
    };

    let scheduled = service
        .schedule_with_receipt(
            port_context(tenant_id, actor_id, "schedule-payout"),
            tenant_id,
            actor_id,
            "schedule-payout",
            input.clone(),
        )
        .await
        .unwrap();
    assert_eq!(scheduled.total_amount, 3_000);
    assert_eq!(scheduled.items.len(), 2);
    assert_eq!(scheduled.currency_code, "USD");
    assert_eq!(provider.read_count(), 1);

    let replayed = service
        .schedule_with_receipt(
            port_context(tenant_id, actor_id, "schedule-payout"),
            tenant_id,
            actor_id,
            "schedule-payout",
            input.clone(),
        )
        .await
        .unwrap();
    assert_eq!(replayed, scheduled);
    assert_eq!(provider.read_count(), 1, "receipt replay must precede ledger reads");

    let conflict = service
        .schedule_with_receipt(
            port_context(tenant_id, actor_id, "schedule-payout"),
            tenant_id,
            actor_id,
            "schedule-payout",
            ScheduleMarketplacePayoutInput {
                scheduled_for: input.scheduled_for + Duration::minutes(1),
                ..input.clone()
            },
        )
        .await;
    assert!(matches!(
        conflict,
        Err(MarketplacePayoutError::IdempotencyConflict)
    ));
    assert_eq!(provider.read_count(), 1);

    let second_key = service
        .schedule_with_receipt(
            port_context(tenant_id, actor_id, "schedule-payout-second-key"),
            tenant_id,
            actor_id,
            "schedule-payout-second-key",
            input,
        )
        .await;
    assert!(matches!(
        second_key,
        Err(MarketplacePayoutError::LedgerEntryAlreadyAssigned(_))
    ));
    assert_eq!(provider.read_count(), 2);
    assert_eq!(payout::Entity::find().count(&db).await.unwrap(), 1);
    assert_eq!(item::Entity::find().count(&db).await.unwrap(), 2);
    assert_eq!(schedule_receipt::Entity::find().count(&db).await.unwrap(), 1);
}

#[tokio::test]
async fn non_credit_seller_payable_entry_is_rejected_before_receipt_admission() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let seller_id = Uuid::new_v4();
    let mut entry = seller_payable_entry(tenant_id, seller_id, 1_000);
    entry.direction = MarketplaceLedgerEntryDirection::Debit;
    let provider = Arc::new(FakeLedgerReader::new(vec![entry.clone()]));
    let service = MarketplacePayoutService::new(db.clone(), provider.clone());

    let result = service
        .schedule_with_receipt(
            port_context(tenant_id, actor_id, "invalid-entry"),
            tenant_id,
            actor_id,
            "invalid-entry",
            ScheduleMarketplacePayoutInput {
                seller_id,
                currency_code: "USD".to_string(),
                ledger_entry_ids: vec![entry.id],
                scheduled_for: Utc::now().fixed_offset(),
                destination_reference: None,
                metadata: serde_json::json!({}),
            },
        )
        .await;

    assert!(matches!(result, Err(MarketplacePayoutError::Validation(_))));
    assert_eq!(provider.read_count(), 1);
    assert_eq!(payout::Entity::find().count(&db).await.unwrap(), 0);
    assert_eq!(schedule_receipt::Entity::find().count(&db).await.unwrap(), 0);
}

async fn setup_database() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:marketplace_payout_{}?mode=memory&cache=shared",
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

fn seller_payable_entry(
    tenant_id: Uuid,
    seller_id: Uuid,
    amount: i64,
) -> MarketplaceLedgerEntryResponse {
    MarketplaceLedgerEntryResponse {
        id: Uuid::new_v4(),
        tenant_id,
        transaction_id: Uuid::new_v4(),
        order_id: Uuid::new_v4(),
        assessment_id: Uuid::new_v4(),
        allocation_id: Uuid::new_v4(),
        order_line_item_id: Uuid::new_v4(),
        seller_id: Some(seller_id),
        account_code: MarketplaceLedgerAccountCode::SellerPayable,
        direction: MarketplaceLedgerEntryDirection::Credit,
        currency_code: "USD".to_string(),
        amount,
        metadata: serde_json::json!({}),
        created_at: Utc::now().fixed_offset(),
    }
}

fn port_context(tenant_id: Uuid, actor_id: Uuid, key: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        format!("payout-test-{}", Uuid::new_v4()),
    )
    .with_deadline(std::time::Duration::from_secs(5))
    .with_idempotency_key(key)
}

struct FakeLedgerReader {
    entries: Vec<MarketplaceLedgerEntryResponse>,
    reads: AtomicUsize,
}

impl FakeLedgerReader {
    fn new(entries: Vec<MarketplaceLedgerEntryResponse>) -> Self {
        Self {
            entries,
            reads: AtomicUsize::new(0),
        }
    }

    fn read_count(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl MarketplaceLedgerReadPort for FakeLedgerReader {
    async fn read_order_ledger(
        &self,
        _context: PortContext,
        _request: ReadMarketplaceOrderLedgerRequest,
    ) -> Result<MarketplaceLedgerTransactionResponse, PortError> {
        Err(PortError::not_found(
            "test.not_found",
            "order ledger is not used by payout fixtures",
        ))
    }

    async fn list_seller_entries(
        &self,
        _context: PortContext,
        request: ListMarketplaceSellerLedgerEntriesRequest,
    ) -> Result<MarketplaceLedgerEntryListResponse, PortError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        let filtered = self
            .entries
            .iter()
            .filter(|entry| entry.seller_id == Some(request.seller_id))
            .filter(|entry| {
                request
                    .currency_code
                    .as_deref()
                    .is_none_or(|currency| entry.currency_code == currency)
            })
            .cloned()
            .collect::<Vec<_>>();
        let start = request
            .page
            .max(1)
            .saturating_sub(1)
            .saturating_mul(request.per_page) as usize;
        let end = start.saturating_add(request.per_page as usize).min(filtered.len());
        let items = if start >= filtered.len() {
            Vec::new()
        } else {
            filtered[start..end].to_vec()
        };
        Ok(MarketplaceLedgerEntryListResponse {
            total: filtered.len() as u64,
            items,
            page: request.page.max(1),
            per_page: request.per_page.max(1),
        })
    }
}
