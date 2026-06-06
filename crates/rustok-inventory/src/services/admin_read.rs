use std::collections::HashMap;

use rustok_commerce_foundation::entities;
use rustok_commerce_foundation::error::CommerceResult;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const DEFAULT_PAGE: u64 = 1;
const DEFAULT_PER_PAGE: u64 = 24;
const MAX_PER_PAGE: u64 = 100;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AdminInventoryProductsFilter {
    pub status: Option<entities::product::ProductStatus>,
    pub search: Option<String>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AdminInventoryProductList {
    pub items: Vec<AdminInventoryProductListItem>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub has_next: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AdminInventoryProductListItem {
    pub id: Uuid,
    pub status: entities::product::ProductStatus,
    pub title: String,
    pub handle: String,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub published_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AdminInventoryProductDetail {
    pub id: Uuid,
    pub status: entities::product::ProductStatus,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
    pub translations: Vec<AdminInventoryProductTranslation>,
    pub variants: Vec<AdminInventoryVariant>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AdminInventoryProductTranslation {
    pub locale: String,
    pub title: String,
    pub handle: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AdminInventoryVariant {
    pub id: Uuid,
    pub sku: Option<String>,
    pub barcode: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub title: String,
    pub option1: Option<String>,
    pub option2: Option<String>,
    pub option3: Option<String>,
    pub prices: Vec<AdminInventoryPrice>,
    pub inventory_quantity: i32,
    pub inventory_policy: String,
    pub in_stock: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AdminInventoryPrice {
    pub currency_code: String,
    pub amount: String,
    pub compare_at_amount: Option<String>,
    pub on_sale: bool,
}

pub struct AdminInventoryReadService {
    db: DatabaseConnection,
}

impl AdminInventoryReadService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn list_products(
        &self,
        tenant_id: Uuid,
        locale: Option<&str>,
        filter: AdminInventoryProductsFilter,
    ) -> CommerceResult<AdminInventoryProductList> {
        let locale = locale.unwrap_or("en");
        let page = filter.page.unwrap_or(DEFAULT_PAGE).max(1);
        let per_page = filter
            .per_page
            .unwrap_or(DEFAULT_PER_PAGE)
            .clamp(1, MAX_PER_PAGE);

        let mut query = entities::product::Entity::find()
            .filter(entities::product::Column::TenantId.eq(tenant_id));

        if let Some(status) = filter.status {
            query = query.filter(entities::product::Column::Status.eq(status));
        }

        let products = query
            .order_by_desc(entities::product::Column::CreatedAt)
            .all(&self.db)
            .await?;

        let product_ids = products
            .iter()
            .map(|product| product.id)
            .collect::<Vec<_>>();
        let translations_by_product = self
            .load_product_translations_by_product(product_ids)
            .await?;
        let search = normalized_search(filter.search);

        let mut items = products
            .into_iter()
            .filter_map(|product| {
                let translations = translations_by_product
                    .get(&product.id)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                let translation = select_product_translation(translations, locale);
                if let Some(search) = search.as_deref() {
                    let matches = translations.iter().any(|translation| {
                        translation.title.to_ascii_lowercase().contains(search)
                            || translation.handle.to_ascii_lowercase().contains(search)
                    });
                    if !matches {
                        return None;
                    }
                }

                Some(AdminInventoryProductListItem {
                    id: product.id,
                    status: product.status,
                    title: translation
                        .map(|translation| translation.title.clone())
                        .unwrap_or_else(|| "Untitled product".to_string()),
                    handle: translation
                        .map(|translation| translation.handle.clone())
                        .unwrap_or_default(),
                    vendor: product.vendor,
                    product_type: product.product_type,
                    shipping_profile_slug: product.shipping_profile_slug,
                    tags: tags_from_metadata(&product.metadata),
                    created_at: product.created_at.to_rfc3339(),
                    published_at: product.published_at.map(|value| value.to_rfc3339()),
                })
            })
            .collect::<Vec<_>>();

        let total = items.len() as u64;
        let offset = ((page - 1) * per_page) as usize;
        items = items
            .into_iter()
            .skip(offset)
            .take(per_page as usize)
            .collect();

        Ok(AdminInventoryProductList {
            items,
            total,
            page,
            per_page,
            has_next: page * per_page < total,
        })
    }

    pub async fn get_product(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: Option<&str>,
    ) -> CommerceResult<Option<AdminInventoryProductDetail>> {
        let locale = locale.unwrap_or("en");
        let Some(product) = entities::product::Entity::find_by_id(product_id)
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
        else {
            return Ok(None);
        };

        let translations = entities::product_translation::Entity::find()
            .filter(entities::product_translation::Column::ProductId.eq(product_id))
            .all(&self.db)
            .await?;
        let variants = entities::product_variant::Entity::find()
            .filter(entities::product_variant::Column::ProductId.eq(product_id))
            .order_by_asc(entities::product_variant::Column::Position)
            .all(&self.db)
            .await?;
        let variant_ids = variants
            .iter()
            .map(|variant| variant.id)
            .collect::<Vec<_>>();
        let prices_by_variant = self.load_prices_by_variant(variant_ids.clone()).await?;
        let available_quantities_by_variant = self
            .load_available_quantities_by_variant(variant_ids.clone())
            .await?;
        let variant_translations_by_variant = self
            .load_variant_translations_by_variant(variant_ids)
            .await?;

        Ok(Some(AdminInventoryProductDetail {
            id: product.id,
            status: product.status,
            vendor: product.vendor,
            product_type: product.product_type,
            shipping_profile_slug: product.shipping_profile_slug,
            created_at: product.created_at.to_rfc3339(),
            updated_at: product.updated_at.to_rfc3339(),
            published_at: product.published_at.map(|value| value.to_rfc3339()),
            translations: translations
                .into_iter()
                .map(|translation| AdminInventoryProductTranslation {
                    locale: translation.locale,
                    title: translation.title,
                    handle: translation.handle,
                    description: translation.description,
                })
                .collect(),
            variants: variants
                .into_iter()
                .map(|variant| {
                    let title = variant_translations_by_variant
                        .get(&variant.id)
                        .and_then(|translations| select_variant_title(translations, locale))
                        .unwrap_or_else(|| fallback_variant_title(&variant));
                    let prices = prices_by_variant
                        .get(&variant.id)
                        .map(|prices| prices.iter().map(map_price).collect())
                        .unwrap_or_default();
                    let inventory_quantity = available_quantity_for_variant(
                        variant.inventory_quantity,
                        available_quantities_by_variant.get(&variant.id).copied(),
                    );
                    let in_stock = inventory_quantity > 0 || variant.inventory_policy == "continue";

                    AdminInventoryVariant {
                        id: variant.id,
                        sku: variant.sku,
                        barcode: variant.barcode,
                        shipping_profile_slug: variant.shipping_profile_slug,
                        title,
                        option1: variant.option1,
                        option2: variant.option2,
                        option3: variant.option3,
                        prices,
                        inventory_quantity,
                        inventory_policy: variant.inventory_policy,
                        in_stock,
                    }
                })
                .collect(),
        }))
    }

    async fn load_product_translations_by_product(
        &self,
        product_ids: Vec<Uuid>,
    ) -> CommerceResult<HashMap<Uuid, Vec<entities::product_translation::Model>>> {
        if product_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let translations = entities::product_translation::Entity::find()
            .filter(entities::product_translation::Column::ProductId.is_in(product_ids))
            .all(&self.db)
            .await?;
        Ok(group_by(translations, |translation| translation.product_id))
    }

    async fn load_prices_by_variant(
        &self,
        variant_ids: Vec<Uuid>,
    ) -> CommerceResult<HashMap<Uuid, Vec<entities::price::Model>>> {
        if variant_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let prices = entities::price::Entity::find()
            .filter(entities::price::Column::VariantId.is_in(variant_ids))
            .all(&self.db)
            .await?;
        Ok(group_by(prices, |price| price.variant_id))
    }

    async fn load_available_quantities_by_variant(
        &self,
        variant_ids: Vec<Uuid>,
    ) -> CommerceResult<HashMap<Uuid, i32>> {
        if variant_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let items = entities::inventory_item::Entity::find()
            .filter(entities::inventory_item::Column::VariantId.is_in(variant_ids))
            .all(&self.db)
            .await?;
        if items.is_empty() {
            return Ok(HashMap::new());
        }

        let item_to_variant = items
            .iter()
            .map(|item| (item.id, item.variant_id))
            .collect::<HashMap<_, _>>();
        let item_ids = item_to_variant.keys().copied().collect::<Vec<_>>();
        let levels = entities::inventory_level::Entity::find()
            .filter(entities::inventory_level::Column::InventoryItemId.is_in(item_ids))
            .all(&self.db)
            .await?;

        let mut available_by_variant = HashMap::new();
        for level in levels {
            if let Some(variant_id) = item_to_variant.get(&level.inventory_item_id) {
                *available_by_variant.entry(*variant_id).or_insert(0) +=
                    level.stocked_quantity - level.reserved_quantity;
            }
        }

        Ok(available_by_variant)
    }

    async fn load_variant_translations_by_variant(
        &self,
        variant_ids: Vec<Uuid>,
    ) -> CommerceResult<HashMap<Uuid, Vec<entities::variant_translation::Model>>> {
        if variant_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let translations = entities::variant_translation::Entity::find()
            .filter(entities::variant_translation::Column::VariantId.is_in(variant_ids))
            .all(&self.db)
            .await?;
        Ok(group_by(translations, |translation| translation.variant_id))
    }
}

fn group_by<T>(items: Vec<T>, key: impl Fn(&T) -> Uuid) -> HashMap<Uuid, Vec<T>> {
    let mut grouped = HashMap::new();
    for item in items {
        grouped
            .entry(key(&item))
            .or_insert_with(Vec::new)
            .push(item);
    }
    grouped
}

fn available_quantity_for_variant(
    legacy_variant_quantity: i32,
    inventory_level_available_quantity: Option<i32>,
) -> i32 {
    inventory_level_available_quantity.unwrap_or(legacy_variant_quantity)
}

fn normalized_search(search: Option<String>) -> Option<String> {
    search
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
}

fn select_product_translation<'a>(
    translations: &'a [entities::product_translation::Model],
    locale: &str,
) -> Option<&'a entities::product_translation::Model> {
    translations
        .iter()
        .find(|translation| locale_tags_match(translation.locale.as_str(), locale))
        .or_else(|| translations.first())
}

fn select_variant_title(
    translations: &[entities::variant_translation::Model],
    locale: &str,
) -> Option<String> {
    translations
        .iter()
        .find(|translation| locale_tags_match(translation.locale.as_str(), locale))
        .and_then(|translation| translation.title.clone())
        .or_else(|| {
            translations
                .iter()
                .find_map(|translation| translation.title.clone())
        })
}

fn locale_tags_match(left: &str, right: &str) -> bool {
    let left = left.trim().replace('_', "-").to_ascii_lowercase();
    let right = right.trim().replace('_', "-").to_ascii_lowercase();

    left == right
        || left
            .split('-')
            .next()
            .zip(right.split('-').next())
            .map(|(left_language, right_language)| left_language == right_language)
            .unwrap_or(false)
}

fn fallback_variant_title(variant: &entities::product_variant::Model) -> String {
    variant
        .sku
        .clone()
        .or_else(|| variant.option1.clone())
        .unwrap_or_else(|| "Variant".to_string())
}

fn tags_from_metadata(metadata: &serde_json::Value) -> Vec<String> {
    metadata
        .get("tags")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToOwned::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn map_price(price: &entities::price::Model) -> AdminInventoryPrice {
    AdminInventoryPrice {
        currency_code: price.currency_code.clone(),
        amount: price.amount.to_string(),
        compare_at_amount: price.compare_at_amount.map(|amount| amount.to_string()),
        on_sale: price
            .compare_at_amount
            .map(|compare_at_amount| compare_at_amount > price.amount)
            .unwrap_or(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use sea_orm::prelude::DateTimeWithTimeZone;

    fn product_translation(locale: &str, title: &str) -> entities::product_translation::Model {
        entities::product_translation::Model {
            id: Uuid::new_v4(),
            product_id: Uuid::new_v4(),
            locale: locale.to_string(),
            title: title.to_string(),
            handle: title.to_ascii_lowercase().replace(' ', "-"),
            description: None,
            meta_title: None,
            meta_description: None,
        }
    }

    fn variant_translation(
        locale: &str,
        title: Option<&str>,
    ) -> entities::variant_translation::Model {
        entities::variant_translation::Model {
            id: Uuid::new_v4(),
            variant_id: Uuid::new_v4(),
            locale: locale.to_string(),
            title: title.map(ToOwned::to_owned),
        }
    }

    fn variant(sku: Option<&str>, option1: Option<&str>) -> entities::product_variant::Model {
        let now: DateTimeWithTimeZone = Utc::now().into();
        entities::product_variant::Model {
            id: Uuid::new_v4(),
            product_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            sku: sku.map(ToOwned::to_owned),
            barcode: None,
            shipping_profile_slug: None,
            ean: None,
            upc: None,
            inventory_policy: "deny".to_string(),
            inventory_management: "shopify".to_string(),
            inventory_quantity: 1,
            weight: None,
            weight_unit: None,
            option1: option1.map(ToOwned::to_owned),
            option2: None,
            option3: None,
            position: 1,
            created_at: now,
            updated_at: now,
        }
    }

    fn price(amount: i64, compare_at_amount: Option<i64>) -> entities::price::Model {
        entities::price::Model {
            id: Uuid::new_v4(),
            variant_id: Uuid::new_v4(),
            price_list_id: None,
            channel_id: None,
            channel_slug: None,
            currency_code: "USD".to_string(),
            region_id: None,
            amount: amount.to_string().parse().expect("valid amount decimal"),
            compare_at_amount: compare_at_amount
                .map(|value| value.to_string().parse().expect("valid compare-at decimal")),
            legacy_amount: None,
            legacy_compare_at_amount: None,
            min_quantity: None,
            max_quantity: None,
        }
    }

    #[test]
    fn select_product_translation_prefers_requested_locale_and_falls_back_to_first() {
        let translations = vec![
            product_translation("en", "English"),
            product_translation("ru", "Russian"),
        ];

        assert_eq!(
            select_product_translation(translations.as_slice(), "ru")
                .map(|translation| translation.title.as_str()),
            Some("Russian")
        );
        assert_eq!(
            select_product_translation(translations.as_slice(), "de")
                .map(|translation| translation.title.as_str()),
            Some("English")
        );
    }

    #[test]
    fn select_variant_title_prefers_requested_locale_then_first_title() {
        let translations = vec![
            variant_translation("en", Some("English")),
            variant_translation("ru", Some("Russian")),
        ];

        assert_eq!(
            select_variant_title(translations.as_slice(), "ru"),
            Some("Russian".to_string())
        );
        assert_eq!(
            select_variant_title(translations.as_slice(), "de"),
            Some("English".to_string())
        );
    }

    #[test]
    fn locale_tags_match_accepts_underscore_and_language_only_fallback() {
        assert!(locale_tags_match("en_US", "en-US"));
        assert!(locale_tags_match("pt-BR", "pt"));
        assert!(!locale_tags_match("en", "ru"));
    }

    #[test]
    fn fallback_variant_title_uses_sku_then_option_then_generic_label() {
        assert_eq!(
            fallback_variant_title(&variant(Some("SKU-1"), Some("Red"))),
            "SKU-1"
        );
        assert_eq!(fallback_variant_title(&variant(None, Some("Red"))), "Red");
        assert_eq!(fallback_variant_title(&variant(None, None)), "Variant");
    }

    #[test]
    fn available_quantity_prefers_inventory_levels_and_falls_back_to_variant_snapshot() {
        assert_eq!(available_quantity_for_variant(12, Some(7)), 7);
        assert_eq!(available_quantity_for_variant(12, Some(0)), 0);
        assert_eq!(available_quantity_for_variant(12, None), 12);
    }

    #[test]
    fn tags_from_metadata_reads_string_array_only() {
        let metadata = serde_json::json!({ "tags": ["low", 3, "fragile"] });
        assert_eq!(
            tags_from_metadata(&metadata),
            vec!["low".to_string(), "fragile".to_string()]
        );
        assert!(tags_from_metadata(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn map_price_marks_sale_when_compare_at_exceeds_amount() {
        assert!(map_price(&price(1000, Some(1200))).on_sale);
        assert!(!map_price(&price(1000, Some(900))).on_sale);
        assert!(!map_price(&price(1000, None)).on_sale);
    }

    #[test]
    fn normalized_search_trims_lowercases_and_drops_blank_values() {
        assert_eq!(
            normalized_search(Some("  Winter Coat  ".to_string())),
            Some("winter coat".to_string())
        );
        assert_eq!(normalized_search(Some("   ".to_string())), None);
        assert_eq!(normalized_search(None), None);
    }
}
