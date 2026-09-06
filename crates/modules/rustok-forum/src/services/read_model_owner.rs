use std::collections::HashMap;

use rustok_api::{Action, PLATFORM_FALLBACK_LOCALE, Resource};
use rustok_content::normalize_locale_code;
use rustok_core::SecurityContext;
use rustok_taxonomy::{
    TaxonomyError, TaxonomyOwnerCategory, TaxonomyOwnerCategoryReader, TaxonomyScopeType,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect};
use uuid::Uuid;

use crate::dto::{
    CategoryCursorPage, CategoryCursorQuery, CategoryReadModel, MAX_FORUM_CATEGORY_TREE_NODES,
    ReplyCursorPage, ReplyCursorQuery, TopicCursorPage, TopicCursorQuery, TopicUnreadCursorPage,
    TopicUnreadCursorQuery, TopicUnreadSummaryReadModel, bounded_forum_read_limit,
};
use crate::entities::{forum_category, forum_category_taxonomy_binding};
use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;
use crate::services::subscription::SubscriptionService;

const CATEGORY_CURSOR_VERSION: &str = "c1";

pub struct ForumReadModelService {
    db: DatabaseConnection,
    legacy: super::read_model_legacy::ForumReadModelService,
}

impl ForumReadModelService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            legacy: super::read_model_legacy::ForumReadModelService::new(db.clone()),
            db,
        }
    }

    pub async fn list_categories(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        query: CategoryCursorQuery,
    ) -> ForumResult<CategoryCursorPage> {
        enforce_scope(&security, Resource::ForumCategories, Action::List)?;
        let requested_locale = normalized_locale(query.locale.as_deref())?;
        let fallback_locale = normalized_optional_locale(query.fallback_locale.as_deref())?;
        let limit = bounded_forum_read_limit(query.limit) as usize;
        let cursor = query
            .cursor
            .as_deref()
            .map(decode_category_cursor)
            .transpose()?;

        let categories = forum_category::Entity::find()
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .limit(MAX_FORUM_CATEGORY_TREE_NODES + 1)
            .all(&self.db)
            .await?;
        if categories.len() > MAX_FORUM_CATEGORY_TREE_NODES as usize {
            return Err(ForumError::Validation(format!(
                "Forum category read model exceeds the bounded limit of {MAX_FORUM_CATEGORY_TREE_NODES} categories"
            )));
        }
        if categories.is_empty() {
            return Ok(CategoryCursorPage {
                items: Vec::new(),
                next_cursor: None,
                has_more: false,
            });
        }

        let category_ids = categories
            .iter()
            .map(|category| category.id)
            .collect::<Vec<_>>();
        let bindings = forum_category_taxonomy_binding::Entity::find()
            .filter(forum_category_taxonomy_binding::Column::TenantId.eq(tenant_id))
            .filter(
                forum_category_taxonomy_binding::Column::ForumCategoryId
                    .is_in(category_ids.clone()),
            )
            .all(&self.db)
            .await?;
        let binding_by_forum_id = bindings
            .iter()
            .map(|binding| (binding.forum_category_id, binding.taxonomy_category_id))
            .collect::<HashMap<_, _>>();
        let forum_id_by_taxonomy_id = bindings
            .iter()
            .map(|binding| (binding.taxonomy_category_id, binding.forum_category_id))
            .collect::<HashMap<_, _>>();
        for category_id in &category_ids {
            if !binding_by_forum_id.contains_key(category_id) {
                return Err(ForumError::Validation(format!(
                    "Forum category {category_id} has no Taxonomy Category binding"
                )));
            }
        }

        let taxonomy_ids = bindings
            .iter()
            .map(|binding| binding.taxonomy_category_id)
            .collect::<Vec<_>>();
        let projections = TaxonomyOwnerCategoryReader::new(self.db.clone())
            .load_scoped_categories(
                tenant_id,
                TaxonomyScopeType::Module,
                Some("forum"),
                Some(&taxonomy_ids),
                &requested_locale,
                fallback_locale.as_deref(),
            )
            .await
            .map_err(map_taxonomy_read_error)?;
        let mut projection_by_taxonomy_id = projections
            .into_iter()
            .map(|projection| (projection.id, projection))
            .collect::<HashMap<_, _>>();

        let mut rows = Vec::with_capacity(categories.len());
        for category in categories {
            let taxonomy_id = *binding_by_forum_id.get(&category.id).ok_or_else(|| {
                ForumError::Validation(format!(
                    "Forum category {} lost its Taxonomy Category binding",
                    category.id
                ))
            })?;
            let canonical = projection_by_taxonomy_id
                .remove(&taxonomy_id)
                .ok_or_else(|| {
                    ForumError::Validation(format!(
                        "Forum category {} Taxonomy Category {taxonomy_id} projection is missing",
                        category.id
                    ))
                })?;
            let parent_id = canonical
                .parent_id
                .map(|taxonomy_parent_id| {
                    forum_id_by_taxonomy_id
                        .get(&taxonomy_parent_id)
                        .copied()
                        .ok_or_else(|| {
                            ForumError::Validation(format!(
                                "Taxonomy parent Category {taxonomy_parent_id} has no Forum category binding"
                            ))
                        })
                })
                .transpose()?;
            rows.push(BoundCategoryReadModel {
                owner: category,
                canonical,
                parent_id,
            });
        }

        rows.sort_by_key(|row| (row.canonical.position, row.owner.id));
        if let Some(cursor) = cursor {
            rows.retain(|row| {
                row.canonical.position > cursor.position
                    || (row.canonical.position == cursor.position && row.owner.id > cursor.id)
            });
        }

        let has_more = rows.len() > limit;
        rows.truncate(limit);
        let page_ids = rows.iter().map(|row| row.owner.id).collect::<Vec<_>>();
        let subscriptions = SubscriptionService::new(self.db.clone())
            .category_subscription_flags(tenant_id, &page_ids, security.user_id)
            .await?;
        let next_cursor = if has_more {
            rows.last()
                .map(|row| encode_category_cursor(row.canonical.position, row.owner.id))
        } else {
            None
        };

        let items = rows
            .into_iter()
            .map(|row| CategoryReadModel {
                id: row.owner.id,
                parent_id: row.parent_id,
                position: row.canonical.position,
                requested_locale: row.canonical.requested_locale,
                effective_locale: row.canonical.effective_locale,
                available_locales: row.canonical.available_locales,
                name: row.canonical.name,
                slug: row.canonical.slug,
                description: row.canonical.description,
                icon: row.canonical.icon_key,
                color: row.canonical.color,
                moderated: row.owner.moderated,
                topic_count: row.owner.topic_count,
                reply_count: row.owner.reply_count,
                is_subscribed: subscriptions.get(&row.owner.id).copied().unwrap_or(false),
            })
            .collect();

        Ok(CategoryCursorPage {
            items,
            next_cursor,
            has_more,
        })
    }

    pub async fn list_topics(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        query: TopicCursorQuery,
    ) -> ForumResult<TopicCursorPage> {
        self.legacy.list_topics(tenant_id, security, query).await
    }

    pub async fn list_topics_with_unread(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        query: TopicUnreadCursorQuery,
    ) -> ForumResult<TopicUnreadCursorPage> {
        self.legacy
            .list_topics_with_unread(tenant_id, security, query)
            .await
    }

    pub async fn summarize_topic_ids(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_ids: Vec<Uuid>,
    ) -> ForumResult<Vec<TopicUnreadSummaryReadModel>> {
        self.legacy
            .summarize_topic_ids(tenant_id, security, topic_ids)
            .await
    }

    pub async fn list_replies(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        query: ReplyCursorQuery,
    ) -> ForumResult<ReplyCursorPage> {
        self.legacy
            .list_replies(tenant_id, security, topic_id, query)
            .await
    }
}

