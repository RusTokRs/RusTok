use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, Utc};
use rustok_marketplace_payout::{
    entities::{payout, provider_operation},
    providers::{
        CancelPayoutProviderRequest, LookupPayoutProviderRequest, PayoutProvider,
        PayoutProviderCapabilities, PayoutProviderDescriptor, PayoutProviderHealth,
        PayoutProviderRegistration, PayoutProviderRegistry, PayoutProviderResult,
        PayoutProviderTransferStatus, SubmitPayoutProviderRequest,
    },
    BeginMarketplacePayoutProviderOperation, MarketplacePayoutError,
    MarketplacePayoutProviderSubmissionService,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
    EntityTrait, IntoActiveModel, QueryFilter, Set,
};
use sea_orm_migration::SchemaManager;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Clone, Copy)]
enum MockMode {
    Success,
    ConfirmedFailed,
    UnknownResult,
    OutcomeUnknown,
}

struct MockPayoutProvider {
    calls: Arc<AtomicUsize>,
    mode: MockMode,
}

#[async_trait]
impl PayoutProvider for MockPayoutProvider {
    fn descriptor(&self) -> PayoutProviderDescriptor {
        PayoutProviderDescriptor {
            provider_id: "mock".to_string(),
            display_name: "Mock payout provider".to_string(),
            capabilities: PayoutProviderCapabilities {
                submit: true,
                lookup: true,
                cancel: true,
                webhook_ingress: false,
            },
            default_for_new_payouts: false,
        }
    }

    async fn submit(
        &self,
        request: SubmitPayoutProviderRequest,
    ) -> rustok_marketplace_payout::MarketplacePayoutResult<PayoutProviderResult> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.mode {
            MockMode::Success => Ok(PayoutProviderResult {
                provider_id: "mock".to_string(),
                status: PayoutProviderTransferStatus::Submitted,
                external_reference: Some(format!("transfer-{}", request.payout_id)),
                failure_code: None,
                metadata: serde_json::json!({"accepted": true}),
            }),
            MockMode::ConfirmedFailed => Ok(PayoutProviderResult {
                provider_id: "mock".to_string(),
                status: PayoutProviderTransferStatus::Failed,
                external_reference: Some(format!("transfer-{}", request.payout_id)),
                failure_code: Some("beneficiary_rejected".to_string()),
                metadata: serde_json::json!({"accepted": false}),
            }),
            MockMode::UnknownResult => Ok(PayoutProviderResult {
                provider_id: "mock".to_string(),
                status: PayoutProviderTransferStatus::Unknown,
                external_reference: Some(format!("transfer-{}", request.payout_id)),
                failure_code: None,
                metadata: serde_json::json!({"provider_state": "unknown"}),
            }),
            MockMode::OutcomeUnknown => Err(MarketplacePayoutError::ProviderOutcomeUnknown {
                provider_id: "mock".to_string(),
                operation: "submit".to_string(),
            }),
        }
    }

    async fn lookup(
        &self,
        _request: LookupPayoutProviderRequest,
    ) -> rustok_marketplace_payout::MarketplacePayoutResult<PayoutProviderResult> {
        Err(MarketplacePayoutError::ProviderRejected {
            provider_id: "mock".to_string(),
            operation: "lookup".to_string(),
        })
    }

    async fn cancel(
        &self,
        _request: CancelPayoutProviderRequest,
    ) -> rustok_marketplace_payout::MarketplacePayoutResult<PayoutProviderResult> {
        Err(MarketplacePayoutError::ProviderRejected {
            provider_id: "mock".to_string(),
            operation: "cancel".to_string(),
        })
    }
}

#[tokio::test]
async fn submit_replays_checkpoint_without_repeating_provider_or_marking_paid() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let payout_id = insert_payout(&db, tenant_id).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let service = submission_service(&db, calls.clone(), MockMode::Success);

    let first = service
        .submit(tenant_id, payout_id, "mock", "submit-once")
        .await
        .unwrap();
    let replay = service
        .submit(tenant_id, payout_id, "mock", "submit-once")
        .await
        .unwrap();

    assert_eq!(first, replay);
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let payout = payout::Entity::find_by_id(payout_id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(payout.status, "scheduled");
    assert!(payout.external_reference.is_none());
    assert!(payout.paid_at.is_none());

    let operation = provider_operation::Entity::find()
        .filter(provider_operation::Column::TenantId.eq(tenant_id))
        .filter(provider_operation::Column::PayoutId.eq(payout_id))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        operation.status,
        provider_operation::MarketplacePayoutProviderOperationStatus::ProviderSucceeded
    );
    assert!(operation.provider_result_json.is_some());
    assert!(operation.provider_completed_at.is_some());
    assert!(operation.committed_at.is_none());
}

