use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction, DbBackend,
    QueryResult, Statement, TransactionTrait, Value as SqlValue,
};
use serde_json::Value as JsonValue;
use thiserror::Error;
use uuid::Uuid;

pub const MAX_PRODUCT_SALES_CHANNEL_RELATION_CHANNELS: usize = 1024;
pub const MAX_PRODUCT_SALES_CHANNEL_RELATION_PAGE: usize = 256;
pub const MAX_PRODUCT_SALES_CHANNEL_RELATION_TARGETS: usize = 64;

const RELATION_LOCK_DOMAIN: &str = "product-sales-channel-index-relation";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductSalesChannelIndexRelationRecord {
    sequence_no: i64,
    tenant_id: Uuid,
    product_id: Uuid,
    relation_epoch: u64,
    channel_ids: Vec<Uuid>,
}

impl ProductSalesChannelIndexRelationRecord {
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

    pub fn channel_ids(&self) -> &[Uuid] {
        &self.channel_ids
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProductSalesChannelIndexRelationWriteOutcome {
    Initial(ProductSalesChannelIndexRelationRecord),
    Unchanged(ProductSalesChannelIndexRelationRecord),
    Advanced(ProductSalesChannelIndexRelationRecord),
}

impl ProductSalesChannelIndexRelationWriteOutcome {
    pub fn record(&self) -> &ProductSalesChannelIndexRelationRecord {
        match self {
            Self::Initial(record) | Self::Unchanged(record) | Self::Advanced(record) => record,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ProductSalesChannelIndexRelationError {
    #[error("Product-SalesChannel relation tenant identity is invalid")]
    InvalidTenant,
    #[error("Product-SalesChannel relation Product identity is invalid")]
    InvalidProduct,
    #[error("Product-SalesChannel relation Product does not exist")]
    ProductNotFound,
    #[error("Product-SalesChannel relation Channel identity is invalid")]
    InvalidChannel,
    #[error("Product-SalesChannel relation Channel identities must be unique")]
    DuplicateChannel,
    #[error("Product-SalesChannel relation contains {count} channels; maximum is {maximum}")]
    TooManyChannels { count: usize, maximum: usize },
    #[error("Product-SalesChannel relation page request is invalid")]
    InvalidPage,
    #[error("Product-SalesChannel relation cursor is invalid")]
    InvalidCursor,
    #[error("Product-SalesChannel relation targeted load is invalid")]
    InvalidTargets,
    #[error("Product-SalesChannel relation epoch is exhausted")]
    EpochExhausted,
    #[error("Product-SalesChannel relation storage returned invalid state")]
    InvalidStoredState,
    #[error("Product-SalesChannel relation storage is unavailable")]
    Unavailable,
}

/// Product-owned append-only authority for the resolved Product -> SalesChannel membership.
///
/// This store owns only the durable relation epoch and resolved Channel UUID set. It does not read
/// Channel tables, resolve Product metadata slugs, construct Index mutations, publish events, or
/// start a worker. A future cross-owner resolver can submit an already resolved membership and use
/// the returned monotonic epoch as the source version for locale-specific relation snapshots.
#[derive(Clone)]
pub struct ProductSalesChannelIndexRelationStore {
    db: DatabaseConnection,
}

impl ProductSalesChannelIndexRelationStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn replace(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        channel_ids: impl IntoIterator<Item = Uuid>,
    ) -> Result<ProductSalesChannelIndexRelationWriteOutcome, ProductSalesChannelIndexRelationError>
    {
        validate_scope(tenant_id, product_id)?;
        ensure_postgres(&self.db)?;
        let channel_ids = canonical_channel_ids(channel_ids)?;

        let transaction = self
            .db
            .begin()
            .await
            .map_err(|_| ProductSalesChannelIndexRelationError::Unavailable)?;
        let result = self
            .replace_in_transaction(&transaction, tenant_id, product_id, channel_ids)
            .await;

        match result {
            Ok(outcome) => {
                transaction
                    .commit()
                    .await
                    .map_err(|_| ProductSalesChannelIndexRelationError::Unavailable)?;
                Ok(outcome)
            }
            Err(error) => {
                transaction
                    .rollback()
                    .await
                    .map_err(|_| ProductSalesChannelIndexRelationError::Unavailable)?;
                Err(error)
            }
        }
    }

    pub async fn list_changes(
        &self,
        tenant_id: Uuid,
        after_sequence_no: i64,
        limit: usize,
    ) -> Result<Vec<ProductSalesChannelIndexRelationRecord>, ProductSalesChannelIndexRelationError>
    {
        if tenant_id.is_nil() {
            return Err(ProductSalesChannelIndexRelationError::InvalidTenant);
        }
        validate_page(after_sequence_no, limit)?;
        ensure_postgres(&self.db)?;

        self.db
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT sequence_no, tenant_id, product_id, relation_epoch, channel_ids
                FROM product_sales_channel_index_relation_snapshots
                WHERE tenant_id = $1
                  AND sequence_no > $2
                ORDER BY sequence_no ASC
                LIMIT $3
                "#,
                vec![
                    tenant_id.into(),
                    after_sequence_no.into(),
                    i64::try_from(limit)
                        .expect("relation page limit is bounded below i64::MAX")
                        .into(),
                ],
            ))
            .await
            .map_err(|_| ProductSalesChannelIndexRelationError::Unavailable)?
            .into_iter()
            .map(|row| decode_record(row, tenant_id))
            .collect()
    }

    pub async fn scan_current(
        &self,
        tenant_id: Uuid,
        after_product_id: Option<Uuid>,
        limit: usize,
    ) -> Result<Vec<ProductSalesChannelIndexRelationRecord>, ProductSalesChannelIndexRelationError>
    {
        if tenant_id.is_nil() {
            return Err(ProductSalesChannelIndexRelationError::InvalidTenant);
        }
        if after_product_id.is_some_and(|value| value.is_nil()) {
            return Err(ProductSalesChannelIndexRelationError::InvalidCursor);
        }
        if limit == 0 || limit > MAX_PRODUCT_SALES_CHANNEL_RELATION_PAGE {
            return Err(ProductSalesChannelIndexRelationError::InvalidPage);
        }
        ensure_postgres(&self.db)?;

        let limit = i64::try_from(limit).expect("relation page limit is bounded below i64::MAX");
        let (sql, values) = match after_product_id {
            Some(after_product_id) => (
                r#"
                SELECT DISTINCT ON (product_id)
                    sequence_no, tenant_id, product_id, relation_epoch, channel_ids
                FROM product_sales_channel_index_relation_snapshots
                WHERE tenant_id = $1
                  AND product_id > $2
                ORDER BY product_id ASC, relation_epoch DESC
                LIMIT $3
                "#,
                vec![tenant_id.into(), after_product_id.into(), limit.into()],
            ),
            None => (
                r#"
                SELECT DISTINCT ON (product_id)
                    sequence_no, tenant_id, product_id, relation_epoch, channel_ids
                FROM product_sales_channel_index_relation_snapshots
                WHERE tenant_id = $1
                ORDER BY product_id ASC, relation_epoch DESC
                LIMIT $2
                "#,
                vec![tenant_id.into(), limit.into()],
            ),
        };

        self.db
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                sql,
                values,
            ))
            .await
            .map_err(|_| ProductSalesChannelIndexRelationError::Unavailable)?
            .into_iter()
            .map(|row| decode_record(row, tenant_id))
            .collect()
    }

