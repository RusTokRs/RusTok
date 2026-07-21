use std::sync::Arc;

use rustok_payment::PROVIDER_EVENT_PROCESSED;
use rustok_payment::entities::provider_event;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect,
    sea_query::{Expr, SimpleExpr},
};
use thiserror::Error;

use super::{
    MarketplaceProviderReversalAdaptFailure, MarketplaceProviderReversalAdaptReport,
    MarketplaceProviderReversalEventAdapter, MarketplaceProviderReversalEventAdapterError,
    MarketplaceReversalAdaptationFailureError, MarketplaceReversalAdaptationFailureJournal,
};

const REFUND_COMPLETED_EVENT: &str = "refund.completed";
const CHARGEBACK_COMPLETED_EVENT: &str = "chargeback.completed";
const MAX_ADAPT_ITEMS: u64 = 200;

#[derive(Debug, Error)]
pub enum MarketplaceProviderReversalBackfillError {
    #[error(transparent)]
    Adapter(#[from] MarketplaceProviderReversalEventAdapterError),
    #[error(transparent)]
    FailureJournal(#[from] MarketplaceReversalAdaptationFailureError),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

pub type MarketplaceProviderReversalBackfillResult<T> =
    Result<T, MarketplaceProviderReversalBackfillError>;

pub struct MarketplaceProviderReversalBackfillService {
    db: DatabaseConnection,
    adapter: MarketplaceProviderReversalEventAdapter,
    failures: MarketplaceReversalAdaptationFailureJournal,
}

impl MarketplaceProviderReversalBackfillService {
    pub fn new(
        db: DatabaseConnection,
        financial_port: Arc<dyn rustok_marketplace::MarketplaceFinancialCommandPort>,
    ) -> Self {
        Self {
            adapter: MarketplaceProviderReversalEventAdapter::new(db.clone(), financial_port),
            failures: MarketplaceReversalAdaptationFailureJournal::new(db.clone()),
            db,
        }
    }

    pub fn failure_journal(&self) -> &MarketplaceReversalAdaptationFailureJournal {
        &self.failures
    }

    pub async fn adapt_pending(
        &self,
        limit: u64,
    ) -> MarketplaceProviderReversalBackfillResult<MarketplaceProviderReversalAdaptReport> {
        let candidates = provider_event::Entity::find()
            .filter(provider_event::Column::Status.eq(PROVIDER_EVENT_PROCESSED))
            .filter(
                provider_event::Column::EventType
                    .is_in([REFUND_COMPLETED_EVENT, CHARGEBACK_COMPLETED_EVENT]),
            )
            .filter(marketplace_extension_filter(self.db.get_database_backend()))
            .filter(Expr::cust(
                "NOT EXISTS (SELECT 1 FROM marketplace_reversal_event_inbox mre WHERE mre.tenant_id = payment_provider_events.tenant_id AND mre.provider_event_id = payment_provider_events.id)",
            ))
            .filter(Expr::cust(
                "NOT EXISTS (SELECT 1 FROM marketplace_reversal_adaptation_failures mraf WHERE mraf.tenant_id = payment_provider_events.tenant_id AND mraf.provider_event_id = payment_provider_events.id AND (mraf.status <> 'retryable_error' OR mraf.next_retry_at IS NULL OR mraf.next_retry_at > CURRENT_TIMESTAMP))",
            ))
            .order_by_asc(provider_event::Column::ProcessedAt)
            .order_by_asc(provider_event::Column::Id)
            .limit(limit.clamp(1, MAX_ADAPT_ITEMS))
            .all(&self.db)
            .await?;

        let mut report = MarketplaceProviderReversalAdaptReport {
            selected: candidates.len(),
            ..Default::default()
        };
        for event in candidates {
            match self.adapter.ingest_provider_event(&event).await {
                Ok(Some(_)) => {
                    self.failures
                        .mark_resolved(event.tenant_id, event.id)
                        .await?;
                    report.adapted += 1;
                }
                Ok(None) => {
                    self.failures
                        .mark_resolved(event.tenant_id, event.id)
                        .await?;
                    report.ignored += 1;
                }
                Err(error) => {
                    let retryable = error.retryable();
                    let safe_message = error.safe_message();
                    self.failures
                        .record_failure(&event, error.code(), safe_message, retryable)
                        .await?;
                    report.failed += 1;
                    report
                        .failures
                        .push(MarketplaceProviderReversalAdaptFailure {
                            provider_event_id: event.id,
                            retryable,
                            message: safe_message.to_string(),
                        });
                }
            }
        }
        Ok(report)
    }
}

pub(super) fn safe_reversal_adapter_message(
    error: &MarketplaceProviderReversalEventAdapterError,
) -> &'static str {
    error.safe_message()
}

fn marketplace_extension_filter(backend: DatabaseBackend) -> SimpleExpr {
    match backend {
        DatabaseBackend::Postgres => {
            Expr::cust("event_metadata::text LIKE '%marketplace_reversal%'")
        }
        DatabaseBackend::Sqlite => {
            Expr::cust("CAST(event_metadata AS TEXT) LIKE '%marketplace_reversal%'")
        }
        DatabaseBackend::MySql => {
            Expr::cust("CAST(event_metadata AS CHAR) LIKE '%marketplace_reversal%'")
        }
    }
}
