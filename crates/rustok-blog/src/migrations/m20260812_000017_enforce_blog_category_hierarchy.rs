use std::collections::{HashMap, HashSet};

use sea_orm::sea_query::{Expr, Query};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use sea_orm_migration::prelude::*;
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Debug, Clone, Copy)]
struct CategoryNode {
    id: Uuid,
    tenant_id: Uuid,
    parent_id: Option<Uuid>,
    stored_depth: i32,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        let nodes = load_category_nodes(connection).await?;
        let depths = validate_and_compute_depths(&nodes)?;
        backfill_depths(connection, &nodes, &depths).await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_blog_categories_tenant_identity")
                    .table(BlogCategories::Table)
                    .col(BlogCategories::TenantId)
                    .col(BlogCategories::Id)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // SQLite cannot add a foreign-key constraint to an existing table
        // without rebuilding it. Runtime/entity validation still protects the
        // test backend; production PostgreSQL/MySQL get the storage constraint.
        if manager.get_database_backend() != DatabaseBackend::Sqlite {
            manager
                .create_foreign_key(
                    ForeignKey::create()
                        .name("fk_blog_categories_tenant_parent")
                        .from_tbl(BlogCategories::Table)
                        .from_col(BlogCategories::TenantId)
                        .from_col(BlogCategories::ParentId)
                        .to_tbl(BlogCategories::Table)
                        .to_col(BlogCategories::TenantId)
                        .to_col(BlogCategories::Id)
                        .on_update(ForeignKeyAction::Cascade)
                        .on_delete(ForeignKeyAction::Restrict)
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Sqlite {
            manager
                .drop_foreign_key(
                    ForeignKey::drop()
                        .table(BlogCategories::Table)
                        .name("fk_blog_categories_tenant_parent")
                        .to_owned(),
                )
                .await?;
        }

        manager
            .drop_index(
                Index::drop()
                    .name("idx_blog_categories_tenant_identity")
                    .table(BlogCategories::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

async fn load_category_nodes<C>(connection: &C) -> Result<Vec<CategoryNode>, DbErr>
where
    C: ConnectionTrait,
{
    let rows = connection
        .query_all(Statement::from_string(
            connection.get_database_backend(),
            "SELECT id, tenant_id, parent_id, depth FROM blog_categories ORDER BY tenant_id, id"
                .to_string(),
        ))
        .await?;

    rows.into_iter()
        .map(|row| {
            Ok(CategoryNode {
                id: row.try_get("", "id")?,
                tenant_id: row.try_get("", "tenant_id")?,
                parent_id: row.try_get("", "parent_id")?,
                stored_depth: row.try_get("", "depth")?,
            })
        })
        .collect()
}

fn validate_and_compute_depths(nodes: &[CategoryNode]) -> Result<HashMap<Uuid, i32>, DbErr> {
    let by_id = nodes
        .iter()
        .map(|node| (node.id, *node))
        .collect::<HashMap<_, _>>();
    let mut depths = HashMap::with_capacity(nodes.len());
    let mut active_path = HashSet::new();

    for node in nodes {
        compute_depth(node.id, &by_id, &mut depths, &mut active_path)?;
    }

    Ok(depths)
}

fn compute_depth(
    category_id: Uuid,
    nodes: &HashMap<Uuid, CategoryNode>,
    depths: &mut HashMap<Uuid, i32>,
    active_path: &mut HashSet<Uuid>,
) -> Result<i32, DbErr> {
    if let Some(depth) = depths.get(&category_id) {
        return Ok(*depth);
    }
    if !active_path.insert(category_id) {
        return Err(DbErr::Migration(format!(
            "blog category hierarchy contains a cycle at category {category_id}"
        )));
    }

    let node = nodes.get(&category_id).ok_or_else(|| {
        DbErr::Migration(format!(
            "blog category hierarchy references missing category {category_id}"
        ))
    })?;
    let depth = match node.parent_id {
        None => 0,
        Some(parent_id) => {
            let parent = nodes.get(&parent_id).ok_or_else(|| {
                DbErr::Migration(format!(
                    "blog category {} references missing parent {parent_id}",
                    node.id
                ))
            })?;
            if parent.tenant_id != node.tenant_id {
                return Err(DbErr::Migration(format!(
                    "blog category {} in tenant {} references parent {parent_id} in tenant {}",
                    node.id, node.tenant_id, parent.tenant_id
                )));
            }
            compute_depth(parent_id, nodes, depths, active_path)?
                .checked_add(1)
                .ok_or_else(|| {
                    DbErr::Migration(format!(
                        "blog category hierarchy depth is exhausted beneath parent {parent_id}"
                    ))
                })?
        }
    };

    active_path.remove(&category_id);
    depths.insert(category_id, depth);
    Ok(depth)
}

async fn backfill_depths<C>(
    connection: &C,
    nodes: &[CategoryNode],
    depths: &HashMap<Uuid, i32>,
) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    for node in nodes {
        let depth = *depths.get(&node.id).ok_or_else(|| {
            DbErr::Migration(format!(
                "blog category depth was not computed for category {}",
                node.id
            ))
        })?;
        if depth == node.stored_depth {
            continue;
        }

        let mut update = Query::update();
        update
            .table(BlogCategories::Table)
            .value(BlogCategories::Depth, depth)
            .and_where(Expr::col(BlogCategories::Id).eq(node.id))
            .and_where(Expr::col(BlogCategories::TenantId).eq(node.tenant_id));
        let result = connection
            .execute(connection.get_database_backend().build(&update))
            .await?;
        if result.rows_affected() != 1 {
            return Err(DbErr::Migration(format!(
                "blog category {} changed while hierarchy depth was being backfilled",
                node.id
            )));
        }
    }

    Ok(())
}

#[derive(DeriveIden)]
enum BlogCategories {
    Table,
    Id,
    TenantId,
    ParentId,
    Depth,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: u128, tenant: u128, parent: Option<u128>, stored_depth: i32) -> CategoryNode {
        CategoryNode {
            id: Uuid::from_u128(id),
            tenant_id: Uuid::from_u128(tenant),
            parent_id: parent.map(Uuid::from_u128),
            stored_depth,
        }
    }

    #[test]
    fn computes_depth_for_valid_tree_independently_of_stored_depth() {
        let nodes = [
            node(1, 10, None, 99),
            node(2, 10, Some(1), 0),
            node(3, 10, Some(2), -4),
        ];
        let depths = validate_and_compute_depths(&nodes).expect("valid tree");

        assert_eq!(depths[&Uuid::from_u128(1)], 0);
        assert_eq!(depths[&Uuid::from_u128(2)], 1);
        assert_eq!(depths[&Uuid::from_u128(3)], 2);
    }

    #[test]
    fn rejects_missing_parent() {
        let error = validate_and_compute_depths(&[node(2, 10, Some(1), 0)])
            .expect_err("orphan must block migration");
        assert!(error.to_string().contains("missing parent"));
    }

    #[test]
    fn rejects_cross_tenant_parent() {
        let error = validate_and_compute_depths(&[node(1, 10, None, 0), node(2, 11, Some(1), 0)])
            .expect_err("foreign parent must block migration");
        assert!(error.to_string().contains("references parent"));
    }

    #[test]
    fn rejects_cycle() {
        let error =
            validate_and_compute_depths(&[node(1, 10, Some(2), 0), node(2, 10, Some(1), 0)])
                .expect_err("cycle must block migration");
        assert!(error.to_string().contains("cycle"));
    }
}