    pub async fn load_current(
        &self,
        tenant_id: Uuid,
        product_ids: impl IntoIterator<Item = Uuid>,
    ) -> Result<Vec<ProductSalesChannelIndexRelationRecord>, ProductSalesChannelIndexRelationError>
    {
        if tenant_id.is_nil() {
            return Err(ProductSalesChannelIndexRelationError::InvalidTenant);
        }
        ensure_postgres(&self.db)?;
        let product_ids = canonical_product_ids(product_ids)?;

        let mut placeholders = Vec::with_capacity(product_ids.len());
        let mut values = Vec::<SqlValue>::with_capacity(1 + product_ids.len());
        values.push(tenant_id.into());
        for (offset, product_id) in product_ids.iter().enumerate() {
            placeholders.push(format!("${}", offset + 2));
            values.push((*product_id).into());
        }
        let sql = format!(
            "SELECT DISTINCT ON (product_id) sequence_no, tenant_id, product_id, relation_epoch, channel_ids FROM product_sales_channel_index_relation_snapshots WHERE tenant_id = $1 AND product_id IN ({}) ORDER BY product_id ASC, relation_epoch DESC",
            placeholders.join(", ")
        );

        self.db
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                sql,
                values,
            ))
            .await
            .map_err(|_| ProductSalesChannelIndexRelationError::Unavailable)?
            .into_iter()
            .map(|row| decode_record(row, tenant_id))
            .collect()
    }

    async fn replace_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        tenant_id: Uuid,
        product_id: Uuid,
        channel_ids: Vec<Uuid>,
    ) -> Result<ProductSalesChannelIndexRelationWriteOutcome, ProductSalesChannelIndexRelationError>
    {
        require_live_product(transaction, tenant_id, product_id).await?;
        lock_relation(transaction, tenant_id, product_id).await?;
        let previous = load_latest(transaction, tenant_id, product_id).await?;
        if let Some(previous) = previous
            .as_ref()
            .filter(|previous| previous.channel_ids == channel_ids)
        {
            return Ok(ProductSalesChannelIndexRelationWriteOutcome::Unchanged(
                previous.clone(),
            ));
        }

        let relation_epoch = match previous.as_ref() {
            Some(previous) => previous
                .relation_epoch
                .checked_add(1)
                .filter(|value| *value <= i64::MAX as u64)
                .ok_or(ProductSalesChannelIndexRelationError::EpochExhausted)?,
            None => 1,
        };
        let relation_epoch_i64 = i64::try_from(relation_epoch)
            .map_err(|_| ProductSalesChannelIndexRelationError::EpochExhausted)?;
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                INSERT INTO product_sales_channel_index_relation_snapshots (
                    tenant_id,
                    product_id,
                    relation_epoch,
                    channel_ids
                ) VALUES ($1, $2, $3, $4)
                RETURNING sequence_no, tenant_id, product_id, relation_epoch, channel_ids
                "#,
                vec![
                    tenant_id.into(),
                    product_id.into(),
                    relation_epoch_i64.into(),
                    SqlValue::Json(Some(Box::new(channel_ids_json(&channel_ids)))),
                ],
            ))
            .await
            .map_err(|_| ProductSalesChannelIndexRelationError::Unavailable)?
            .ok_or(ProductSalesChannelIndexRelationError::InvalidStoredState)?;
        let record = decode_record(row, tenant_id)?;

        Ok(if previous.is_some() {
            ProductSalesChannelIndexRelationWriteOutcome::Advanced(record)
        } else {
            ProductSalesChannelIndexRelationWriteOutcome::Initial(record)
        })
    }
}

