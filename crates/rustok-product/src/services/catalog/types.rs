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
    pub(crate) fn parse(value: Option<String>) -> CommerceResult<Self> {
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
    pub(crate) fn parse(value: Option<String>) -> CommerceResult<Self> {
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
            search: normalize_optional_text(search),
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
        Self::try_new(
            search,
            parse_optional_uuid(category_id, "category_id")?,
            sort_by,
            sort_direction,
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AdminProductListQuery {
    pub search: Option<String>,
    pub status: Option<entities::product::ProductStatus>,
    pub category_id: Option<Uuid>,
    pub sort_by: StorefrontProductSortBy,
    pub sort_direction: StorefrontProductSortDirection,
}

impl AdminProductListQuery {
    pub fn try_from_transport(
        search: Option<String>,
        status: Option<String>,
        category_id: Option<String>,
        sort_by: Option<String>,
        sort_direction: Option<String>,
    ) -> CommerceResult<Self> {
        let status = normalize_optional_text(status)
            .map(|value| match value.to_ascii_lowercase().as_str() {
                "draft" => Ok(entities::product::ProductStatus::Draft),
                "active" => Ok(entities::product::ProductStatus::Active),
                "archived" => Ok(entities::product::ProductStatus::Archived),
                _ => Err(CommerceError::Validation(
                    "status must be `draft`, `active`, or `archived`".to_string(),
                )),
            })
            .transpose()?;
        Ok(Self {
            search: normalize_optional_text(search),
            status,
            category_id: parse_optional_uuid(category_id, "category_id")?,
            sort_by: StorefrontProductSortBy::parse(sort_by)?,
            sort_direction: StorefrontProductSortDirection::parse(sort_direction)?,
        })
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminProductList {
    pub items: Vec<AdminProductListItem>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub has_next: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminProductListItem {
    pub id: Uuid,
    pub status: entities::product::ProductStatus,
    pub title: String,
    pub handle: String,
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub primary_category_id: Option<Uuid>,
    pub tags: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub struct ProductTagState {
    pub tags: Vec<String>,
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_optional_uuid(value: Option<String>, field: &str) -> CommerceResult<Option<Uuid>> {
    normalize_optional_text(value)
        .map(|value| {
            Uuid::parse_str(value.as_str())
                .map_err(|_| CommerceError::Validation(format!("{field} must be a UUID")))
        })
        .transpose()
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

    #[test]
    fn admin_list_query_normalizes_and_validates_transport_values() {
        let query = AdminProductListQuery::try_from_transport(
            Some("  camera  ".to_string()),
            Some("ACTIVE".to_string()),
            None,
            Some("created_at".to_string()),
            Some("asc".to_string()),
        )
        .expect("valid admin list query");
        assert_eq!(query.search.as_deref(), Some("camera"));
        assert_eq!(query.status, Some(entities::product::ProductStatus::Active));
        assert_eq!(query.sort_by, StorefrontProductSortBy::CreatedAt);
        assert_eq!(query.sort_direction, StorefrontProductSortDirection::Asc);

        assert!(
            AdminProductListQuery::try_from_transport(
                None,
                Some("deleted".to_string()),
                None,
                None,
                None,
            )
            .is_err()
        );
    }
}
