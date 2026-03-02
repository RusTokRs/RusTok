use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, Order, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::entity::{self as index_products_entity, Column};
use super::model::{IndexProductImage, IndexProductModel, IndexProductOption};
use crate::error::IndexResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProductSortBy {
    PublishedAt,
    CreatedAt,
    UpdatedAt,
    Title,
    PriceMin,
    PriceMax,
    SalesCount,
    ViewCount,
    Rating,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortOrder {
    Asc,
    Desc,
}

#[derive(Debug, Clone)]
pub struct ProductQuery {
    pub tenant_id: Uuid,
    pub locale: String,
    pub category_id: Option<Uuid>,
    pub tag: Option<String>,
    pub search: Option<String>,
    pub in_stock: Option<bool>,
    pub is_published: Option<bool>,
    pub price_min: Option<i64>,
    pub price_max: Option<i64>,
    pub sort_by: ProductSortBy,
    pub sort_order: SortOrder,
    pub limit: u64,
    pub offset: u64,
}

pub struct ProductQueryBuilder {
    query: ProductQuery,
}

impl ProductQueryBuilder {
    pub fn new(tenant_id: Uuid, locale: impl Into<String>) -> Self {
        Self {
            query: ProductQuery {
                tenant_id,
                locale: locale.into(),
                category_id: None,
                tag: None,
                search: None,
                in_stock: None,
                is_published: Some(true),
                price_min: None,
                price_max: None,
                sort_by: ProductSortBy::PublishedAt,
                sort_order: SortOrder::Desc,
                limit: 20,
                offset: 0,
            },
        }
    }

    pub fn category(mut self, category_id: Uuid) -> Self {
        self.query.category_id = Some(category_id);
        self
    }

    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.query.tag = Some(tag.into());
        self
    }

    pub fn search(mut self, q: impl Into<String>) -> Self {
        self.query.search = Some(q.into());
        self
    }

    pub fn in_stock(mut self, in_stock: bool) -> Self {
        self.query.in_stock = Some(in_stock);
        self
    }

    pub fn published(mut self, published: bool) -> Self {
        self.query.is_published = Some(published);
        self
    }

    pub fn price_range(mut self, min: Option<i64>, max: Option<i64>) -> Self {
        self.query.price_min = min;
        self.query.price_max = max;
        self
    }

    pub fn sort(mut self, sort_by: ProductSortBy, order: SortOrder) -> Self {
        self.query.sort_by = sort_by;
        self.query.sort_order = order;
        self
    }

    pub fn paginate(mut self, limit: u64, offset: u64) -> Self {
        self.query.limit = limit;
        self.query.offset = offset;
        self
    }

    pub fn build(self) -> ProductQuery {
        self.query
    }
}

fn sort_column(sort_by: &ProductSortBy) -> Column {
    match sort_by {
        ProductSortBy::PublishedAt => Column::PublishedAt,
        ProductSortBy::CreatedAt => Column::CreatedAt,
        ProductSortBy::UpdatedAt => Column::UpdatedAt,
        ProductSortBy::Title => Column::Title,
        ProductSortBy::PriceMin => Column::PriceMin,
        ProductSortBy::PriceMax => Column::PriceMax,
        ProductSortBy::SalesCount => Column::SalesCount,
        ProductSortBy::ViewCount => Column::ViewCount,
        ProductSortBy::Rating => Column::Rating,
    }
}

fn sort_order(order: &SortOrder) -> Order {
    match order {
        SortOrder::Asc => Order::Asc,
        SortOrder::Desc => Order::Desc,
    }
}

