use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend,
    DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder, Statement,
    TransactionTrait,
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

/// Crate-private persistence seam shared by Forum category owner commands.
///
/// Canonical Category reads and localized copy no longer live here: public reads
/// are Taxonomy-backed and public mutations are owned by
/// `CategoryProjectionOwnerService`. This type remains only for transaction
/// helpers reused by topic/reply/import owners.
pub(super) struct CategoryService;

impl CategoryService {
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
        let existing = forum_category::Entity::find_by_id(category_id)
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .one(txn)
            .await?;
        existing.ok_or(ForumError::CategoryNotFound(category_id))
    }

    pub(crate) async fn adjust_counters_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        topic_delta: i32,
        reply_delta: i32,
    ) -> ForumResult<()> {
        let category = forum_category::Entity::find_by_id(category_id)
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .one(txn)
            .await?
            .ok_or(ForumError::CategoryNotFound(category_id))?;

        let mut active: forum_category::ActiveModel = category.clone().into();
        active.topic_count = Set((category.topic_count + topic_delta).max(0));
        active.reply_count = Set((category.reply_count + reply_delta).max(0));
        active.updated_at = Set(Utc::now().into());
        active.update(txn).await?;
        Ok(())
    }
}

async fn lock_category_tree_in_tx(txn: &DatabaseTransaction, tenant_id: Uuid) -> ForumResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            txn.execute(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
                [tenant_id.to_string().into()],
            ))
            .await?;
            Ok(())
        }
        DatabaseBackend::Sqlite => Ok(()),
        backend => Err(ForumError::Validation(format!(
            "Forum category creation does not support {backend:?}"
        ))),
    }
}

async fn shift_siblings_for_insert_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    parent_id: Option<Uuid>,
    requested_position: i32,
    now: chrono::DateTime<Utc>,
) -> ForumResult<()> {
    let siblings = match parent_id {
        Some(parent_id) => {
            forum_category::Entity::find()
                .filter(forum_category::Column::TenantId.eq(tenant_id))
                .filter(forum_category::Column::ParentId.eq(parent_id))
                .filter(forum_category::Column::Position.gte(requested_position))
                .order_by_desc(forum_category::Column::Position)
                .order_by_desc(forum_category::Column::Id)
                .all(txn)
                .await?
        }
        None => {
            forum_category::Entity::find()
                .filter(forum_category::Column::TenantId.eq(tenant_id))
                .filter(forum_category::Column::ParentId.is_null())
                .filter(forum_category::Column::Position.gte(requested_position))
                .order_by_desc(forum_category::Column::Position)
                .order_by_desc(forum_category::Column::Id)
                .all(txn)
                .await?
        }
    };

    for sibling in siblings {
        let next_position = sibling.position.checked_add(1).ok_or_else(|| {
            ForumError::Validation("Category sibling position exceeds i32 range".to_string())
        })?;
        let mut active: forum_category::ActiveModel = sibling.into();
        active.position = Set(next_position);
        active.updated_at = Set(now.into());
        active.update(txn).await?;
    }

    Ok(())
}

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
