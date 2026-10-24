use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use chrono::Utc;
use rustok_product::entities::product;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, Order,
    QueryFilter, QueryOrder, Set, TransactionTrait,
};
use uuid::Uuid;

use crate::dto::{
    CreateProductRelationInput, ProductRelationDto, RelationType, ReorderProductRelationsInput,
    UpdateProductRelationInput,
};
use crate::entities::product_relation::{
    ActiveModel, Column, Entity as ProductRelation, Model as ProductRelationModel,
};
use crate::error::{ProductRelationError, ProductRelationResult};
use crate::ports::ProductRelationsPort;

#[derive(Clone)]
pub struct ProductRelationService {
    db: DatabaseConnection,
}

async fn verify_product_exists(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
) -> ProductRelationResult<()> {
    let exists = product::Entity::find_by_id(product_id)
        .filter(product::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?
        .is_some();
    if !exists {
        return Err(ProductRelationError::ProductNotFound(product_id));
    }
    Ok(())
}

async fn verify_products_exist_in_order(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    first_product_id: Uuid,
    second_product_id: Uuid,
) -> ProductRelationResult<()> {
    if first_product_id == second_product_id {
        return Err(ProductRelationError::SelfRelationNotAllowed(
            first_product_id,
        ));
    }
    let (left, right) = if first_product_id < second_product_id {
        (first_product_id, second_product_id)
    } else {
        (second_product_id, first_product_id)
    };
    verify_product_exists(txn, tenant_id, left).await?;
    verify_product_exists(txn, tenant_id, right).await?;
    Ok(())
}

async fn relation_group_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
    relation_type: &str,
) -> ProductRelationResult<Vec<ProductRelationModel>> {
    Ok(ProductRelation::find()
        .filter(Column::TenantId.eq(tenant_id))
        .filter(Column::ProductId.eq(product_id))
        .filter(Column::RelationType.eq(relation_type))
        .order_by(Column::Position, Order::Asc)
        .order_by(Column::CreatedAt, Order::Asc)
        .order_by(Column::Id, Order::Asc)
        .all(txn)
        .await?)
}

/// Resolves a relation's owner, takes the owner Product lock, then reloads the relation.
///
/// Relation writes serialize on their source Product. The reload closes the read-before-lock
/// window: a concurrent delete may have removed the relation while this transaction waited for
/// that Product lock, in which case callers receive the stable not-found result rather than
/// updating or deleting a stale model.
async fn lock_relation_for_mutation(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    relation_id: Uuid,
) -> ProductRelationResult<ProductRelationModel> {
    ProductRelation::find_by_id(relation_id)
        .filter(Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?
        .ok_or(ProductRelationError::RelationNotFound(relation_id))
}

fn relation_metadata_is_database_safe(metadata: &serde_json::Value) -> bool {
    match metadata {
        serde_json::Value::String(text) => !text.contains('\0'),
        serde_json::Value::Array(values) => values.iter().all(relation_metadata_is_database_safe),
        serde_json::Value::Object(values) => values
            .iter()
            .all(|(key, value)| !key.contains('\0') && relation_metadata_is_database_safe(value)),
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => true,
    }
}

fn validate_relation_metadata(metadata: &serde_json::Value) -> ProductRelationResult<()> {
    if relation_metadata_is_database_safe(metadata) {
        Ok(())
    } else {
        Err(ProductRelationError::InvalidInput(
            "relation metadata cannot contain a NUL character".to_owned(),
        ))
    }
}

fn checked_position(position: i32, maximum: usize) -> ProductRelationResult<usize> {
    let position = usize::try_from(position)
        .map_err(|_| ProductRelationError::PositionOutOfRange { position, maximum })?;
    if position > maximum {
        return Err(ProductRelationError::PositionOutOfRange {
            position: i32::try_from(position).unwrap_or(i32::MAX),
            maximum,
        });
    }
    Ok(position)
}

fn checked_position_from_index(index: usize) -> ProductRelationResult<i32> {
    i32::try_from(index).map_err(|_| {
        ProductRelationError::InvalidInput("too many relations in one ordered group".to_owned())
    })
}

/// Assigns a complete, caller-specified order without ever exposing duplicate positions to the
/// unique database constraint. Callers must hold the owning Product lock for the relation group.
async fn assign_relation_positions_in_tx(
    txn: &DatabaseTransaction,
    assignments: &[(ProductRelationModel, i32)],
) -> ProductRelationResult<()> {
    if assignments.is_empty() {
        return Ok(());
    }

    let count = i32::try_from(assignments.len()).map_err(|_| {
        ProductRelationError::InvalidInput("too many relations in one ordered group".to_owned())
    })?;
    let max_position = assignments
        .iter()
        .map(|(model, _)| model.position.max(0))
        .max()
        .unwrap_or_default();
    let temporary_base = max_position
        .checked_add(count)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| {
            ProductRelationError::InvalidInput(
                "relation positions exceed the supported range".to_owned(),
            )
        })?;

    let now = Utc::now();
    for (offset, (model, _)) in assignments.iter().enumerate() {
        let temporary_position = temporary_base
            .checked_add(checked_position_from_index(offset)?)
            .ok_or_else(|| {
                ProductRelationError::InvalidInput(
                    "relation positions exceed the supported range".to_owned(),
                )
            })?;
        let mut active: ActiveModel = model.clone().into();
        active.position = Set(temporary_position);
        active.updated_at = Set(now.into());
        active.update(txn).await?;
    }

    for (model, position) in assignments {
        let mut active: ActiveModel = model.clone().into();
        active.position = Set(*position);
        active.updated_at = Set(now.into());
        active.update(txn).await?;
    }

    Ok(())
}