async fn require_live_product(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
) -> Result<(), ProductSalesChannelIndexRelationError> {
    let row = transaction
        .query_one_raw(Statement::from_sql_and_values(
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
        .map_err(|_| ProductSalesChannelIndexRelationError::Unavailable)?;
    if row.is_none() {
        return Err(ProductSalesChannelIndexRelationError::ProductNotFound);
    }
    Ok(())
}

async fn lock_relation(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
) -> Result<(), ProductSalesChannelIndexRelationError> {
    let lock_key = relation_lock_key(tenant_id, product_id);
    transaction
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
            vec![lock_key.into()],
        ))
        .await
        .map_err(|_| ProductSalesChannelIndexRelationError::Unavailable)?;
    Ok(())
}

async fn load_latest(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
) -> Result<Option<ProductSalesChannelIndexRelationRecord>, ProductSalesChannelIndexRelationError> {
    transaction
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT sequence_no, tenant_id, product_id, relation_epoch, channel_ids
            FROM product_sales_channel_index_relation_snapshots
            WHERE tenant_id = $1
              AND product_id = $2
            ORDER BY relation_epoch DESC
            LIMIT 1
            "#,
            vec![tenant_id.into(), product_id.into()],
        ))
        .await
        .map_err(|_| ProductSalesChannelIndexRelationError::Unavailable)?
        .map(|row| decode_record(row, tenant_id))
        .transpose()
}

