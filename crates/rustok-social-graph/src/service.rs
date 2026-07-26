use std::collections::BTreeSet;

use chrono::Utc;
use sea_orm::prelude::DateTimeWithTimeZone;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, DatabaseConnection,
    DatabaseTransaction, EntityTrait, QueryFilter, TransactionTrait,
};
use uuid::Uuid;

use crate::entities::relation;
use crate::error::{SocialGraphError, SocialGraphResult};
use crate::model::SocialRelationKind;
use crate::receipts::{
    self, SocialGraphCommandReceiptAdmission, SocialGraphCommandReceiptRequest,
};

#[derive(Clone, Debug)]
pub struct SocialGraphService {
    db: DatabaseConnection,
}

impl SocialGraphService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn relation_state(
        &self,
        tenant_id: Uuid,
        source_user_id: Uuid,
        target_user_id: Uuid,
        relation_kind: SocialRelationKind,
    ) -> SocialGraphResult<Option<relation::Model>> {
        Ok(relation::Entity::find()
            .filter(relation::Column::TenantId.eq(tenant_id))
            .filter(relation::Column::SourceUserId.eq(source_user_id))
            .filter(relation::Column::TargetUserId.eq(target_user_id))
            .filter(relation::Column::RelationKind.eq(relation_kind))
            .one(&self.db)
            .await?)
    }

    pub async fn set_relation_state(
        &self,
        tenant_id: Uuid,
        source_user_id: Uuid,
        target_user_id: Uuid,
        relation_kind: SocialRelationKind,
        active: bool,
        expected_revision: Option<i64>,
    ) -> SocialGraphResult<relation::Model> {
        validate_pair(source_user_id, target_user_id)?;
        let txn = self.db.begin().await?;
        let model = self
            .set_relation_state_in(
                &txn,
                tenant_id,
                source_user_id,
                target_user_id,
                relation_kind,
                active,
                expected_revision,
            )
            .await?;
        txn.commit().await?;
        Ok(model)
    }

    pub async fn set_relation_state_with_receipt(
        &self,
        tenant_id: Uuid,
        source_user_id: Uuid,
        target_user_id: Uuid,
        relation_kind: SocialRelationKind,
        active: bool,
        expected_revision: Option<i64>,
        idempotency_key: String,
    ) -> SocialGraphResult<relation::Model> {
        validate_pair(source_user_id, target_user_id)?;
        let request = SocialGraphCommandReceiptRequest {
            source_user_id,
            target_user_id,
            relation_kind,
            active,
            expected_revision,
        };

        match receipts::admit(&self.db, tenant_id, idempotency_key, &request).await? {
            SocialGraphCommandReceiptAdmission::Replay(receipt) => {
                receipts::replay(receipt, &request)
            }
            SocialGraphCommandReceiptAdmission::New(receipt) => {
                let result = self
                    .set_relation_state_in(
                        &receipt.transaction,
                        tenant_id,
                        source_user_id,
                        target_user_id,
                        relation_kind,
                        active,
                        expected_revision,
                    )
                    .await;
                match result {
                    Ok(model) => receipts::complete(receipt, &model).await,
                    Err(error) => receipts::rollback(receipt, error).await,
                }
            }
        }
    }

    async fn set_relation_state_in(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        source_user_id: Uuid,
        target_user_id: Uuid,
        relation_kind: SocialRelationKind,
        active: bool,
        expected_revision: Option<i64>,
    ) -> SocialGraphResult<relation::Model> {
        let existing = relation::Entity::find()
            .filter(relation::Column::TenantId.eq(tenant_id))
            .filter(relation::Column::SourceUserId.eq(source_user_id))
            .filter(relation::Column::TargetUserId.eq(target_user_id))
            .filter(relation::Column::RelationKind.eq(relation_kind))
            .one(txn)
            .await?;

        let model = match existing {
            Some(existing) => {
                if expected_revision.is_some_and(|expected| expected != existing.revision) {
                    return Err(SocialGraphError::RevisionConflict);
                }
                if existing.active == active {
                    return Ok(existing);
                }

                let next_revision = existing
                    .revision
                    .checked_add(1)
                    .ok_or(SocialGraphError::RevisionConflict)?;
                let now: DateTimeWithTimeZone = Utc::now().into();
                let updated = relation::Entity::update_many()
                    .col_expr(relation::Column::Active, Expr::value(active))
                    .col_expr(relation::Column::Revision, Expr::value(next_revision))
                    .col_expr(relation::Column::UpdatedAt, Expr::value(now))
                    .filter(relation::Column::Id.eq(existing.id))
                    .filter(relation::Column::Revision.eq(existing.revision))
                    .exec(txn)
                    .await?;
                if updated.rows_affected != 1 {
                    return Err(SocialGraphError::RevisionConflict);
                }

                relation::Entity::find_by_id(existing.id)
                    .one(txn)
                    .await?
                    .ok_or(SocialGraphError::RevisionConflict)?
            }
            None => {
                if expected_revision.is_some() {
                    return Err(SocialGraphError::RevisionConflict);
                }
                let now: DateTimeWithTimeZone = Utc::now().into();
                relation::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    tenant_id: Set(tenant_id),
                    source_user_id: Set(source_user_id),
                    target_user_id: Set(target_user_id),
                    relation_kind: Set(relation_kind),
                    active: Set(active),
                    revision: Set(1),
                    created_at: Set(now),
                    updated_at: Set(now),
                }
                .insert(txn)
                .await?
            }
        };

        Ok(model)
    }

    pub async fn blocks_between(
        &self,
        tenant_id: Uuid,
        left_user_id: Uuid,
        right_user_id: Uuid,
    ) -> SocialGraphResult<bool> {
        validate_pair(left_user_id, right_user_id)?;
        Ok(relation::Entity::find()
            .filter(relation::Column::TenantId.eq(tenant_id))
            .filter(relation::Column::RelationKind.eq(SocialRelationKind::Block))
            .filter(relation::Column::Active.eq(true))
            .filter(
                Condition::any()
                    .add(
                        Condition::all()
                            .add(relation::Column::SourceUserId.eq(left_user_id))
                            .add(relation::Column::TargetUserId.eq(right_user_id)),
                    )
                    .add(
                        Condition::all()
                            .add(relation::Column::SourceUserId.eq(right_user_id))
                            .add(relation::Column::TargetUserId.eq(left_user_id)),
                    ),
            )
            .one(&self.db)
            .await?
            .is_some())
    }

    pub async fn source_mutes_target(
        &self,
        tenant_id: Uuid,
        source_user_id: Uuid,
        target_user_id: Uuid,
    ) -> SocialGraphResult<bool> {
        validate_pair(source_user_id, target_user_id)?;
        Ok(relation::Entity::find()
            .filter(relation::Column::TenantId.eq(tenant_id))
            .filter(relation::Column::SourceUserId.eq(source_user_id))
            .filter(relation::Column::TargetUserId.eq(target_user_id))
            .filter(relation::Column::RelationKind.eq(SocialRelationKind::Mute))
            .filter(relation::Column::Active.eq(true))
            .one(&self.db)
            .await?
            .is_some())
    }

    pub async fn source_follows_target(
        &self,
        tenant_id: Uuid,
        source_user_id: Uuid,
        target_user_id: Uuid,
    ) -> SocialGraphResult<bool> {
        Ok(self
            .source_follows_targets(tenant_id, source_user_id, &[target_user_id])
            .await?
            .contains(&target_user_id))
    }

    pub async fn source_follows_targets(
        &self,
        tenant_id: Uuid,
        source_user_id: Uuid,
        target_user_ids: &[Uuid],
    ) -> SocialGraphResult<Vec<Uuid>> {
        let target_user_ids = target_user_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        for target_user_id in &target_user_ids {
            validate_pair(source_user_id, *target_user_id)?;
        }
        if target_user_ids.is_empty() {
            return Ok(Vec::new());
        }

        let followed = relation::Entity::find()
            .filter(relation::Column::TenantId.eq(tenant_id))
            .filter(relation::Column::SourceUserId.eq(source_user_id))
            .filter(relation::Column::TargetUserId.is_in(target_user_ids))
            .filter(relation::Column::RelationKind.eq(SocialRelationKind::Follow))
            .filter(relation::Column::Active.eq(true))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|relation| relation.target_user_id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();

        Ok(followed)
    }
}

fn validate_pair(source_user_id: Uuid, target_user_id: Uuid) -> SocialGraphResult<()> {
    if source_user_id == target_user_id {
        return Err(SocialGraphError::SelfRelation);
    }
    Ok(())
}
