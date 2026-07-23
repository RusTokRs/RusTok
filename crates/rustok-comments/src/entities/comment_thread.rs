use chrono::Utc;
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::{Expr, OnConflict};
use sea_orm::{ActiveValue, PaginatorTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dto::CommentThreadStatus;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "comment_threads")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub target_type: String,
    pub target_id: Uuid,
    pub status: CommentThreadStatus,
    pub comment_count: i32,
    pub last_commented_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::comment::Entity")]
    Comments,
}

impl Related<super::comment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Comments.def()
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if insert {
            serialize_thread_identity(db, &self).await?;
            return Ok(self);
        }

        // Status, timestamps, and other thread metadata updates must not start a
        // counter write. The owner recomputation is activated only by the
        // explicit counter refresh performed after a comment insert/delete.
        if !matches!(&self.comment_count, ActiveValue::Set(_)) {
            return Ok(self);
        }

        let thread_id = self.id.try_as_ref().copied().ok_or_else(|| {
            DbErr::Custom("comment thread update requires id".to_string())
        })?;
        let tenant_id = self.tenant_id.try_as_ref().copied().ok_or_else(|| {
            DbErr::Custom("comment thread update requires tenant_id".to_string())
        })?;

        // Serialize explicit counter refreshes on the owner row before deriving
        // the denormalized count. Service create/delete paths call this inside
        // their surrounding database transaction.
        let lock = Entity::update_many()
            .col_expr(Column::UpdatedAt, Expr::col(Column::UpdatedAt).into())
            .filter(Column::Id.eq(thread_id))
            .filter(Column::TenantId.eq(tenant_id))
            .exec(db)
            .await?;
        if lock.rows_affected != 1 {
            return Err(DbErr::Custom(format!(
                "comment thread {thread_id} is missing while refreshing counters"
            )));
        }

        let count = super::comment::Entity::find()
            .filter(super::comment::Column::TenantId.eq(tenant_id))
            .filter(super::comment::Column::ThreadId.eq(thread_id))
            .filter(super::comment::Column::DeletedAt.is_null())
            .count(db)
            .await?;
        let count = i32::try_from(count).map_err(|_| {
            DbErr::Custom(format!(
                "comment count exceeds i32 capacity for thread {thread_id}"
            ))
        })?;
        self.comment_count = Set(count);

        Ok(self)
    }
}

async fn serialize_thread_identity<C>(db: &C, model: &ActiveModel) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    let tenant_id = model.tenant_id.try_as_ref().copied().ok_or_else(|| {
        DbErr::Custom("comment thread insert requires tenant_id".to_string())
    })?;
    let target_type = model.target_type.try_as_ref().cloned().ok_or_else(|| {
        DbErr::Custom("comment thread insert requires target_type".to_string())
    })?;
    let target_id = model.target_id.try_as_ref().copied().ok_or_else(|| {
        DbErr::Custom("comment thread insert requires target_id".to_string())
    })?;

    use super::comment_thread_identity_lock as identity_lock;

    identity_lock::Entity::insert(identity_lock::ActiveModel {
        tenant_id: Set(tenant_id),
        target_type: Set(target_type.clone()),
        target_id: Set(target_id),
        created_at: Set(Utc::now().into()),
    })
    .on_conflict(
        OnConflict::columns([
            identity_lock::Column::TenantId,
            identity_lock::Column::TargetType,
            identity_lock::Column::TargetId,
        ])
        .do_nothing()
        .to_owned(),
    )
    .exec_without_returning(db)
    .await?;

    // The no-op update acquires the persistent identity row lock. In an explicit
    // transaction, a concurrent creator cannot pass this point until the first
    // creator commits or rolls back.
    let identity_row = identity_lock::Entity::update_many()
        .col_expr(
            identity_lock::Column::CreatedAt,
            Expr::col(identity_lock::Column::CreatedAt).into(),
        )
        .filter(identity_lock::Column::TenantId.eq(tenant_id))
        .filter(identity_lock::Column::TargetType.eq(target_type.clone()))
        .filter(identity_lock::Column::TargetId.eq(target_id))
        .exec(db)
        .await?;
    if identity_row.rows_affected != 1 {
        return Err(DbErr::Custom(format!(
            "comment thread identity lock is missing for {target_type}:{target_id}"
        )));
    }

    if let Some(existing) = Entity::find()
        .filter(Column::TenantId.eq(tenant_id))
        .filter(Column::TargetType.eq(target_type.clone()))
        .filter(Column::TargetId.eq(target_id))
        .one(db)
        .await?
    {
        // Returning an application DbErr before the INSERT statement keeps the
        // surrounding PostgreSQL transaction usable. The service's existing
        // find-or-create fallback then loads this canonical thread.
        return Err(DbErr::Custom(format!(
            "comment thread identity {target_type}:{target_id} already belongs to {}",
            existing.id
        )));
    }

    Ok(())
}
