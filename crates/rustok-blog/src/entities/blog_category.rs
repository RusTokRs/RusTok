use sea_orm::entity::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, Set, Statement};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "blog_categories")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub position: i32,
    pub depth: i32,
    pub post_count: i32,
    pub settings: Json,
    pub revision: i64,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if !insert {
            return Ok(self);
        }

        let tenant_id =
            self.tenant_id.try_as_ref().copied().ok_or_else(|| {
                DbErr::Custom("blog category insert requires tenant_id".to_string())
            })?;
        lock_category_tree_for_insert(db, tenant_id).await?;
        let parent_id = self.parent_id.try_as_ref().copied().flatten();

        let depth = match parent_id {
            None => 0,
            Some(parent_id) => {
                let parent = Entity::find_by_id(parent_id)
                    .filter(Column::TenantId.eq(tenant_id))
                    .one(db)
                    .await?
                    .ok_or_else(|| {
                        DbErr::Custom(format!(
                            "blog category parent {parent_id} is missing from tenant {tenant_id}"
                        ))
                    })?;
                child_depth(parent.depth, parent_id)?
            }
        };
        self.depth = Set(depth);

        Ok(self)
    }
}

async fn lock_category_tree_for_insert<C>(db: &C, tenant_id: Uuid) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    if db.get_database_backend() == DatabaseBackend::Postgres {
        db.execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
            [format!("blog-category-tree:{tenant_id}").into()],
        ))
        .await?;
    }
    Ok(())
}

fn child_depth(parent_depth: i32, parent_id: Uuid) -> Result<i32, DbErr> {
    if parent_depth < 0 {
        return Err(DbErr::Custom(format!(
            "blog category parent {parent_id} has invalid negative depth {parent_depth}"
        )));
    }
    parent_depth.checked_add(1).ok_or_else(|| {
        DbErr::Custom(format!(
            "blog category hierarchy depth is exhausted beneath parent {parent_id}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_depth_is_parent_depth_plus_one() {
        let parent_id = Uuid::from_u128(1);
        assert_eq!(child_depth(0, parent_id).expect("depth"), 1);
        assert_eq!(child_depth(7, parent_id).expect("depth"), 8);
    }

    #[test]
    fn child_depth_rejects_invalid_or_exhausted_parent_depth() {
        let parent_id = Uuid::from_u128(1);
        assert!(child_depth(-1, parent_id).is_err());
        assert!(child_depth(i32::MAX, parent_id).is_err());
    }
}
