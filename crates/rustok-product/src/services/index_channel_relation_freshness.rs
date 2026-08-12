use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction, DbBackend,
    QueryResult, Statement, TransactionTrait,
};
use thiserror::Error;
use uuid::Uuid;

pub const MAX_PRODUCT_SALES_CHANNEL_VISIBILITY_KEY_BYTES: usize = 131_072;

const RELATION_LOCK_DOMAIN: &str = "product-sales-channel-index-relation";
const FRESHNESS_LOCK_DOMAIN: &str = "product-sales-channel-index-relation-freshness";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductSalesChannelIndexRelationFreshnessRecord {
    sequence_no: i64,
    tenant_id: Uuid,
    product_id: Uuid,
    relation_epoch: u64,
    product_source_version: u64,
    visibility_key: String,
    channel_identity_generation: u64,
}

impl ProductSalesChannelIndexRelationFreshnessRecord {
    pub fn sequence_no(&self) -> i64 {
        self.sequence_no
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn product_id(&self) -> Uuid {
        self.product_id
    }

    pub fn relation_epoch(&self) -> u64 {
        self.relation_epoch
    }

    pub fn product_source_version(&self) -> u64 {
        self.product_source_version
    }

    pub fn visibility_key(&self) -> &str {
        &self.visibility_key
    }

    pub fn channel_identity_generation(&self) -> u64 {
        self.channel_identity_generation
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProductSalesChannelIndexRelationFreshnessWriteOutcome {
    Initial(ProductSalesChannelIndexRelationFreshnessRecord),
    Unchanged(ProductSalesChannelIndexRelationFreshnessRecord),
    Advanced(ProductSalesChannelIndexRelationFreshnessRecord),
}

impl ProductSalesChannelIndexRelationFreshnessWriteOutcome {
    pub fn record(&self) -> &ProductSalesChannelIndexRelationFreshnessRecord {
        match self {
            Self::Initial(record) | Self::Unchanged(record) | Self::Advanced(record) => record,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ProductSalesChannelIndexRelationFreshnessError {
    #[error("Product-SalesChannel freshness tenant identity is invalid")]
    InvalidTenant,
    #[error("Product-SalesChannel freshness Product identity is invalid")]
    InvalidProduct,
    #[error("Product-SalesChannel freshness Product does not exist")]
    ProductNotFound,
    #[error("Product-SalesChannel freshness relation epoch is invalid")]
    InvalidRelationEpoch,
    #[error("Product-SalesChannel freshness relation epoch is no longer current")]
    RelationNotCurrent,
    #[error("Product-SalesChannel freshness Product source version is invalid")]
    InvalidProductSourceVersion,
    #[error("Product-SalesChannel freshness visibility key is invalid")]
    InvalidVisibilityKey,
    #[error("Product-SalesChannel freshness Channel identity generation is invalid")]
    InvalidChannelIdentityGeneration,
    #[error("Product-SalesChannel freshness watermark regressed")]
    WatermarkRegressed,
    #[error("Product-SalesChannel freshness storage returned invalid state")]
    InvalidStoredState,
    #[error("Product-SalesChannel freshness storage is unavailable")]
    Unavailable,
}

/// Product-owned append-only witness that one current relation epoch was resolved from exact Product
/// visibility and a durable Channel identity generation.
///
/// Lock order is Product row -> relation advisory lock -> freshness advisory lock. This matches the
/// relation owner and prevents a witness from being committed for an epoch concurrently superseded by
/// another relation writer. The store accepts opaque visibility evidence and never reads Channel
/// tables or imports Channel types.
#[derive(Clone)]
pub struct ProductSalesChannelIndexRelationFreshnessStore {
    db: DatabaseConnection,
}

impl ProductSalesChannelIndexRelationFreshnessStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn record(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        relation_epoch: u64,
        product_source_version: u64,
        visibility_key: impl Into<String>,
        channel_identity_generation: u64,
    ) -> Result<
        ProductSalesChannelIndexRelationFreshnessWriteOutcome,
        ProductSalesChannelIndexRelationFreshnessError,
    > {
        validate_scope(tenant_id, product_id)?;
        ensure_postgres(&self.db)?;
        let relation_epoch = positive_i64(
            relation_epoch,
            ProductSalesChannelIndexRelationFreshnessError::InvalidRelationEpoch,
        )?;
        let product_source_version = positive_i64(
            product_source_version,
            ProductSalesChannelIndexRelationFreshnessError::InvalidProductSourceVersion,
        )?;
        let channel_identity_generation =
            i64::try_from(channel_identity_generation).map_err(|_| {
                ProductSalesChannelIndexRelationFreshnessError::InvalidChannelIdentityGeneration
            })?;
        let visibility_key = visibility_key.into();
        if visibility_key.is_empty()
            || visibility_key.len() > MAX_PRODUCT_SALES_CHANNEL_VISIBILITY_KEY_BYTES
        {
            return Err(ProductSalesChannelIndexRelationFreshnessError::InvalidVisibilityKey);
        }

        let transaction = self
            .db
            .begin()
            .await
            .map_err(|_| ProductSalesChannelIndexRelationFreshnessError::Unavailable)?;
        let result = self
            .record_in_transaction(
                &transaction,
                tenant_id,
                product_id,
                relation_epoch,
                product_source_version,
                visibility_key,
                channel_identity_generation,
            )
            .await;

        match result {
            Ok(outcome) => {
                transaction
                    .commit()
                    .await
                    .map_err(|_| ProductSalesChannelIndexRelationFreshnessError::Unavailable)?;
                Ok(outcome)
            }
            Err(error) => {
                transaction
                    .rollback()
                    .await
                    .map_err(|_| ProductSalesChannelIndexRelationFreshnessError::Unavailable)?;
                Err(error)
            }
        }
    }

    async fn record_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        tenant_id: Uuid,
        product_id: Uuid,
        relation_epoch: i64,
        product_source_version: i64,
        visibility_key: String,
        channel_identity_generation: i64,
    ) -> Result<
        ProductSalesChannelIndexRelationFreshnessWriteOutcome,
        ProductSalesChannelIndexRelationFreshnessError,
    > {
        require_live_product(transaction, tenant_id, product_id).await?;
        lock_domain(transaction, tenant_id, product_id, RELATION_LOCK_DOMAIN).await?;
        lock_domain(transaction, tenant_id, product_id, FRESHNESS_LOCK_DOMAIN).await?;
        require_current_relation_epoch(transaction, tenant_id, product_id, relation_epoch).await?;
        let previous = load_latest(transaction, tenant_id, product_id).await?;

        if let Some(previous) = previous.as_ref() {
            let relation_epoch_u64 = u64::try_from(relation_epoch)
                .map_err(|_| ProductSalesChannelIndexRelationFreshnessError::InvalidStoredState)?;
            let product_source_version_u64 = u64::try_from(product_source_version)
                .map_err(|_| ProductSalesChannelIndexRelationFreshnessError::InvalidStoredState)?;
            let channel_identity_generation_u64 = u64::try_from(channel_identity_generation)
                .map_err(|_| ProductSalesChannelIndexRelationFreshnessError::InvalidStoredState)?;

            if relation_epoch_u64 < previous.relation_epoch
                || product_source_version_u64 < previous.product_source_version
                || channel_identity_generation_u64 < previous.channel_identity_generation
            {
                return Err(ProductSalesChannelIndexRelationFreshnessError::WatermarkRegressed);
            }
            if relation_epoch_u64 == previous.relation_epoch
                && product_source_version_u64 == previous.product_source_version
                && visibility_key == previous.visibility_key
                && channel_identity_generation_u64 == previous.channel_identity_generation
            {
                return Ok(
                    ProductSalesChannelIndexRelationFreshnessWriteOutcome::Unchanged(
                        previous.clone(),
                    ),
                );
            }
        }

        let row = transaction
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                INSERT INTO product_sales_channel_index_relation_freshness_snapshots (
                    tenant_id,
                    product_id,
                    relation_epoch,
                    product_source_version,
                    visibility_key,
                    channel_identity_generation
                ) VALUES ($1, $2, $3, $4, $5, $6)
                RETURNING
                    sequence_no,
                    tenant_id,
                    product_id,
                    relation_epoch,
                    product_source_version,
                    visibility_key,
                    channel_identity_generation
                "#,
                vec![
                    tenant_id.into(),
                    product_id.into(),
                    relation_epoch.into(),
                    product_source_version.into(),
                    visibility_key.into(),
                    channel_identity_generation.into(),
                ],
            ))
            .await
            .map_err(|_| ProductSalesChannelIndexRelationFreshnessError::Unavailable)?
            .ok_or(ProductSalesChannelIndexRelationFreshnessError::InvalidStoredState)?;
        let record = decode_record(row, tenant_id)?;

        Ok(if previous.is_some() {
            ProductSalesChannelIndexRelationFreshnessWriteOutcome::Advanced(record)
        } else {
            ProductSalesChannelIndexRelationFreshnessWriteOutcome::Initial(record)
        })
    }
}

async fn require_live_product(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
) -> Result<(), ProductSalesChannelIndexRelationFreshnessError> {
    let row = transaction
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT 1 AS present
            FROM products
            WHERE tenant_id = $1
              AND id = $2
            FOR KEY SHARE
            "#,
            vec![tenant_id.into(), product_id.into()],
        ))
        .await
        .map_err(|_| ProductSalesChannelIndexRelationFreshnessError::Unavailable)?;
    if row.is_none() {
        return Err(ProductSalesChannelIndexRelationFreshnessError::ProductNotFound);
    }
    Ok(())
}

