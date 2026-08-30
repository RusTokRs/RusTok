use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustok_core::search_projection::{
    MAX_SEARCH_PROJECTION_PAGE_SIZE, SearchProjectionDocument, SearchProjectionPage,
    SearchProjectionSource, SearchProjectionSourceFactory,
};
use rustok_core::{Error, Result};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::{TaxonomyError, TaxonomyOwnerCategoryReader, TaxonomyScopeType};
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde_json::json;
use uuid::Uuid;

use crate::entities::{
    forum_category, forum_category_taxonomy_binding, forum_reply_body, forum_topic_translation,
};
use crate::search_projection_author::{
    load_public_author_summary, public_author_handle, public_author_id, public_author_keywords,
    public_author_payload,
};
use crate::state_machine::ReplyStatus;
use crate::{ForumCategoryRouteService, ForumPublicDiscoveryService, ForumTopicRouteService};

const FORUM_SOURCE_MODULE: &str = "forum";
const FORUM_CATEGORY_ENTITY_TYPE: &str = "forum_category";
const FORUM_TOPIC_ENTITY_TYPE: &str = "forum_topic";
const FORUM_REPLY_ENTITY_TYPE: &str = "forum_reply";
const MAX_ENTITY_LOCALES: u64 = 32;
const CATEGORY_LOCALE_PROJECTION_REQUEST_LOCALE: &str = "en";

#[derive(Clone, Default)]
pub struct ForumSearchProjectionSourceFactory;

impl SearchProjectionSourceFactory for ForumSearchProjectionSourceFactory {
    fn source_module(&self) -> &'static str {
        FORUM_SOURCE_MODULE
    }

    fn build(&self, db: DatabaseConnection) -> Arc<dyn SearchProjectionSource> {
        Arc::new(ForumSearchProjectionSource::new(db))
    }
}

#[derive(Clone)]
struct ForumSearchProjectionSource {
    db: DatabaseConnection,
    discovery: Arc<ForumPublicDiscoveryService>,
}

impl ForumSearchProjectionSource {
    fn new(db: DatabaseConnection) -> Self {
        let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
        Self {
            discovery: Arc::new(ForumPublicDiscoveryService::new(db.clone(), event_bus)),
            db,
        }
    }

    async fn candidate_page(
        &self,
        tenant_id: Uuid,
        cursor: Option<ProjectionCursor>,
        limit: usize,
    ) -> Result<Vec<ProjectionCandidate>> {
        let mut candidates = Vec::with_capacity(limit);

        if !matches!(
            cursor.as_ref(),
            Some(ProjectionCursor::Topic { .. } | ProjectionCursor::Reply { .. })
        ) {
            candidates.extend(
                self.category_candidates(tenant_id, cursor.as_ref(), limit)
                    .await?,
            );
        }

        let remaining = limit.saturating_sub(candidates.len());
        if remaining == 0 {
            return Ok(candidates);
        }

        if !matches!(cursor.as_ref(), Some(ProjectionCursor::Reply { .. })) {
            let mut query = forum_topic_translation::Entity::find()
                .filter(forum_topic_translation::Column::TenantId.eq(tenant_id))
                .order_by_asc(forum_topic_translation::Column::TopicId)
                .order_by_asc(forum_topic_translation::Column::Locale)
                .limit(remaining as u64);
            if let Some(ProjectionCursor::Topic { entity_id, locale }) = cursor.as_ref() {
                query = query.filter(
                    Condition::any()
                        .add(forum_topic_translation::Column::TopicId.gt(*entity_id))
                        .add(
                            Condition::all()
                                .add(forum_topic_translation::Column::TopicId.eq(*entity_id))
                                .add(forum_topic_translation::Column::Locale.gt(locale.clone())),
                        ),
                );
            }
            let rows = query.all(&self.db).await.map_err(Error::Database)?;
            for row in rows {
                candidates.push(ProjectionCandidate::Topic {
                    entity_id: row.topic_id,
                    locale: row.locale,
                });
            }
        }

        let remaining = limit.saturating_sub(candidates.len());
        if remaining == 0 {
            return Ok(candidates);
        }

        let mut query = forum_reply_body::Entity::find()
            .filter(forum_reply_body::Column::TenantId.eq(tenant_id))
            .order_by_asc(forum_reply_body::Column::ReplyId)
            .order_by_asc(forum_reply_body::Column::Locale)
            .limit(remaining as u64);
        if let Some(ProjectionCursor::Reply { entity_id, locale }) = cursor.as_ref() {
            query = query.filter(
                Condition::any()
                    .add(forum_reply_body::Column::ReplyId.gt(*entity_id))
                    .add(
                        Condition::all()
                            .add(forum_reply_body::Column::ReplyId.eq(*entity_id))
                            .add(forum_reply_body::Column::Locale.gt(locale.clone())),
                    ),
            );
        }
        let rows = query.all(&self.db).await.map_err(Error::Database)?;
        for row in rows {
            candidates.push(ProjectionCandidate::Reply {
                entity_id: row.reply_id,
                locale: row.locale,
            });
        }
        Ok(candidates)
    }