async fn normalize_relation_group_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
    relation_type: &str,
    ordered_models: Vec<ProductRelationModel>,
) -> ProductRelationResult<Vec<ProductRelationModel>> {
    let mut assignments = Vec::with_capacity(ordered_models.len());
    for (index, model) in ordered_models.into_iter().enumerate() {
        assignments.push((model, checked_position_from_index(index)?));
    }
    assign_relation_positions_in_tx(txn, &assignments).await?;
    relation_group_in_tx(txn, tenant_id, product_id, relation_type).await
}

impl ProductRelationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub(crate) fn database(&self) -> &DatabaseConnection {
        &self.db
    }
}

/// Creates one relation inside the caller transaction.
///
/// Both the plain owner port method and the receipt-bound owner command use this exact body, so an
/// idempotent retry can never diverge from the non-idempotent write path.
pub(crate) async fn create_relation_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    input: CreateProductRelationInput,
) -> ProductRelationResult<ProductRelationDto> {
    let CreateProductRelationInput {
        product_id,
        related_product_id,
        relation_type,
        position: requested_position,
        metadata,
    } = input;
    if product_id == related_product_id {
        return Err(ProductRelationError::SelfRelationNotAllowed(product_id));
    }

    let rel_type_str = relation_type.storage_key()?;
    let metadata = metadata.unwrap_or_else(|| serde_json::json!({}));
    validate_relation_metadata(&metadata)?;
    verify_products_exist_in_order(txn, tenant_id, product_id, related_product_id).await?;

    let exists = ProductRelation::find()
        .filter(Column::TenantId.eq(tenant_id))
        .filter(Column::ProductId.eq(product_id))
        .filter(Column::RelatedProductId.eq(related_product_id))
        .filter(Column::RelationType.eq(&rel_type_str))
        .one(txn)
        .await?;

    if exists.is_some() {
        return Err(ProductRelationError::RelationAlreadyExists {
            product_id,
            related_product_id,
            relation_type: rel_type_str,
        });
    }

    let existing_group =
        relation_group_in_tx(txn, tenant_id, product_id, rel_type_str.as_str()).await?;
    let insertion_index = match requested_position {
        Some(position) => checked_position(position, existing_group.len())?,
        None => existing_group.len(),
    };

    // Make room for the new relation while retaining an unambiguous, contiguous order even when
    // data created before the positional uniqueness migration contained gaps.
    let mut assignments = Vec::with_capacity(existing_group.len());
    for (index, model) in existing_group.into_iter().enumerate() {
        let target_index = if index < insertion_index {
            index
        } else {
            index + 1
        };
        assignments.push((model, checked_position_from_index(target_index)?));
    }
    assign_relation_positions_in_tx(txn, &assignments).await?;
    let position = checked_position_from_index(insertion_index)?;

    let now = Utc::now();
    let id = Uuid::new_v4();

    let active = ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        product_id: Set(product_id),
        related_product_id: Set(related_product_id),
        relation_type: Set(rel_type_str),
        position: Set(position),
        metadata: Set(metadata),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    };

    let model = active.insert(txn).await?;
    Ok(model.into())
}