struct BoundCategoryReadModel {
    owner: forum_category::Model,
    canonical: TaxonomyOwnerCategory,
    parent_id: Option<Uuid>,
}

#[derive(Clone, Copy)]
struct CategoryCursor {
    position: i32,
    id: Uuid,
}

fn encode_category_cursor(position: i32, id: Uuid) -> String {
    format!("{CATEGORY_CURSOR_VERSION}:{position}:{id}")
}

fn decode_category_cursor(value: &str) -> ForumResult<CategoryCursor> {
    let mut parts = value.splitn(3, ':');
    if parts.next() != Some(CATEGORY_CURSOR_VERSION) {
        return Err(invalid_category_cursor());
    }
    let position = parts
        .next()
        .and_then(|value| value.parse().ok())
        .ok_or_else(invalid_category_cursor)?;
    let id = parts
        .next()
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(invalid_category_cursor)?;
    Ok(CategoryCursor { position, id })
}

fn invalid_category_cursor() -> ForumError {
    ForumError::Validation("Invalid category cursor".to_string())
}

fn normalized_locale(locale: Option<&str>) -> ForumResult<String> {
    normalize_locale_code(locale.unwrap_or(PLATFORM_FALLBACK_LOCALE))
        .ok_or_else(|| ForumError::Validation("Invalid locale".to_string()))
}

fn normalized_optional_locale(locale: Option<&str>) -> ForumResult<Option<String>> {
    locale
        .map(|value| {
            normalize_locale_code(value)
                .ok_or_else(|| ForumError::Validation("Invalid fallback locale".to_string()))
        })
        .transpose()
}

fn map_taxonomy_read_error(error: TaxonomyError) -> ForumError {
    match error {
        TaxonomyError::Database(error) => ForumError::Database(error),
        other => ForumError::Validation(format!(
            "Forum Taxonomy category read projection failed: {other}"
        )),
    }
}
