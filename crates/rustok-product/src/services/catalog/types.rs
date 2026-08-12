use crate::entities;
use crate::error::{CommerceError, CommerceResult};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

const MAX_ATTRIBUTE_FILTERS: usize = 8;
const MAX_ATTRIBUTE_FILTER_CODE_LENGTH: usize = 128;
const MAX_ATTRIBUTE_FILTER_VALUE_LENGTH: usize = 512;

/// Maximum effective Storefront Product title-search input in UTF-8 bytes.
///
/// The owner query wraps the effective search with one leading and one trailing `%`. The shared Index
/// `TextLike` contract accepts at most 1024 bytes, so 1022 keeps the owner-owned search surface exactly
/// representable without truncation in the non-serving Index shadow path.
pub const MAX_STOREFRONT_PRODUCT_SEARCH_BYTES: usize = 1022;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StorefrontProductSortBy {
    #[default]
    PublishedAt,
    CreatedAt,
}

impl StorefrontProductSortBy {
    pub(crate) fn parse(value: Option<String>) -> CommerceResult<Self> {
        match value
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
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
        match value
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            None | Some("desc") => Ok(Self::Desc),
            Some("asc") => Ok(Self::Asc),
            Some(_) => Err(CommerceError::Validation(
                "sort_direction must be `asc` or `desc`".to_string(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductAttributeFilter {
    pub code: String,
    pub value: String,
}

impl ProductAttributeFilter {
    fn parse(value: String) -> CommerceResult<Self> {
        let (code, raw_value) = value.split_once('=').ok_or_else(|| {
            CommerceError::Validation("attribute_filters entries must use `code=value`".to_string())
        })?;
        let filter = Self {
            code: code.trim().to_string(),
            value: raw_value.trim().to_string(),
        };
        validate_attribute_filter(&filter)?;
        Ok(filter)
    }
}

fn validate_attribute_filter(filter: &ProductAttributeFilter) -> CommerceResult<()> {
    let code = filter.code.as_str();
    if code.is_empty()
        || code.len() > MAX_ATTRIBUTE_FILTER_CODE_LENGTH
        || !code
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        return Err(CommerceError::Validation(
            "attribute filter code must contain 1..128 ASCII letters, digits, `_`, or `-`"
                .to_string(),
        ));
    }
    if filter.value.is_empty() || filter.value.len() > MAX_ATTRIBUTE_FILTER_VALUE_LENGTH {
        return Err(CommerceError::Validation(
            "attribute filter value must contain 1..512 characters".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_product_attribute_filters(
    filters: &[ProductAttributeFilter],
) -> CommerceResult<()> {
    if filters.len() > MAX_ATTRIBUTE_FILTERS {
        return Err(CommerceError::Validation(format!(
            "attribute_filters supports at most {MAX_ATTRIBUTE_FILTERS} entries"
        )));
    }
    let mut seen = HashSet::new();
    for filter in filters {
        validate_attribute_filter(filter)?;
        if !seen.insert(filter.code.to_ascii_lowercase()) {
            return Err(CommerceError::Validation(format!(
                "attribute filter {} occurs more than once",
                filter.code
            )));
        }
    }
    Ok(())
}

fn parse_attribute_filters(values: Vec<String>) -> CommerceResult<Vec<ProductAttributeFilter>> {
    let filters = values
        .into_iter()
        .map(ProductAttributeFilter::parse)
        .collect::<CommerceResult<Vec<_>>>()?;
    validate_product_attribute_filters(&filters)?;
    Ok(filters)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorefrontProductListQuery {
    pub search: Option<String>,
    pub category_id: Option<Uuid>,
    pub sort_by: StorefrontProductSortBy,
    pub sort_direction: StorefrontProductSortDirection,
    pub attribute_filters: Vec<ProductAttributeFilter>,
    pub page: u64,
    pub per_page: u64,
}

impl Default for StorefrontProductListQuery {
    fn default() -> Self {
        Self {
            search: None,
            category_id: None,
            sort_by: StorefrontProductSortBy::default(),
            sort_direction: StorefrontProductSortDirection::default(),
            attribute_filters: Vec::new(),
            page: 1,
            per_page: 12,
        }
    }
}

impl StorefrontProductListQuery {
    pub fn with_pagination(mut self, page: u64, per_page: u64) -> Self {
        self.page = page;
        self.per_page = per_page;
        self
    }

    pub fn try_new(
        search: Option<String>,
        category_id: Option<Uuid>,
        sort_by: Option<String>,
        sort_direction: Option<String>,
    ) -> CommerceResult<Self> {
        Self::try_new_with_attribute_filters(
            search,
            category_id,
            sort_by,
            sort_direction,
            Vec::new(),
        )
    }

    pub fn try_new_with_attribute_filters(
        search: Option<String>,
        category_id: Option<Uuid>,
        sort_by: Option<String>,
        sort_direction: Option<String>,
        attribute_filters: Vec<String>,
    ) -> CommerceResult<Self> {
        Ok(Self {
            search: normalize_storefront_product_search(search)?,
            category_id,
            sort_by: StorefrontProductSortBy::parse(sort_by)?,
            sort_direction: StorefrontProductSortDirection::parse(sort_direction)?,
            attribute_filters: parse_attribute_filters(attribute_filters)?,
            ..Self::default()
        })
    }

    pub fn try_from_transport(
        search: Option<String>,
        category_id: Option<String>,
        sort_by: Option<String>,
        sort_direction: Option<String>,
    ) -> CommerceResult<Self> {
        Self::try_from_transport_with_attribute_filters(
            search,
            category_id,
            sort_by,
            sort_direction,
            Vec::new(),
        )
    }

    pub fn try_from_transport_with_attribute_filters(
        search: Option<String>,
        category_id: Option<String>,
        sort_by: Option<String>,
        sort_direction: Option<String>,
        attribute_filters: Vec<String>,
    ) -> CommerceResult<Self> {
        Self::try_new_with_attribute_filters(
            search,
            parse_optional_uuid(category_id, "category_id")?,
            sort_by,
            sort_direction,
            attribute_filters,
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
    pub attribute_filters: Vec<ProductAttributeFilter>,
}

impl AdminProductListQuery {
    pub fn try_from_transport(
        search: Option<String>,
        status: Option<String>,
        category_id: Option<String>,
        sort_by: Option<String>,
        sort_direction: Option<String>,
    ) -> CommerceResult<Self> {
        Self::try_from_transport_with_attribute_filters(
            search,
            status,
            category_id,
            sort_by,
            sort_direction,
            Vec::new(),
        )
    }

    pub fn try_from_transport_with_attribute_filters(
        search: Option<String>,
        status: Option<String>,
        category_id: Option<String>,
        sort_by: Option<String>,
        sort_direction: Option<String>,
        attribute_filters: Vec<String>,
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
            attribute_filters: parse_attribute_filters(attribute_filters)?,
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

pub(crate) fn validate_storefront_product_search(search: Option<&str>) -> CommerceResult<()> {
    let Some(search) = search.map(str::trim).filter(|search| !search.is_empty()) else {
        return Ok(());
    };
    if search.len() > MAX_STOREFRONT_PRODUCT_SEARCH_BYTES {
        return Err(CommerceError::Validation(format!(
            "search must contain at most {MAX_STOREFRONT_PRODUCT_SEARCH_BYTES} UTF-8 bytes"
        )));
    }
    Ok(())
}

fn normalize_storefront_product_search(value: Option<String>) -> CommerceResult<Option<String>> {
    let value = normalize_optional_text(value);
    validate_storefront_product_search(value.as_deref())?;
    Ok(value)
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
            StorefrontProductListQuery::try_new(None, None, Some("title".to_string()), None,)
                .is_err()
        );
        assert!(
            StorefrontProductListQuery::try_new_with_attribute_filters(
                None,
                None,
                None,
                None,
                vec!["color".to_string()],
            )
            .is_err()
        );
    }

    #[test]
    fn storefront_search_bound_uses_effective_utf8_bytes() {
        let ascii = "a".repeat(MAX_STOREFRONT_PRODUCT_SEARCH_BYTES);
        assert_eq!(
            StorefrontProductListQuery::try_new(Some(format!("  {ascii}  ")), None, None, None)
                .expect("bounded search must be accepted")
                .search
                .as_deref(),
            Some(ascii.as_str())
        );
        assert!(
            StorefrontProductListQuery::try_new(
                Some("a".repeat(MAX_STOREFRONT_PRODUCT_SEARCH_BYTES + 1)),
                None,
                None,
                None,
            )
            .is_err()
        );

        let multibyte = "é".repeat(MAX_STOREFRONT_PRODUCT_SEARCH_BYTES / 2);
        assert_eq!(multibyte.len(), MAX_STOREFRONT_PRODUCT_SEARCH_BYTES);
        assert!(StorefrontProductListQuery::try_new(Some(multibyte), None, None, None).is_ok());
        assert!(
            StorefrontProductListQuery::try_new(
                Some("é".repeat(MAX_STOREFRONT_PRODUCT_SEARCH_BYTES / 2 + 1)),
                None,
                None,
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn typed_attribute_filters_normalize_and_reject_duplicates() {
        let query = StorefrontProductListQuery::try_new_with_attribute_filters(
            None,
            None,
            None,
            None,
            vec![" color = red ".to_string(), "weight=12.5".to_string()],
        )
        .expect("valid typed filter syntax");
        assert_eq!(query.attribute_filters[0].code, "color");
        assert_eq!(query.attribute_filters[0].value, "red");
        assert!(
            StorefrontProductListQuery::try_new_with_attribute_filters(
                None,
                None,
                None,
                None,
                vec!["color=red".to_string(), "COLOR=blue".to_string()],
            )
            .is_err()
        );
    }

    #[test]
    fn admin_list_query_normalizes_and_validates_transport_values() {
        let query = AdminProductListQuery::try_from_transport_with_attribute_filters(
            Some("  camera  ".to_string()),
            Some("ACTIVE".to_string()),
            None,
            Some("created_at".to_string()),
            Some("asc".to_string()),
            vec!["color=red".to_string()],
        )
        .expect("valid admin list query");
        assert_eq!(query.search.as_deref(), Some("camera"));
        assert_eq!(query.status, Some(entities::product::ProductStatus::Active));
        assert_eq!(query.sort_by, StorefrontProductSortBy::CreatedAt);
        assert_eq!(query.sort_direction, StorefrontProductSortDirection::Asc);
        assert_eq!(query.attribute_filters.len(), 1);

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