async fn require_current_relation_epoch(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
    relation_epoch: i64,
) -> Result<(), ProductSalesChannelIndexRelationFreshnessError> {
    let row = transaction
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT relation_epoch
            FROM product_sales_channel_index_relation_snapshots
            WHERE tenant_id = $1
              AND product_id = $2
            ORDER BY relation_epoch DESC
            LIMIT 1
            "#,
            vec![tenant_id.into(), product_id.into()],
        ))
        .await
        .map_err(|_| ProductSalesChannelIndexRelationFreshnessError::Unavailable)?
        .ok_or(ProductSalesChannelIndexRelationFreshnessError::RelationNotCurrent)?;
    let current_epoch: i64 = row
        .try_get("", "relation_epoch")
        .map_err(|_| ProductSalesChannelIndexRelationFreshnessError::InvalidStoredState)?;
    if current_epoch <= 0 || current_epoch != relation_epoch {
        return Err(ProductSalesChannelIndexRelationFreshnessError::RelationNotCurrent);
    }
    Ok(())
}

async fn lock_domain(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
    domain: &str,
) -> Result<(), ProductSalesChannelIndexRelationFreshnessError> {
    let lock_key = format!("{tenant_id}\u{1f}{product_id}\u{1f}{domain}");
    transaction
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
            vec![lock_key.into()],
        ))
        .await
        .map_err(|_| ProductSalesChannelIndexRelationFreshnessError::Unavailable)?;
    Ok(())
}

