use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, Order, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait,
};
use uuid::Uuid;

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

impl ProductRelationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

impl From<crate::entities::product_relation::Model> for ProductRelationDto {
    fn from(model: crate::entities::product_relation::Model) -> Self {
        let relation_type = model
            .relation_type
            .parse()
            .unwrap_or_else(|_| RelationType::Custom(model.relation_type));
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

        let exists = ProductRelation::find()
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::ProductId.eq(input.product_id))
            .filter(Column::RelatedProductId.eq(input.related_product_id))
            .filter(Column::RelationType.eq(&rel_type_str))
            .one(&self.db)
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
                    .one(&self.db)
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

        let model = active.insert(&self.db).await?;
        Ok(model.into())
    }

    async fn update_relation(
        &self,
        tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        relation_id: Uuid,
        input: UpdateProductRelationInput,
    ) -> ProductRelationResult<ProductRelationDto> {
        let model = ProductRelation::find_by_id(relation_id)
            .filter(Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ProductRelationError::RelationNotFound(relation_id))?;

        let mut active: ActiveModel = model.into();
        let now = Utc::now();

        if let Some(pos) = input.position {
            active.position = Set(pos);
        }
        if let Some(meta) = input.metadata {
            active.metadata = Set(meta);
        }
        active.updated_at = Set(now.into());

        let updated = active.update(&self.db).await?;
        Ok(updated.into())
    }

    async fn delete_relation(
        &self,
        tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        relation_id: Uuid,
    ) -> ProductRelationResult<()> {
        let model = ProductRelation::find_by_id(relation_id)
            .filter(Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ProductRelationError::RelationNotFound(relation_id))?;

        ProductRelation::delete_by_id(model.id)
            .exec(&self.db)
            .await?;
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
