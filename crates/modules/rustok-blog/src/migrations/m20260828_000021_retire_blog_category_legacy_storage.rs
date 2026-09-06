use std::collections::HashMap;

use rustok_taxonomy::{TaxonomyScopeType, TaxonomyTermKind, entities::taxonomy_term};
use sea_orm::{ColumnTrait, DatabaseBackend, EntityTrait, QueryFilter};
use sea_orm_migration::prelude::*;

use crate::entities::{blog_category, blog_category_taxonomy_binding};

const BLOG_SCOPE_VALUE: &str = "blog";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres | DatabaseBackend::Sqlite => {}
            backend => {
                return Err(DbErr::Custom(format!(
                    "Blog Category legacy-storage retirement does not support {backend:?}",
                )));
            }
        }

        ensure_complete_taxonomy_ownership(manager).await?;

        manager
            .drop_table(
                Table::drop()
                    .table(BlogCategoryTranslations::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(BlogTranslationChanges::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Intentionally irreversible. Taxonomy owns canonical Blog Category
        // localized copy before this migration runs. Recreating empty donor
        // tables would falsely imply that rollback can restore retired data.
        Ok(())
    }
}

async fn ensure_complete_taxonomy_ownership(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    let categories = blog_category::Entity::find().all(connection).await?;
    if categories.is_empty() {
        return Ok(());
    }

    let bindings = blog_category_taxonomy_binding::Entity::find()
        .all(connection)
        .await?;
    let binding_by_blog = bindings
        .into_iter()
        .map(|binding| {
            (
                (binding.tenant_id, binding.blog_category_id),
                binding.taxonomy_category_id,
            )
        })
        .collect::<HashMap<_, _>>();

    let taxonomy_ids = categories
        .iter()
        .map(|category| {
            binding_by_blog
                .get(&(category.tenant_id, category.id))
                .copied()
                .ok_or_else(|| {
                    DbErr::Migration(format!(
                        "Blog Category legacy-storage retirement blocked: category {} in tenant {} has no Taxonomy binding",
                        category.id, category.tenant_id,
                    ))
                })
                .and_then(|taxonomy_id| {
                    if taxonomy_id == category.id {
                        Ok(taxonomy_id)
                    } else {
                        Err(DbErr::Migration(format!(
                            "Blog Category legacy-storage retirement blocked: category {} is bound to non-canonical Taxonomy UUID {taxonomy_id}",
                            category.id,
                        )))
                    }
                })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let terms = taxonomy_term::Entity::find()
        .filter(taxonomy_term::Column::Id.is_in(taxonomy_ids))
        .all(connection)
        .await?;
    let term_by_id = terms
        .into_iter()
        .map(|term| (term.id, term))
        .collect::<HashMap<_, _>>();

    for category in categories {
        let taxonomy_id = binding_by_blog[&(category.tenant_id, category.id)];
        let term = term_by_id.get(&taxonomy_id).ok_or_else(|| {
            DbErr::Migration(format!(
                "Blog Category legacy-storage retirement blocked: Taxonomy Category {taxonomy_id} is missing for Blog category {}",
                category.id,
            ))
        })?;
        if term.tenant_id != category.tenant_id
            || term.kind != TaxonomyTermKind::Category
            || term.scope_type != TaxonomyScopeType::Module
            || term.scope_value != BLOG_SCOPE_VALUE
        {
            return Err(DbErr::Migration(format!(
                "Blog Category legacy-storage retirement blocked: Taxonomy term {taxonomy_id} has incompatible tenant/kind/scope ownership",
            )));
        }
    }

    Ok(())
}

#[derive(Iden)]
enum BlogCategoryTranslations {
    #[iden = "blog_category_translations"]
    Table,
}

#[derive(Iden)]
enum BlogTranslationChanges {
    #[iden = "blog_translation_changes"]
    Table,
}