async fn load_latest(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
) -> Result<
    Option<ProductSalesChannelIndexRelationFreshnessRecord>,
    ProductSalesChannelIndexRelationFreshnessError,
> {
    transaction
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT
                sequence_no,
                tenant_id,
                product_id,
                relation_epoch,
                product_source_version,
                visibility_key,
                channel_identity_generation
            FROM product_sales_channel_index_relation_freshness_snapshots
            WHERE tenant_id = $1
              AND product_id = $2
            ORDER BY sequence_no DESC
            LIMIT 1
            "#,
            vec![tenant_id.into(), product_id.into()],
        ))
        .await
        .map_err(|_| ProductSalesChannelIndexRelationFreshnessError::Unavailable)?
        .map(|row| decode_record(row, tenant_id))
        .transpose()
}

fn decode_record(
    row: QueryResult,
    expected_tenant: Uuid,
) -> Result<
    ProductSalesChannelIndexRelationFreshnessRecord,
    ProductSalesChannelIndexRelationFreshnessError,
> {
    let sequence_no: i64 = row
        .try_get("", "sequence_no")
        .map_err(|_| ProductSalesChannelIndexRelationFreshnessError::InvalidStoredState)?;
    let tenant_id: Uuid = row
        .try_get("", "tenant_id")
        .map_err(|_| ProductSalesChannelIndexRelationFreshnessError::InvalidStoredState)?;
    let product_id: Uuid = row
        .try_get("", "product_id")
        .map_err(|_| ProductSalesChannelIndexRelationFreshnessError::InvalidStoredState)?;
    let relation_epoch: i64 = row
        .try_get("", "relation_epoch")
        .map_err(|_| ProductSalesChannelIndexRelationFreshnessError::InvalidStoredState)?;
    let product_source_version: i64 = row
        .try_get("", "product_source_version")
        .map_err(|_| ProductSalesChannelIndexRelationFreshnessError::InvalidStoredState)?;
    let visibility_key: String = row
        .try_get("", "visibility_key")
        .map_err(|_| ProductSalesChannelIndexRelationFreshnessError::InvalidStoredState)?;
    let channel_identity_generation: i64 = row
        .try_get("", "channel_identity_generation")
        .map_err(|_| ProductSalesChannelIndexRelationFreshnessError::InvalidStoredState)?;

    if sequence_no <= 0
        || tenant_id != expected_tenant
        || tenant_id.is_nil()
        || product_id.is_nil()
        || relation_epoch <= 0
        || product_source_version <= 0
        || visibility_key.is_empty()
        || visibility_key.len() > MAX_PRODUCT_SALES_CHANNEL_VISIBILITY_KEY_BYTES
        || channel_identity_generation < 0
    {
        return Err(ProductSalesChannelIndexRelationFreshnessError::InvalidStoredState);
    }

    Ok(ProductSalesChannelIndexRelationFreshnessRecord {
        sequence_no,
        tenant_id,
        product_id,
        relation_epoch: u64::try_from(relation_epoch)
            .map_err(|_| ProductSalesChannelIndexRelationFreshnessError::InvalidStoredState)?,
        product_source_version: u64::try_from(product_source_version)
            .map_err(|_| ProductSalesChannelIndexRelationFreshnessError::InvalidStoredState)?,
        visibility_key,
        channel_identity_generation: u64::try_from(channel_identity_generation)
            .map_err(|_| ProductSalesChannelIndexRelationFreshnessError::InvalidStoredState)?,
    })
}