/// Updates one relation inside the caller transaction.
///
/// The caller owns transaction completion and may therefore pair this mutation with a durable
/// idempotency receipt without exposing a post-commit response race.
pub(crate) async fn update_relation_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    relation_id: Uuid,
    input: UpdateProductRelationInput,
) -> ProductRelationResult<ProductRelationDto> {
    let relation = lock_relation_for_mutation(txn, tenant_id, relation_id).await?;
    let UpdateProductRelationInput {
        position: requested_position,
        metadata,
    } = input;
    // Reject malformed data before position normalization can issue any writes. Receipt-bound
    // callers explicitly roll back on later storage failures, while this keeps validation
    // failures write-free for every caller.
    if let Some(metadata) = &metadata {
        validate_relation_metadata(metadata)?;
    }

    let mut updated = relation.clone();
    if let Some(position) = requested_position {
        let mut ordered = relation_group_in_tx(
            txn,
            tenant_id,
            relation.product_id,
            relation.relation_type.as_str(),
        )
        .await?;
        let current_index = ordered
            .iter()
            .position(|model| model.id == relation_id)
            .ok_or(ProductRelationError::RelationNotFound(relation_id))?;
        let relation_to_move = ordered.remove(current_index);
        let target_index = checked_position(position, ordered.len())?;
        ordered.insert(target_index, relation_to_move);

        let normalized = normalize_relation_group_in_tx(
            txn,
            tenant_id,
            relation.product_id,
            relation.relation_type.as_str(),
            ordered,
        )
        .await?;
        updated = normalized
            .into_iter()
            .find(|model| model.id == relation_id)
            .ok_or(ProductRelationError::RelationNotFound(relation_id))?;
    }

    if let Some(metadata) = metadata {
        let mut active: ActiveModel = updated.into();
        active.metadata = Set(metadata);
        active.updated_at = Set(Utc::now().into());
        updated = active.update(txn).await?;
    } else if requested_position.is_none() {
        let mut active: ActiveModel = updated.into();
        active.updated_at = Set(Utc::now().into());
        updated = active.update(txn).await?;
    }

    Ok(updated.into())
}

/// Deletes one relation and closes its ordered group inside the caller transaction.
pub(crate) async fn delete_relation_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    relation_id: Uuid,
) -> ProductRelationResult<()> {
    // Resolve, lock, and reload so a concurrent deletion that won the source-Product lock is
    // reported as not found rather than becoming a successful zero-row delete.
    let model = lock_relation_for_mutation(txn, tenant_id, relation_id).await?;
    let delete = ProductRelation::delete_by_id(model.id).exec(txn).await?;
    if delete.rows_affected != 1 {
        return Err(ProductRelationError::RelationNotFound(relation_id));
    }
    let remaining = relation_group_in_tx(
        txn,
        tenant_id,
        model.product_id,
        model.relation_type.as_str(),
    )
    .await?;
    normalize_relation_group_in_tx(
        txn,
        tenant_id,
        model.product_id,
        model.relation_type.as_str(),
        remaining,
    )
    .await?;

    Ok(())
}

/// Reorders a complete relation group inside the caller transaction.
pub(crate) async fn reorder_relations_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    input: ReorderProductRelationsInput,
) -> ProductRelationResult<Vec<ProductRelationDto>> {
    let ReorderProductRelationsInput {
        product_id,
        relation_type,
        ordered_relation_ids,
    } = input;
    let rel_type_str = relation_type.storage_key()?;

    verify_product_exists(txn, tenant_id, product_id).await?;
    let existing = relation_group_in_tx(txn, tenant_id, product_id, rel_type_str.as_str()).await?;

    let expected_ids: HashSet<Uuid> = existing.iter().map(|model| model.id).collect();
    let mut seen_ids = HashSet::with_capacity(ordered_relation_ids.len());
    for relation_id in &ordered_relation_ids {
        if !expected_ids.contains(relation_id) {
            return Err(ProductRelationError::RelationNotFound(*relation_id));
        }
        if !seen_ids.insert(*relation_id) {
            return Err(ProductRelationError::DuplicateRelationInReorder(
                *relation_id,
            ));
        }
    }
    if seen_ids.len() != expected_ids.len() {
        return Err(ProductRelationError::IncompleteReorder {
            expected: expected_ids.len(),
            received: seen_ids.len(),
        });
    }

    let mut by_id: HashMap<Uuid, ProductRelationModel> = existing
        .into_iter()
        .map(|model| (model.id, model))
        .collect();
    let mut ordered = Vec::with_capacity(ordered_relation_ids.len());
    for relation_id in ordered_relation_ids {
        // The exact-set validation above makes absence impossible; retain a typed result if
        // storage changes unexpectedly before this point.
        ordered.push(
            by_id
                .remove(&relation_id)
                .ok_or(ProductRelationError::RelationNotFound(relation_id))?,
        );
    }

    let normalized =
        normalize_relation_group_in_tx(txn, tenant_id, product_id, rel_type_str.as_str(), ordered)
            .await?;

    Ok(normalized.into_iter().map(Into::into).collect())
}

