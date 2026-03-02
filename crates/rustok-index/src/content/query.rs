use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, Order, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::entity::{self as index_content_entity, Column};
use super::model::IndexContentModel;
use crate::error::IndexResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentSortBy {
    PublishedAt,
    CreatedAt,
    UpdatedAt,
    Title,
    Position,
    ViewCount,
    ReplyCount,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortOrder {
    Asc,
    Desc,
}

#[derive(Debug, Clone)]
pub struct ContentQuery {
    pub tenant_id: Uuid,
    pub locale: String,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub category_id: Option<Uuid>,
    pub author_id: Option<Uuid>,
    pub tag_slug: Option<String>,
    pub search: Option<String>,
    pub parent_id: Option<Uuid>,
    pub sort_by: ContentSortBy,
    pub sort_order: SortOrder,
    pub limit: u64,
    pub offset: u64,
}

pub struct ContentQueryBuilder {
    query: ContentQuery,
}

impl ContentQueryBuilder {
    pub fn new(tenant_id: Uuid, locale: impl Into<String>) -> Self {
        Self {
            query: ContentQuery {
                tenant_id,
                locale: locale.into(),
                kind: None,
                status: Some("published".to_string()),
                category_id: None,
                author_id: None,
                tag_slug: None,
                search: None,
                parent_id: None,
                sort_by: ContentSortBy::PublishedAt,
                sort_order: SortOrder::Desc,
                limit: 20,
                offset: 0,
            },
        }
    }

    pub fn kind(mut self, kind: impl Into<String>) -> Self {
        self.query.kind = Some(kind.into());
        self
    }

    pub fn status(mut self, status: impl Into<String>) -> Self {
        self.query.status = Some(status.into());
        self
    }

    pub fn category(mut self, category_id: Uuid) -> Self {
        self.query.category_id = Some(category_id);
        self
    }

    pub fn author(mut self, author_id: Uuid) -> Self {
        self.query.author_id = Some(author_id);
        self
    }

    pub fn tag(mut self, tag_slug: impl Into<String>) -> Self {
        self.query.tag_slug = Some(tag_slug.into());
        self
    }

    pub fn search(mut self, q: impl Into<String>) -> Self {
        self.query.search = Some(q.into());
        self
    }

    pub fn parent(mut self, parent_id: Uuid) -> Self {
        self.query.parent_id = Some(parent_id);
        self
    }

    pub fn sort(mut self, sort_by: ContentSortBy, order: SortOrder) -> Self {
        self.query.sort_by = sort_by;
        self.query.sort_order = order;
        self
    }

    pub fn paginate(mut self, limit: u64, offset: u64) -> Self {
        self.query.limit = limit;
        self.query.offset = offset;
        self
    }

    pub fn build(self) -> ContentQuery {
        self.query
    }
}

fn sort_column(sort_by: &ContentSortBy) -> Column {
    match sort_by {
        ContentSortBy::PublishedAt => Column::PublishedAt,
        ContentSortBy::CreatedAt => Column::CreatedAt,
        ContentSortBy::UpdatedAt => Column::UpdatedAt,
        ContentSortBy::Title => Column::Title,
        ContentSortBy::Position => Column::Position,
        ContentSortBy::ViewCount => Column::ViewCount,
        ContentSortBy::ReplyCount => Column::ReplyCount,
    }
}

fn sort_order(order: &SortOrder) -> Order {
    match order {
        SortOrder::Asc => Order::Asc,
        SortOrder::Desc => Order::Desc,
    }
}

fn row_to_model(row: index_content_entity::Model) -> IndexContentModel {
    use chrono::DateTime;
    IndexContentModel {
        id: row.id,
        tenant_id: row.tenant_id,
        node_id: row.node_id,
        locale: row.locale,
        kind: row.kind,
        status: row.status,
        title: row.title,
        slug: row.slug,
        excerpt: row.excerpt,
        body: row.body,
        body_format: row.body_format,
        author_id: row.author_id,
        author_name: row.author_name,
        author_avatar: row.author_avatar,
        category_id: row.category_id,
        category_name: row.category_name,
        category_slug: row.category_slug,
        tags: serde_json::from_value(row.tags).unwrap_or_default(),
        meta_title: row.meta_title,
        meta_description: row.meta_description,
        og_image: row.og_image,
        featured_image_url: row.featured_image_url,
        featured_image_alt: row.featured_image_alt,
        parent_id: row.parent_id,
        depth: row.depth,
        position: row.position,
        reply_count: row.reply_count,
        view_count: row.view_count,
        published_at: row.published_at.map(|dt| DateTime::from(dt)),
        created_at: DateTime::from(row.created_at),
        updated_at: DateTime::from(row.updated_at),
    }
}

/// Query service for content index
pub struct ContentQueryService {
    db: DatabaseConnection,
}

impl ContentQueryService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn find(&self, query: ContentQuery) -> IndexResult<Vec<IndexContentModel>> {
        let mut select = index_content_entity::Entity::find()
            .filter(Column::TenantId.eq(query.tenant_id))
            .filter(Column::Locale.eq(&query.locale));

        if let Some(kind) = &query.kind {
            select = select.filter(Column::Kind.eq(kind));
        }
        if let Some(status) = &query.status {
            select = select.filter(Column::Status.eq(status));
        }
        if let Some(category_id) = query.category_id {
            select = select.filter(Column::CategoryId.eq(category_id));
        }
        if let Some(author_id) = query.author_id {
            select = select.filter(Column::AuthorId.eq(author_id));
        }
        if let Some(parent_id) = query.parent_id {
            select = select.filter(Column::ParentId.eq(parent_id));
        }

        let col = sort_column(&query.sort_by);
        let ord = sort_order(&query.sort_order);
        select = select.order_by(col, ord);
        select = select.limit(query.limit).offset(query.offset);

        let rows = select.all(&self.db).await?;
        Ok(rows.into_iter().map(row_to_model).collect())
    }

    pub async fn find_by_slug(
        &self,
        tenant_id: Uuid,
        locale: &str,
        kind: &str,
        slug: &str,
    ) -> IndexResult<Option<IndexContentModel>> {
        let row = index_content_entity::Entity::find()
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::Locale.eq(locale))
            .filter(Column::Kind.eq(kind))
            .filter(Column::Slug.eq(slug))
            .one(&self.db)
            .await?;
        Ok(row.map(row_to_model))
    }

    pub async fn count(&self, query: ContentQuery) -> IndexResult<u64> {
        let mut select = index_content_entity::Entity::find()
            .filter(Column::TenantId.eq(query.tenant_id))
            .filter(Column::Locale.eq(&query.locale));

        if let Some(kind) = &query.kind {
            select = select.filter(Column::Kind.eq(kind));
        }
        if let Some(status) = &query.status {
            select = select.filter(Column::Status.eq(status));
        }
        if let Some(category_id) = query.category_id {
            select = select.filter(Column::CategoryId.eq(category_id));
        }
        if let Some(author_id) = query.author_id {
            select = select.filter(Column::AuthorId.eq(author_id));
        }
        if let Some(parent_id) = query.parent_id {
            select = select.filter(Column::ParentId.eq(parent_id));
        }

        let count = select.count(&self.db).await?;
        Ok(count)
    }

    pub async fn search(
        &self,
        tenant_id: Uuid,
        locale: &str,
        q: &str,
        limit: u64,
    ) -> IndexResult<Vec<IndexContentModel>> {
        use sea_orm::sea_query::Expr;

        let q_escaped = q.replace('\'', "''");
        let tsquery = format!("plainto_tsquery('simple', '{}')", q_escaped);
        let rank_expr = format!("ts_rank(search_vector, {})", tsquery);
        let match_expr = format!("search_vector @@ {}", tsquery);

        let rows = index_content_entity::Entity::find()
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::Locale.eq(locale))
            .filter(Column::Status.eq("published"))
            .filter(Expr::cust(match_expr))
            .order_by(Expr::cust(rank_expr), Order::Desc)
            .limit(limit)
            .all(&self.db)
            .await?;

        Ok(rows.into_iter().map(row_to_model).collect())
    }
}
