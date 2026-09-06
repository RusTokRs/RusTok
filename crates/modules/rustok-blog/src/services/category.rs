use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend,
    DatabaseConnection, DatabaseTransaction, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    Statement, TransactionTrait, sea_query::Expr,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource, TenantLocale};
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::dto::{CreateCategoryInput, MAX_BLOG_CATEGORY_TREE_NODES, UpdateCategoryInput};
use crate::entities::blog_category;
use crate::error::{BlogError, BlogResult};
use crate::services::{category_taxonomy_sync, rbac::enforce_scope};

/// Blog-owned Category command core.
///
/// Blog persists membership/settings and authorizes commands. Canonical localized
/// copy, routes and hierarchy are written through Taxonomy in the same transaction.
/// The retired `blog_category_translation` table is not a command source or sink.
pub struct CategoryService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl CategoryService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    #[instrument(skip(self, security, input))]
    pub async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateCategoryInput,
    ) -> BlogResult<Uuid> {
        enforce_scope(&security, Resource::BlogCategories, Action::Create)?;
        validate_category_name(&input.name)?;
        validate_optional_description(input.description.as_deref())?;
        let requested_position = input.position.unwrap_or(0);
        if requested_position < 0 {
            return Err(BlogError::validation(
                "Category position cannot be negative",
            ));
        }

        let locale = normalize_locale(&input.locale)?;
        let name = input.name;
        let slug = normalize_category_slug(input.slug.as_deref(), &name)?;
        let description = input.description;
        let parent_id = input.parent_id;
        let now = Utc::now();
        let id = Uuid::new_v4();
        let txn = self.db.begin().await.map_err(BlogError::from)?;

        lock_category_tree_in_tx(&txn, tenant_id).await?;
        ensure_category_tree_capacity_in_tx(&txn, tenant_id).await?;
        if let Some(parent_id) = parent_id {
            Self::ensure_exists_in_tx(&txn, tenant_id, parent_id).await?;
        }
        canonicalize_siblings_for_insert_in_tx(&txn, tenant_id, parent_id, requested_position, now)
            .await?;

        blog_category::ActiveModel {
            id: Set(id),
            tenant_id: Set(tenant_id),
            parent_id: Set(parent_id),
            position: Set(requested_position),
            depth: Set(0),
            post_count: Set(0),
            settings: Set(input.settings),
            revision: Set(1),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        category_taxonomy_sync::sync_category_copy_in_tx(
            &txn,
            tenant_id,
            id,
            locale,
            name,
            slug,
            description,
        )
        .await?;

        txn.commit().await.map_err(BlogError::from)?;
        Ok(id)
    }

    #[instrument(skip(self, security, input))]
    pub async fn update(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        input: UpdateCategoryInput,
    ) -> BlogResult<()> {
        enforce_scope(&security, Resource::BlogCategories, Action::Update)?;
        if input.position.is_some() {
            return Err(BlogError::validation(
                "Category position is structural; use the category move command",
            ));
        }

        let locale = normalize_locale(&input.locale)?;
        let requested_name = input.name.clone();
        let requested_slug = input.slug.clone();
        let requested_description = input.description.clone();
        let txn = self.db.begin().await.map_err(BlogError::from)?;
        let category = blog_category::Entity::find_by_id(category_id)
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or_else(|| BlogError::category_not_found(category_id))?;

        let next_resource_revision = next_category_revision(&category)?;
        let now = Utc::now().fixed_offset();
        let settings = input
            .settings
            .clone()
            .unwrap_or_else(|| category.settings.clone());
        let resource_updated = blog_category::Entity::update_many()
            .col_expr(blog_category::Column::Settings, Expr::value(settings))
            .col_expr(
                blog_category::Column::Revision,
                Expr::value(next_resource_revision),
            )
            .col_expr(blog_category::Column::UpdatedAt, Expr::value(now))
            .filter(blog_category::Column::Id.eq(category_id))
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .filter(blog_category::Column::Revision.eq(category.revision))
            .exec(&txn)
            .await?;
        if resource_updated.rows_affected != 1 {
            return Err(BlogError::conflict(
                "blog category changed before the update could commit",
            ));
        }

        let existing_canonical = category_taxonomy_sync::load_category_locale_copy_in_tx(
            &txn,
            tenant_id,
            category_id,
            &locale,
        )
        .await?;
        let (canonical_name, canonical_slug, canonical_description) = match existing_canonical {
            Some(existing) => {
                let name = requested_name.clone().unwrap_or(existing.name);
                validate_category_name(&name)?;
                let slug = match requested_slug.as_deref() {
                    Some(slug) => normalize_non_empty_slug(slug)?,
                    None if requested_name.is_some() => normalize_non_empty_slug(&name)?,
                    None => normalize_non_empty_slug(&existing.slug)?,
                };
                let description = requested_description.clone().or(existing.description);
                validate_optional_description(description.as_deref())?;
                (name, slug, description)
            }
            None => {
                let name = requested_name
                    .clone()
                    .ok_or_else(|| BlogError::validation("Category name is required"))?;
                validate_category_name(&name)?;
                let slug = requested_slug
                    .as_deref()
                    .map(normalize_non_empty_slug)
                    .transpose()?
                    .unwrap_or_else(|| normalize_slug_like(&name));
                let slug = normalize_non_empty_slug(&slug)?;
                validate_optional_description(requested_description.as_deref())?;
                (name, slug, requested_description.clone())
            }
        };

        category_taxonomy_sync::sync_category_copy_in_tx(
            &txn,
            tenant_id,
            category_id,
            locale,
            canonical_name,
            canonical_slug,
            canonical_description,
        )
        .await?;

        self.publish_blog_reindex_in_tx(&txn, tenant_id, security.user_id)
            .await?;
        txn.commit().await.map_err(BlogError::from)?;
        Ok(())
    }

    pub(crate) async fn ensure_exists_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> BlogResult<()> {
        let exists = blog_category::Entity::find_by_id(category_id)
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .one(txn)
            .await?;
        if exists.is_none() {
            return Err(BlogError::category_not_found(category_id));
        }
        Ok(())
    }

    async fn publish_blog_reindex_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
    ) -> BlogResult<()> {
        self.event_bus
            .publish_in_tx(
                txn,
                tenant_id,
                actor_id,
                DomainEvent::ReindexRequested {
                    target_type: "blog".to_string(),
                    target_id: None,
                },
            )
            .await
            .map_err(BlogError::from)
    }
}

