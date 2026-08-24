use rustok_taxonomy::{TaxonomyError, TaxonomyOwnerCategoryReader, TaxonomyScopeType};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::entities::{forum_category, forum_category_taxonomy_binding};

impl CategoryService {
    pub const MAX_FORUM_CATEGORY_LOCALE_ENUMERATION_IDS: usize = 512;

    pub async fn available_locales_for_categories(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        category_ids: &[Uuid],
    ) -> ForumResult<Vec<(Uuid, Vec<String>)>> {
        if security.is_public_read() {
            return Err(ForumError::forbidden(
                "Forum category locale enumeration requires an authenticated operator context",
            ));
        }
        enforce_scope(&security, Resource::ForumCategories, Action::Manage)?;
        if tenant_id.is_nil() {
            return Err(ForumError::Validation(
                "Forum category locale enumeration requires a non-nil tenant id".to_string(),
            ));
        }
        if category_ids.len() > Self::MAX_FORUM_CATEGORY_LOCALE_ENUMERATION_IDS {
            return Err(ForumError::Validation(format!(
                "Forum category locale enumeration is limited to {} category IDs",
                Self::MAX_FORUM_CATEGORY_LOCALE_ENUMERATION_IDS
            )));
        }
        if category_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut seen = std::collections::BTreeSet::new();
        for category_id in category_ids {
            if category_id.is_nil() {
                return Err(ForumError::Validation(
                    "Forum category locale enumeration requires non-nil category IDs".to_string(),
                ));
            }
            if !seen.insert(*category_id) {
                return Err(ForumError::Validation(format!(
                    "Forum category locale enumeration repeats category {category_id}"
                )));
            }
        }

        let existing = forum_category::Entity::find()
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .filter(forum_category::Column::Id.is_in(category_ids.to_vec()))
            .all(&self.db)
            .await?;
        let existing_ids = existing
            .into_iter()
            .map(|category| category.id)
            .collect::<std::collections::BTreeSet<_>>();
        for category_id in category_ids {
            if !existing_ids.contains(category_id) {
                return Err(ForumError::CategoryNotFound(*category_id));
            }
        }

        let bindings = forum_category_taxonomy_binding::Entity::find()
            .filter(forum_category_taxonomy_binding::Column::TenantId.eq(tenant_id))
            .filter(
                forum_category_taxonomy_binding::Column::ForumCategoryId
                    .is_in(category_ids.to_vec()),
            )
            .all(&self.db)
            .await?;
        let taxonomy_by_forum = bindings
            .into_iter()
            .map(|binding| (binding.forum_category_id, binding.taxonomy_category_id))
            .collect::<std::collections::HashMap<_, _>>();
        let mut taxonomy_ids = Vec::with_capacity(category_ids.len());
        for category_id in category_ids {
            taxonomy_ids.push(*taxonomy_by_forum.get(category_id).ok_or_else(|| {
                ForumError::Validation(format!(
                    "Forum category {category_id} has no Taxonomy Category binding"
                ))
            })?);
        }

        let projections = TaxonomyOwnerCategoryReader::new(self.db.clone())
            .load_scoped_categories(
                tenant_id,
                TaxonomyScopeType::Module,
                Some("forum"),
                Some(&taxonomy_ids),
                rustok_api::PLATFORM_FALLBACK_LOCALE,
                None,
            )
            .await
            .map_err(|error| match error {
                TaxonomyError::Database(error) => ForumError::Database(error),
                other => ForumError::Validation(format!(
                    "Forum Taxonomy category locale enumeration failed: {other}"
                )),
            })?;
        let locales_by_taxonomy = projections
            .into_iter()
            .map(|projection| (projection.id, projection.available_locales))
            .collect::<std::collections::HashMap<_, _>>();

        let mut result = Vec::with_capacity(category_ids.len());
        for category_id in category_ids {
            let taxonomy_id = taxonomy_by_forum[category_id];
            let locales = locales_by_taxonomy.get(&taxonomy_id).cloned().ok_or_else(|| {
                ForumError::Validation(format!(
                    "Forum category {category_id} Taxonomy Category {taxonomy_id} projection is missing"
                ))
            })?;
            if locales.is_empty() {
                return Err(ForumError::Validation(format!(
                    "Forum category {category_id} has no Taxonomy-owned locale translation"
                )));
            }
            result.push((*category_id, locales));
        }

        Ok(result)
    }
}
