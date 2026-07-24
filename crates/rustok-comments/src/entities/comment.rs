use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Expr;
use sea_orm::{QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dto::CommentStatus;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "comments")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub thread_id: Uuid,
    pub author_id: Uuid,
    pub parent_comment_id: Option<Uuid>,
    pub status: CommentStatus,
    pub position: i64,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub deleted_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::comment_thread::Entity",
        from = "Column::ThreadId",
        to = "super::comment_thread::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Thread,
    #[sea_orm(has_many = "super::comment_body::Entity")]
    Bodies,
}

impl Related<super::comment_thread::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Thread.def()
    }
}

impl Related<super::comment_body::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Bodies.def()
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if !insert {
            return Ok(self);
        }

        let tenant_id = self.tenant_id.try_as_ref().copied().ok_or_else(|| {
            DbErr::Custom(
                "comment insert requires tenant_id before position allocation".to_string(),
            )
        })?;
        let thread_id = self.thread_id.try_as_ref().copied().ok_or_else(|| {
            DbErr::Custom(
                "comment insert requires thread_id before position allocation".to_string(),
            )
        })?;

        // A no-op UPDATE takes the database write/row lock for this thread and keeps it
        // until the surrounding transaction completes. Position allocation below is
        // therefore serialized across all normal ActiveModel insert paths.
        let lock = super::comment_thread::Entity::update_many()
            .col_expr(
                super::comment_thread::Column::UpdatedAt,
                Expr::col(super::comment_thread::Column::UpdatedAt).into(),
            )
            .filter(super::comment_thread::Column::Id.eq(thread_id))
            .filter(super::comment_thread::Column::TenantId.eq(tenant_id))
            .exec(db)
            .await?;
        if lock.rows_affected != 1 {
            return Err(DbErr::Custom(format!(
                "comment thread {thread_id} is missing while allocating a position"
            )));
        }

        let next_position = Entity::find()
            .filter(Column::ThreadId.eq(thread_id))
            .order_by_desc(Column::Position)
            .one(db)
            .await?
            .map(|comment| {
                comment.position.checked_add(1).ok_or_else(|| {
                    DbErr::Custom(format!("comment position overflow for thread {thread_id}"))
                })
            })
            .transpose()?
            .unwrap_or(1);
        self.position = Set(next_position);

        Ok(self)
    }
}