async fn lock_category_tree_in_tx(txn: &DatabaseTransaction, tenant_id: Uuid) -> BlogResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            txn.execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
                [format!("blog-category-tree:{tenant_id}").into()],
            ))
            .await?;
            Ok(())
        }
        DatabaseBackend::Sqlite => Ok(()),
        backend => Err(BlogError::validation(format!(
            "Blog category hierarchy writes do not support {backend:?}"
        ))),
    }
}

async fn ensure_category_tree_capacity_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
) -> BlogResult<()> {
    let count = blog_category::Entity::find()
        .filter(blog_category::Column::TenantId.eq(tenant_id))
        .count(txn)
        .await?;
    if count >= MAX_BLOG_CATEGORY_TREE_NODES {
        return Err(BlogError::validation(format!(
            "Blog category tree cannot exceed {MAX_BLOG_CATEGORY_TREE_NODES} nodes"
        )));
    }
    Ok(())
}

async fn load_siblings_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    parent_id: Option<Uuid>,
) -> BlogResult<Vec<blog_category::Model>> {
    let mut query =
        blog_category::Entity::find().filter(blog_category::Column::TenantId.eq(tenant_id));
    query = match parent_id {
        Some(parent_id) => query.filter(blog_category::Column::ParentId.eq(parent_id)),
        None => query.filter(blog_category::Column::ParentId.is_null()),
    };
    Ok(query
        .order_by_asc(blog_category::Column::Position)
        .order_by_asc(blog_category::Column::Id)
        .all(txn)
        .await?)
}

async fn canonicalize_siblings_for_insert_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    parent_id: Option<Uuid>,
    requested_position: i32,
    now: chrono::DateTime<Utc>,
) -> BlogResult<()> {
    let siblings = load_siblings_in_tx(txn, tenant_id, parent_id).await?;
    let insertion_index = usize::try_from(requested_position)
        .map_err(|_| BlogError::validation("Category position cannot be negative"))?;
    if insertion_index > siblings.len() {
        return Err(BlogError::validation(format!(
            "Category position {requested_position} exceeds sibling count {}",
            siblings.len()
        )));
    }

    for (index, sibling) in siblings.into_iter().enumerate() {
        let desired_index = if index >= insertion_index {
            index.checked_add(1).ok_or_else(|| {
                BlogError::validation("Category sibling position exceeds usize range")
            })?
        } else {
            index
        };
        let desired_position = i32::try_from(desired_index)
            .map_err(|_| BlogError::validation("Category sibling position exceeds i32 range"))?;
        update_sibling_position_in_tx(txn, sibling, desired_position, now).await?;
    }
    Ok(())
}

async fn update_sibling_position_in_tx(
    txn: &DatabaseTransaction,
    sibling: blog_category::Model,
    desired_position: i32,
    now: chrono::DateTime<Utc>,
) -> BlogResult<()> {
    if sibling.position == desired_position {
        return Ok(());
    }
    let mut active: blog_category::ActiveModel = sibling.into();
    active.position = Set(desired_position);
    active.updated_at = Set(now.into());
    active.update(txn).await?;
    Ok(())
}

fn validate_category_name(name: &str) -> BlogResult<()> {
    if name.trim().is_empty() {
        return Err(BlogError::validation("Category name cannot be empty"));
    }
    if name.chars().count() > 255 {
        return Err(BlogError::validation(
            "Category name cannot exceed 255 characters",
        ));
    }
    Ok(())
}

fn normalize_locale(locale: &str) -> BlogResult<String> {
    TenantLocale::new(locale)
        .map(TenantLocale::into_inner)
        .map_err(|_| BlogError::validation("Invalid locale"))
}

fn validate_optional_description(description: Option<&str>) -> BlogResult<()> {
    if let Some(description) = description
        && description.chars().count() > 1_000
    {
        return Err(BlogError::validation(
            "Category description cannot exceed 1000 characters",
        ));
    }
    Ok(())
}

fn next_category_revision(category: &blog_category::Model) -> BlogResult<i64> {
    category
        .revision
        .checked_add(1)
        .filter(|revision| category.revision > 0 && *revision > 0)
        .ok_or_else(|| {
            BlogError::conflict(format!(
                "blog category {} has an invalid or exhausted resource revision",
                category.id
            ))
        })
}

fn normalize_category_slug(input: Option<&str>, fallback_name: &str) -> BlogResult<String> {
    let value = input
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback_name);
    normalize_non_empty_slug(value)
}

fn normalize_non_empty_slug(slug: &str) -> BlogResult<String> {
    let normalized = normalize_slug_like(slug);
    if normalized.is_empty() {
        return Err(BlogError::validation(
            "Slug must contain at least one ASCII letter or digit",
        ));
    }
    Ok(normalized)
}

fn normalize_slug_like(value: &str) -> String {
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