fn decode_record(
    row: QueryResult,
    expected_tenant: Uuid,
) -> Result<ProductSalesChannelIndexRelationRecord, ProductSalesChannelIndexRelationError> {
    let sequence_no: i64 = row
        .try_get("", "sequence_no")
        .map_err(|_| ProductSalesChannelIndexRelationError::InvalidStoredState)?;
    let tenant_id: Uuid = row
        .try_get("", "tenant_id")
        .map_err(|_| ProductSalesChannelIndexRelationError::InvalidStoredState)?;
    let product_id: Uuid = row
        .try_get("", "product_id")
        .map_err(|_| ProductSalesChannelIndexRelationError::InvalidStoredState)?;
    let relation_epoch: i64 = row
        .try_get("", "relation_epoch")
        .map_err(|_| ProductSalesChannelIndexRelationError::InvalidStoredState)?;
    let channel_ids: JsonValue = row
        .try_get("", "channel_ids")
        .map_err(|_| ProductSalesChannelIndexRelationError::InvalidStoredState)?;

    if sequence_no <= 0
        || tenant_id != expected_tenant
        || tenant_id.is_nil()
        || product_id.is_nil()
        || relation_epoch <= 0
    {
        return Err(ProductSalesChannelIndexRelationError::InvalidStoredState);
    }

    Ok(ProductSalesChannelIndexRelationRecord {
        sequence_no,
        tenant_id,
        product_id,
        relation_epoch: u64::try_from(relation_epoch)
            .map_err(|_| ProductSalesChannelIndexRelationError::InvalidStoredState)?,
        channel_ids: decode_channel_ids(channel_ids)?,
    })
}

fn decode_channel_ids(
    value: JsonValue,
) -> Result<Vec<Uuid>, ProductSalesChannelIndexRelationError> {
    let values = value
        .as_array()
        .ok_or(ProductSalesChannelIndexRelationError::InvalidStoredState)?;
    if values.len() > MAX_PRODUCT_SALES_CHANNEL_RELATION_CHANNELS {
        return Err(ProductSalesChannelIndexRelationError::InvalidStoredState);
    }

    let mut decoded = Vec::with_capacity(values.len());
    for value in values {
        let raw = value
            .as_str()
            .ok_or(ProductSalesChannelIndexRelationError::InvalidStoredState)?;
        let channel_id = Uuid::parse_str(raw)
            .map_err(|_| ProductSalesChannelIndexRelationError::InvalidStoredState)?;
        if channel_id.is_nil() || channel_id.to_string() != raw {
            return Err(ProductSalesChannelIndexRelationError::InvalidStoredState);
        }
        if decoded
            .last()
            .is_some_and(|previous| channel_id <= *previous)
        {
            return Err(ProductSalesChannelIndexRelationError::InvalidStoredState);
        }
        decoded.push(channel_id);
    }
    Ok(decoded)
}

fn canonical_channel_ids(
    channel_ids: impl IntoIterator<Item = Uuid>,
) -> Result<Vec<Uuid>, ProductSalesChannelIndexRelationError> {
    let mut channel_ids = channel_ids.into_iter().collect::<Vec<_>>();
    if channel_ids.len() > MAX_PRODUCT_SALES_CHANNEL_RELATION_CHANNELS {
        return Err(ProductSalesChannelIndexRelationError::TooManyChannels {
            count: channel_ids.len(),
            maximum: MAX_PRODUCT_SALES_CHANNEL_RELATION_CHANNELS,
        });
    }
    if channel_ids.iter().any(Uuid::is_nil) {
        return Err(ProductSalesChannelIndexRelationError::InvalidChannel);
    }
    channel_ids.sort_unstable();
    if channel_ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ProductSalesChannelIndexRelationError::DuplicateChannel);
    }
    Ok(channel_ids)
}

