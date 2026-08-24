use std::collections::HashMap;

use rustok_taxonomy::{
    TaxonomyScopeType, TaxonomyTermKind,
    entities::taxonomy_term,
};
use sea_orm::{ColumnTrait, DatabaseBackend, EntityTrait, QueryFilter};
use sea_orm_migration::prelude::*;

use crate::entities::{forum_category, forum_category_taxonomy_binding};

const FORUM_SCOPE_VALUE: &str = "forum";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres | DatabaseBackend::Sqlite => {}
            backend => {
                return Err(DbErr::Custom(format!(
                    "Forum Category legacy-storage retirement does not support {backend:?}",
                )));
            }
        }

        ensure_complete_taxonomy_ownership(manager).await?;
        drop_legacy_route_triggers(manager).await?;

        manager
            .drop_table(
                Table::drop()
                    .table(ForumCategoryRouteAliases::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(ForumCategoryTranslations::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(ForumTranslationChanges::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        if manager.get_database_backend() == DatabaseBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
DROP FUNCTION IF EXISTS forum_guard_category_route_alias_insert();
DROP FUNCTION IF EXISTS forum_guard_category_translation_route_alias();
DROP FUNCTION IF EXISTS forum_reject_category_route_alias_mutation();
"#,
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Intentionally irreversible. By the time this migration runs, Taxonomy
        // is the canonical owner and Forum has stopped reading/writing these
        // tables. Recreating empty donor tables would suggest that rollback can
        // recover stale localized copy/alias/evidence data when it cannot.
        Ok(())
    }
}

async fn ensure_complete_taxonomy_ownership(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    let categories = forum_category::Entity::find().all(connection).await?;
    if categories.is_empty() {
        return Ok(());
    }

    let bindings = forum_category_taxonomy_binding::Entity::find()
        .all(connection)
        .await?;
    let binding_by_forum = bindings
        .into_iter()
        .map(|binding| {
            (
                (binding.tenant_id, binding.forum_category_id),
                binding.taxonomy_category_id,
            )
        })
        .collect::<HashMap<_, _>>();

    let taxonomy_ids = categories
        .iter()
        .map(|category| {
            binding_by_forum
                .get(&(category.tenant_id, category.id))
                .copied()
                .ok_or_else(|| {
                    DbErr::Migration(format!(
                        "Forum Category legacy-storage retirement blocked: category {} in tenant {} has no Taxonomy binding",
                        category.id, category.tenant_id,
                    ))
                })
                .and_then(|taxonomy_id| {
                    if taxonomy_id == category.id {
                        Ok(taxonomy_id)
                    } else {
                        Err(DbErr::Migration(format!(
                            "Forum Category legacy-storage retirement blocked: category {} is bound to non-canonical Taxonomy UUID {taxonomy_id}",
                            category.id,
                        )))
                    }
                })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let terms = taxonomy_term::Entity::find()
        .filter(taxonomy_term::Column::Id.is_in(taxonomy_ids.clone()))
        .all(connection)
        .await?;
    let term_by_id = terms
        .into_iter()
        .map(|term| (term.id, term))
        .collect::<HashMap<_, _>>();

    for category in categories {
        let taxonomy_id = binding_by_forum[&(category.tenant_id, category.id)];
        let term = term_by_id.get(&taxonomy_id).ok_or_else(|| {
            DbErr::Migration(format!(
                "Forum Category legacy-storage retirement blocked: Taxonomy Category {taxonomy_id} is missing for Forum category {}",
                category.id,
            ))
        })?;
        if term.tenant_id != category.tenant_id
            || term.kind != TaxonomyTermKind::Category
            || term.scope_type != TaxonomyScopeType::Module
            || term.scope_value != FORUM_SCOPE_VALUE
        {
            return Err(DbErr::Migration(format!(
                "Forum Category legacy-storage retirement blocked: Taxonomy term {taxonomy_id} has incompatible tenant/kind/scope ownership",
            )));
        }
    }

    Ok(())
}

async fn drop_legacy_route_triggers(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let sql = match manager.get_database_backend() {
        DatabaseBackend::Postgres => {
            r#"
DROP TRIGGER IF EXISTS forum_category_route_alias_insert_guard
    ON forum_category_route_aliases;
DROP TRIGGER IF EXISTS forum_category_translation_route_alias_guard
    ON forum_category_translations;
DROP TRIGGER IF EXISTS forum_category_route_alias_delete
    ON forum_category_route_aliases;
DROP TRIGGER IF EXISTS forum_category_route_alias_update
    ON forum_category_route_aliases;
"#
        }
        DatabaseBackend::Sqlite => {
            r#"
DROP TRIGGER IF EXISTS forum_category_route_alias_insert_guard;
DROP TRIGGER IF EXISTS forum_category_translation_route_alias_update_guard;
DROP TRIGGER IF EXISTS forum_category_translation_route_alias_insert_guard;
DROP TRIGGER IF EXISTS forum_category_route_alias_delete;
DROP TRIGGER IF EXISTS forum_category_route_alias_update;
"#
        }
        backend => {
            return Err(DbErr::Custom(format!(
                "Forum Category legacy-storage trigger retirement does not support {backend:?}",
            )));
        }
    };

    manager
        .get_connection()
        .execute_unprepared(sql)
        .await
        .map(|_| ())
}

#[derive(Iden)]
enum ForumCategoryTranslations {
    #[iden = "forum_category_translations"]
    Table,
}

#[derive(Iden)]
enum ForumCategoryRouteAliases {
    #[iden = "forum_category_route_aliases"]
    Table,
}

#[derive(Iden)]
enum ForumTranslationChanges {
    #[iden = "forum_translation_changes"]
    Table,
}