#[tokio::test]
async fn second_submit_key_for_same_payout_is_rejected_before_provider_call() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let payout_id = insert_payout(&db, tenant_id).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let service = submission_service(&db, calls.clone(), MockMode::Success);

    service
        .submit(tenant_id, payout_id, "mock", "submit-first")
        .await
        .unwrap();
    let error = service
        .submit(tenant_id, payout_id, "mock", "submit-second")
        .await
        .unwrap_err();

    assert!(matches!(error, MarketplacePayoutError::IdempotencyConflict));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn confirmed_failed_result_is_persisted_without_marking_provider_success() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let payout_id = insert_payout(&db, tenant_id).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let service = submission_service(&db, calls.clone(), MockMode::ConfirmedFailed);

    let error = service
        .submit(tenant_id, payout_id, "mock", "submit-failed")
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        MarketplacePayoutError::OperationFailed { .. }
    ));
    let replay = service
        .submit(tenant_id, payout_id, "mock", "submit-failed")
        .await
        .unwrap_err();
    assert!(matches!(
        replay,
        MarketplacePayoutError::OperationFailed { .. }
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let operation = provider_operation::Entity::find()
        .filter(provider_operation::Column::TenantId.eq(tenant_id))
        .filter(provider_operation::Column::PayoutId.eq(payout_id))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        operation.status,
        provider_operation::MarketplacePayoutProviderOperationStatus::ProviderFailed
    );
    assert_eq!(
        operation.last_error_code.as_deref(),
        Some("marketplace_payout.provider_failed")
    );
    assert!(operation.provider_result_json.is_some());
    assert!(operation.provider_completed_at.is_some());
    assert!(operation.committed_at.is_none());

    let payout = payout::Entity::find_by_id(payout_id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(payout.status, "scheduled");
    assert!(payout.paid_at.is_none());
}

#[tokio::test]
async fn unknown_result_is_checkpointed_for_reconciliation() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let payout_id = insert_payout(&db, tenant_id).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let service = submission_service(&db, calls.clone(), MockMode::UnknownResult);

    let error = service
        .submit(tenant_id, payout_id, "mock", "submit-unknown-result")
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        MarketplacePayoutError::ReconciliationRequired(_)
    ));
    let replay = service
        .submit(tenant_id, payout_id, "mock", "submit-unknown-result")
        .await
        .unwrap_err();
    assert!(matches!(
        replay,
        MarketplacePayoutError::ReconciliationRequired(_)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let operation = provider_operation::Entity::find()
        .filter(provider_operation::Column::TenantId.eq(tenant_id))
        .filter(provider_operation::Column::PayoutId.eq(payout_id))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        operation.status,
        provider_operation::MarketplacePayoutProviderOperationStatus::ReconciliationRequired
    );
    assert_eq!(
        operation.last_error_code.as_deref(),
        Some("marketplace_payout.provider_status_unknown")
    );
    assert!(operation.provider_reference.is_some());
    assert!(operation.provider_result_json.is_some());
    assert!(operation.provider_completed_at.is_some());
}

#[tokio::test]
async fn unknown_provider_outcome_is_durable_and_never_reexecuted() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let payout_id = insert_payout(&db, tenant_id).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let service = submission_service(&db, calls.clone(), MockMode::OutcomeUnknown);

    let first = service
        .submit(tenant_id, payout_id, "mock", "submit-unknown")
        .await
        .unwrap_err();
    assert!(matches!(
        first,
        MarketplacePayoutError::ReconciliationRequired(_)
    ));

    let replay = service
        .submit(tenant_id, payout_id, "mock", "submit-unknown")
        .await
        .unwrap_err();
    assert!(matches!(
        replay,
        MarketplacePayoutError::ReconciliationRequired(_)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let operation = provider_operation::Entity::find()
        .filter(provider_operation::Column::TenantId.eq(tenant_id))
        .filter(provider_operation::Column::PayoutId.eq(payout_id))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        operation.status,
        provider_operation::MarketplacePayoutProviderOperationStatus::ReconciliationRequired
    );
    assert_eq!(
        operation.last_error_code.as_deref(),
        Some("marketplace_payout.provider_outcome_unknown")
    );
    assert!(operation.lease_owner.is_none());
    assert!(operation.lease_expires_at.is_none());
}

