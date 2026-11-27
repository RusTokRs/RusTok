mod attribute_lifecycle;
pub use attribute_lifecycle::*;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CurrentTenant {
    pub id: String,
    pub slug: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CurrentUser {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductAdminBootstrap {
    #[serde(rename = "currentTenant")]
    pub current_tenant: CurrentTenant,
    pub me: CurrentUser,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ProductList {
    pub items: Vec<ProductListItem>,
    pub total: u64,
    pub page: u64,
    #[serde(rename = "perPage")]
    pub per_page: u64,
    #[serde(rename = "hasNext")]
    pub has_next: bool,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ProductListItem {
    pub id: String,
    pub status: String,
    pub title: String,
    pub handle: String,
    #[serde(rename = "sellerId")]
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    #[serde(rename = "productType")]
    pub product_type: Option<String>,
    #[serde(rename = "shippingProfileSlug")]
    pub shipping_profile_slug: Option<String>,
    #[serde(rename = "primaryCategoryId")]
    pub primary_category_id: Option<String>,
    pub tags: Vec<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "publishedAt")]
    pub published_at: Option<String>,
}

/// One bucket of an enumerable admin facet.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct AdminCatalogFacetValue {
    /// `code=<value>` payload: an option id for dictionary attributes, `true`/`false` for booleans.
    pub value: String,
    pub label: String,
    pub count: u64,
}

/// A facet the Product owner resolved for the current admin filter set.
///
/// The serde names match the GraphQL answer (`adminProductCatalogFacets`) and the native server
/// function payload, so the admin grid reads one model on both transports.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct AdminCatalogFacet {
    pub code: String,
    pub label: String,
    /// Stored attribute value type, e.g. `select`.
    #[serde(rename = "valueType")]
    pub value_type: String,
    #[serde(rename = "isLocalized")]
    pub is_localized: bool,
    /// False when the value domain is unbounded (text, numeric, date): `values` stays empty.
    #[serde(rename = "isEnumerable")]
    pub is_enumerable: bool,
    /// True when the owner cut the bucket list at its facet-value limit.
    #[serde(rename = "isTruncated")]
    pub is_truncated: bool,
    /// Products matching every other active facet that carry a value for this attribute.
    #[serde(rename = "totalProducts")]
    pub total_products: u64,
    pub values: Vec<AdminCatalogFacetValue>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductDetail {
    pub id: String,
    pub status: String,
    #[serde(rename = "sellerId")]
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    #[serde(rename = "productType")]
    pub product_type: Option<String>,
    #[serde(rename = "shippingProfileSlug")]
    pub shipping_profile_slug: Option<String>,
    #[serde(rename = "primaryCategoryId")]
    pub primary_category_id: Option<String>,
    pub tags: Vec<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "publishedAt")]
    pub published_at: Option<String>,
    pub translations: Vec<ProductTranslation>,
    #[serde(rename = "variantAxes", default)]
    pub variant_axes: Vec<VariantAxisConfig>,
    pub variants: Vec<ProductVariant>,
    #[serde(default)]
    pub images: Vec<ProductImage>,
    #[serde(rename = "effectiveForm", default)]
    pub effective_form: Option<ProductEffectiveForm>,
    /// Editorial revision of the product aggregate at read time.
    ///
    /// Sent back as `revision` on the next `updateProduct`, so a save that would overwrite another
    /// operator's change is refused instead of silently winning.
    pub revision: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductPricingDetail {
    pub variants: Vec<ProductPricingVariant>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductPricingVariant {
    pub id: String,
    pub prices: Vec<ProductScopedPrice>,
    #[serde(rename = "effectivePrice")]
    pub effective_price: Option<ProductEffectivePrice>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductScopedPrice {
    #[serde(rename = "currencyCode")]
    pub currency_code: String,
    pub amount: String,
    #[serde(rename = "compareAtAmount")]
    pub compare_at_amount: Option<String>,
    #[serde(rename = "discountPercent", default)]
    pub discount_percent: Option<String>,
    #[serde(rename = "onSale")]
    pub on_sale: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductPriceSource {
    #[serde(rename = "priceId")]
    pub price_id: String,
    pub kind: String,
    #[serde(rename = "ruleKind")]
    pub rule_kind: Option<String>,
    #[serde(rename = "adjustmentPercent")]
    pub adjustment_percent: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductEffectivePrice {
    pub source: ProductPriceSource,
    #[serde(rename = "currencyCode")]
    pub currency_code: String,
    pub amount: String,
    #[serde(rename = "compareAtAmount")]
    pub compare_at_amount: Option<String>,
    #[serde(rename = "discountPercent", default)]
    pub discount_percent: Option<String>,
    #[serde(rename = "onSale")]
    pub on_sale: bool,
    #[serde(rename = "priceListId", default)]
    pub price_list_id: Option<String>,
    #[serde(rename = "channelId", default)]
    pub channel_id: Option<String>,
    #[serde(rename = "channelSlug", default)]
    pub channel_slug: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductTranslation {
    pub locale: String,
    pub title: String,
    pub handle: String,
    pub description: Option<String>,
    #[serde(rename = "metaTitle")]
    pub meta_title: Option<String>,
    #[serde(rename = "metaDescription")]
    pub meta_description: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct VariantAxisConfig {
    pub id: String,
    #[serde(rename = "attributeId")]
    pub attribute_id: String,
    pub code: String,
    pub name: String,
    pub position: i32,
    #[serde(rename = "allowedValues", default)]
    pub allowed_values: Vec<AxisAllowedValue>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct AxisAllowedValue {
    #[serde(rename = "optionId")]
    pub option_id: String,
    pub value: String,
    pub position: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VariantAxisValue {
    #[serde(rename = "attributeId")]
    pub attribute_id: String,
    #[serde(rename = "optionId")]
    pub option_id: String,
    pub code: Option<String>,
    pub label: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductVariant {
    pub id: String,
    pub revision: String,
    pub sku: Option<String>,
    pub barcode: Option<String>,
    #[serde(rename = "shippingProfileSlug")]
    pub shipping_profile_slug: Option<String>,
    pub title: String,
    #[serde(rename = "combinationIdentity")]
    pub combination_identity: Option<String>,
    #[serde(rename = "axisValues", default)]
    pub axis_values: Vec<VariantAxisValue>,
    pub prices: Vec<ProductPrice>,
    #[serde(rename = "inventoryQuantity")]
    pub inventory_quantity: i32,
    #[serde(rename = "inventoryPolicy")]
    pub inventory_policy: String,
    #[serde(rename = "inStock")]
    pub in_stock: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductPrice {
    #[serde(rename = "currencyCode")]
    pub currency_code: String,
    pub amount: String,
    #[serde(rename = "compareAtAmount")]
    pub compare_at_amount: Option<String>,
    #[serde(rename = "onSale")]
    pub on_sale: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductImage {
    pub id: String,
    #[serde(rename = "mediaId")]
    pub media_id: String,
    pub url: String,
    #[serde(rename = "altText")]
    pub alt_text: Option<String>,
    pub position: i32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VariantAxisValueDraft {
    #[serde(rename = "attributeId")]
    pub attribute_id: String,
    #[serde(rename = "optionId")]
    pub option_id: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VariantAxisDraft {
    #[serde(rename = "attributeId")]
    pub attribute_id: String,
    pub position: Option<i32>,
    #[serde(rename = "allowedOptionIds", default)]
    pub allowed_option_ids: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SetVariantAxesDraft {
    pub axes: Vec<VariantAxisDraft>,
    #[serde(
        rename = "expectedState",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub expected_state: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reassignments: Vec<VariantAxisReassignmentDraft>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VariantAxisReassignmentDraft {
    #[serde(rename = "variantId")]
    pub variant_id: String,
    #[serde(rename = "axisValues")]
    pub axis_values: Vec<VariantAxisValueDraft>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantAxesPreview {
    pub product_id: String,
    pub product_revision: i32,
    pub state_token: String,
    pub variant_count: i32,
    pub can_apply: bool,
    pub has_changes: bool,
    pub conflicts: Vec<VariantAxesConflict>,
    pub has_more_conflicts: bool,
    pub variants: Vec<VariantAxesPreviewVariant>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantAxesConflict {
    pub kind: String,
    pub attribute_id: Option<String>,
    pub option_id: Option<String>,
    pub variant_ids: Vec<String>,
    pub has_more_variants: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantAxesPreviewVariant {
    pub variant_id: String,
    pub sku: Option<String>,
    pub current_values: Vec<VariantAxisValueDraft>,
    pub proposed_values: Vec<VariantAxisValueDraft>,
    pub changes: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum VariantAxesApplyOutcome {
    Confirmed {
        axes: Vec<VariantAxisConfig>,
    },
    /// A recognized owner decision was durably finalized, not inferred from HTTP status/text.
    Rejected {
        code: String,
    },
    Unknown,
}

/// Owner acknowledgement of standalone creation, not a Product revision or a read inference.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum VariantCreateOutcome {
    Confirmed { variant_id: String },
    Rejected { code: String },
    Unknown,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VariantDraft {
    pub sku: Option<String>,
    pub barcode: Option<String>,
    #[serde(rename = "shippingProfileSlug")]
    pub shipping_profile_slug: Option<String>,
    #[serde(rename = "axisValues", default)]
    pub axis_values: Vec<VariantAxisValueDraft>,
    #[serde(rename = "attributeValues", default, skip_serializing_if = "Vec::is_empty")]
    pub attribute_values: Vec<ProductAttributeValueBatchDraft>,
    pub prices: Vec<VariantPriceDraft>,
    #[serde(rename = "inventoryQuantity")]
    pub inventory_quantity: Option<i32>,
    #[serde(rename = "inventoryPolicy")]
    pub inventory_policy: Option<String>,
}

/// Update-only intent. Creation continues to use VariantDraft.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct VariantUpdateDraft {
    #[serde(default, skip_serializing_if = "rustok_api::Patch::is_keep")]
    pub sku: rustok_api::Patch<String>,
    #[serde(default, skip_serializing_if = "rustok_api::Patch::is_keep")]
    pub barcode: rustok_api::Patch<String>,
    #[serde(
        rename = "shippingProfileSlug",
        default,
        skip_serializing_if = "rustok_api::Patch::is_keep"
    )]
    pub shipping_profile_slug: rustok_api::Patch<String>,
    #[serde(default, skip_serializing_if = "rustok_api::Patch::is_keep")]
    pub weight: rustok_api::Patch<String>,
    #[serde(
        rename = "weightUnit",
        default,
        skip_serializing_if = "rustok_api::Patch::is_keep"
    )]
    pub weight_unit: rustok_api::Patch<String>,
    #[serde(rename = "axisValues", default)]
    pub axis_values: Vec<VariantAxisValueDraft>,
    #[serde(rename = "attributeValues", default, skip_serializing_if = "Vec::is_empty")]
    pub attribute_values: Vec<ProductAttributeValueBatchDraft>,
    #[serde(default)]
    pub prices: Vec<VariantPriceDraft>,
    /// Distinguishes an explicit empty replacement (clear all prices) from an untouched price set.
    #[serde(default)]
    pub prices_changed: bool,
    #[serde(rename = "inventoryQuantity")]
    pub inventory_quantity: Option<i32>,
    #[serde(
        rename = "expectedInventoryQuantity",
        skip_serializing_if = "Option::is_none"
    )]
    pub expected_inventory_quantity: Option<i32>,
    #[serde(rename = "expectedRevision", skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<String>,
    #[serde(rename = "inventoryPolicy")]
    pub inventory_policy: Option<String>,
}

impl VariantUpdateDraft {
    /// Builds a sparse PATCH from the editor's original snapshot and current form values.
    /// Cross-owner Inventory/Pricing snapshots are omitted unless the operator changed them.
    pub fn from_editor_state(
        original: &ProductVariant,
        sku: String,
        barcode: String,
        shipping_profile_slug: String,
        currency_code: String,
        amount: String,
        compare_at_amount: String,
        inventory_quantity: i32,
        inventory_policy: String,
    ) -> Self {
        Self::from_editor_state_with_price_clear(
            original,
            sku,
            barcode,
            shipping_profile_slug,
            currency_code,
            amount,
            compare_at_amount,
            inventory_quantity,
            inventory_policy,
            false,
        )
    }

    /// Explicitly clear the target Variant's primary-currency price only when requested.
    /// An empty amount field by itself is an unchanged value, never an implicit deletion.
    pub fn from_editor_state_with_price_clear(
        original: &ProductVariant,
        sku: String,
        barcode: String,
        shipping_profile_slug: String,
        currency_code: String,
        amount: String,
        compare_at_amount: String,
        inventory_quantity: i32,
        inventory_policy: String,
        clear_primary_price: bool,
    ) -> Self {
        let sku = Self::edited_text_if_changed(sku, original.sku.as_deref());
        let barcode = Self::edited_text_if_changed(barcode, original.barcode.as_deref());
        let shipping_profile_slug = Self::edited_text_if_changed(
            shipping_profile_slug,
            original.shipping_profile_slug.as_deref(),
        );

        let original_prices = original
            .prices
            .iter()
            .map(|price| VariantPriceDraft {
                currency_code: price.currency_code.clone(),
                amount: price.amount.clone(),
                compare_at_amount: price.compare_at_amount.clone(),
            })
            .collect::<Vec<_>>();
        let mut next_prices = original_prices.clone();
        let primary_currency = next_prices.first().map(|price| price.currency_code.clone());
        let currency_code = currency_code.trim().to_ascii_uppercase();
        let amount = amount.trim().to_owned();
        let compare_at_amount = rustok_ui_core::normalize_ui_text(&compare_at_amount);

        if amount.is_empty() {
            if clear_primary_price && let Some(primary_currency) = primary_currency {
                next_prices.retain(|price| price.currency_code != primary_currency);
            }
        } else {
            let updated_price = VariantPriceDraft {
                currency_code: currency_code.clone(),
                amount,
                compare_at_amount,
            };
            if let Some(primary_currency) = primary_currency {
                if currency_code == primary_currency {
                    if let Some(index) = next_prices
                        .iter()
                        .position(|price| price.currency_code == primary_currency)
                    {
                        next_prices[index] = updated_price;
                    }
                } else if next_prices
                    .iter()
                    .any(|price| price.currency_code == currency_code)
                {
                    next_prices.retain(|price| price.currency_code != primary_currency);
                    if let Some(index) = next_prices
                        .iter()
                        .position(|price| price.currency_code == currency_code)
                    {
                        next_prices[index] = updated_price;
                    }
                } else if let Some(index) = next_prices
                    .iter()
                    .position(|price| price.currency_code == primary_currency)
                {
                    next_prices[index] = updated_price;
                } else {
                    next_prices.push(updated_price);
                }
            } else if let Some(index) = next_prices
                .iter()
                .position(|price| price.currency_code == currency_code)
            {
                next_prices[index] = updated_price;
            } else {
                next_prices.push(updated_price);
            }
        }

        let prices_changed = next_prices != original_prices;
        Self {
            sku,
            barcode,
            shipping_profile_slug,
            prices: if prices_changed {
                next_prices
            } else {
                Vec::new()
            },
            prices_changed,
            inventory_quantity: (inventory_quantity != original.inventory_quantity)
                .then_some(inventory_quantity),
            expected_inventory_quantity: (inventory_quantity != original.inventory_quantity)
                .then_some(original.inventory_quantity),
            expected_revision: Some(original.revision.clone()),
            inventory_policy: (inventory_policy != original.inventory_policy)
                .then_some(inventory_policy),
            ..Default::default()
        }
    }

    /// Whether this draft contains an effective owner mutation.
    ///
    /// A non-empty `prices` buffer alone is not a replacement: callers must set
    /// `prices_changed` so an untouched snapshot cannot be replayed accidentally.
    pub fn has_changes(&self) -> bool {
        self.sku.is_changed()
            || self.barcode.is_changed()
            || self.shipping_profile_slug.is_changed()
            || self.weight.is_changed()
            || self.weight_unit.is_changed()
            || !self.axis_values.is_empty()
            || self.attribute_values.iter().any(|batch| !batch.patches.is_empty())
            || self.prices_changed
            || self.inventory_quantity.is_some()
            || self.inventory_policy.is_some()
    }

    fn edited_text_if_changed(value: String, current: Option<&str>) -> rustok_api::Patch<String> {
        let normalized = rustok_ui_core::normalize_ui_text(&value);
        if Some(value.as_str()) == current || normalized.as_deref() == current {
            rustok_api::Patch::Keep
        } else {
            match normalized {
                Some(value) => rustok_api::Patch::Set(value),
                None => rustok_api::Patch::Clear,
            }
        }
    }

    /// Only call for a field actually edited by the form; untouched fields remain Keep.
    pub fn edited_text(value: String) -> rustok_api::Patch<String> {
        match rustok_ui_core::normalize_ui_text(&value) {
            Some(value) => rustok_api::Patch::Set(value),
            None => rustok_api::Patch::Clear,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VariantPriceDraft {
    #[serde(rename = "currencyCode")]
    pub currency_code: String,
    pub amount: String,
    #[serde(rename = "compareAtAmount")]
    pub compare_at_amount: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ProductImageDraft {
    #[serde(rename = "mediaId")]
    pub media_id: String,
    pub position: Option<i32>,
    #[serde(rename = "altText")]
    pub alt_text: Option<String>,
    pub locale: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct UpdateProductImageDraft {
    pub position: Option<i32>,
    #[serde(rename = "altText")]
    pub alt_text: Option<String>,
    pub locale: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ProductDraft {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variant_axes: Vec<VariantAxisDraft>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_variants: Option<Vec<VariantDraft>>,
    #[serde(default)]
    pub attribute_values: Vec<ProductAttributeValueBatchDraft>,
    pub locale: String,
    pub title: String,
    pub handle: String,
    pub description: String,
    pub seller_id: String,
    pub vendor: String,
    pub product_type: String,
    pub shipping_profile_slug: Option<String>,
    pub primary_category_id: Option<String>,
    pub sku: String,
    pub barcode: String,
    pub currency_code: String,
    pub amount: String,
    pub compare_at_amount: String,
    pub inventory_quantity: i32,
    pub inventory_policy: String,
    pub publish_now: bool,
    pub status: Option<String>,
    pub meta_title: Option<String>,
    pub meta_description: Option<String>,
    pub tags: Vec<String>,
    /// Revision this draft was loaded from; `None` keeps the create path unconditional.
    pub revision: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShippingProfileList {
    pub items: Vec<ShippingProfile>,
    pub total: u64,
    pub page: u64,
    #[serde(rename = "perPage")]
    pub per_page: u64,
    #[serde(rename = "hasNext")]
    pub has_next: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShippingProfile {
    pub id: String,
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub active: bool,
    pub metadata: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductAttributeList {
    pub items: Vec<ProductAttributeSummary>,
    pub total: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ProductAttributeSummary {
    pub id: String,
    pub code: String,
    #[serde(rename = "valueType")]
    pub value_type: String,
    pub scope: String,
    #[serde(rename = "isLocalized")]
    pub is_localized: bool,
    #[serde(rename = "isFilterable")]
    pub is_filterable: bool,
    #[serde(rename = "isSearchable")]
    pub is_searchable: bool,
    #[serde(rename = "isSortable")]
    pub is_sortable: bool,
    #[serde(rename = "showOnStorefront")]
    pub show_on_storefront: bool,
    pub label: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CatalogCategoryList {
    pub items: Vec<CatalogCategorySummary>,
    pub total: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct CatalogCategorySummary {
    pub id: String,
    pub code: String,
    pub slug: String,
    pub path: String,
    pub kind: String,
    pub name: String,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductCatalogSearchOption {
    pub value: String,
    pub label: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ProductCatalogSearchOptions {
    pub category_options: Vec<ProductCatalogSearchOption>,
    pub attribute_options: Vec<ProductCatalogSearchOption>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductAttributeSchemaList {
    pub items: Vec<ProductAttributeSchemaSummary>,
    pub total: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ProductAttributeSchemaSummary {
    pub id: String,
    pub code: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductEffectiveForm {
    #[serde(rename = "categoryId")]
    pub category_id: String,
    pub attributes: Vec<ProductEffectiveFormAttribute>,
    #[serde(rename = "detachedAttributeIds")]
    pub detached_attribute_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductEffectiveFormAttribute {
    #[serde(rename = "attributeId")]
    pub attribute_id: String,
    pub code: String,
    pub label: String,
    #[serde(rename = "valueType")]
    pub value_type: String,
    pub scope: String,
    #[serde(rename = "isLocalized")]
    pub is_localized: bool,
    pub options: Vec<ProductAttributeOptionSummary>,
    #[serde(rename = "groupCode")]
    pub group_code: Option<String>,
    #[serde(rename = "groupLabel")]
    pub group_label: Option<String>,
    #[serde(rename = "isRequired")]
    pub is_required: bool,
    #[serde(rename = "isDisabled")]
    pub is_disabled: bool,
    pub position: i32,
    pub source: String,
    #[serde(rename = "variantAxisPolicy", default = "default_variant_axis_policy")]
    pub variant_axis_policy: String,
    #[serde(rename = "defaultVariantAxis", default)]
    pub default_variant_axis: bool,
}

fn default_variant_axis_policy() -> String {
    "forbidden".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductAttributeOptionSummary {
    pub id: String,
    pub code: String,
    pub label: String,
    pub position: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductAttributeValueItem {
    #[serde(rename = "attributeId")]
    pub attribute_id: String,
    pub kind: String,
    pub text: Option<String>,
    pub integer: Option<i64>,
    pub decimal: Option<String>,
    pub boolean: Option<bool>,
    pub date: Option<String>,
    pub datetime: Option<String>,
    #[serde(rename = "optionId")]
    pub option_id: Option<String>,
    #[serde(rename = "optionIds")]
    pub option_ids: Option<Vec<String>>,
    pub json: Option<serde_json::Value>,
    pub detached: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductAttributeValuePatchDraft {
    #[serde(rename = "attributeId")]
    pub attribute_id: String,
    pub kind: String,
    pub text: Option<String>,
    pub integer: Option<i64>,
    pub decimal: Option<String>,
    pub boolean: Option<bool>,
    pub date: Option<String>,
    pub datetime: Option<String>,
    #[serde(rename = "optionId")]
    pub option_id: Option<String>,
    #[serde(rename = "optionIds")]
    pub option_ids: Option<Vec<String>>,
    pub json: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductAttributeDraft {
    pub code: String,
    #[serde(rename = "valueType")]
    pub value_type: String,
    pub scope: String,
    pub label: String,
    #[serde(rename = "helpText")]
    pub help_text: Option<String>,
    #[serde(rename = "isLocalized")]
    pub is_localized: bool,
    #[serde(rename = "isFilterable")]
    pub is_filterable: bool,
    #[serde(rename = "isSearchable")]
    pub is_searchable: bool,
    #[serde(rename = "isSortable")]
    pub is_sortable: bool,
    #[serde(rename = "showOnStorefront")]
    pub show_on_storefront: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductAttributeOptionDraft {
    #[serde(rename = "attributeId")]
    pub attribute_id: String,
    pub code: String,
    pub label: String,
    pub position: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CatalogCategoryDraft {
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
    pub code: String,
    pub slug: String,
    pub kind: String,
    #[serde(rename = "ruleJson", skip_serializing_if = "Option::is_none")]
    pub rule_json: Option<String>,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductAttributeSchemaDraft {
    pub code: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductAttributeSchemaGroupDraft {
    #[serde(rename = "schemaId")]
    pub schema_id: String,
    pub code: String,
    pub label: String,
    pub position: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CategoryAttributeGroupDraft {
    #[serde(rename = "categoryId")]
    pub category_id: String,
    pub code: String,
    pub label: String,
    pub position: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SetCategorySchemaModeDraft {
    #[serde(rename = "categoryId")]
    pub category_id: String,
    pub mode: String,
    #[serde(rename = "schemaId")]
    pub schema_id: Option<String>,
    #[serde(rename = "cloneFromCategoryId")]
    pub clone_from_category_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BindSchemaAttributeDraft {
    #[serde(rename = "schemaId")]
    pub schema_id: String,
    #[serde(rename = "attributeId")]
    pub attribute_id: String,
    #[serde(rename = "groupCode")]
    pub group_code: Option<String>,
    #[serde(rename = "isRequired")]
    pub is_required: bool,
    #[serde(rename = "isDisabled")]
    pub is_disabled: bool,
    pub position: i32,
    #[serde(rename = "variantAxisPolicy", skip_serializing_if = "Option::is_none")]
    pub variant_axis_policy: Option<String>,
    #[serde(rename = "defaultVariantAxis", skip_serializing_if = "Option::is_none")]
    pub default_variant_axis: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BindCategoryAttributeDraft {
    #[serde(rename = "categoryId")]
    pub category_id: String,
    #[serde(rename = "attributeId")]
    pub attribute_id: String,
    #[serde(rename = "groupCode")]
    pub group_code: Option<String>,
    #[serde(rename = "bindingKind")]
    pub binding_kind: String,
    #[serde(rename = "isRequired")]
    pub is_required: Option<bool>,
    #[serde(rename = "isDisabled")]
    pub is_disabled: bool,
    pub position: Option<i32>,
    #[serde(rename = "variantAxisPolicy", skip_serializing_if = "Option::is_none")]
    pub variant_axis_policy: Option<String>,
    #[serde(rename = "defaultVariantAxis", skip_serializing_if = "Option::is_none")]
    pub default_variant_axis: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductAttributeValueBatchDraft {
    pub locale: String,
    pub patches: Vec<ProductAttributeValuePatchDraft>,
}

#[cfg(test)]
mod variant_update_draft_tests {
    use super::{ProductPrice, ProductVariant, VariantUpdateDraft};
    use rustok_api::Patch;

    fn original_variant() -> ProductVariant {
        ProductVariant {
            id: "variant-1".to_owned(),
            revision: "1".to_owned(),
            sku: Some("SKU-1".to_owned()),
            barcode: Some("BAR-1".to_owned()),
            shipping_profile_slug: None,
            title: "Variant".to_owned(),
            combination_identity: None,
            axis_values: Vec::new(),
            prices: vec![
                ProductPrice {
                    currency_code: "USD".to_owned(),
                    amount: "12.50".to_owned(),
                    compare_at_amount: None,
                    on_sale: false,
                },
                ProductPrice {
                    currency_code: "EUR".to_owned(),
                    amount: "11.50".to_owned(),
                    compare_at_amount: None,
                    on_sale: false,
                },
            ],
            inventory_quantity: 8,
            inventory_policy: "deny".to_owned(),
            in_stock: true,
        }
    }

    fn draft(
        original: &ProductVariant,
        sku: &str,
        amount: &str,
        currency: &str,
    ) -> VariantUpdateDraft {
        VariantUpdateDraft::from_editor_state(
            original,
            sku.to_owned(),
            "BAR-1".to_owned(),
            original.shipping_profile_slug.clone().unwrap_or_default(),
            currency.to_owned(),
            amount.to_owned(),
            String::new(),
            8,
            "deny".to_owned(),
        )
    }

    #[test]
    fn unchanged_form_values_do_not_replay_inventory_or_pricing_snapshots() {
        let original = original_variant();
        let draft = draft(&original, "SKU-1", "12.50", "USD");
        assert!(matches!(draft.sku, Patch::Keep));
        assert!(matches!(draft.barcode, Patch::Keep));
        assert!(matches!(draft.shipping_profile_slug, Patch::Keep));
        assert!(draft.inventory_quantity.is_none());
        assert!(draft.inventory_policy.is_none());
        assert!(!draft.prices_changed);
        assert!(draft.prices.is_empty());
    }

    #[test]
    fn sku_only_edit_does_not_submit_stale_inventory_or_price_values() {
        let original = original_variant();
        let draft = draft(&original, "SKU-2", "12.50", "USD");
        assert!(matches!(draft.sku, Patch::Set(value) if value == "SKU-2"));
        assert!(draft.inventory_quantity.is_none());
        assert!(draft.inventory_policy.is_none());
        assert!(!draft.prices_changed);
        assert!(draft.prices.is_empty());
    }

    #[test]
    fn effective_change_detection_ignores_unmarked_price_buffers_and_keeps_explicit_clears() {
        let mut draft = VariantUpdateDraft::default();
        draft.prices.push(super::VariantPriceDraft {
            currency_code: "USD".to_owned(),
            amount: "1.00".to_owned(),
            compare_at_amount: None,
        });
        assert!(!draft.has_changes());

        draft.prices_changed = true;
        assert!(draft.has_changes());

        draft.prices.clear();
        assert!(
            draft.has_changes(),
            "an explicit empty replacement clears prices"
        );

        draft.prices_changed = false;
        draft.sku = Patch::Clear;
        assert!(draft.has_changes());
    }

    #[test]
    fn shipping_profile_override_is_sparse_and_explicitly_clearable() {
        let mut original = original_variant();
        original.shipping_profile_slug = Some("parcel".to_owned());

        let updated = VariantUpdateDraft::from_editor_state(
            &original,
            "SKU-1".to_owned(),
            "BAR-1".to_owned(),
            "express".to_owned(),
            "USD".to_owned(),
            "12.50".to_owned(),
            String::new(),
            8,
            "deny".to_owned(),
        );
        assert!(matches!(
            updated.shipping_profile_slug,
            Patch::Set(value) if value == "express"
        ));

        let cleared = VariantUpdateDraft::from_editor_state(
            &original,
            "SKU-1".to_owned(),
            "BAR-1".to_owned(),
            String::new(),
            "USD".to_owned(),
            "12.50".to_owned(),
            String::new(),
            8,
            "deny".to_owned(),
        );
        assert!(matches!(cleared.shipping_profile_slug, Patch::Clear));
    }

    #[test]
    fn unchanged_canonical_inventory_policy_is_not_replayed() {
        let mut original = original_variant();
        original.inventory_policy = "continue".to_owned();

        let draft = VariantUpdateDraft::from_editor_state(
            &original,
            "SKU-1".to_owned(),
            "BAR-1".to_owned(),
            String::new(),
            "USD".to_owned(),
            "12.50".to_owned(),
            String::new(),
            8,
            "continue".to_owned(),
        );

        assert!(draft.inventory_policy.is_none());
        assert!(!draft.has_changes());
    }

    #[test]
    fn blank_sku_and_barcode_are_explicit_nullable_clears() {
        let mut original = original_variant();
        original.shipping_profile_slug = Some("parcel".to_owned());

        let draft = VariantUpdateDraft::from_editor_state(
            &original,
            String::new(),
            String::new(),
            "parcel".to_owned(),
            "USD".to_owned(),
            "12.50".to_owned(),
            String::new(),
            8,
            "deny".to_owned(),
        );

        assert!(matches!(draft.sku, Patch::Clear));
        assert!(matches!(draft.barcode, Patch::Clear));
        assert!(matches!(draft.shipping_profile_slug, Patch::Keep));
        assert!(!draft.prices_changed);
        assert!(crate::core::validate_editor_variant_draft(&draft).is_ok());
    }

    #[test]
    fn primary_price_edit_preserves_other_currency_prices() {
        let original = original_variant();
        let draft = draft(&original, "SKU-1", "13.00", "USD");
        assert!(draft.prices_changed);
        assert_eq!(draft.prices.len(), 2);
        assert_eq!(draft.prices[0].amount, "13.00");
        assert_eq!(draft.prices[1].currency_code, "EUR");
        assert_eq!(draft.prices[1].amount, "11.50");
    }

    #[test]
    fn empty_primary_price_input_leaves_all_base_prices_unchanged() {
        let original = original_variant();
        let draft = draft(&original, "SKU-1", "", "USD");
        assert!(!draft.prices_changed);
        assert!(draft.prices.is_empty());
        assert!(!draft.has_changes());
    }

    #[test]
    fn explicit_primary_price_clear_preserves_other_currency_prices() {
        let original = original_variant();
        let draft = VariantUpdateDraft::from_editor_state_with_price_clear(
            &original,
            "SKU-1".to_owned(),
            "BAR-1".to_owned(),
            String::new(),
            "USD".to_owned(),
            String::new(),
            String::new(),
            8,
            "deny".to_owned(),
            true,
        );
        assert!(draft.prices_changed);
        assert_eq!(draft.prices.len(), 1);
        assert_eq!(draft.prices[0].currency_code, "EUR");
    }

    #[test]
    fn product_variant_deserialization_rejects_missing_revision() {
        let value = serde_json::json!({
            "id": "00000000-0000-4000-8000-000000000001",
            "sku": null,
            "barcode": null,
            "shippingProfileSlug": null,
            "title": "Variant",
            "combinationIdentity": null,
            "axisValues": [],
            "prices": [],
            "inventoryQuantity": 0,
            "inventoryPolicy": "deny",
            "inStock": false
        });

        assert!(
            serde_json::from_value::<ProductVariant>(value).is_err(),
            "a variant without its concurrency revision must not enter an editable snapshot"
        );
    }
}
