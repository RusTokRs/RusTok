use crate::error::{CommerceError, CommerceResult};
use uuid::Uuid;

pub use rustok_product_ports::{
    AdminProductList, AdminProductListItem, AdminProductListQuery, ProductAttributeFilter,
    StorefrontProductList, StorefrontProductListImage, StorefrontProductListItem,
    StorefrontProductListPrice, StorefrontProductListQuery, StorefrontProductSortBy,
    StorefrontProductSortDirection,
};

pub(crate) const MAX_ATTRIBUTE_FILTERS: usize = 8;
const MAX_ATTRIBUTE_FILTER_CODE_LENGTH: usize = 128;
const MAX_ATTRIBUTE_FILTER_VALUE_LENGTH: usize = 512;

/// Maximum effective Storefront Product title-search input in UTF-8 bytes.
pub const MAX_STOREFRONT_PRODUCT_SEARCH_BYTES: usize = 1022;

pub(crate) fn parse_storefront_sort_by(value: Option<String>) -> CommerceResult<StorefrontProductSortBy> {
    match value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        None | Some("published_at") => Ok(StorefrontProductSortBy::PublishedAt),
        Some("created_at") => Ok(StorefrontProductSortBy::CreatedAt),
        Some(_) => Err(CommerceError::Validation(
            "sort_by must be `published_at` or `created_at`".to_string(),
        )),
    }
}

pub(crate) fn parse_storefront_sort_direction(
    value: Option<String>,
) -> CommerceResult<StorefrontProductSortDirection> {
    match value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        None | Some("desc") => Ok(StorefrontProductSortDirection::Desc),
        Some("asc") => Ok(StorefrontProductSortDirection::Asc),
        Some(_) => Err(CommerceError::Validation(
            "sort_direction must be `asc` or `desc`".to_string(),
        )),
    }
}

pub(crate) fn parse_product_attribute_filter(value: String) -> CommerceResult<ProductAttributeFilter> {
    let (code, raw_value) = value.split_once('=').ok_or_else(|| {
        CommerceError::Validation("attribute_filters entries must use `code=value`".to_string())
    })?;
    let filter = ProductAttributeFilter {
        code: code.trim().to_string(),
        value: raw_value.trim().to_string(),
    };
    validate_attribute_filter(&filter)?;
    Ok(filter)
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

/// Maximum number of `code=value` entries one list query may carry.
pub(crate) const MAX_ATTRIBUTE_FILTER_ENTRIES: usize =
    MAX_ATTRIBUTE_FILTERS * super::facets::MAX_CATALOG_FACET_VALUES;

/// One attribute of a list query together with every value the caller selected for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductAttributeFilterSelection {
    pub code: String,
    pub values: Vec<String>,
}

/// Groups `code=value` entries into one selection per attribute code, keeping first-seen order.
pub fn group_product_attribute_filters(
    filters: &[ProductAttributeFilter],
) -> Vec<ProductAttributeFilterSelection> {
    let mut selections: Vec<ProductAttributeFilterSelection> = Vec::new();
    for filter in filters {
        match selections
            .iter_mut()
            .find(|selection| selection.code.eq_ignore_ascii_case(filter.code.as_str()))
        {
            Some(selection) => {
                if !selection.values.iter().any(|value| value == &filter.value) {
                    selection.values.push(filter.value.clone());
                }
            }
            None => selections.push(ProductAttributeFilterSelection {
                code: filter.code.clone(),
                values: vec![filter.value.clone()],
            }),
        }
    }
    selections
}

/// Validates every `code=value` entry and both list limits.
pub(crate) fn validate_product_attribute_filters(
    filters: &[ProductAttributeFilter],
) -> CommerceResult<()> {
    if filters.len() > MAX_ATTRIBUTE_FILTER_ENTRIES {
        return Err(CommerceError::Validation(format!(
            "attribute_filters supports at most {MAX_ATTRIBUTE_FILTER_ENTRIES} code=value entries"
        )));
    }
    for filter in filters {
        validate_attribute_filter(filter)?;
    }
    let selection_count = group_product_attribute_filters(filters).len();
    if selection_count > MAX_ATTRIBUTE_FILTERS {
        return Err(CommerceError::Validation(format!(
            "attribute_filters supports at most {MAX_ATTRIBUTE_FILTERS} attributes"
        )));
    }
    Ok(())
}

/// Normalizes the optional storefront display currency to `AAA` upper case.
#[allow(dead_code)]
pub(crate) fn normalize_storefront_currency_code(
    currency_code: Option<String>,
) -> CommerceResult<Option<String>> {
    let Some(currency_code) = normalize_optional_text(currency_code) else {
        return Ok(None);
    };
    let normalized = currency_code.to_ascii_uppercase();
    if normalized.len() != 3
        || !normalized
            .chars()
            .all(|character| character.is_ascii_alphabetic())
    {
        return Err(CommerceError::Validation(
            "currency_code must be a three-letter ISO 4217 code".to_string(),
        ));
    }
    Ok(Some(normalized))
}

