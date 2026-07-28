use crate::entities;
use crate::error::{CommerceError, CommerceResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StorefrontProductSortBy {
    #[default]
    PublishedAt,
    CreatedAt,
}

impl StorefrontProductSortBy {
    fn parse(value: Option<String>) -> CommerceResult<Self> {
        match value.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
            None | Some("published_at") => Ok(Self::PublishedAt),
            Some("created_at") => Ok(Self::CreatedAt),
            Some(_) => Err(CommerceError::Validation(
                "sort_by must be `published_at` or `created_at`".to_string(),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StorefrontProductSortDirection {
    Asc,
    #[default]
    Desc,
}

impl StorefrontProductSortDirection {
    fn parse(value: Option<String>) -> CommerceResult<Self> {
        match value.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
            None | Some("desc") => Ok(Self::Desc),
            Some("asc") => Ok(Self::Asc),
            Some(_) => Err(CommerceError::Validation(
                "sort_direction must be `asc` or `desc`".to_string(),
            )),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StorefrontProductListQuery {
    pub search: Option<String>,
    pub category_id: Option<Uuid>,
    pub sort_by: StorefrontProductSortBy,
    pub sort_direction: StorefrontProductSortDirection,
}

impl StorefrontProductListQuery {
    pub fn try_new(
        search: Option<String>,
        category_id: Option<Uuid>,
        sort_by: Option<String>,
        sort_direction: Option<String>,
    ) -> CommerceResult<Self> {
        Ok(Self {
            search: search
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            category_id,
            sort_by: StorefrontProductSortBy::parse(sort_by)?,
            sort_direction: StorefrontProductSortDirection::parse(sort_direction)?,
        })
    }

    pub fn try_from_transport(
        search: Option<String>,
        category_id: Option<String>,
        sort_by: Option<String>,
        sort_direction: Option<String>,
    ) -> CommerceResult<Self> {
        let category_id = category_id
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(|value| {
                Uuid::parse_str(value.as_str()).map_err(|_| {
                    CommerceError::Validation("category_id must be a UUID".to_string())
                })
            })
            .transpose()?;
        Self::try_new(search, category_id, sort_by, sort_direction)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorefrontProductList {
    pub items: Vec<StorefrontProductListItem>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub has_next: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorefrontProductListItem {
    pub id: Uuid,
    pub status: entities::product::ProductStatus,
    pub title: String,
    pub handle: String,
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub tags: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub struct ProductTagState {
    pub tags: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storefront_list_query_rejects_invalid_transport_values() {
        assert!(
            StorefrontProductListQuery::try_from_transport(
                None,
                Some("invalid".to_string()),
                None,
                None,
            )
            .is_err()
        );
        assert!(
            StorefrontProductListQuery::try_new(
                None,
                None,
                Some("title".to_string()),
                None,
            )
            .is_err()
        );
        assert!(
            StorefrontProductListQuery::try_new(
                None,
                None,
                None,
                Some("sideways".to_string()),
            )
            .is_err()
        );
    }
}
