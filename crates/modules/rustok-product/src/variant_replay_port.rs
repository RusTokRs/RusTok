/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::time::Duration;

use async_trait::async_trait;
use rustok_api::{PortActorKind, PortContext, PortError};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, Value};
use uuid::Uuid;

use crate::index_projection::{PRODUCT_VARIANT_ROW_SELECT, PRODUCT_VARIANT_ROWS_CTE};
use crate::{
    PRODUCT_VARIANT_REPLAY_READ_CLAIM, ProductVariantReplayEntry, ProductVariantReplayFields,
    ProductVariantReplayPage, ProductVariantReplayPort, ProductVariantReplayScan,
    ProductVariantReplayState,
};

#[derive(Clone, Debug)]
pub struct PostgresProductVariantReplay {
    db: DatabaseConnection,
}

impl PostgresProductVariantReplay {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn verify_readable_schema(&self) -> Result<(), sea_orm::DbErr> {
        let sql = format!("WITH {PRODUCT_VARIANT_ROWS_CTE}\n{PRODUCT_VARIANT_ROW_SELECT}\nLIMIT 0");
        self.db
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                sql,
                [Uuid::nil().into()],
            ))
            .await?;
        Ok(())
    }

    fn authorize(&self, context: &PortContext) -> Result<Uuid, PortError> {
        context.require_read_semantics()?;
        if context.actor.kind != PortActorKind::Service
            || !context
                .claims
                .iter()
                .any(|claim| claim == PRODUCT_VARIANT_REPLAY_READ_CLAIM)
        {
            return Err(PortError::forbidden(
                "product.variant_replay_forbidden",
                "Product variant replay requires a service grant",
            ));
        }
        let tenant_id = Uuid::parse_str(&context.tenant_id)
            .ok()
            .filter(|tenant| !tenant.is_nil())
            .ok_or_else(invalid_request)?;
        if self.db.get_database_backend() != DbBackend::Postgres {
            return Err(PortError::invariant_violation(
                "product.variant_replay_backend_unsupported",
                "Product variant replay requires PostgreSQL",
            ));
        }
        Ok(tenant_id)
    }

    async fn read(
        &self,
        context: &PortContext,
        tenant_id: Uuid,
        predicate: &str,
        parameters: Vec<Value>,
    ) -> Result<Vec<ProductVariantReplayEntry>, PortError> {
        let sql =
            format!("WITH {PRODUCT_VARIANT_ROWS_CTE}\n{PRODUCT_VARIANT_ROW_SELECT}\n{predicate}");
        let query = Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            std::iter::once(tenant_id.into()).chain(parameters),
        );
        let deadline = Duration::from_millis(context.deadline_ms.unwrap_or_default().min(30_000));
        let rows = tokio::time::timeout(deadline, self.db.query_all_raw(query))
            .await
            .map_err(|_| {
                PortError::timeout(
                    "product.variant_replay_timeout",
                    "Product variant replay timed out",
                )
            })?
            .map_err(|error| {
                tracing::warn!(%error, "Product variant replay storage read failed");
                PortError::unavailable(
                    "product.variant_replay_unavailable",
                    "Product variant replay is unavailable",
                )
            })?;
        rows.iter().map(|row| decode_row(row, tenant_id)).collect()
    }
}

#[async_trait]
impl ProductVariantReplayPort for PostgresProductVariantReplay {
    async fn scan_variant_replay(
        &self,
        context: PortContext,
        request: ProductVariantReplayScan,
    ) -> Result<ProductVariantReplayPage, PortError> {
        let tenant_id = self.authorize(&context)?;
        if !(1..=1_000).contains(&request.limit)
            || request.after_variant_id.is_some_and(|id| id.is_nil())
        {
            return Err(invalid_request());
        }
        let fetch_limit = i64::try_from(request.limit + 1).map_err(|_| invalid_request())?;
        let mut entries = self.read(
            &context, tenant_id,
            "WHERE ($2::uuid IS NULL OR row.variant_id > $2) ORDER BY row.variant_id ASC LIMIT $3",
            vec![request.after_variant_id.into(), fetch_limit.into()],
        ).await?;
        let has_more = entries.len() > request.limit;
        entries.truncate(request.limit);
        let next_after_variant_id = if has_more {
            entries.last().map(|entry| entry.variant_id)
        } else {
            None
        };
        Ok(ProductVariantReplayPage {
            entries,
            next_after_variant_id,
        })
    }

    async fn load_variant_replay(
        &self,
        context: PortContext,
        variant_ids: Vec<Uuid>,
    ) -> Result<Vec<ProductVariantReplayEntry>, PortError> {
        let tenant_id = self.authorize(&context)?;
        if variant_ids.is_empty() || variant_ids.len() > 256 || variant_ids.iter().any(Uuid::is_nil)
        {
            return Err(invalid_request());
        }
        self.read(
            &context,
            tenant_id,
            "WHERE row.variant_id = ANY($2::uuid[]) ORDER BY row.variant_id ASC",
            vec![variant_ids.into()],
        )
        .await
    }
}

fn decode_row(
    row: &QueryResult,
    expected_tenant: Uuid,
) -> Result<ProductVariantReplayEntry, PortError> {
    let tenant_id: Uuid = column(row, "tenant_id")?;
    let variant_id: Uuid = column(row, "variant_id")?;
    let source_version =
        u64::try_from(column::<i64>(row, "index_revision")?).map_err(|_| invalid_row())?;
    if column::<i64>(row, "identity_count")? != 1
        || tenant_id != expected_tenant
        || variant_id.is_nil()
        || source_version == 0
    {
        return Err(invalid_row());
    }
    let state = if column::<bool>(row, "is_deleted")? {
        ProductVariantReplayState::Deleted
    } else {
        let fields = ProductVariantReplayFields {
            product_id: column(row, "product_id")?,
            sku: column(row, "sku")?,
            barcode: column(row, "barcode")?,
            shipping_profile_slug: column(row, "shipping_profile_slug")?,
            ean: column(row, "ean")?,
            upc: column(row, "upc")?,
            inventory_policy: column(row, "inventory_policy")?,
            inventory_management: column(row, "inventory_management")?,
            inventory_quantity: column(row, "inventory_quantity")?,
            weight_unit: column(row, "weight_unit")?,
            combination_identity: column(row, "combination_identity")?,
            position: column(row, "position")?,
        };
        if fields.product_id.is_nil()
            || fields.inventory_policy.is_empty()
            || fields.inventory_management.is_empty()
        {
            return Err(invalid_row());
        }
        ProductVariantReplayState::Live(fields)
    };
    Ok(ProductVariantReplayEntry {
        tenant_id,
        variant_id,
        source_version,
        state,
    })
}

fn column<T: sea_orm::TryGetable>(row: &QueryResult, name: &str) -> Result<T, PortError> {
    row.try_get("", name).map_err(|_| invalid_row())
}

fn invalid_row() -> PortError {
    PortError::invariant_violation(
        "product.variant_replay_invalid_row",
        "Invalid Product variant replay row",
    )
}

fn invalid_request() -> PortError {
    PortError::validation(
        "product.variant_replay_invalid_request",
        "Invalid Product variant replay request",
    )
}
