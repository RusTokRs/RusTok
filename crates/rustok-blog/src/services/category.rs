use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend,
    DatabaseConnection, DatabaseTransaction, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    Statement, TransactionTrait, sea_query::Expr,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, PLATFORM_FALLBACK_LOCALE, Resource, TenantLocale};
use rustok_content::{available_locales_from, resolve_by_locale};
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::dto::{
    CategoryListItem, CategoryResponse, CreateCategoryInput, ListCategoriesFilter,
    MAX_BLOG_CATEGORY_TREE_NODES, UpdateCategoryInput,
};
use crate::entities::{blog_category, blog_category_translation};
use crate::error::{BlogError, BlogResult};
use crate::services::rbac::enforce_scope;
use crate::translation_evidence::{
    TRANSLATION_RESOURCE_KIND, TranslationChangeEvidence, record_translation_change_in_tx,
};

pub struct CategoryService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApplyExactCategoryTranslationInput {
    pub source_locale: TenantLocale,
    pub target_locale: TenantLocale,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub expected_resource_revision: i64,
    pub expected_source_revision: i64,
    pub expected_target_revision: Option<i64>,
    pub actor_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CategoryTranslationApplyResult {
    pub resource_revision: i64,
    pub target_revision: i64,
}

impl CategoryService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    pub(crate) fn database(&self) -> &DatabaseConnection {
        &self.db
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
        let slug = normalize_category_slug(input.slug.as_deref(), &input.name)?;
        let locale = normalize_locale(&input.locale)?;
        let now = Utc::now();
        let id = Uuid::new_v4();
        let txn = self.db.begin().await.map_err(BlogError::from)?;
        lock_category_tree_in_tx(&txn, tenant_id).await?;
        ensure_category_tree_capacity_in_tx(&txn, tenant_id).await?;

        if let Some(parent_id) = input.parent_id {
            Self::ensure_exists_in_tx(&txn, tenant_id, parent_id).await?;
        }
        canonicalize_siblings_for_insert_in_tx(
            &txn,
            tenant_id,
            input.parent_id,
            requested_position,
            now,
        )
        .await?;
        self.ensure_translation_slug_available_in_tx(&txn, tenant_id, &locale, &slug, None)
            .await?;

        let category = blog_category::ActiveModel {
            id: Set(id),
            tenant_id: Set(tenant_id),
            parent_id: Set(input.parent_id),
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

        let translation = blog_category_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            category_id: Set(id),
            tenant_id: Set(tenant_id),
            locale: Set(locale.clone()),
            name: Set(input.name),
            slug: Set(slug),
            description: Set(input.description),
            revision: Set(1),
        }
        .insert(&txn)
        .await?;

        record_translation_change_in_tx(
            &txn,
            TranslationChangeEvidence {
                tenant_id,
                resource_kind: TRANSLATION_RESOURCE_KIND,
                resource_id: id,
                locale: &locale,
                resource_revision: category.revision,
                target_revision: translation.revision,
                operation: "upsert",
                lifecycle: "active",
            },
        )
        .await?;

        txn.commit().await.map_err(BlogError::from)?;
        Ok(id)
    }

    #[instrument(skip(self, security))]
    pub async fn get(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        category_id: Uuid,
        locale: &str,
    ) -> BlogResult<CategoryResponse> {
        enforce_scope(&security, Resource::BlogCategories, Action::Read)?;
        let locale = normalize_locale(locale)?;
        let category = blog_category::Entity::find_by_id(category_id)
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| BlogError::category_not_found(category_id))?;

        let translations = blog_category_translation::Entity::find()
            .filter(blog_category_translation::Column::CategoryId.eq(category_id))
            .filter(blog_category_translation::Column::TenantId.eq(tenant_id))
            .all(&self.db)
            .await?;

