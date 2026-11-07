impl CategoryService {
    pub(crate) async fn insert_import_category_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        record: &crate::import_write_preparation::ForumPreparedImportCategory,
    ) -> ForumResult<()> {
        if tenant_id.is_nil() || record.id.is_nil() {
            return Err(ForumError::Validation(
                "Forum import category insert requires non-nil tenant and category IDs".to_string(),
            ));
        }
        validate_category_name(&record.name)?;
        let locale = normalize_locale(&record.locale)?;
        if locale != record.locale {
            return Err(ForumError::Validation(
                "Forum import category locale must already be normalized".to_string(),
            ));
        }
        let slug = normalize_required_slug(&record.slug)?;
        if slug != record.slug {
            return Err(ForumError::Validation(
                "Forum import category slug must already be owner-normalized".to_string(),
            ));
        }
        if record.position < 0 {
            return Err(ForumError::Validation(
                "Forum import category position cannot be negative".to_string(),
            ));
        }
        let created_at = chrono::DateTime::<Utc>::from_timestamp_millis(record.created_at_ms)
            .ok_or_else(|| {
                ForumError::Validation(
                    "Forum import category creation timestamp is outside owner range".to_string(),
                )
            })?;

        rustok_taxonomy::lock_category_hierarchy_writer_in_tx(txn, tenant_id)
            .await
            .map_err(taxonomy_sync::map_taxonomy_error)?;
        if let Some(parent_id) = record.parent_id {
            Self::find_category_in_tx(txn, tenant_id, parent_id).await?;
        }
        forum_category::ActiveModel {
            id: Set(record.id),
            tenant_id: Set(tenant_id),
            moderated: Set(record.moderated),
            topic_count: Set(0),
            reply_count: Set(0),
            created_at: Set(created_at.into()),
            updated_at: Set(created_at.into()),
        }
        .insert(txn)
        .await?;

        taxonomy_sync::sync_category_copy_in_tx(
            txn,
            tenant_id,
            record.id,
            record.parent_id,
            record.position,
            record.icon.clone(),
            record.color.clone(),
            locale,
            record.name.clone(),
            slug,
            record.description.clone(),
        )
        .await?;

        taxonomy_sync::shift_category_siblings_for_insert_in_tx(
            txn,
            tenant_id,
            record.id,
            record.parent_id,
            record.position,
        )
        .await?;

        Ok(())
    }
}

use chrono::Utc;
use sea_orm::{
    sea_query::{Expr, ExprTrait},
    ActiveModelTrait,
    ActiveValue::Set,
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction,
    EntityTrait, QueryFilter, QueryOrder, QuerySelect, Statement, TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_content::normalize_locale_code;
use rustok_core::SecurityContext;

use crate::dto::{CreateCategoryInput, UpdateCategoryInput};
use crate::entities::forum_category;
use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;

fn validate_category_name(name: &str) -> ForumResult<()> {
    if name.trim().is_empty() {
        return Err(ForumError::Validation(
            "Category name cannot be empty".to_string(),
        ));
    }
    Ok(())
}

fn normalize_locale(locale: &str) -> ForumResult<String> {
    normalize_locale_code(locale)
        .ok_or_else(|| ForumError::Validation("Invalid locale".to_string()))
}

fn normalize_required_slug(value: &str) -> ForumResult<String> {
    let slug = normalize_slug(value);
    if slug.is_empty() {
        return Err(ForumError::Validation(
            "Category slug cannot be empty".to_string(),
        ));
    }
    Ok(slug)
}

fn normalize_slug(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut previous_dash = false;
    for ch in value.chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
            previous_dash = false;
        } else if !previous_dash {
            normalized.push('-');
            previous_dash = true;
        }
    }
    normalized.trim_matches('-').to_string()
}


use crate::dto::MAX_FORUM_CATEGORY_TREE_NODES;

/// Crate-private persistence seam shared by Forum category owner commands.
///
/// Canonical Category reads and localized copy no longer live here: public reads
/// are Taxonomy-backed and public mutations are owned by
/// `CategoryProjectionOwnerService`. This type remains only for transaction
/// helpers reused by topic/reply/import owners.
pub(super) struct CategoryService;

