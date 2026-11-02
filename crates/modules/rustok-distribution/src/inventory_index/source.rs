use std::{collections::BTreeMap, time::Duration};

use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use rustok_auth::{WorkerServiceContextRequest, WorkerServiceContextRuntime};
use rustok_index::{
    EntityKey, FieldName, IndexMutation, IndexRecord, IndexSource, IndexSourceCursor,
    IndexSourceFailure, IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage,
    IndexSourceScanRequest, IndexValue, SchemaRef,
};
use rustok_inventory::source_history::{
    InventorySourceError, InventorySourceHistory, InventorySourceRecord, MAX_SOURCE_PAGE,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{INVENTORY_STOCK_INDEX_SOURCE, schema::schema_ref};

const READ_BUDGET: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub(super) struct InventoryStockSource {
    pub(super) history: InventorySourceHistory,
    authority: Option<WorkerServiceContextRuntime>,
    backend: DbBackend,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StockCursor {
    tenant_id: Uuid,
    baseline_id: Uuid,
    after_variant_id: Uuid,
}

impl InventoryStockSource {
    pub(super) fn new(
        db: DatabaseConnection,
        authority: Option<WorkerServiceContextRuntime>,
    ) -> Self {
        Self {
            backend: db.get_database_backend(),
            history: InventorySourceHistory::new(db),
            authority,
        }
    }

    fn validate(&self, schema: &SchemaRef) -> Result<(), IndexSourceFailure> {
        if self.backend != DbBackend::Postgres && !(cfg!(test) && self.backend == DbBackend::Sqlite)
        {
            return Err(permanent("inventory_index_backend_unsupported"));
        }
        if schema_ref().map_err(|_| permanent("inventory_index_contract_invalid"))? != *schema {
            return Err(permanent("inventory_index_schema_mismatch"));
        }
        Ok(())
    }

    pub(super) async fn authenticated_baseline(
        &self,
        tenant: Uuid,
        correlation: Uuid,
    ) -> Result<(PortContext, Uuid), IndexSourceFailure> {
        let authority = self
            .authority
            .as_ref()
            .ok_or_else(|| retryable("inventory_index_authority_unconfigured"))?;
        let request = WorkerServiceContextRequest::new(
            tenant,
            INVENTORY_STOCK_INDEX_SOURCE,
            correlation,
            READ_BUDGET,
        )
        .map_err(authority_failure)?;
        let context = authority
            .current_context(request)
            .await
            .map_err(authority_failure)?;
        let baseline = self
            .history
            .baseline_status(&context)
            .await
            .map_err(owner_failure)?
            .ok_or_else(|| retryable("inventory_source_baseline_unavailable"))?;
        if !baseline.active {
            return Err(retryable("inventory_source_baseline_unavailable"));
        }
        if baseline.tenant_id != tenant || baseline.baseline_id.is_nil() {
            return Err(permanent("inventory_source_state_unverified"));
        }
        Ok((context, baseline.baseline_id))
    }

    pub(super) async fn verify_fact(
        &self,
        tenant: Uuid,
        correlation: Uuid,
        baseline_id: Uuid,
        changed: Option<(Uuid, u64, Uuid)>,
    ) -> Result<(), IndexSourceFailure> {
        self.validate(&schema_ref().map_err(|_| permanent("inventory_index_contract_invalid"))?)?;
        within_budget(async {
            let (context, current) = self.authenticated_baseline(tenant, correlation).await?;
            if current != baseline_id {
                return Err(permanent("inventory_index_baseline_mismatch"));
            }
            if let Some((variant, version, change_id)) = changed {
                let after = version
                    .checked_sub(1)
                    .ok_or_else(|| permanent("inventory_index_change_identity_invalid"))?;
                let changes = self
                    .history
                    .changes(&context, baseline_id, variant, after, 1)
                    .await
                    .map_err(owner_failure)?;
                let Some(change) = changes.first() else {
                    return Err(permanent("inventory_index_change_identity_invalid"));
                };
                if changes.len() != 1
                    || change.tenant_id != tenant
                    || change.baseline_id != baseline_id
                    || change.variant_id != variant
                    || change.source_version != version
                    || change.change_id != change_id
                    || !matches!(change.kind.as_str(), "created" | "changed" | "deleted")
                {
                    return Err(permanent("inventory_index_change_identity_invalid"));
                }
            }
            Ok(())
        })
        .await
    }

    async fn scan_current(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        self.validate(request.schema())?;
        if request.locale().is_some() {
            return Err(permanent("inventory_index_locale_mismatch"));
        }
        let (context, baseline) = self
            .authenticated_baseline(request.tenant_id(), Uuid::new_v4())
            .await?;
        let after = match request.cursor() {
            None => None,
            Some(cursor) => {
                let cursor: StockCursor = serde_json::from_value(cursor.value().clone())
                    .map_err(|_| permanent("inventory_index_cursor_invalid"))?;
                if cursor.tenant_id != request.tenant_id()
                    || cursor.baseline_id != baseline
                    || cursor.after_variant_id.is_nil()
                {
                    return Err(permanent("inventory_index_cursor_invalid"));
                }
                Some(cursor.after_variant_id)
            }
        };
        let page = self
            .history
            .scan(
                &context,
                baseline,
                after,
                request.limit().min(MAX_SOURCE_PAGE),
            )
            .await
            .map_err(owner_failure)?;
        if page.baseline_id != baseline {
            return Err(permanent("inventory_index_baseline_mismatch"));
        }
        let mutations = page
            .records
            .iter()
            .map(|row| mutation(row, request.tenant_id(), baseline))
            .collect::<Result<Vec<_>, _>>()?;
        let next = page
            .next_after
            .map(|after_variant_id| {
                serde_json::to_value(StockCursor {
                    tenant_id: request.tenant_id(),
                    baseline_id: baseline,
                    after_variant_id,
                })
                .map_err(|_| permanent("inventory_index_cursor_invalid"))
                .and_then(|value| {
                    IndexSourceCursor::new(value)
                        .map_err(|_| permanent("inventory_index_cursor_invalid"))
                })
            })
            .transpose()?;
        IndexSourcePage::new(&request, mutations, next)
            .map_err(|_| permanent("inventory_index_page_invalid"))
    }

    async fn load_current(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        self.validate(request.schema())?;
        if request.keys().len() > MAX_SOURCE_PAGE
            || request.keys().iter().any(|key| key.locale.is_some())
        {
            return Err(permanent("inventory_index_load_scope_invalid"));
        }
        let (context, baseline) = self
            .authenticated_baseline(request.tenant_id(), Uuid::new_v4())
            .await?;
        let variants = request
            .keys()
            .iter()
            .map(|key| key.entity_id)
            .collect::<Vec<_>>();
        let records = self
            .history
            .load(&context, baseline, &variants)
            .await
            .map_err(owner_failure)?;
        // Native load refuses missing keys; never convert a missing key into deletion or zero stock.
        if records.len() != variants.len() {
            return Err(permanent("inventory_source_state_unverified"));
        }
        let mutations = records
            .iter()
            .map(|row| mutation(row, request.tenant_id(), baseline))
            .collect::<Result<Vec<_>, _>>()?;
        IndexSourceLoadBatch::new(&request, mutations)
            .map_err(|_| permanent("inventory_index_load_scope_invalid"))
    }
}

#[async_trait]
impl IndexSource for InventoryStockSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        within_budget(self.scan_current(request)).await
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        within_budget(self.load_current(request)).await
    }
}