        Ok(to_category_response(category, translations, &locale))
    }

    #[instrument(skip(self, security, input))]
    pub async fn update(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        input: UpdateCategoryInput,
    ) -> BlogResult<CategoryResponse> {
        enforce_scope(&security, Resource::BlogCategories, Action::Update)?;
        if input.position.is_some() {
            return Err(BlogError::validation(
                "Category position is structural; use the category move command",
            ));
        }
        let txn = self.db.begin().await.map_err(BlogError::from)?;
        let category = blog_category::Entity::find_by_id(category_id)
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or_else(|| BlogError::category_not_found(category_id))?;
        let locale = normalize_locale(&input.locale)?;
        let next_resource_revision = next_category_revision(&category)?;
        let now = Utc::now().fixed_offset();
        let settings = input
            .settings
            .clone()
            .unwrap_or_else(|| category.settings.clone());
        let resource_updated = blog_category::Entity::update_many()
            .col_expr(
                blog_category::Column::Settings,
                Expr::value(settings.clone()),
            )
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
        let category = blog_category::Model {
            settings,
            revision: next_resource_revision,
            updated_at: now,
            ..category
        };

        let existing_translation = blog_category_translation::Entity::find()
            .filter(blog_category_translation::Column::CategoryId.eq(category_id))
            .filter(blog_category_translation::Column::TenantId.eq(tenant_id))
            .filter(blog_category_translation::Column::Locale.eq(&locale))
            .one(&txn)
            .await?;

        let target_revision = match existing_translation {
            Some(translation) => {
                let name = input
                    .name
                    .as_deref()
                    .map(str::to_owned)
                    .unwrap_or_else(|| translation.name.clone());
                validate_category_name(&name)?;
                let slug = match input.slug.as_deref() {
                    Some(slug) => normalize_non_empty_slug(slug)?,
                    None if input.name.is_some() => normalize_non_empty_slug(&name)?,
                    None => translation.slug.clone(),
                };
                self.ensure_translation_slug_available_in_tx(
                    &txn,
                    tenant_id,
                    &locale,
                    &slug,
                    Some(category_id),
                )
                .await?;
                let description = input
                    .description
                    .clone()
                    .or_else(|| translation.description.clone());
                validate_optional_description(description.as_deref())?;
                let revision =
                    next_category_translation_revision(category_id, &locale, translation.revision)?;
                let translation_updated = blog_category_translation::Entity::update_many()
                    .col_expr(blog_category_translation::Column::Name, Expr::value(name))
                    .col_expr(blog_category_translation::Column::Slug, Expr::value(slug))
                    .col_expr(
                        blog_category_translation::Column::Description,
                        Expr::value(description),
                    )
                    .col_expr(
                        blog_category_translation::Column::Revision,
                        Expr::value(revision),
                    )
                    .filter(blog_category_translation::Column::Id.eq(translation.id))
                    .filter(blog_category_translation::Column::Revision.eq(translation.revision))
                    .exec(&txn)
                    .await?;
                if translation_updated.rows_affected != 1 {
                    return Err(BlogError::conflict(
                        "category translation changed before the update could commit",
                    ));
                }
                revision
            }
            None => {
                let name = input
                    .name
                    .as_deref()
                    .map(str::to_owned)
                    .ok_or_else(|| BlogError::validation("Category name is required"))?;
                validate_category_name(&name)?;
                let slug = match input.slug.as_deref() {
                    Some(slug_value) => normalize_non_empty_slug(slug_value)?,
                    None => normalize_non_empty_slug(&name)?,
                };
                self.ensure_translation_slug_available_in_tx(
                    &txn,
                    tenant_id,
                    &locale,
                    &slug,
                    Some(category_id),
                )
                .await?;
                validate_optional_description(input.description.as_deref())?;
                let translation = blog_category_translation::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    category_id: Set(category_id),
                    tenant_id: Set(tenant_id),
                    locale: Set(locale.clone()),
                    name: Set(name),
                    slug: Set(slug),
                    description: Set(input.description.clone()),
                    revision: Set(1),
                }
                .insert(&txn)
                .await;
                match translation {
                    Ok(_) => 1,
                    Err(error) if is_unique_constraint(&error) => {
                        return Err(BlogError::conflict(
                            "category translation was created before the update could commit",
                        ));
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        };

        record_translation_change_in_tx(
            &txn,
            TranslationChangeEvidence {
                tenant_id,
                resource_kind: TRANSLATION_RESOURCE_KIND,
                resource_id: category_id,
                locale: &locale,
                resource_revision: category.revision,
                target_revision,
                operation: "upsert",
                lifecycle: "active",
            },
        )
        .await?;

        self.publish_blog_reindex_in_tx(&txn, tenant_id, security.user_id)
            .await?;

        let translations = blog_category_translation::Entity::find()
            .filter(blog_category_translation::Column::CategoryId.eq(category_id))
            .filter(blog_category_translation::Column::TenantId.eq(tenant_id))
            .all(&txn)
            .await?;
        let response = to_category_response(category, translations, &locale);

        txn.commit().await.map_err(BlogError::from)?;
        Ok(response)
    }

    #[instrument(skip(self, security))]
    pub async fn delete(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
    ) -> BlogResult<()> {
        enforce_scope(&security, Resource::BlogCategories, Action::Delete)?;
        let txn = self.db.begin().await.map_err(BlogError::from)?;
        lock_category_tree_in_tx(&txn, tenant_id).await?;
        let category = blog_category::Entity::find_by_id(category_id)
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or_else(|| BlogError::category_not_found(category_id))?;
        ensure_category_is_leaf_in_tx(&txn, tenant_id, category_id).await?;

        let translations = blog_category_translation::Entity::find()
            .filter(blog_category_translation::Column::CategoryId.eq(category_id))
            .filter(blog_category_translation::Column::TenantId.eq(tenant_id))
            .all(&txn)
            .await?;
        let (locale, target_revision) = translations
            .iter()
            .min_by(|left, right| {
                left.locale
                    .cmp(&right.locale)
                    .then_with(|| left.id.cmp(&right.id))
            })
            .map(|translation| (translation.locale.clone(), translation.revision))
            .unwrap_or_else(|| (PLATFORM_FALLBACK_LOCALE.to_string(), 0));
        let resource_revision = next_category_revision(&category)?;
        record_translation_change_in_tx(
            &txn,
            TranslationChangeEvidence {
                tenant_id,
                resource_kind: TRANSLATION_RESOURCE_KIND,
                resource_id: category_id,
                locale: &locale,
                resource_revision,
                target_revision,
                operation: "delete",
                lifecycle: "deleted",
            },
        )
        .await?;
        let deleted = blog_category::Entity::delete_many()
            .filter(blog_category::Column::Id.eq(category_id))
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .filter(blog_category::Column::Revision.eq(category.revision))
            .exec(&txn)
            .await?;
        if deleted.rows_affected != 1 {
            return Err(BlogError::conflict(
                "blog category changed before deletion could commit",
            ));
        }
        canonicalize_siblings_in_tx(&txn, tenant_id, category.parent_id, Utc::now()).await?;

        self.publish_blog_reindex_in_tx(&txn, tenant_id, security.user_id)
            .await?;

        txn.commit().await.map_err(BlogError::from)?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn list(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        filter: ListCategoriesFilter,
    ) -> BlogResult<(Vec<CategoryListItem>, u64)> {
        enforce_scope(&security, Resource::BlogCategories, Action::List)?;
        let locale = filter
            .locale
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        let locale = normalize_locale(&locale)?;
        let page = filter.page.max(1);
        let per_page = filter.per_page.clamp(1, 100);

        let paginator = blog_category::Entity::find()
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .order_by_asc(blog_category::Column::Position)
            .paginate(&self.db, per_page);

        let total = paginator.num_items().await?;
        let categories = paginator.fetch_page(page - 1).await?;
        let category_ids: Vec<Uuid> = categories.iter().map(|category| category.id).collect();
        let all_translations = if category_ids.is_empty() {
            Vec::new()
        } else {
            blog_category_translation::Entity::find()
                .filter(blog_category_translation::Column::TenantId.eq(tenant_id))
                .filter(blog_category_translation::Column::CategoryId.is_in(category_ids))
                .all(&self.db)
                .await?
        };

        let items = categories
            .into_iter()
            .map(|category| {
                let translations: Vec<&blog_category_translation::Model> = all_translations
                    .iter()
                    .filter(|translation| translation.category_id == category.id)
                    .collect();
                let resolved = resolve_by_locale(&translations, &locale, |translation| {
                    translation.locale.as_str()
                });
                let translation = resolved.item.copied();

                CategoryListItem {
                    id: category.id,
                    locale: locale.clone(),
                    effective_locale: resolved.effective_locale,
                    name: translation
                        .map(|translation| translation.name.clone())
                        .unwrap_or_default(),
                    slug: translation
                        .map(|translation| translation.slug.clone())
                        .unwrap_or_default(),
                    parent_id: category.parent_id,
                    position: category.position,
                    settings: category.settings,
                    created_at: category.created_at.into(),
                }
            })
            .collect();

        Ok((items, total))
    }

    pub(crate) async fn apply_exact_translation_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        input: ApplyExactCategoryTranslationInput,
    ) -> BlogResult<CategoryTranslationApplyResult> {
        if input.source_locale == input.target_locale {
            return Err(BlogError::validation(
                "Source and target locales must differ for an exact category translation",
            ));
        }

        let category = blog_category::Entity::find_by_id(category_id)
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .one(txn)
            .await?
            .ok_or_else(|| BlogError::category_not_found(category_id))?;
        if category.revision != input.expected_resource_revision {
            return Err(BlogError::conflict(
                "blog category resource revision does not match the translation proposal",
            ));
        }

        let source = blog_category_translation::Entity::find()
            .filter(blog_category_translation::Column::CategoryId.eq(category_id))
            .filter(blog_category_translation::Column::TenantId.eq(tenant_id))
            .filter(blog_category_translation::Column::Locale.eq(input.source_locale.as_str()))
            .one(txn)
            .await?
            .ok_or_else(|| BlogError::validation("Exact source locale is not present"))?;
        if source.revision != input.expected_source_revision {
            return Err(BlogError::conflict(
                "source locale revision does not match the translation proposal",
            ));
        }

        validate_category_name(&input.name)?;
        let slug = normalize_non_empty_slug(&input.slug)?;
        let description = normalize_optional_description(input.description.as_deref());
        validate_optional_description(description.as_deref())?;
        self.ensure_translation_slug_available_in_tx(
            txn,
            tenant_id,
            input.target_locale.as_str(),
            &slug,
            Some(category_id),
        )
        .await?;

        let existing_target = blog_category_translation::Entity::find()
            .filter(blog_category_translation::Column::CategoryId.eq(category_id))
            .filter(blog_category_translation::Column::TenantId.eq(tenant_id))
            .filter(blog_category_translation::Column::Locale.eq(input.target_locale.as_str()))
            .one(txn)
            .await?;
        let target_revision = match existing_target {
            Some(target) => {
                if input.expected_target_revision != Some(target.revision) {
                    return Err(BlogError::conflict(
                        "target locale revision does not match the translation proposal",
                    ));
                }
                let revision = next_category_translation_revision(
                    category_id,
                    input.target_locale.as_str(),
                    target.revision,
                )?;
                let updated = blog_category_translation::Entity::update_many()
                    .col_expr(
                        blog_category_translation::Column::Name,
                        Expr::value(input.name.clone()),
                    )
                    .col_expr(
                        blog_category_translation::Column::Slug,
                        Expr::value(slug.clone()),
                    )
                    .col_expr(
                        blog_category_translation::Column::Description,
                        Expr::value(description.clone()),
                    )
                    .col_expr(
                        blog_category_translation::Column::Revision,
                        Expr::value(revision),
                    )
                    .filter(blog_category_translation::Column::Id.eq(target.id))
                    .filter(blog_category_translation::Column::Revision.eq(target.revision))
                    .exec(txn)
                    .await?;
                if updated.rows_affected != 1 {
                    return Err(BlogError::conflict(
                        "target locale changed before translation apply could commit",
                    ));
                }
                revision
            }
            None => {
                if input.expected_target_revision.is_some() {
                    return Err(BlogError::conflict(
                        "translation proposal expected a target locale that does not exist",
                    ));
                }
                let inserted = blog_category_translation::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    category_id: Set(category_id),
                    tenant_id: Set(tenant_id),
                    locale: Set(input.target_locale.as_str().to_string()),
                    name: Set(input.name.clone()),
                    slug: Set(slug),
                    description: Set(description),
                    revision: Set(1),
                }
                .insert(txn)
                .await;
                match inserted {
                    Ok(_) => 1,
                    Err(error) if is_unique_constraint(&error) => {
                        return Err(BlogError::conflict(
                            "target locale was created before translation apply could commit",
                        ));
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        };

        let resource_revision = next_category_revision(&category)?;
        let category_updated = blog_category::Entity::update_many()
            .col_expr(
                blog_category::Column::Revision,
                Expr::value(resource_revision),
            )
            .col_expr(
                blog_category::Column::UpdatedAt,
                Expr::value(Utc::now().fixed_offset()),
            )
            .filter(blog_category::Column::Id.eq(category_id))
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .filter(blog_category::Column::Revision.eq(category.revision))
            .exec(txn)
            .await?;
        if category_updated.rows_affected != 1 {
            return Err(BlogError::conflict(
                "blog category changed before translation apply could commit",
            ));
        }

        record_translation_change_in_tx(
            txn,
            TranslationChangeEvidence {
                tenant_id,
                resource_kind: TRANSLATION_RESOURCE_KIND,
                resource_id: category_id,
                locale: input.target_locale.as_str(),
                resource_revision,
                target_revision,
                operation: "upsert",
                lifecycle: "active",
            },
        )
        .await?;
        self.publish_blog_reindex_in_tx(txn, tenant_id, input.actor_id)
            .await?;

        Ok(CategoryTranslationApplyResult {
            resource_revision,
            target_revision,
        })
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

    async fn ensure_translation_slug_available_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        locale: &str,
        slug: &str,
        exclude_category_id: Option<Uuid>,
    ) -> BlogResult<()> {
        let mut select = blog_category_translation::Entity::find()
            .filter(blog_category_translation::Column::TenantId.eq(tenant_id))
            .filter(blog_category_translation::Column::Locale.eq(locale))
            .filter(blog_category_translation::Column::Slug.eq(slug));
        if let Some(category_id) = exclude_category_id {
            select = select.filter(blog_category_translation::Column::CategoryId.ne(category_id));
        }
        if select.one(txn).await?.is_some() {
            return Err(BlogError::duplicate_slug(slug, locale));
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

async fn lock_category_tree_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
) -> BlogResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            txn.execute(Statement::from_sql_and_values(
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

async fn ensure_category_is_leaf_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
) -> BlogResult<()> {
    let child = blog_category::Entity::find()
        .filter(blog_category::Column::TenantId.eq(tenant_id))
        .filter(blog_category::Column::ParentId.eq(category_id))
        .one(txn)
        .await?;
    if child.is_some() {
        return Err(BlogError::validation(
            "Category must be a leaf before deletion; move or delete its children first",
        ));
    }
    Ok(())
}

async fn load_siblings_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    parent_id: Option<Uuid>,
) -> BlogResult<Vec<blog_category::Model>> {
    let mut query = blog_category::Entity::find()
        .filter(blog_category::Column::TenantId.eq(tenant_id));
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

async fn canonicalize_siblings_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    parent_id: Option<Uuid>,
    now: chrono::DateTime<Utc>,
) -> BlogResult<()> {
    let siblings = load_siblings_in_tx(txn, tenant_id, parent_id).await?;
    for (index, sibling) in siblings.into_iter().enumerate() {
        let desired_position = i32::try_from(index)
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

fn normalize_optional_description(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
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

fn next_category_translation_revision(
    category_id: Uuid,
    locale: &str,
    revision: i64,
) -> BlogResult<i64> {
    revision
        .checked_add(1)
        .filter(|next_revision| revision > 0 && *next_revision > 0)
        .ok_or_else(|| BlogError::CategoryTranslationRevisionExhausted {
            category_id,
            locale: locale.to_string(),
        })
}

fn is_unique_constraint(error: &sea_orm::DbErr) -> bool {
    matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
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

fn to_category_response(
    category: blog_category::Model,
    translations: Vec<blog_category_translation::Model>,
    locale: &str,
) -> CategoryResponse {
    let resolved = resolve_by_locale(&translations, locale, |translation| {
        translation.locale.as_str()
    });
    let available_locales =
        available_locales_from(&translations, |translation| translation.locale.as_str());
    let translation = resolved.item;

    CategoryResponse {
        id: category.id,
        tenant_id: category.tenant_id,
        locale: locale.to_string(),
        effective_locale: resolved.effective_locale,
        available_locales,
        name: translation
            .map(|translation| translation.name.clone())
            .unwrap_or_default(),
        slug: translation
            .map(|translation| translation.slug.clone())
            .unwrap_or_default(),
        description: translation.and_then(|translation| translation.description.clone()),
        parent_id: category.parent_id,
        position: category.position,
        settings: category.settings,
        created_at: category.created_at.into(),
        updated_at: category.updated_at.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn category_response_uses_deterministic_shared_locale_fallback() {
        let tenant_id = Uuid::from_u128(1);
        let category_id = Uuid::from_u128(2);
        let now = Utc::now().fixed_offset();
        let category = blog_category::Model {
            id: category_id,
            tenant_id,
            parent_id: None,
            position: 0,
            depth: 0,
            post_count: 0,
            settings: json!({}),
            revision: 1,
            created_at: now,
            updated_at: now,
        };
        let translations = vec![
            blog_category_translation::Model {
                id: Uuid::from_u128(3),
                category_id,
                tenant_id,
                locale: "fr".to_string(),
                name: "Français".to_string(),
                slug: "francais".to_string(),
                description: None,
                revision: 1,
            },
            blog_category_translation::Model {
                id: Uuid::from_u128(4),
                category_id,
                tenant_id,
                locale: "de".to_string(),
                name: "Deutsch".to_string(),
                slug: "deutsch".to_string(),
                description: None,
                revision: 1,
            },
        ];

        let response = to_category_response(category, translations, "ru");

        assert_eq!(response.effective_locale, "de");
        assert_eq!(response.name, "Deutsch");
        assert_eq!(
            response.available_locales,
            vec!["de".to_string(), "fr".to_string()]
        );
    }
}