impl CategoryService {
    pub(crate) async fn load_categories_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
    ) -> ForumResult<Vec<forum_category::Model>> {
        let categories = forum_category::Entity::find()
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .order_by_asc(forum_category::Column::Id)
            .limit(MAX_FORUM_CATEGORY_TREE_NODES + 1)
            .all(txn)
            .await?;
        if categories.len() > MAX_FORUM_CATEGORY_TREE_NODES as usize {
            return Err(ForumError::Validation(format!(
                "Forum category tree exceeds the bounded limit of {MAX_FORUM_CATEGORY_TREE_NODES} nodes"
            )));
        }
        Ok(categories)
    }

    pub(crate) async fn ensure_exists_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> ForumResult<()> {
        Self::find_category_in_tx(txn, tenant_id, category_id).await?;
        Ok(())
    }

    pub(crate) async fn find_category_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> ForumResult<forum_category::Model> {
        if tenant_id.is_nil() {
            return Err(ForumError::Validation(
                "Tenant ID must not be nil".to_string(),
            ));
        }
        if category_id.is_nil() {
            return Err(ForumError::Validation(
                "Category ID must not be nil".to_string(),
            ));
        }
        let existing = forum_category::Entity::find_by_id(category_id)
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .one(txn)
            .await?;
        existing.ok_or(ForumError::CategoryNotFound(category_id))
    }

    pub(crate) async fn find_category_for_update_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> ForumResult<forum_category::Model> {
        if tenant_id.is_nil() {
            return Err(ForumError::Validation(
                "Tenant ID must not be nil".to_string(),
            ));
        }
        if category_id.is_nil() {
            return Err(ForumError::Validation(
                "Category ID must not be nil".to_string(),
            ));
        }
        let query = forum_category::Entity::find_by_id(category_id)
            .filter(forum_category::Column::TenantId.eq(tenant_id));
        match txn.get_database_backend() {
            DatabaseBackend::Postgres => query
                .lock_exclusive()
                .one(txn)
                .await?
                .ok_or(ForumError::CategoryNotFound(category_id)),
            DatabaseBackend::Sqlite => {
                let statement = Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    "UPDATE forum_categories SET updated_at = updated_at WHERE tenant_id = ?1 AND id = ?2",
                    vec![tenant_id.into(), category_id.into()],
                );
                if txn.execute_raw(statement).await?.rows_affected() != 1 {
                    return Err(ForumError::CategoryNotFound(category_id));
                }
                query
                    .one(txn)
                    .await?
                    .ok_or(ForumError::CategoryNotFound(category_id))
            }
            backend => Err(ForumError::Validation(format!(
                "Forum category row locking does not support database backend {backend:?}"
            ))),
        }
    }

    pub(crate) async fn adjust_counters_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        topic_delta: i32,
        reply_delta: i32,
    ) -> ForumResult<()> {
        let now = Utc::now();
        let mut query = forum_category::Entity::update_many()
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .filter(forum_category::Column::Id.eq(category_id))
            .filter(forum_category::Column::TopicCount.gte(0))
            .filter(forum_category::Column::ReplyCount.gte(0));

        for (column, delta, label) in [
            (forum_category::Column::TopicCount, topic_delta, "topic"),
            (forum_category::Column::ReplyCount, reply_delta, "reply"),
        ] {
            if delta > 0 {
                query = query.col_expr(column, Expr::col(column).add(delta));
            } else if delta < 0 {
                let decrement = delta.checked_abs().ok_or_else(|| {
                    ForumError::Validation(format!("Forum category {label} counter delta overflow"))
                })?;
                query = query
                    .filter(column.gte(decrement))
                    .col_expr(column, Expr::col(column).sub(decrement));
            }
        }

        let updated = query
            .col_expr(forum_category::Column::UpdatedAt, Expr::val(now))
            .exec(txn)
            .await?;

        if updated.rows_affected != 1 {
            let exists = forum_category::Entity::find_by_id(category_id)
                .filter(forum_category::Column::TenantId.eq(tenant_id))
                .one(txn)
                .await?
                .is_some();
            if !exists {
                return Err(ForumError::CategoryNotFound(category_id));
            }
            return Err(ForumError::Validation(
                "Forum category counters do not permit the requested delta".to_string(),
            ));
        }

        Ok(())
    }
}

/// Transactional owner facade for canonical Forum Category mutations.
///
/// Forum owns membership, policy/counters and command authorization; Taxonomy
/// owns canonical localized copy, routes, hierarchy and presentation. Public
/// reads are composed separately through Taxonomy-backed read adapters.
pub(super) struct CategoryProjectionOwnerService {
    db: DatabaseConnection,
}