fn row_to_model(row: index_products_entity::Model) -> IndexProductModel {
    use chrono::DateTime;
    IndexProductModel {
        id: row.id,
        tenant_id: row.tenant_id,
        product_id: row.product_id,
        locale: row.locale,
        status: row.status,
        is_published: row.is_published,
        title: row.title,
        subtitle: row.subtitle,
        handle: row.handle,
        description: row.description,
        category_id: row.category_id,
        category_name: row.category_name,
        category_path: row.category_path,
        tags: serde_json::from_value(row.tags).unwrap_or_default(),
        brand: row.brand,
        currency: row.currency,
        price_min: row.price_min,
        price_max: row.price_max,
        compare_at_price_min: row.compare_at_price_min,
        compare_at_price_max: row.compare_at_price_max,
        on_sale: row.on_sale,
        in_stock: row.in_stock,
        total_inventory: row.total_inventory,
        variant_count: row.variant_count,
        options: serde_json::from_value(row.options).unwrap_or_default(),
        thumbnail_url: row.thumbnail_url,
        images: serde_json::from_value(row.images).unwrap_or_default(),
        meta_title: row.meta_title,
        meta_description: row.meta_description,
        attributes: row.attributes,
        sales_count: row.sales_count,
        view_count: row.view_count,
        rating: row.rating.map(|d| {
            use rust_decimal::prelude::ToPrimitive;
            d.to_f32().unwrap_or(0.0)
        }),
        review_count: row.review_count,
        published_at: row.published_at.map(DateTime::from),
        created_at: DateTime::from(row.created_at),
        updated_at: DateTime::from(row.updated_at),
    }
}

/// Query service for product index
pub struct ProductQueryService {
    db: DatabaseConnection,
}

impl ProductQueryService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn find(&self, query: ProductQuery) -> IndexResult<Vec<IndexProductModel>> {
        let mut select = index_products_entity::Entity::find()
            .filter(Column::TenantId.eq(query.tenant_id))
            .filter(Column::Locale.eq(&query.locale));

        if let Some(is_published) = query.is_published {
            select = select.filter(Column::IsPublished.eq(is_published));
        }
        if let Some(category_id) = query.category_id {
            select = select.filter(Column::CategoryId.eq(category_id));
        }
        if let Some(in_stock) = query.in_stock {
            select = select.filter(Column::InStock.eq(in_stock));
        }
        if let Some(price_min) = query.price_min {
            select = select.filter(Column::PriceMin.gte(price_min));
        }
        if let Some(price_max) = query.price_max {
            select = select.filter(Column::PriceMax.lte(price_max));
        }

        let col = sort_column(&query.sort_by);
        let ord = sort_order(&query.sort_order);
        select = select.order_by(col, ord);
        select = select.limit(query.limit).offset(query.offset);

        let rows = select.all(&self.db).await?;
        Ok(rows.into_iter().map(row_to_model).collect())
    }

    pub async fn find_by_handle(
        &self,
        tenant_id: Uuid,
        locale: &str,
        handle: &str,
    ) -> IndexResult<Option<IndexProductModel>> {
        let row = index_products_entity::Entity::find()
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::Locale.eq(locale))
            .filter(Column::Handle.eq(handle))
            .one(&self.db)
            .await?;
        Ok(row.map(row_to_model))
    }

    pub async fn count(&self, query: ProductQuery) -> IndexResult<u64> {
        let mut select = index_products_entity::Entity::find()
            .filter(Column::TenantId.eq(query.tenant_id))
            .filter(Column::Locale.eq(&query.locale));

        if let Some(is_published) = query.is_published {
            select = select.filter(Column::IsPublished.eq(is_published));
        }
        if let Some(category_id) = query.category_id {
            select = select.filter(Column::CategoryId.eq(category_id));
        }
        if let Some(in_stock) = query.in_stock {
            select = select.filter(Column::InStock.eq(in_stock));
        }
        if let Some(price_min) = query.price_min {
            select = select.filter(Column::PriceMin.gte(price_min));
        }
        if let Some(price_max) = query.price_max {
            select = select.filter(Column::PriceMax.lte(price_max));
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
    ) -> IndexResult<Vec<IndexProductModel>> {
        use sea_orm::sea_query::Expr;

        let q_escaped = q.replace('\'', "''");
        let tsquery = format!("plainto_tsquery('simple', '{}')", q_escaped);
        let rank_expr = format!("ts_rank(search_vector, {})", tsquery);
        let match_expr = format!("search_vector @@ {}", tsquery);

        let rows = index_products_entity::Entity::find()
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::Locale.eq(locale))
            .filter(Column::IsPublished.eq(true))
            .filter(Expr::cust(match_expr))
            .order_by(Expr::cust(rank_expr), Order::Desc)
            .limit(limit)
            .all(&self.db)
            .await?;

        Ok(rows.into_iter().map(row_to_model).collect())
    }
}
