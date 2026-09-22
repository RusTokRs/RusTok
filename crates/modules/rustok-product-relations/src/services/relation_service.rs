use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    DatabaseTransaction, EntityTrait, Order, QueryFilter, QueryOrder, QuerySelect, Set, Statement,
    TransactionTrait,
};
use uuid::Uuid;
use rustok_product::entities::product;

use crate::dto::{
    CreateProductRelationInput, ProductRelationDto, RelationType, ReorderProductRelationsInput,
    UpdateProductRelationInput,
};
use crate::entities::product_relation::{ActiveModel, Column, Entity as ProductRelation};
use crate::error::{ProductRelationError, ProductRelationResult};
use crate::ports::ProductRelationsPort;

#[derive(Clone)]
pub struct ProductRelationService {
    db: DatabaseConnection,
}

async fn lock_product_for_update(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
) -> ProductRelationResult<product::Model> {
    let query = product::Entity::find_by_id(product_id)
        .filter(product::Column::TenantId.eq(tenant_id));
    let model = match txn.get_database_backend() {
        DatabaseBackend::Postgres | DatabaseBackend::MySql => query.lock_exclusive().one(txn).await?,
        DatabaseBackend::Sqlite => {
            let statement = Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "UPDATE products SET updated_at = updated_at WHERE tenant_id = ?1 AND id = ?2",
                [tenant_id.into(), product_id.into()],
            );
            txn.execute_raw(statement).await?;
            query.one(txn).await?
        }
        _ => query.one(txn).await?,
    };
    model.ok_or(ProductRelationError::ProductNotFound(product_id))
}

async fn lock_products_in_order(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    first_product_id: Uuid,
    second_product_id: Uuid,
) -> ProductRelationResult<()> {
    if first_product_id == second_product_id {
        return Err(ProductRelationError::SelfRelationNotAllowed(first_product_id));
    }
    let (left, right) = if first_product_id < second_product_id {
        (first_product_id, second_product_id)
    } else {
        (second_product_id, first_product_id)
    };
    lock_product_for_update(txn, tenant_id, left).await?;
    lock_product_for_update(txn, tenant_id, right).await?;
    Ok(())
}

impl ProductRelationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
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
            query = query.filter(Column::RelationType.eq(rel_type.as_str()));
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
            query = query.filter(Column::RelationType.eq(rel_type.as_str()));
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
        if input.product_id == input.related_product_id {
            return Err(ProductRelationError::SelfRelationNotAllowed(input.product_id));
        }

        let rel_type_str = input.relation_type.as_str().to_owned();
        let txn = self.db.begin().await?;
        lock_products_in_order(
            &txn,
            tenant_id,
            input.product_id,
            input.related_product_id,
        )
        .await?;

        let exists = ProductRelation::find()
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::ProductId.eq(input.product_id))
            .filter(Column::RelatedProductId.eq(input.related_product_id))
            .filter(Column::RelationType.eq(&rel_type_str))
            .one(&txn)
            .await?;

        if exists.is_some() {
            return Err(ProductRelationError::RelationAlreadyExists {
                product_id: input.product_id,
                related_product_id: input.related_product_id,
                relation_type: rel_type_str,
            });
        }

        let position = match input.position {
            Some(pos) => pos,
            None => {
                let max_pos: Option<i32> = ProductRelation::find()
                    .filter(Column::TenantId.eq(tenant_id))
                    .filter(Column::ProductId.eq(input.product_id))
                    .filter(Column::RelationType.eq(&rel_type_str))
                    .select_only()
                    .column_as(Column::Position.max(), "max_pos")
                    .into_tuple()
                    .one(&txn)
                    .await?
                    .flatten();

                max_pos.map_or(0, |m| m + 1)
            }
        };

        let now = Utc::now();
        let id = Uuid::new_v4();

        let active = ActiveModel {
            id: Set(id),
            tenant_id: Set(tenant_id),
            product_id: Set(input.product_id),
            related_product_id: Set(input.related_product_id),
            relation_type: Set(rel_type_str),
            position: Set(position),
            metadata: Set(input.metadata.unwrap_or_else(|| serde_json::json!({}))),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };

        let model = active.insert(&txn).await?;
        txn.commit().await?;
        Ok(model.into())
    }

    async fn update_relation(
        &self,
        tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        relation_id: Uuid,
        input: UpdateProductRelationInput,
    ) -> ProductRelationResult<ProductRelationDto> {
        let txn = self.db.begin().await?;
        let relation = ProductRelation::find_by_id(relation_id)
            .filter(Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(ProductRelationError::RelationNotFound(relation_id))?;
        lock_product_for_update(&txn, tenant_id, relation.product_id).await?;

        let mut active: ActiveModel = relation.into();
        let now = Utc::now();

        if let Some(pos) = input.position {
            active.position = Set(pos);
        }
        if let Some(meta) = input.metadata {
            active.metadata = Set(meta);
        }
        active.updated_at = Set(now.into());

        let updated = active.update(&txn).await?;
        txn.commit().await?;
        Ok(updated.into())
    }

    async fn delete_relation(
        &self,
        tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        relation_id: Uuid,
    ) -> ProductRelationResult<()> {
        let txn = self.db.begin().await?;
        let model = ProductRelation::find_by_id(relation_id)
            .filter(Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(ProductRelationError::RelationNotFound(relation_id))?;
        lock_product_for_update(&txn, tenant_id, model.product_id).await?;

        ProductRelation::delete_by_id(model.id)
            .exec(&txn)
            .await?;
        txn.commit().await?;
        Ok(())
    }

    async fn reorder_relations(
        &self,
        tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        input: ReorderProductRelationsInput,
    ) -> ProductRelationResult<Vec<ProductRelationDto>> {
        let rel_type_str = input.relation_type.as_str();

        let txn = self.db.begin().await?;
        lock_product_for_update(&txn, tenant_id, input.product_id).await?;

        for (idx, relation_id) in input.ordered_relation_ids.iter().enumerate() {
            let model = ProductRelation::find_by_id(*relation_id)
                .filter(Column::TenantId.eq(tenant_id))
                .filter(Column::ProductId.eq(input.product_id))
                .filter(Column::RelationType.eq(rel_type_str))
                .one(&txn)
                .await?
                .ok_or(ProductRelationError::RelationNotFound(*relation_id))?;

            let mut active: ActiveModel = model.into();
            active.position = Set(idx as i32);
            active.updated_at = Set(Utc::now().into());
            active.update(&txn).await?;
        }

        txn.commit().await?;

        self.list_relations(tenant_id, input.product_id, Some(input.relation_type))
            .await
    }
}