    async fn category_candidates(
        &self,
        tenant_id: Uuid,
        cursor: Option<&ProjectionCursor>,
        limit: usize,
    ) -> Result<Vec<ProjectionCandidate>> {
        let include_cursor_category = matches!(cursor, Some(ProjectionCursor::Category { .. }));
        let category_limit = limit.saturating_add(usize::from(include_cursor_category));
        let category_limit = u64::try_from(category_limit).map_err(|_| {
            Error::Validation("Forum Search category candidate bound overflow".to_string())
        })?;

        let mut query = forum_category::Entity::find()
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .order_by_asc(forum_category::Column::Id)
            .limit(category_limit);
        if let Some(ProjectionCursor::Category { entity_id, .. }) = cursor {
            query = query.filter(forum_category::Column::Id.gte(*entity_id));
        }
        let categories = query.all(&self.db).await.map_err(Error::Database)?;
        if categories.is_empty() {
            return Ok(Vec::new());
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
            .await
            .map_err(Error::Database)?;
        let binding_by_forum_id = bindings
            .iter()
            .map(|binding| (binding.forum_category_id, binding.taxonomy_category_id))
            .collect::<HashMap<_, _>>();
        for category_id in &category_ids {
            if !binding_by_forum_id.contains_key(category_id) {
                return Err(Error::External(format!(
                    "Forum Search category {category_id} has no Taxonomy Category binding"
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
                CATEGORY_LOCALE_PROJECTION_REQUEST_LOCALE,
                None,
            )
            .await
            .map_err(map_taxonomy_category_error)?;
        let projection_by_taxonomy_id = projections
            .into_iter()
            .map(|projection| (projection.id, projection))
            .collect::<HashMap<_, _>>();

        let mut candidates = Vec::with_capacity(limit);
        for category in categories {
            let taxonomy_id = *binding_by_forum_id.get(&category.id).ok_or_else(|| {
                Error::External(format!(
                    "Forum Search category {} lost its Taxonomy Category binding",
                    category.id
                ))
            })?;
            let projection = projection_by_taxonomy_id.get(&taxonomy_id).ok_or_else(|| {
                Error::External(format!(
                    "Forum Search category {} Taxonomy Category {taxonomy_id} projection is missing",
                    category.id
                ))
            })?;
            ensure_locale_bound(projection.available_locales.len())?;
            if projection.available_locales.is_empty() {
                return Err(Error::External(format!(
                    "Forum Search category {} Taxonomy Category {taxonomy_id} has no canonical locales",
                    category.id
                )));
            }

            for locale in &projection.available_locales {
                if let Some(ProjectionCursor::Category {
                    entity_id,
                    locale: after_locale,
                }) = cursor
                    && (category.id < *entity_id
                        || (category.id == *entity_id && locale <= after_locale))
                {
                    continue;
                }
                candidates.push(ProjectionCandidate::Category {
                    entity_id: category.id,
                    locale: locale.clone(),
                });
                if candidates.len() == limit {
                    return Ok(candidates);
                }
            }
        }
        Ok(candidates)
    }

    async fn project_candidate(
        &self,
        tenant_id: Uuid,
        candidate: &ProjectionCandidate,
    ) -> Result<Option<SearchProjectionDocument>> {
        match candidate {
            ProjectionCandidate::Category { entity_id, locale } => {
                self.project_category(tenant_id, *entity_id, locale).await
            }
            ProjectionCandidate::Topic { entity_id, locale } => {
                self.project_topic(tenant_id, *entity_id, locale).await
            }
            ProjectionCandidate::Reply { entity_id, locale } => {
                self.project_reply(tenant_id, *entity_id, locale).await
            }
        }
    }

    async fn project_category(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        locale: &str,
    ) -> Result<Option<SearchProjectionDocument>> {
        let category = self
            .discovery
            .get_public_category_with_locale_fallback(tenant_id, category_id, locale, None)
            .await
            .map_err(map_forum_error)?;
        let Some(category) = category else {
            return Ok(None);
        };
        if category.effective_locale != locale {
            return Ok(None);
        }
        let route = exact_category_route(&self.db, tenant_id, category.id, locale).await?;
        let owner = forum_category::Entity::find_by_id(category.id)
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(Error::Database)?;
        let Some(owner) = owner else {
            return Ok(None);
        };
        let created_at = owner.created_at.with_timezone(&Utc);
        let updated_at = owner.updated_at.with_timezone(&Utc);
        let description = category.description.clone().unwrap_or_default();
        Ok(Some(SearchProjectionDocument {
            document_key: format!("forum_category:{}:{locale}", category.id),
            tenant_id,
            document_id: category.id,
            source_module: FORUM_SOURCE_MODULE.to_string(),
            entity_type: FORUM_CATEGORY_ENTITY_TYPE.to_string(),
            locale: locale.to_string(),
            status: "public".to_string(),
            is_public: true,
            title: category.name.clone(),
            subtitle: None,
            slug: Some(category.slug.clone()),
            handle: None,
            body: description.clone(),
            keywords_text: format!("{} {} {}", category.name, category.slug, description),
            facets: json!({
                "kind": "forum_category",
                "has_parent": category.parent_id.is_some()
            }),
            payload: json!({
                "category_id": category.id,
                "parent_id": category.parent_id,
                "topic_count": category.topic_count,
                "reply_count": category.reply_count,
                "route": route
            }),
            published_at: Some(created_at),
            created_at,
            updated_at,
        }))
    }

    async fn project_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        locale: &str,
    ) -> Result<Option<SearchProjectionDocument>> {
        let topic = self
            .discovery
            .get_public_topic_with_locale_fallback(tenant_id, topic_id, locale, None, None)
            .await
            .map_err(map_forum_error)?;
        let Some(topic) = topic else {
            return Ok(None);
        };
        if topic.effective_locale != locale {
            return Ok(None);
        }
        let route = exact_topic_route(&self.db, tenant_id, topic.id, locale).await?;
        let category = self
            .discovery
            .get_public_category_with_locale_fallback(tenant_id, topic.category_id, locale, None)
            .await
            .map_err(map_forum_error)?;
        let Some(category) = category else {
            return Ok(None);
        };
        let author =
            load_public_author_summary(&self.db, tenant_id, topic.author_id, locale).await?;
        let author_id = public_author_id(author.as_ref());
        let author_handle = public_author_handle(author.as_ref());
        let author_keywords = public_author_keywords(author.as_ref());
        let author_payload = public_author_payload(author.as_ref());
        let created_at = parse_timestamp(&topic.created_at, "created_at")?;
        let updated_at = parse_timestamp(&topic.updated_at, "updated_at")?;
        let tags = topic.tags.clone();
        let channels = topic.channel_slugs.clone();
        let body = topic.body_plain_text.clone();
        Ok(Some(SearchProjectionDocument {
            document_key: format!("forum_topic:{}:{locale}", topic.id),
            tenant_id,
            document_id: topic.id,
            source_module: FORUM_SOURCE_MODULE.to_string(),
            entity_type: FORUM_TOPIC_ENTITY_TYPE.to_string(),
            locale: locale.to_string(),
            status: topic.status.clone(),
            is_public: true,
            title: topic.title.clone(),
            subtitle: Some(category.name.clone()),
            slug: Some(topic.slug.clone()),
            handle: author_handle,
            body,
            keywords_text: format!(
                "{} {} {} {} {}",
                category.name,
                topic.slug,
                tags.join(" "),
                channels.join(" "),
                author_keywords
            ),
            facets: json!({
                "kind": "forum_topic",
                "category_id": topic.category_id,
                "author_id": author_id,
                "has_public_author": author_id.is_some(),
                "has_tags": !tags.is_empty(),
                "has_channels": !channels.is_empty(),
                "channel_slugs": channels
            }),
            payload: json!({
                "topic_id": topic.id,
                "category_id": topic.category_id,
                "author": author_payload,
                "tags": tags,
                "channel_slugs": topic.channel_slugs,
                "reply_count": topic.reply_count,
                "is_pinned": topic.is_pinned,
                "is_locked": topic.is_locked,
                "solution_reply_id": topic.solution_reply_id,
                "published_at": created_at.to_rfc3339(),
                "route": route
            }),
            published_at: Some(created_at),
            created_at,
            updated_at,
        }))
    }

    async fn project_reply(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        locale: &str,
    ) -> Result<Option<SearchProjectionDocument>> {
        let reply = self
            .discovery
            .get_public_reply_with_locale_fallback(
                tenant_id,
                reply_id,
                locale,
                None,
                None,
                Some(&[ReplyStatus::Approved]),
            )
            .await
            .map_err(map_forum_error)?;
        let Some(reply) = reply else {
            return Ok(None);
        };
        if reply.effective_locale != locale {
            return Ok(None);
        }
        let topic = self
            .discovery
            .get_public_topic_with_locale_fallback(tenant_id, reply.topic_id, locale, None, None)
            .await
            .map_err(map_forum_error)?;
        let Some(topic) = topic else {
            return Ok(None);
        };
        if topic.effective_locale != locale {
            return Ok(None);
        }
        let topic_route = exact_topic_route(&self.db, tenant_id, topic.id, locale).await?;
        let category = self
            .discovery
            .get_public_category_with_locale_fallback(tenant_id, topic.category_id, locale, None)
            .await
            .map_err(map_forum_error)?;
        let Some(category) = category else {
            return Ok(None);
        };
        let author =
            load_public_author_summary(&self.db, tenant_id, reply.author_id, locale).await?;
        let author_id = public_author_id(author.as_ref());
        let author_handle = public_author_handle(author.as_ref());
        let author_keywords = public_author_keywords(author.as_ref());
        let author_payload = public_author_payload(author.as_ref());
        let topic_tags = topic.tags.clone();
        let topic_channel_slugs = topic.channel_slugs.clone();

        let created_at = parse_timestamp(&reply.created_at, "reply.created_at")?;
        let updated_at = parse_timestamp(&reply.updated_at, "reply.updated_at")?;
        let is_solution = reply.is_solution && topic.solution_reply_id == Some(reply_id);
        let route = format!("{topic_route}?reply={reply_id}");
        let body = reply.content_plain_text;
        Ok(Some(SearchProjectionDocument {
            document_key: format!("forum_reply:{reply_id}:{locale}"),
            tenant_id,
            document_id: reply_id,
            source_module: FORUM_SOURCE_MODULE.to_string(),
            entity_type: FORUM_REPLY_ENTITY_TYPE.to_string(),
            locale: locale.to_string(),
            status: reply.status,
            is_public: true,
            title: topic.title.clone(),
            subtitle: Some(category.name.clone()),
            slug: None,
            handle: author_handle,
            body,
            keywords_text: format!(
                "{} {} {} {}",
                category.name, topic.title, topic.slug, author_keywords
            ),
            facets: json!({
                "kind": "forum_reply",
                "category_id": topic.category_id,
                "topic_id": topic.id,
                "author_id": author_id,
                "has_public_author": author_id.is_some(),
                "has_parent": reply.parent_reply_id.is_some(),
                "is_solution": is_solution
            }),
            payload: json!({
                "reply_id": reply_id,
                "topic_id": topic.id,
                "category_id": topic.category_id,
                "author": author_payload,
                "topic_tags": topic_tags,
                "topic_channel_slugs": topic_channel_slugs,
                "parent_reply_id": reply.parent_reply_id,
                "is_solution": is_solution,
                "published_at": created_at.to_rfc3339(),
                "route": route
            }),
            published_at: Some(created_at),
            created_at,
            updated_at,
        }))
    }

    async fn entity_candidates(
        &self,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
    ) -> Result<Vec<ProjectionCandidate>> {
        match entity_type {
            FORUM_CATEGORY_ENTITY_TYPE => {
                let category = forum_category::Entity::find_by_id(entity_id)
                    .filter(forum_category::Column::TenantId.eq(tenant_id))
                    .one(&self.db)
                    .await
                    .map_err(Error::Database)?;
                if category.is_none() {
                    return Ok(Vec::new());
                }
                let binding = forum_category_taxonomy_binding::Entity::find_by_id((
                    tenant_id,
                    entity_id,
                ))
                .one(&self.db)
                .await
                .map_err(Error::Database)?
                .ok_or_else(|| {
                    Error::External(format!(
                        "Forum Search category {entity_id} has no Taxonomy Category binding"
                    ))
                })?;
                let mut projections = TaxonomyOwnerCategoryReader::new(self.db.clone())
                    .load_scoped_categories(
                        tenant_id,
                        TaxonomyScopeType::Module,
                        Some("forum"),
                        Some(&[binding.taxonomy_category_id]),
                        CATEGORY_LOCALE_PROJECTION_REQUEST_LOCALE,
                        None,
                    )
                    .await
                    .map_err(map_taxonomy_category_error)?;
                let projection = projections.pop().ok_or_else(|| {
                    Error::External(format!(
                        "Forum Search category {entity_id} Taxonomy Category {} projection is missing",
                        binding.taxonomy_category_id
                    ))
                })?;
                if !projections.is_empty() || projection.id != binding.taxonomy_category_id {
                    return Err(Error::External(format!(
                        "Forum Search category {entity_id} Taxonomy projection returned an inconsistent identity"
                    )));
                }
                ensure_locale_bound(projection.available_locales.len())?;
                if projection.available_locales.is_empty() {
                    return Err(Error::External(format!(
                        "Forum Search category {entity_id} Taxonomy Category {} has no canonical locales",
                        binding.taxonomy_category_id
                    )));
                }
                Ok(projection
                    .available_locales
                    .into_iter()
                    .map(|locale| ProjectionCandidate::Category { entity_id, locale })
                    .collect())
            }
            FORUM_TOPIC_ENTITY_TYPE => {
                let rows = forum_topic_translation::Entity::find()
                    .filter(forum_topic_translation::Column::TenantId.eq(tenant_id))
                    .filter(forum_topic_translation::Column::TopicId.eq(entity_id))
                    .order_by_asc(forum_topic_translation::Column::Locale)
                    .limit(MAX_ENTITY_LOCALES + 1)
                    .all(&self.db)
                    .await
                    .map_err(Error::Database)?;
                ensure_locale_bound(rows.len())?;
                Ok(rows
                    .into_iter()
                    .map(|row| ProjectionCandidate::Topic {
                        entity_id,
                        locale: row.locale,
                    })
                    .collect())
            }
            FORUM_REPLY_ENTITY_TYPE => {
                let rows = forum_reply_body::Entity::find()
                    .filter(forum_reply_body::Column::TenantId.eq(tenant_id))
                    .filter(forum_reply_body::Column::ReplyId.eq(entity_id))
                    .order_by_asc(forum_reply_body::Column::Locale)
                    .limit(MAX_ENTITY_LOCALES + 1)
                    .all(&self.db)
                    .await
                    .map_err(Error::Database)?;
                ensure_locale_bound(rows.len())?;
                Ok(rows
                    .into_iter()
                    .map(|row| ProjectionCandidate::Reply {
                        entity_id,
                        locale: row.locale,
                    })
                    .collect())
            }
            _ => Err(Error::Validation(format!(
                "Unsupported Forum Search projection entity type `{entity_type}`"
            ))),
        }
    }
}

#[async_trait]
impl SearchProjectionSource for ForumSearchProjectionSource {
    fn source_module(&self) -> &'static str {
        FORUM_SOURCE_MODULE
    }

    async fn list_public_documents(
        &self,
        tenant_id: Uuid,
        after: Option<String>,
        limit: usize,
    ) -> Result<SearchProjectionPage> {
        if !(1..=MAX_SEARCH_PROJECTION_PAGE_SIZE).contains(&limit) {
            return Err(Error::Validation(format!(
                "Forum Search projection page size must be between 1 and {MAX_SEARCH_PROJECTION_PAGE_SIZE}"
            )));
        }
        let cursor = after.as_deref().map(ProjectionCursor::parse).transpose()?;
        let candidates = self.candidate_page(tenant_id, cursor, limit).await?;
        let next_cursor = (candidates.len() == limit)
            .then(|| candidates.last().map(ProjectionCandidate::cursor))
            .flatten();
        let mut documents = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            if let Some(document) = self.project_candidate(tenant_id, &candidate).await? {
                documents.push(document);
            }
        }
        Ok(SearchProjectionPage {
            documents,
            next_cursor,
        })
    }

    async fn load_public_entity(
        &self,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
    ) -> Result<Vec<SearchProjectionDocument>> {
        let candidates = self
            .entity_candidates(tenant_id, entity_type, entity_id)
            .await?;
        let mut documents = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            if let Some(document) = self.project_candidate(tenant_id, &candidate).await? {
                documents.push(document);
            }
        }
        Ok(documents)
    }
}

#[derive(Clone, Debug)]
enum ProjectionCandidate {
    Category { entity_id: Uuid, locale: String },
    Topic { entity_id: Uuid, locale: String },
    Reply { entity_id: Uuid, locale: String },
}

impl ProjectionCandidate {
    fn cursor(&self) -> String {
        match self {
            Self::Category { entity_id, locale } => format!("category:{entity_id}:{locale}"),
            Self::Topic { entity_id, locale } => format!("topic:{entity_id}:{locale}"),
            Self::Reply { entity_id, locale } => format!("reply:{entity_id}:{locale}"),
        }
    }
}

#[derive(Clone, Debug)]
enum ProjectionCursor {
    Category { entity_id: Uuid, locale: String },
    Topic { entity_id: Uuid, locale: String },
    Reply { entity_id: Uuid, locale: String },
}

impl ProjectionCursor {
    fn parse(value: &str) -> Result<Self> {
        let mut parts = value.splitn(3, ':');
        let kind = parts.next().unwrap_or_default();
        let entity_id = parts
            .next()
            .ok_or_else(|| Error::Validation("Invalid Forum Search projection cursor".to_string()))
            .and_then(|value| {
                Uuid::parse_str(value).map_err(|_| {
                    Error::Validation("Invalid Forum Search projection cursor UUID".to_string())
                })
            })?;
        let locale = parts.next().unwrap_or_default().trim().to_string();
        if locale.is_empty() || locale.len() > 16 || locale.contains(':') {
            return Err(Error::Validation(
                "Invalid Forum Search projection cursor locale".to_string(),
            ));
        }
        match kind {
            "category" => Ok(Self::Category { entity_id, locale }),
            "topic" => Ok(Self::Topic { entity_id, locale }),
            "reply" => Ok(Self::Reply { entity_id, locale }),
            _ => Err(Error::Validation(
                "Invalid Forum Search projection cursor kind".to_string(),
            )),
        }
    }
}

async fn exact_category_route(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
    locale: &str,
) -> Result<String> {
    let descriptor = ForumCategoryRouteService::new(db.clone())
        .canonical_descriptor(tenant_id, category_id, locale, None)
        .await
        .map_err(map_forum_error)?;
    if descriptor.category_id != category_id || descriptor.locale != locale {
        return Err(Error::External(
            "Forum Search category route owner returned a non-exact descriptor".to_string(),
        ));
    }
    Ok(descriptor.path)
}

async fn exact_topic_route(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
    locale: &str,
) -> Result<String> {
    let descriptor = ForumTopicRouteService::new(db.clone())
        .canonical_descriptor(tenant_id, topic_id, locale)
        .await
        .map_err(map_forum_error)?;
    if descriptor.topic_id != topic_id || descriptor.locale != locale {
        return Err(Error::External(
            "Forum Search topic route owner returned a non-exact descriptor".to_string(),
        ));
    }
    Ok(descriptor.path)
}

fn ensure_locale_bound(count: usize) -> Result<()> {
    if count as u64 > MAX_ENTITY_LOCALES {
        Err(Error::Validation(format!(
            "Forum Search projection entity exceeds {MAX_ENTITY_LOCALES} locales"
        )))
    } else {
        Ok(())
    }
}

fn parse_timestamp(value: &str, field: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|_| Error::Validation(format!("Forum Search projection {field} is not RFC3339")))
}

fn map_forum_error(error: crate::ForumError) -> Error {
    Error::External(format!("Forum Search projection source failed: {error}"))
}

fn map_taxonomy_category_error(error: TaxonomyError) -> Error {
    match error {
        TaxonomyError::Database(error) => Error::Database(error),
        other => Error::External(format!(
            "Forum Search Taxonomy category projection failed: {other}"
        )),
    }
}