impl From<crate::entities::product_relation::Model> for ProductRelationDto {
    fn from(model: crate::entities::product_relation::Model) -> Self {
        let relation_type = model
            .relation_type
            .as_str()
            .parse()
            .unwrap_or(RelationType::Custom(model.relation_type));
        Self {
            id: model.id,
            tenant_id: model.tenant_id,
            product_id: model.product_id,
            related_product_id: model.related_product_id,
            relation_type,
            position: model.position,
            metadata: model.metadata,
            created_at: model.created_at.into(),
            updated_at: model.updated_at.into(),
        }
    }
}

#[async_trait]
impl ProductRelationsPort for ProductRelationService {
    async fn list_relations(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        relation_type: Option<RelationType>,
    ) -> ProductRelationResult<Vec<ProductRelationDto>> {
        let mut query = ProductRelation::find()
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::ProductId.eq(product_id));

        if let Some(rel_type) = relation_type {
            query = query.filter(Column::RelationType.eq(rel_type.storage_key()?));
        }

        let models = query
            .order_by(Column::Position, Order::Asc)
            .order_by(Column::CreatedAt, Order::Asc)
            .all(&self.db)
            .await?;

        Ok(models.into_iter().map(Into::into).collect())
    }

    async fn list_reverse_relations(
        &self,
        tenant_id: Uuid,
        related_product_id: Uuid,
        relation_type: Option<RelationType>,
    ) -> ProductRelationResult<Vec<ProductRelationDto>> {
        let mut query = ProductRelation::find()
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::RelatedProductId.eq(related_product_id));

        if let Some(rel_type) = relation_type {
            query = query.filter(Column::RelationType.eq(rel_type.storage_key()?));
        }

        let models = query
            .order_by(Column::Position, Order::Asc)
            .order_by(Column::CreatedAt, Order::Asc)
            .all(&self.db)
            .await?;

        Ok(models.into_iter().map(Into::into).collect())
    }

    async fn get_relation(
        &self,
        tenant_id: Uuid,
        relation_id: Uuid,
    ) -> ProductRelationResult<ProductRelationDto> {
        let model = ProductRelation::find_by_id(relation_id)
            .filter(Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ProductRelationError::RelationNotFound(relation_id))?;

        Ok(model.into())
    }

    async fn create_relation(
        &self,
        tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        input: CreateProductRelationInput,
    ) -> ProductRelationResult<ProductRelationDto> {
        let txn = self.db.begin().await?;
        let relation = create_relation_in_tx(&txn, tenant_id, input).await?;
        txn.commit().await?;
        Ok(relation)
    }

    async fn update_relation(
        &self,
        tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        relation_id: Uuid,
        input: UpdateProductRelationInput,
    ) -> ProductRelationResult<ProductRelationDto> {
        let txn = self.db.begin().await?;
        let relation = update_relation_in_tx(&txn, tenant_id, relation_id, input).await?;
        txn.commit().await?;
        Ok(relation)
    }

    async fn delete_relation(
        &self,
        tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        relation_id: Uuid,
    ) -> ProductRelationResult<()> {
        let txn = self.db.begin().await?;
        delete_relation_in_tx(&txn, tenant_id, relation_id).await?;
        txn.commit().await?;
        Ok(())
    }

    async fn reorder_relations(
        &self,
        tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        input: ReorderProductRelationsInput,
    ) -> ProductRelationResult<Vec<ProductRelationDto>> {
        let txn = self.db.begin().await?;
        let relations = reorder_relations_in_tx(&txn, tenant_id, input).await?;
        txn.commit().await?;
        Ok(relations)
    }
}

#[cfg(test)]
mod tests {
    use super::relation_metadata_is_database_safe;

    #[test]
    fn relation_metadata_rejects_nul_in_values_and_keys() {
        assert!(!relation_metadata_is_database_safe(&serde_json::json!({
            "key": "\0"
        })));
        assert!(!relation_metadata_is_database_safe(&serde_json::json!({
            "\0": "value"
        })));
        assert!(relation_metadata_is_database_safe(&serde_json::json!({
            "key": ["safe", 1]
        })));
    }
}