pub(super) fn mutation(
    row: &InventorySourceRecord,
    tenant: Uuid,
    baseline: Uuid,
) -> Result<IndexMutation, IndexSourceFailure> {
    if tenant.is_nil()
        || baseline.is_nil()
        || row.tenant_id != tenant
        || row.baseline_id != baseline
        || row.variant_id.is_nil()
        || row.product_id.is_nil()
        || row.inventory_item_id.is_nil()
        || row.change_id.is_nil()
        || row.source_version == 0
        || i64::try_from(row.source_version).is_err()
        || row.stocked < 0
        || row.reserved < 0
        || row.incoming < 0
        || row.stocked.checked_sub(row.reserved) != Some(row.available)
        || (row.deleted && (row.stocked != 0 || row.reserved != 0 || row.incoming != 0))
    {
        return Err(permanent("inventory_source_state_unverified"));
    }
    let key = EntityKey {
        tenant_id: tenant,
        schema: schema_ref().map_err(|_| permanent("inventory_index_contract_invalid"))?,
        entity_id: row.variant_id,
        locale: None,
    };
    if row.deleted {
        return Ok(IndexMutation::Delete {
            event_id: row.change_id,
            key,
            source_version: row.source_version,
        });
    }
    let mut fields = BTreeMap::new();
    for (name, value) in [
        ("id", IndexValue::Uuid(row.variant_id)),
        ("product_id", IndexValue::Uuid(row.product_id)),
        ("inventory_item_id", IndexValue::Uuid(row.inventory_item_id)),
        ("baseline_id", IndexValue::Uuid(row.baseline_id)),
        ("stocked", IndexValue::Integer(row.stocked)),
        ("reserved", IndexValue::Integer(row.reserved)),
        ("incoming", IndexValue::Integer(row.incoming)),
        ("available", IndexValue::Integer(row.available)),
    ] {
        fields.insert(
            FieldName::new(name).map_err(|_| permanent("inventory_index_contract_invalid"))?,
            value,
        );
    }
    Ok(IndexMutation::Upsert {
        event_id: row.change_id,
        record: IndexRecord {
            key,
            source_version: row.source_version,
            fields,
            links: Vec::new(),
        },
    })
}

async fn within_budget<T>(
    future: impl std::future::Future<Output = Result<T, IndexSourceFailure>>,
) -> Result<T, IndexSourceFailure> {
    rustok_core::resilience::with_timeout(READ_BUDGET, || future)
        .await
        .map_err(|_| retryable("inventory_index_read_timeout"))?
}

fn authority_failure(error: PortError) -> IndexSourceFailure {
    if error.retryable {
        retryable("inventory_index_authority_unavailable")
    } else {
        permanent("inventory_index_authority_denied")
    }
}

fn owner_failure(error: InventorySourceError) -> IndexSourceFailure {
    match error {
        InventorySourceError::Database(_)
        | InventorySourceError::DeadlineExceeded
        | InventorySourceError::BaselineUnavailable
        | InventorySourceError::PublicationUnavailable => retryable("inventory_source_unavailable"),
        InventorySourceError::Forbidden => permanent("inventory_source_forbidden"),
        InventorySourceError::Conflict => permanent("inventory_source_identity_conflict"),
        _ => permanent("inventory_source_state_unverified"),
    }
}

fn permanent(code: &'static str) -> IndexSourceFailure {
    // INVARIANT: every caller passes a bounded, literal machine code declared in this module.
    IndexSourceFailure::permanent(code).expect("literal Inventory Index failure code is valid")
}
fn retryable(code: &'static str) -> IndexSourceFailure {
    // INVARIANT: every caller passes a bounded, literal machine code declared in this module.
    IndexSourceFailure::retryable(code).expect("literal Inventory Index failure code is valid")
}