fn canonical_product_ids(
    product_ids: impl IntoIterator<Item = Uuid>,
) -> Result<Vec<Uuid>, ProductSalesChannelIndexRelationError> {
    let mut product_ids = product_ids.into_iter().collect::<Vec<_>>();
    if product_ids.is_empty() || product_ids.len() > MAX_PRODUCT_SALES_CHANNEL_RELATION_TARGETS {
        return Err(ProductSalesChannelIndexRelationError::InvalidTargets);
    }
    if product_ids.iter().any(Uuid::is_nil) {
        return Err(ProductSalesChannelIndexRelationError::InvalidTargets);
    }
    product_ids.sort_unstable();
    if product_ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ProductSalesChannelIndexRelationError::InvalidTargets);
    }
    Ok(product_ids)
}

fn channel_ids_json(channel_ids: &[Uuid]) -> JsonValue {
    JsonValue::Array(
        channel_ids
            .iter()
            .map(|channel_id| JsonValue::String(channel_id.to_string()))
            .collect(),
    )
}

fn validate_scope(
    tenant_id: Uuid,
    product_id: Uuid,
) -> Result<(), ProductSalesChannelIndexRelationError> {
    if tenant_id.is_nil() {
        return Err(ProductSalesChannelIndexRelationError::InvalidTenant);
    }
    if product_id.is_nil() {
        return Err(ProductSalesChannelIndexRelationError::InvalidProduct);
    }
    Ok(())
}

fn validate_page(
    after_sequence_no: i64,
    limit: usize,
) -> Result<(), ProductSalesChannelIndexRelationError> {
    if after_sequence_no < 0 || limit == 0 || limit > MAX_PRODUCT_SALES_CHANNEL_RELATION_PAGE {
        return Err(ProductSalesChannelIndexRelationError::InvalidPage);
    }
    Ok(())
}

fn ensure_postgres(db: &DatabaseConnection) -> Result<(), ProductSalesChannelIndexRelationError> {
    if db.get_database_backend() != DatabaseBackend::Postgres {
        return Err(ProductSalesChannelIndexRelationError::Unavailable);
    }
    Ok(())
}

fn relation_lock_key(tenant_id: Uuid, product_id: Uuid) -> String {
    format!("{tenant_id}\u{1f}{product_id}\u{1f}{RELATION_LOCK_DOMAIN}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relation_membership_is_canonical_and_duplicate_ids_fail_closed() {
        let first = Uuid::from_u128(1);
        let second = Uuid::from_u128(2);
        assert_eq!(
            canonical_channel_ids([second, first]).unwrap(),
            vec![first, second]
        );
        assert_eq!(
            canonical_channel_ids([first, first]),
            Err(ProductSalesChannelIndexRelationError::DuplicateChannel)
        );
        assert_eq!(
            canonical_channel_ids([Uuid::nil()]),
            Err(ProductSalesChannelIndexRelationError::InvalidChannel)
        );
    }

    #[test]
    fn relation_membership_and_target_requests_are_bounded() {
        let too_many_channels = (1..=MAX_PRODUCT_SALES_CHANNEL_RELATION_CHANNELS + 1)
            .map(|value| Uuid::from_u128(value as u128));
        assert!(matches!(
            canonical_channel_ids(too_many_channels),
            Err(ProductSalesChannelIndexRelationError::TooManyChannels { .. })
        ));

        let too_many_products = (1..=MAX_PRODUCT_SALES_CHANNEL_RELATION_TARGETS + 1)
            .map(|value| Uuid::from_u128(value as u128));
        assert_eq!(
            canonical_product_ids(too_many_products),
            Err(ProductSalesChannelIndexRelationError::InvalidTargets)
        );
    }

    #[test]
    fn stored_channel_ids_must_be_canonical_uuid_strings() {
        let first = Uuid::from_u128(1);
        let second = Uuid::from_u128(2);
        assert_eq!(
            decode_channel_ids(channel_ids_json(&[first, second])).unwrap(),
            vec![first, second]
        );
        assert_eq!(
            decode_channel_ids(JsonValue::Array(vec![
                JsonValue::String(second.to_string()),
                JsonValue::String(first.to_string()),
            ])),
            Err(ProductSalesChannelIndexRelationError::InvalidStoredState)
        );
    }
}
