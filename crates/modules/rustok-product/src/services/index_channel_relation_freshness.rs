use sea_orm::{ConnectionTrait, DatabaseTransaction, DbBackend, Statement};
use uuid::Uuid;

use super::index_channel_relation::{
    ProductSalesChannelIndexRelationError as RelationError, decode_channel_ids,
};

pub const MAX_PRODUCT_SALES_CHANNEL_VISIBILITY_KEY_BYTES: usize = 131_072;

pub(super) struct ProductSalesChannelIndexRelationWitness {
    pub(super) relation_epoch: u64,
    pub(super) product_source_version: i64,
    pub(super) visibility_key: String,
    pub(super) channel_identity_generation: i64,
    pub(super) channel_ids: Vec<Uuid>,
}

/// Call while holding the Product row and relation locks. The storage trigger
/// reacquires those locks and takes the freshness lock before checking the witness.
pub(super) async fn append_witness(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
    relation_epoch: u64,
    product_source_version: i64,
    visibility_key: String,
    channel_identity_generation: i64,
) -> Result<(), RelationError> {
    let relation_epoch =
        i64::try_from(relation_epoch).map_err(|_| RelationError::InvalidStoredState)?;
    transaction
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            INSERT INTO product_sales_channel_index_relation_freshness_snapshots (
                tenant_id, product_id, relation_epoch, product_source_version,
                visibility_key, channel_identity_generation
            ) VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            [
                tenant_id.into(), product_id.into(), relation_epoch.into(),
                product_source_version.into(), visibility_key.into(), channel_identity_generation.into(),
            ],
        ))
        .await
        .map_err(|error| {
            tracing::warn!(%error, %tenant_id, %product_id, "Product-SalesChannel witness insert failed");
            RelationError::Unavailable
        })?;
    Ok(())
}

pub(super) async fn load_latest_witness(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
) -> Result<Option<ProductSalesChannelIndexRelationWitness>, RelationError> {
    let row = transaction
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT witness.relation_epoch, witness.product_source_version,
                   witness.visibility_key, witness.channel_identity_generation, relation.channel_ids
            FROM product_sales_channel_index_relation_freshness_snapshots witness
            JOIN product_sales_channel_index_relation_snapshots relation
              ON relation.tenant_id = witness.tenant_id AND relation.product_id = witness.product_id
             AND relation.relation_epoch = witness.relation_epoch
            WHERE witness.tenant_id = $1 AND witness.product_id = $2
            ORDER BY witness.sequence_no DESC
            LIMIT 1
            "#,
            [tenant_id.into(), product_id.into()],
        ))
        .await
        .map_err(|_| RelationError::Unavailable)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let invalid = |_| RelationError::InvalidStoredState;
    let relation_epoch: i64 = row.try_get("", "relation_epoch").map_err(invalid)?;
    let product_source_version: i64 = row.try_get("", "product_source_version").map_err(invalid)?;
    let visibility_key: String = row.try_get("", "visibility_key").map_err(invalid)?;
    let channel_identity_generation: i64 = row
        .try_get("", "channel_identity_generation")
        .map_err(invalid)?;
    if relation_epoch <= 0
        || product_source_version <= 0
        || channel_identity_generation < 0
        || visibility_key.is_empty()
        || visibility_key.len() > MAX_PRODUCT_SALES_CHANNEL_VISIBILITY_KEY_BYTES
    {
        return Err(RelationError::InvalidStoredState);
    }
    Ok(Some(ProductSalesChannelIndexRelationWitness {
        relation_epoch: u64::try_from(relation_epoch)
            .map_err(|_| RelationError::InvalidStoredState)?,
        product_source_version,
        visibility_key,
        channel_identity_generation,
        channel_ids: decode_channel_ids(row.try_get("", "channel_ids").map_err(invalid)?)?,
    }))
}