#[tokio::test]
async fn expired_executing_submit_requires_reconciliation_without_reexecution() {
    let db = setup_database().await;
    let tenant_id = Uuid::new_v4();
    let payout_id = insert_payout(&db, tenant_id).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let service = submission_service(&db, calls.clone(), MockMode::Success);
    let payout = payout::Entity::find_by_id(payout_id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let request = SubmitPayoutProviderRequest {
        tenant_id,
        payout_id,
        seller_id: payout.seller_id,
        amount: payout.total_amount,
        currency_code: payout.currency_code,
        destination_reference: payout.destination_reference,
        idempotency_key: "submit-expired".to_string(),
        metadata: payout.metadata,
    };
    let request_json = serde_json::to_value(&request).unwrap();
    let request_hash = hex::encode(Sha256::digest(serde_json::to_vec(&request).unwrap()));
    let operation = service
        .journal()
        .begin(BeginMarketplacePayoutProviderOperation {
            tenant_id,
            payout_id,
            operation: provider_operation::MarketplacePayoutProviderOperationKind::Submit,
            provider_id: "mock".to_string(),
            idempotency_key: "submit-expired".to_string(),
            request_hash,
            request_json,
        })
        .await
        .unwrap();
    let claimed = service.journal().claim_execution(operation).await.unwrap();
    let mut active = claimed.into_active_model();
    let expired_at = Utc::now().fixed_offset() - ChronoDuration::minutes(1);
    active.lease_expires_at = Set(Some(expired_at.clone()));
    active.updated_at = Set(expired_at);
    active.update(&db).await.unwrap();

    let error = service
        .submit(tenant_id, payout_id, "mock", "submit-expired")
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        MarketplacePayoutError::ReconciliationRequired(_)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    let operation = provider_operation::Entity::find()
        .filter(provider_operation::Column::TenantId.eq(tenant_id))
        .filter(provider_operation::Column::PayoutId.eq(payout_id))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        operation.status,
        provider_operation::MarketplacePayoutProviderOperationStatus::ReconciliationRequired
    );
}

fn submission_service(
    db: &DatabaseConnection,
    calls: Arc<AtomicUsize>,
    mode: MockMode,
) -> MarketplacePayoutProviderSubmissionService {
    let mut registry = PayoutProviderRegistry::new();
    registry
        .register_external(
            "mock",
            Arc::new(MockPayoutProvider { calls, mode }),
            PayoutProviderRegistration {
                descriptor: PayoutProviderDescriptor {
                    provider_id: "mock".to_string(),
                    display_name: "Mock payout provider".to_string(),
                    capabilities: PayoutProviderCapabilities {
                        submit: true,
                        lookup: true,
                        cancel: true,
                        webhook_ingress: false,
                    },
                    default_for_new_payouts: false,
                },
                health: PayoutProviderHealth::Ready,
                degraded_mode: None,
            },
        )
        .unwrap();
    MarketplacePayoutProviderSubmissionService::new(db.clone(), Arc::new(registry))
}

async fn setup_database() -> DatabaseConnection {
    let url = format!(
        "sqlite:file:marketplace_payout_provider_submission_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await.unwrap();
    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .unwrap();
    let manager = SchemaManager::new(&db);
    for migration in rustok_marketplace_payout::migrations::migrations() {
        migration.up(&manager).await.unwrap();
    }
    db
}

async fn insert_payout(db: &DatabaseConnection, tenant_id: Uuid) -> Uuid {
    let payout_id = Uuid::new_v4();
    let now = Utc::now().fixed_offset();
    payout::ActiveModel {
        id: Set(payout_id),
        tenant_id: Set(tenant_id),
        seller_id: Set(Uuid::new_v4()),
        currency_code: Set("USD".to_string()),
        total_amount: Set(2_500),
        status: Set("scheduled".to_string()),
        scheduled_for: Set(now.clone() - ChronoDuration::minutes(1)),
        destination_reference: Set(Some("seller-bank-account".to_string())),
        external_reference: Set(None),
        failure_code: Set(None),
        metadata: Set(serde_json::json!({"batch": "weekly"})),
        created_at: Set(now.clone()),
        updated_at: Set(now),
        paid_at: Set(None),
    }
    .insert(db)
    .await
    .unwrap();
    payout_id
}