fn positive_i64(
    value: u64,
    error: ProductSalesChannelIndexRelationFreshnessError,
) -> Result<i64, ProductSalesChannelIndexRelationFreshnessError> {
    if value == 0 {
        return Err(error);
    }
    i64::try_from(value).map_err(|_| error)
}

fn validate_scope(
    tenant_id: Uuid,
    product_id: Uuid,
) -> Result<(), ProductSalesChannelIndexRelationFreshnessError> {
    if tenant_id.is_nil() {
        return Err(ProductSalesChannelIndexRelationFreshnessError::InvalidTenant);
    }
    if product_id.is_nil() {
        return Err(ProductSalesChannelIndexRelationFreshnessError::InvalidProduct);
    }
    Ok(())
}

fn ensure_postgres(
    db: &DatabaseConnection,
) -> Result<(), ProductSalesChannelIndexRelationFreshnessError> {
    if db.get_database_backend() != DatabaseBackend::Postgres {
        return Err(ProductSalesChannelIndexRelationFreshnessError::Unavailable);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freshness_visibility_key_is_bounded() {
        assert_eq!(
            positive_i64(
                0,
                ProductSalesChannelIndexRelationFreshnessError::InvalidRelationEpoch,
            ),
            Err(ProductSalesChannelIndexRelationFreshnessError::InvalidRelationEpoch)
        );
        assert!(
            "x".repeat(MAX_PRODUCT_SALES_CHANNEL_VISIBILITY_KEY_BYTES + 1)
                .len()
                > MAX_PRODUCT_SALES_CHANNEL_VISIBILITY_KEY_BYTES
        );
    }

    #[test]
    fn freshness_uses_membership_lock_before_witness_lock() {
        assert_eq!(RELATION_LOCK_DOMAIN, "product-sales-channel-index-relation");
        assert_eq!(
            FRESHNESS_LOCK_DOMAIN,
            "product-sales-channel-index-relation-freshness"
        );
    }
}
