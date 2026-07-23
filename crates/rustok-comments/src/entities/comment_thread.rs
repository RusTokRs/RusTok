use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Expr;
use sea_orm::{PaginatorTrait, QueryFilter};
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
            return Ok(self);
        }

        let thread_id = self.id.try_as_ref().copied().ok_or_else(|| {
            DbErr::Custom("comment thread update requires id".to_string())
        })?;
        let tenant_id = self.tenant_id.try_as_ref().copied().ok_or_else(|| {
            DbErr::Custom("comment thread update requires tenant_id".to_string())
        })?;

        // Serialize every thread mutation on the owner row before deriving the
        // denormalized count. This replaces stale read-modify-write counters with
        // an exact count taken inside the caller's transaction.
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