impl CategoryProjectionOwnerService {
    pub(super) fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[instrument(skip(self, security, input))]
    pub(super) async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateCategoryInput,
    ) -> ForumResult<Uuid> {
        enforce_scope(&security, Resource::ForumCategories, Action::Create)?;
        validate_category_name(&input.name)?;
        let locale = normalize_locale(&input.locale)?;
        let slug = normalize_required_slug(&input.slug)?;
        let canonical_name = input.name.clone();
        let canonical_description = input.description.clone();
        let requested_position = input.position.unwrap_or(0);
        if requested_position < 0 {
            return Err(ForumError::Validation(
                "Category position cannot be negative".to_string(),
            ));
        }

        let now = Utc::now();
        let id = Uuid::new_v4();
        let txn = self.db.begin().await?;
        rustok_taxonomy::lock_category_hierarchy_writer_in_tx(&txn, tenant_id)
            .await
            .map_err(taxonomy_sync::map_taxonomy_error)?;

        if let Some(parent_id) = input.parent_id {
            CategoryService::find_category_in_tx(&txn, tenant_id, parent_id).await?;
            super::category_lifecycle::ensure_category_tree_target_is_active_in_tx(
                &txn, tenant_id, parent_id,
            )
            .await
            .map_err(|_| {
                ForumError::Validation(
                    "active forum category cannot have archived parent".to_string(),
                )
            })?;
        }

        forum_category::ActiveModel {
            id: Set(id),
            tenant_id: Set(tenant_id),
            moderated: Set(input.moderated),
            topic_count: Set(0),
            reply_count: Set(0),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        taxonomy_sync::sync_category_copy_in_tx(
            &txn,
            tenant_id,
            id,
            input.parent_id,
            requested_position,
            input.icon,
            input.color,
            locale,
            canonical_name,
            slug,
            canonical_description,
        )
        .await?;
        taxonomy_sync::shift_category_siblings_for_insert_in_tx(
            &txn,
            tenant_id,
            id,
            input.parent_id,
            requested_position,
        )
        .await?;
        super::projection_invalidation::publish_forum_projection_scope_direct_in_tx(
            &txn,
            tenant_id,
            security.user_id,
        )
        .await?;
        txn.commit().await?;
        Ok(id)
    }

    #[instrument(skip(self, security, input))]
    pub(super) async fn update(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        input: UpdateCategoryInput,
    ) -> ForumResult<()> {
        enforce_scope(&security, Resource::ForumCategories, Action::Update)?;
        if input.position.is_some() {
            return Err(ForumError::Validation(
                "Category position must be changed through move/reorder commands".to_string(),
            ));
        }
        let locale = normalize_locale(&input.locale)?;
        let requested_name = input.name.clone();
        let requested_slug = input.slug.clone();
        let requested_description = input.description.clone();
        let txn = self.db.begin().await?;
        super::category_audience::lock_category_tree_in_tx(&txn, tenant_id).await?;
        let category =
            CategoryService::find_category_for_update_in_tx(&txn, tenant_id, category_id).await?;

        let mut active: forum_category::ActiveModel = category.into();
        active.updated_at = Set(Utc::now().into());
        if let Some(moderated) = input.moderated {
            active.moderated = Set(moderated);
        }
        active.update(&txn).await?;

        let taxonomy_category = taxonomy_sync::load_category_owner_snapshot_in_tx(
            &txn,
            tenant_id,
            category_id,
            &locale,
        )
        .await?;
        let parent_id = taxonomy_category.parent_id;
        let position = taxonomy_category.position;
        let requested_icon = input.icon.clone();
        let requested_color = input.color.clone();
        let icon = requested_icon.or(taxonomy_category.icon_key);
        let color = requested_color.or(taxonomy_category.color);

        let existing_canonical =
            taxonomy_sync::load_category_locale_copy_in_tx(&txn, tenant_id, category_id, &locale)
                .await?;
        let (canonical_name, canonical_slug, canonical_description) = match existing_canonical {
            Some(existing) => {
                let name = requested_name.clone().unwrap_or(existing.name);
                validate_category_name(&name)?;
                let slug = match requested_slug.as_deref() {
                    Some(slug) => normalize_required_slug(slug)?,
                    None if requested_name.is_some() => normalize_required_slug(&name)?,
                    None => normalize_required_slug(&existing.slug)?,
                };
                let description = if requested_description.is_some() {
                    requested_description.clone()
                } else {
                    existing.description
                };
                (name, slug, description)
            }
            None => {
                let name = requested_name.clone().ok_or_else(|| {
                    ForumError::Validation("Category name is required".to_string())
                })?;
                validate_category_name(&name)?;
                let slug = requested_slug
                    .as_deref()
                    .map(normalize_required_slug)
                    .transpose()?
                    .unwrap_or_else(|| normalize_slug(&name));
                let slug = normalize_required_slug(&slug)?;
                (name, slug, requested_description.clone())
            }
        };

        taxonomy_sync::sync_category_copy_in_tx(
            &txn,
            tenant_id,
            category_id,
            parent_id,
            position,
            icon,
            color,
            locale,
            canonical_name,
            canonical_slug,
            canonical_description,
        )
        .await?;
        super::projection_invalidation::publish_forum_projection_scope_direct_in_tx(
            &txn,
            tenant_id,
            security.user_id,
        )
        .await?;
        txn.commit().await?;
        Ok(())
    }
}

#[path = "category_taxonomy_sync.rs"]
pub(crate) mod taxonomy_sync;

/// Taxonomy-backed canonical Category read adapter used by the public owner facade.
#[path = "category_taxonomy_read.rs"]
pub(super) mod taxonomy_read;