fn parse_attribute_filters(values: Vec<String>) -> CommerceResult<Vec<ProductAttributeFilter>> {
    let filters = values
        .into_iter()
        .map(parse_product_attribute_filter)
        .collect::<CommerceResult<Vec<_>>>()?;
    validate_product_attribute_filters(&filters)?;
    Ok(filters)
}

pub trait StorefrontProductListQueryExt: Sized {
    fn try_new(
        search: Option<String>,
        category_id: Option<Uuid>,
        sort_by: Option<String>,
        sort_direction: Option<String>,
    ) -> CommerceResult<Self>;

    fn try_new_with_attribute_filters(
        search: Option<String>,
        category_id: Option<Uuid>,
        sort_by: Option<String>,
        sort_direction: Option<String>,
        attribute_filters: Vec<String>,
    ) -> CommerceResult<Self>;

    fn try_from_transport(
        search: Option<String>,
        category_id: Option<String>,
        sort_by: Option<String>,
        sort_direction: Option<String>,
    ) -> CommerceResult<Self>;

    fn try_from_transport_with_attribute_filters(
        search: Option<String>,
        category_id: Option<String>,
        sort_by: Option<String>,
        sort_direction: Option<String>,
        attribute_filters: Vec<String>,
    ) -> CommerceResult<Self>;
}

impl StorefrontProductListQueryExt for StorefrontProductListQuery {
    fn try_new(
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

    fn try_new_with_attribute_filters(
        search: Option<String>,
        category_id: Option<Uuid>,
        sort_by: Option<String>,
        sort_direction: Option<String>,
        attribute_filters: Vec<String>,
    ) -> CommerceResult<Self> {
        Ok(Self {
            search: normalize_storefront_product_search(search)?,
            category_id,
            sort_by: parse_storefront_sort_by(sort_by)?,
            sort_direction: parse_storefront_sort_direction(sort_direction)?,
            attribute_filters: parse_attribute_filters(attribute_filters)?,
            ..Self::default()
        })
    }

    fn try_from_transport(
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

    fn try_from_transport_with_attribute_filters(
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

pub trait AdminProductListQueryExt: Sized {
    fn try_from_transport(
        search: Option<String>,
        status: Option<String>,
        category_id: Option<String>,
        sort_by: Option<String>,
        sort_direction: Option<String>,
    ) -> CommerceResult<Self>;

    fn try_from_transport_with_attribute_filters(
        search: Option<String>,
        status: Option<String>,
        category_id: Option<String>,
        sort_by: Option<String>,
        sort_direction: Option<String>,
        attribute_filters: Vec<String>,
    ) -> CommerceResult<Self>;
}

impl AdminProductListQueryExt for AdminProductListQuery {
    fn try_from_transport(
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

    fn try_from_transport_with_attribute_filters(
        search: Option<String>,
        status: Option<String>,
        category_id: Option<String>,
        sort_by: Option<String>,
        sort_direction: Option<String>,
        attribute_filters: Vec<String>,
    ) -> CommerceResult<Self> {
        let status = normalize_optional_text(status)
            .map(|value| match value.to_ascii_lowercase().as_str() {
                "draft" => Ok(rustok_product_ports::ProductStatus::Draft),
                "active" => Ok(rustok_product_ports::ProductStatus::Active),
                "archived" => Ok(rustok_product_ports::ProductStatus::Archived),
                _ => Err(CommerceError::Validation(
                    "status must be `draft`, `active`, or `archived`".to_string(),
                )),
            })
            .transpose()?;
        Ok(Self {
            search: normalize_optional_text(search),
            status,
            category_id: parse_optional_uuid(category_id, "category_id")?,
            sort_by: parse_storefront_sort_by(sort_by)?,
            sort_direction: parse_storefront_sort_direction(sort_direction)?,
            attribute_filters: parse_attribute_filters(attribute_filters)?,
        })
    }
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
    fn typed_attribute_filters_normalize_and_group_repeated_codes() {
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

        let multi_select = StorefrontProductListQuery::try_new_with_attribute_filters(
            None,
            None,
            None,
            None,
            vec![
                "color=red".to_string(),
                "COLOR=blue".to_string(),
                "color=red".to_string(),
            ],
        )
        .expect("repeated attribute codes are the multi-select vocabulary");
        let selections = group_product_attribute_filters(multi_select.attribute_filters.as_slice());
        assert_eq!(selections.len(), 1);
        assert_eq!(selections[0].code, "color");
        assert_eq!(
            selections[0].values,
            vec!["red".to_string(), "blue".to_string()]
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
        assert_eq!(query.status, Some(rustok_product_ports::ProductStatus::Active));
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
