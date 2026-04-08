use chrono::Utc;
use flex::{
    persist_localized_values, prepare_attached_values_create, prepare_attached_values_update,
    resolve_attached_payload,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, QueryFilter, QueryOrder, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap, HashSet};
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;
use validator::Validate;

use rustok_core::field_schema::{CustomFieldsSchema, FieldDefinition, FieldType, ValidationRule};
use rustok_core::{generate_id, PLATFORM_FALLBACK_LOCALE};
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::{TaxonomyService, TaxonomyTermKind};

use rustok_commerce_foundation::dto::*;
use rustok_commerce_foundation::entities;
use rustok_commerce_foundation::error::{CommerceError, CommerceResult};

use crate::entities::product_tag;

const PRODUCT_SCOPE_VALUE: &str = "product";

mod product_field_definitions_storage {
    rustok_core::define_field_definitions_entity!("product_field_definitions");
}

struct ProductTagState {
    tags: Vec<String>,
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
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub tags: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub struct CatalogService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl CatalogService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id))]
    pub async fn create_product(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: CreateProductInput,
    ) -> CommerceResult<ProductResponse> {
        debug!(
            translations_count = input.translations.len(),
            variants_count = input.variants.len(),
            options_count = input.options.len(),
            publish = input.publish,
            "Creating product"
        );

        input
            .validate()
            .map_err(|e| CommerceError::Validation(e.to_string()))?;

        if input.translations.is_empty() {
            warn!("Product creation rejected: no translations");
            return Err(CommerceError::Validation(
                "At least one translation is required".into(),
            ));
        }
        if input.variants.is_empty() {
            warn!("Product creation rejected: no variants");
            return Err(CommerceError::NoVariants);
        }

        let product_id = generate_id();
        let now = Utc::now();
        debug!(product_id = %product_id, "Generated product ID");

        let preferred_locale =
            Self::preferred_product_locale_from_translations(&input.translations);
        let prepared_custom_fields = self
            .prepare_product_custom_fields_for_create(
                tenant_id,
                preferred_locale.as_str(),
                input.metadata.clone(),
            )
            .await?;
        let product_metadata = prepared_custom_fields
            .metadata
            .clone()
            .unwrap_or_else(|| serde_json::json!({}));
        let (normalized_metadata, normalized_tags) = Self::normalize_create_product_metadata(
            input.tags.clone(),
            input.shipping_profile_slug.clone(),
            product_metadata,
        );

        let txn = self.db.begin().await?;

        let product = entities::product::ActiveModel {
            id: Set(product_id),
            tenant_id: Set(tenant_id),
            status: Set(if input.publish {
                entities::product::ProductStatus::Active
            } else {
                entities::product::ProductStatus::Draft
            }),
            vendor: Set(input.vendor.clone()),
            product_type: Set(input.product_type.clone()),
            shipping_profile_slug: Set(input
                .shipping_profile_slug
                .as_deref()
                .and_then(Self::normalize_shipping_profile_slug)),
            metadata: Set(normalized_metadata),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            published_at: Set(if input.publish {
                Some(now.into())
            } else {
                None
            }),
        };
        product.insert(&txn).await?;
        debug!("Product entity inserted");

        if let (Some(locale), Some(values)) = (
            prepared_custom_fields.locale.as_deref(),
            prepared_custom_fields.localized_values.as_ref(),
        ) {
            persist_localized_values(&txn, tenant_id, "product", product_id, locale, values)
                .await
                .map_err(|error| CommerceError::Validation(error.to_string()))?;
        }

        let translation_locales = Self::collect_translation_locales(&input.translations);

        let mut seen = HashSet::new();
        for trans_input in &input.translations {
            let handle = trans_input
                .handle
                .clone()
                .unwrap_or_else(|| Self::slugify(&trans_input.title));

            let key = format!("{}::{}", trans_input.locale, handle.clone());
            if !seen.insert(key) {
                warn!(handle = %handle, locale = %trans_input.locale, "Duplicate handle detected");
                return Err(CommerceError::DuplicateHandle {
                    handle,
                    locale: trans_input.locale.clone(),
                });
            }

            let existing = entities::product_translation::Entity::find()
                .filter(entities::product_translation::Column::Locale.eq(&trans_input.locale))
                .filter(entities::product_translation::Column::Handle.eq(&handle))
                .one(&txn)
                .await?;
            if existing.is_some() {
                return Err(CommerceError::DuplicateHandle {
                    handle,
                    locale: trans_input.locale.clone(),
                });
            }

            let translation = entities::product_translation::ActiveModel {
                id: Set(generate_id()),
                product_id: Set(product_id),
                locale: Set(trans_input.locale.clone()),
                title: Set(trans_input.title.clone()),
                handle: Set(handle),
                description: Set(trans_input.description.clone()),
                meta_title: Set(trans_input.meta_title.clone()),
                meta_description: Set(trans_input.meta_description.clone()),
            };
            translation.insert(&txn).await?;
        }
        debug!(
            translations_count = input.translations.len(),
            "Product translations inserted"
        );

        for (position, opt_input) in input.options.iter().enumerate() {
            let option_id = generate_id();
            let option = entities::product_option::ActiveModel {
                id: Set(option_id),
                product_id: Set(product_id),
                position: Set(position as i32),
                name: Set(opt_input.name.clone()),
                values: Set(serde_json::to_value(&opt_input.values)
                    .map_err(|error| CommerceError::Validation(error.to_string()))?),
            };
            option.insert(&txn).await?;

            for locale in &translation_locales {
                entities::product_option_translation::ActiveModel {
                    id: Set(generate_id()),
                    option_id: Set(option_id),
                    locale: Set(locale.clone()),
                    title: Set(opt_input.name.clone()),
                }
                .insert(&txn)
                .await?;
            }

            for (value_position, value) in opt_input.values.iter().enumerate() {
                let option_value_id = generate_id();
                entities::product_option_value::ActiveModel {
                    id: Set(option_value_id),
                    option_id: Set(option_id),
                    position: Set(value_position as i32),
                    metadata: Set(serde_json::json!({})),
                }
                .insert(&txn)
                .await?;

                for locale in &translation_locales {
                    entities::product_option_value_translation::ActiveModel {
                        id: Set(generate_id()),
                        value_id: Set(option_value_id),
                        locale: Set(locale.clone()),
                        value: Set(value.clone()),
                    }
                    .insert(&txn)
                    .await?;
                }
            }
        }
        debug!(
            options_count = input.options.len(),
            "Product options inserted"
        );

        let default_stock_location = Self::ensure_default_stock_location(&txn, tenant_id).await?;

        for (position, var_input) in input.variants.iter().enumerate() {
            let variant_id = generate_id();

            if let Some(ref sku) = var_input.sku {
                let existing = entities::product_variant::Entity::find()
                    .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
                    .filter(entities::product_variant::Column::Sku.eq(sku))
                    .one(&txn)
                    .await?;
                if existing.is_some() {
                    warn!(sku = %sku, "Duplicate SKU detected");
                    return Err(CommerceError::DuplicateSku(sku.clone()));
                }
            }

            let variant = entities::product_variant::ActiveModel {
                id: Set(variant_id),
                product_id: Set(product_id),
                tenant_id: Set(tenant_id),
                sku: Set(var_input.sku.clone()),
                barcode: Set(var_input.barcode.clone()),
                shipping_profile_slug: Set(var_input
                    .shipping_profile_slug
                    .as_deref()
                    .and_then(Self::normalize_shipping_profile_slug)),
                ean: Set(None),
                upc: Set(None),
                inventory_policy: Set(var_input.inventory_policy.clone()),
                inventory_management: Set("manual".into()),
                inventory_quantity: Set(0),
                weight: Set(var_input.weight),
                weight_unit: Set(var_input.weight_unit.clone()),
                option1: Set(var_input.option1.clone()),
                option2: Set(var_input.option2.clone()),
                option3: Set(var_input.option3.clone()),
                position: Set(position as i32),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            };
            variant.insert(&txn).await?;

            Self::create_initial_inventory_records(
                &txn,
                &default_stock_location,
                variant_id,
                var_input.sku.clone(),
                var_input.inventory_quantity,
            )
            .await?;

            let variant_title = Self::generate_variant_title_from_inputs(
                var_input.option1.as_deref(),
                var_input.option2.as_deref(),
                var_input.option3.as_deref(),
            );
            for locale in &translation_locales {
                entities::variant_translation::ActiveModel {
                    id: Set(generate_id()),
                    variant_id: Set(variant_id),
                    locale: Set(locale.clone()),
                    title: Set(Some(variant_title.clone())),
                }
                .insert(&txn)
                .await?;
            }

            for price_input in &var_input.prices {
                let price = entities::price::ActiveModel {
                    id: Set(generate_id()),
                    variant_id: Set(variant_id),
                    price_list_id: Set(None),
                    currency_code: Set(price_input.currency_code.clone()),
                    region_id: Set(None),
                    amount: Set(price_input.amount),
                    compare_at_amount: Set(price_input.compare_at_amount),
                    legacy_amount: Set(Self::decimal_to_cents(price_input.amount)),
                    legacy_compare_at_amount: Set(price_input
                        .compare_at_amount
                        .and_then(Self::decimal_to_cents)),
                    min_quantity: Set(None),
                    max_quantity: Set(None),
                };
                price.insert(&txn).await?;
            }
        }
        debug!(
            variants_count = input.variants.len(),
            "Product variants and prices inserted"
        );

        if let Some(tags) = normalized_tags.as_deref() {
            let locale = input
                .translations
                .first()
                .map(|translation| translation.locale.as_str())
                .unwrap_or("en");
            self.sync_product_tags_in_tx(&txn, tenant_id, product_id, locale, tags)
                .await?;
        }

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductCreated { product_id },
            )
            .await?;

        txn.commit().await?;
        debug!("Transaction committed");

        info!(
            product_id = %product_id,
            translations_count = input.translations.len(),
            variants_count = input.variants.len(),
            status = if input.publish { "active" } else { "draft" },
            "Product created successfully"
        );

        self.get_product_with_locale_fallback(
            tenant_id,
            product_id,
            preferred_locale.as_str(),
            None,
        )
        .await
    }

    #[instrument(skip(self))]
    pub async fn get_product(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
    ) -> CommerceResult<ProductResponse> {
        self.get_product_with_locale_fallback(tenant_id, product_id, PLATFORM_FALLBACK_LOCALE, None)
            .await
    }

    #[instrument(skip(self))]
    pub async fn get_product_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> CommerceResult<ProductResponse> {
        debug!(product_id = %product_id, "Fetching product");

        let product = entities::product::Entity::find_by_id(product_id)
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                warn!(product_id = %product_id, "Product not found");
                CommerceError::ProductNotFound(product_id)
            })?;

        let translations = entities::product_translation::Entity::find()
            .filter(entities::product_translation::Column::ProductId.eq(product_id))
            .all(&self.db)
            .await?;

        let options = entities::product_option::Entity::find()
            .filter(entities::product_option::Column::ProductId.eq(product_id))
            .order_by_asc(entities::product_option::Column::Position)
            .all(&self.db)
            .await?;

        let variants = entities::product_variant::Entity::find()
            .filter(entities::product_variant::Column::ProductId.eq(product_id))
            .order_by_asc(entities::product_variant::Column::Position)
            .all(&self.db)
            .await?;

        let option_ids: Vec<Uuid> = options.iter().map(|option| option.id).collect();
        let option_translations = if !option_ids.is_empty() {
            entities::product_option_translation::Entity::find()
                .filter(
                    entities::product_option_translation::Column::OptionId
                        .is_in(option_ids.clone()),
                )
                .order_by_asc(entities::product_option_translation::Column::Locale)
                .all(&self.db)
                .await?
        } else {
            Vec::new()
        };
        let option_values = if !option_ids.is_empty() {
            entities::product_option_value::Entity::find()
                .filter(entities::product_option_value::Column::OptionId.is_in(option_ids.clone()))
                .order_by_asc(entities::product_option_value::Column::Position)
                .all(&self.db)
                .await?
        } else {
            Vec::new()
        };
        let option_value_ids: Vec<Uuid> = option_values.iter().map(|value| value.id).collect();
        let option_value_translations = if !option_value_ids.is_empty() {
            entities::product_option_value_translation::Entity::find()
                .filter(
                    entities::product_option_value_translation::Column::ValueId
                        .is_in(option_value_ids),
                )
                .order_by_asc(entities::product_option_value_translation::Column::Locale)
                .all(&self.db)
                .await?
        } else {
            Vec::new()
        };

        // Load all prices for all variants in a single query (fixes N+1)
        let variant_ids: Vec<Uuid> = variants.iter().map(|v| v.id).collect();
        let all_prices = if !variant_ids.is_empty() {
            entities::price::Entity::find()
                .filter(entities::price::Column::VariantId.is_in(variant_ids.clone()))
                .all(&self.db)
                .await?
        } else {
            Vec::new()
        };
        let variant_translations = if !variant_ids.is_empty() {
            entities::variant_translation::Entity::find()
                .filter(entities::variant_translation::Column::VariantId.is_in(variant_ids.clone()))
                .order_by_asc(entities::variant_translation::Column::Locale)
                .all(&self.db)
                .await?
        } else {
            Vec::new()
        };
        let available_inventory_by_variant =
            Self::load_available_quantities(&self.db, &variant_ids).await?;

        // Group prices by variant_id
        let mut prices_by_variant: HashMap<Uuid, Vec<entities::price::Model>> = HashMap::new();
        for price in all_prices {
            prices_by_variant
                .entry(price.variant_id)
                .or_default()
                .push(price);
        }
        let mut option_translations_by_option: HashMap<
            Uuid,
            Vec<entities::product_option_translation::Model>,
        > = HashMap::new();
        for translation in option_translations {
            option_translations_by_option
                .entry(translation.option_id)
                .or_default()
                .push(translation);
        }
        let mut option_values_by_option: HashMap<Uuid, Vec<entities::product_option_value::Model>> =
            HashMap::new();
        for value in option_values {
            option_values_by_option
                .entry(value.option_id)
                .or_default()
                .push(value);
        }
        let mut option_value_translations_by_value: HashMap<
            Uuid,
            Vec<entities::product_option_value_translation::Model>,
        > = HashMap::new();
        for translation in option_value_translations {
            option_value_translations_by_value
                .entry(translation.value_id)
                .or_default()
                .push(translation);
        }
        let mut variant_translations_by_variant: HashMap<
            Uuid,
            Vec<entities::variant_translation::Model>,
        > = HashMap::new();
        for translation in variant_translations {
            variant_translations_by_variant
                .entry(translation.variant_id)
                .or_default()
                .push(translation);
        }

        let variant_responses: Vec<VariantResponse> = variants
            .into_iter()
            .map(|variant| {
                let prices = prices_by_variant.remove(&variant.id).unwrap_or_default();

                let price_responses: Vec<PriceResponse> = prices
                    .into_iter()
                    .map(|price| PriceResponse {
                        currency_code: price.currency_code,
                        amount: price.amount,
                        compare_at_amount: price.compare_at_amount,
                        on_sale: price
                            .compare_at_amount
                            .map(|c| c > price.amount)
                            .unwrap_or(false),
                    })
                    .collect();

                let title = Self::generate_variant_title(&variant);
                let available_inventory = available_inventory_by_variant
                    .get(&variant.id)
                    .copied()
                    .unwrap_or(0);

                VariantResponse {
                    id: variant.id,
                    product_id: variant.product_id,
                    sku: variant.sku,
                    barcode: variant.barcode,
                    shipping_profile_slug: variant.shipping_profile_slug.clone(),
                    title,
                    translations: variant_translations_by_variant
                        .remove(&variant.id)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|translation| VariantTranslationResponse {
                            locale: translation.locale,
                            title: translation.title,
                        })
                        .collect(),
                    option1: variant.option1,
                    option2: variant.option2,
                    option3: variant.option3,
                    prices: price_responses,
                    inventory_quantity: available_inventory,
                    inventory_policy: variant.inventory_policy.clone(),
                    in_stock: available_inventory > 0 || variant.inventory_policy == "continue",
                    weight: variant.weight,
                    weight_unit: variant.weight_unit,
                    position: variant.position,
                }
            })
            .collect();

        let images = entities::product_image::Entity::find()
            .filter(entities::product_image::Column::ProductId.eq(product_id))
            .order_by_asc(entities::product_image::Column::Position)
            .all(&self.db)
            .await?;
        let image_ids: Vec<Uuid> = images.iter().map(|image| image.id).collect();
        let image_translations = if !image_ids.is_empty() {
            entities::product_image_translation::Entity::find()
                .filter(entities::product_image_translation::Column::ImageId.is_in(image_ids))
                .order_by_asc(entities::product_image_translation::Column::Locale)
                .all(&self.db)
                .await?
        } else {
            Vec::new()
        };
        let mut image_translations_by_image: HashMap<
            Uuid,
            Vec<entities::product_image_translation::Model>,
        > = HashMap::new();
        for translation in image_translations {
            image_translations_by_image
                .entry(translation.image_id)
                .or_default()
                .push(translation);
        }

        let tag_locale = locale;
        let product_tags = self
            .load_product_tags(
                tenant_id,
                product_id,
                tag_locale,
                fallback_locale.or(Some(PLATFORM_FALLBACK_LOCALE)),
                &product.metadata,
            )
            .await?;
        let resolved_metadata = self
            .resolve_product_metadata(
                tenant_id,
                product_id,
                &product.metadata,
                locale,
                fallback_locale.unwrap_or(PLATFORM_FALLBACK_LOCALE),
            )
            .await?;

        let response = ProductResponse {
            id: product.id,
            tenant_id: product.tenant_id,
            status: product.status,
            vendor: product.vendor,
            product_type: product.product_type,
            shipping_profile_slug: product
                .shipping_profile_slug
                .clone()
                .or_else(|| Self::extract_shipping_profile_slug(&product.metadata)),
            tags: product_tags.tags,
            metadata: resolved_metadata,
            created_at: product.created_at.into(),
            updated_at: product.updated_at.into(),
            published_at: product.published_at.map(Into::into),
            translations: translations
                .into_iter()
                .map(|translation| ProductTranslationResponse {
                    locale: translation.locale,
                    title: translation.title,
                    handle: translation.handle,
                    description: translation.description,
                    meta_title: translation.meta_title,
                    meta_description: translation.meta_description,
                })
                .collect(),
            options: options
                .into_iter()
                .map(|option| {
                    let option_id = option.id;
                    let translations = Self::build_option_translations(
                        &option,
                        option_translations_by_option
                            .remove(&option_id)
                            .unwrap_or_default(),
                        option_values_by_option
                            .remove(&option_id)
                            .unwrap_or_default(),
                        &option_value_translations_by_value,
                    );

                    ProductOptionResponse {
                        id: option_id,
                        name: option.name,
                        values: serde_json::from_value(option.values.clone()).unwrap_or_default(),
                        position: option.position,
                        translations,
                    }
                })
                .collect(),
            variants: variant_responses,
            images: images
                .into_iter()
                .map(|image| ProductImageResponse {
                    id: image.id,
                    media_id: image.media_id,
                    url: format!("/api/v1/media/{}", image.media_id),
                    alt_text: image.alt_text,
                    position: image.position,
                    translations: image_translations_by_image
                        .remove(&image.id)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|translation| ProductImageTranslationResponse {
                            locale: translation.locale,
                            alt_text: translation.alt_text,
                        })
                        .collect(),
                })
                .collect(),
        };

        debug!(
            product_id = %product_id,
            variants_count = response.variants.len(),
            "Product fetched successfully"
        );

        Ok(response)
    }

    #[instrument(skip(self))]
    pub async fn list_published_products_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        public_channel_slug: Option<&str>,
        page: u64,
        per_page: u64,
    ) -> CommerceResult<StorefrontProductList> {
        let fallback_locale = fallback_locale.unwrap_or(PLATFORM_FALLBACK_LOCALE);
        let page = page.max(1);
        let per_page = per_page.clamp(1, 48);
        let offset = (page.saturating_sub(1)) * per_page;

        let visible_products = entities::product::Entity::find()
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .filter(entities::product::Column::Status.eq(entities::product::ProductStatus::Active))
            .filter(entities::product::Column::PublishedAt.is_not_null())
            .order_by_desc(entities::product::Column::PublishedAt)
            .order_by_desc(entities::product::Column::CreatedAt)
            .all(&self.db)
            .await?
            .into_iter()
            .filter(|product| {
                is_metadata_visible_for_public_channel(&product.metadata, public_channel_slug)
            })
            .collect::<Vec<_>>();
        let total = visible_products.len() as u64;
        let products = visible_products
            .into_iter()
            .skip(offset as usize)
            .take(per_page as usize)
            .collect::<Vec<_>>();
        let product_ids = products
            .iter()
            .map(|product| product.id)
            .collect::<Vec<_>>();
        let translations = if product_ids.is_empty() {
            Vec::new()
        } else {
            entities::product_translation::Entity::find()
                .filter(entities::product_translation::Column::ProductId.is_in(product_ids))
                .all(&self.db)
                .await?
        };
        let mut translations_by_product: HashMap<Uuid, Vec<entities::product_translation::Model>> =
            HashMap::new();
        for translation in translations {
            translations_by_product
                .entry(translation.product_id)
                .or_default()
                .push(translation);
        }
        let product_tags = self
            .load_product_tag_map(tenant_id, &products, locale, Some(fallback_locale))
            .await?;

        let items = products
            .into_iter()
            .map(|product| {
                let translation = translations_by_product.get(&product.id).and_then(|items| {
                    pick_product_translation(items.as_slice(), locale, fallback_locale)
                });
                StorefrontProductListItem {
                    id: product.id,
                    status: product.status,
                    title: translation
                        .map(|value| value.title.clone())
                        .unwrap_or_else(|| "Untitled product".to_string()),
                    handle: translation
                        .map(|value| value.handle.clone())
                        .unwrap_or_default(),
                    vendor: product.vendor,
                    product_type: product.product_type,
                    tags: product_tags.get(&product.id).cloned().unwrap_or_default(),
                    created_at: product.created_at.into(),
                    published_at: product.published_at.map(Into::into),
                }
            })
            .collect::<Vec<_>>();

        Ok(StorefrontProductList {
            items,
            total,
            page,
            per_page,
            has_next: page * per_page < total,
        })
    }

    #[instrument(skip(self))]
    pub async fn get_published_product_by_handle_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        handle: &str,
        locale: &str,
        fallback_locale: Option<&str>,
        public_channel_slug: Option<&str>,
    ) -> CommerceResult<Option<ProductResponse>> {
        let fallback_locale = fallback_locale.unwrap_or(PLATFORM_FALLBACK_LOCALE);
        let Some(product_id) = find_published_product_id_by_handle(
            &self.db,
            tenant_id,
            handle,
            locale,
            fallback_locale,
            public_channel_slug,
        )
        .await?
        else {
            return Ok(None);
        };

        let mut product = match self
            .get_product_with_locale_fallback(tenant_id, product_id, locale, Some(fallback_locale))
            .await
        {
            Ok(product) => product,
            Err(CommerceError::ProductNotFound(_)) => return Ok(None),
            Err(error) => return Err(error),
        };

        if product.status != entities::product::ProductStatus::Active
            || product.published_at.is_none()
            || !is_metadata_visible_for_public_channel(&product.metadata, public_channel_slug)
        {
            return Ok(None);
        }

        apply_public_channel_inventory_to_product(
            &self.db,
            tenant_id,
            &mut product,
            public_channel_slug,
        )
        .await?;

        Ok(Some(localize_product_response(
            product,
            locale,
            fallback_locale,
        )))
    }

    #[instrument(skip(self, input))]
    pub async fn update_product(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        product_id: Uuid,
        input: UpdateProductInput,
    ) -> CommerceResult<ProductResponse> {
        debug!(product_id = %product_id, "Updating product");

        input
            .validate()
            .map_err(|e| CommerceError::Validation(e.to_string()))?;

        let txn = self.db.begin().await?;

        let product = entities::product::Entity::find_by_id(product_id)
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or_else(|| {
                warn!(product_id = %product_id, "Product not found for update");
                CommerceError::ProductNotFound(product_id)
            })?;
        let existing_product = product.clone();
        let mut product_active: entities::product::ActiveModel = product.into();
        product_active.updated_at = Set(Utc::now().into());

        let preferred_locale = input
            .translations
            .as_deref()
            .map(Self::preferred_product_locale_from_translations)
            .unwrap_or_else(|| {
                Self::preferred_product_locale_from_metadata(&existing_product.metadata)
            });
        let prepared_custom_fields = if let Some(metadata) = input.metadata.clone() {
            Some(
                self.prepare_product_custom_fields_for_update(
                    tenant_id,
                    product_id,
                    preferred_locale.as_str(),
                    &existing_product.metadata,
                    metadata,
                )
                .await?,
            )
        } else {
            None
        };
        let metadata_update = Self::normalize_update_product_metadata(
            input.tags.clone(),
            input.shipping_profile_slug.clone(),
            prepared_custom_fields
                .as_ref()
                .and_then(|prepared| prepared.metadata.clone()),
            existing_product.metadata.clone(),
        );
        let shipping_profile_input = input.shipping_profile_slug.clone();

        if let Some(vendor) = input.vendor {
            product_active.vendor = Set(Some(vendor));
        }
        if let Some(product_type) = input.product_type {
            product_active.product_type = Set(Some(product_type));
        }
        if shipping_profile_input.is_some() {
            product_active.shipping_profile_slug = Set(shipping_profile_input
                .as_deref()
                .and_then(Self::normalize_shipping_profile_slug));
        }
        if let Some((metadata, _)) = metadata_update.as_ref() {
            product_active.metadata = Set(metadata.clone());
        }
        if let Some(status) = input.status {
            product_active.status = Set(status);
        }

        product_active.update(&txn).await?;

        if let Some(prepared_custom_fields) = prepared_custom_fields.as_ref() {
            if let (Some(locale), Some(values)) = (
                prepared_custom_fields.locale.as_deref(),
                prepared_custom_fields.localized_values.as_ref(),
            ) {
                persist_localized_values(&txn, tenant_id, "product", product_id, locale, values)
                    .await
                    .map_err(|error| CommerceError::Validation(error.to_string()))?;
            }
        }

        let translation_inputs = input.translations.clone();

        if let Some(translations) = translation_inputs {
            entities::product_translation::Entity::delete_many()
                .filter(entities::product_translation::Column::ProductId.eq(product_id))
                .exec(&txn)
                .await?;

            let mut seen = HashSet::new();
            for translation_input in translations {
                let handle = translation_input
                    .handle
                    .clone()
                    .unwrap_or_else(|| Self::slugify(&translation_input.title));

                let locale = translation_input.locale.clone();
                let key = format!("{}::{}", locale, handle.clone());
                if !seen.insert(key) {
                    return Err(CommerceError::DuplicateHandle { handle, locale });
                }

                let existing = entities::product_translation::Entity::find()
                    .filter(
                        entities::product_translation::Column::Locale.eq(&translation_input.locale),
                    )
                    .filter(entities::product_translation::Column::Handle.eq(&handle))
                    .filter(entities::product_translation::Column::ProductId.ne(product_id))
                    .one(&txn)
                    .await?;
                if existing.is_some() {
                    return Err(CommerceError::DuplicateHandle {
                        handle,
                        locale: translation_input.locale,
                    });
                }

                let translation = entities::product_translation::ActiveModel {
                    id: Set(generate_id()),
                    product_id: Set(product_id),
                    locale: Set(translation_input.locale),
                    title: Set(translation_input.title),
                    handle: Set(handle),
                    description: Set(translation_input.description),
                    meta_title: Set(translation_input.meta_title),
                    meta_description: Set(translation_input.meta_description),
                };
                translation.insert(&txn).await?;
            }
        }

        if let Some((_, Some(tags))) = metadata_update.as_ref() {
            let locale = self
                .resolve_tag_locale_for_update(&txn, product_id, input.translations.as_deref())
                .await?;
            self.sync_product_tags_in_tx(&txn, tenant_id, product_id, &locale, tags)
                .await?;
        }

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductUpdated { product_id },
            )
            .await?;

        txn.commit().await?;
        info!(product_id = %product_id, "Product updated successfully");

        self.get_product_with_locale_fallback(
            tenant_id,
            product_id,
            preferred_locale.as_str(),
            None,
        )
        .await
    }

    #[instrument(skip(self))]
    pub async fn publish_product(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        product_id: Uuid,
    ) -> CommerceResult<ProductResponse> {
        debug!(product_id = %product_id, "Publishing product");

        let txn = self.db.begin().await?;

        let product = entities::product::Entity::find_by_id(product_id)
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or_else(|| {
                warn!(product_id = %product_id, "Product not found for publishing");
                CommerceError::ProductNotFound(product_id)
            })?;

        let mut product_active: entities::product::ActiveModel = product.into();
        product_active.status = Set(entities::product::ProductStatus::Active);
        product_active.published_at = Set(Some(Utc::now().into()));
        product_active.updated_at = Set(Utc::now().into());
        product_active.update(&txn).await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductPublished { product_id },
            )
            .await?;

        txn.commit().await?;
        info!(product_id = %product_id, "Product published successfully");

        self.get_product(tenant_id, product_id).await
    }

    #[instrument(skip(self))]
    pub async fn unpublish_product(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        product_id: Uuid,
    ) -> CommerceResult<ProductResponse> {
        debug!(product_id = %product_id, "Unpublishing product");

        let txn = self.db.begin().await?;

        let product = entities::product::Entity::find_by_id(product_id)
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(CommerceError::ProductNotFound(product_id))?;

        let mut product_active: entities::product::ActiveModel = product.into();
        product_active.status = Set(entities::product::ProductStatus::Draft);
        product_active.updated_at = Set(Utc::now().into());
        product_active.update(&txn).await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductUpdated { product_id },
            )
            .await?;

        txn.commit().await?;
        info!(product_id = %product_id, "Product unpublished successfully");

        self.get_product(tenant_id, product_id).await
    }

    #[instrument(skip(self))]
    pub async fn delete_product(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        product_id: Uuid,
    ) -> CommerceResult<()> {
        debug!(product_id = %product_id, "Deleting product");

        let txn = self.db.begin().await?;

        let product = entities::product::Entity::find_by_id(product_id)
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(CommerceError::ProductNotFound(product_id))?;

        if product.status == entities::product::ProductStatus::Active {
            warn!(product_id = %product_id, "Cannot delete published product");
            return Err(CommerceError::CannotDeletePublished);
        }

        let variants = entities::product_variant::Entity::find()
            .filter(entities::product_variant::Column::ProductId.eq(product_id))
            .all(&txn)
            .await?;
        let variant_ids: Vec<Uuid> = variants.iter().map(|variant| variant.id).collect();

        if !variant_ids.is_empty() {
            let inventory_item_ids: Vec<Uuid> = entities::inventory_item::Entity::find()
                .filter(entities::inventory_item::Column::VariantId.is_in(variant_ids.clone()))
                .all(&txn)
                .await?
                .into_iter()
                .map(|item| item.id)
                .collect();

            if !inventory_item_ids.is_empty() {
                entities::reservation_item::Entity::delete_many()
                    .filter(
                        entities::reservation_item::Column::InventoryItemId
                            .is_in(inventory_item_ids.clone()),
                    )
                    .exec(&txn)
                    .await?;

                entities::inventory_level::Entity::delete_many()
                    .filter(
                        entities::inventory_level::Column::InventoryItemId
                            .is_in(inventory_item_ids.clone()),
                    )
                    .exec(&txn)
                    .await?;

                entities::inventory_item::Entity::delete_many()
                    .filter(entities::inventory_item::Column::Id.is_in(inventory_item_ids))
                    .exec(&txn)
                    .await?;
            }

            entities::price::Entity::delete_many()
                .filter(entities::price::Column::VariantId.is_in(variant_ids.clone()))
                .exec(&txn)
                .await?;

            entities::variant_translation::Entity::delete_many()
                .filter(entities::variant_translation::Column::VariantId.is_in(variant_ids))
                .exec(&txn)
                .await?;

            entities::product_variant::Entity::delete_many()
                .filter(entities::product_variant::Column::ProductId.eq(product_id))
                .exec(&txn)
                .await?;
        }

        entities::product_translation::Entity::delete_many()
            .filter(entities::product_translation::Column::ProductId.eq(product_id))
            .exec(&txn)
            .await?;

        let option_ids: Vec<Uuid> = entities::product_option::Entity::find()
            .filter(entities::product_option::Column::ProductId.eq(product_id))
            .all(&txn)
            .await?
            .into_iter()
            .map(|option| option.id)
            .collect();
        if !option_ids.is_empty() {
            let option_value_ids: Vec<Uuid> = entities::product_option_value::Entity::find()
                .filter(entities::product_option_value::Column::OptionId.is_in(option_ids.clone()))
                .all(&txn)
                .await?
                .into_iter()
                .map(|value| value.id)
                .collect();

            if !option_value_ids.is_empty() {
                entities::product_option_value_translation::Entity::delete_many()
                    .filter(
                        entities::product_option_value_translation::Column::ValueId
                            .is_in(option_value_ids.clone()),
                    )
                    .exec(&txn)
                    .await?;

                entities::product_option_value::Entity::delete_many()
                    .filter(entities::product_option_value::Column::Id.is_in(option_value_ids))
                    .exec(&txn)
                    .await?;
            }

            entities::product_option_translation::Entity::delete_many()
                .filter(
                    entities::product_option_translation::Column::OptionId
                        .is_in(option_ids.clone()),
                )
                .exec(&txn)
                .await?;
        }

        entities::product_option::Entity::delete_many()
            .filter(entities::product_option::Column::ProductId.eq(product_id))
            .exec(&txn)
            .await?;

        let image_ids: Vec<Uuid> = entities::product_image::Entity::find()
            .filter(entities::product_image::Column::ProductId.eq(product_id))
            .all(&txn)
            .await?
            .into_iter()
            .map(|image| image.id)
            .collect();
        if !image_ids.is_empty() {
            entities::product_image_translation::Entity::delete_many()
                .filter(entities::product_image_translation::Column::ImageId.is_in(image_ids))
                .exec(&txn)
                .await?;
        }

        entities::product_image::Entity::delete_many()
            .filter(entities::product_image::Column::ProductId.eq(product_id))
            .exec(&txn)
            .await?;

        entities::product::Entity::delete_by_id(product_id)
            .exec(&txn)
            .await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductDeleted { product_id },
            )
            .await?;

        txn.commit().await?;
        info!(product_id = %product_id, "Product deleted successfully");

        Ok(())
    }

    fn slugify(text: &str) -> String {
        use unicode_normalization::UnicodeNormalization;

        const MAX_LENGTH: usize = 255;
        const RESERVED_NAMES: &[&str] =
            &["admin", "api", "null", "undefined", "new", "edit", "delete"];

        // 1. Unicode normalization (NFC) to prevent homograph attacks
        let normalized: String = text.nfc().collect();

        // 2. Convert to lowercase and filter valid characters
        // Allow: a-z, 0-9, hyphen, space (will become hyphen)
        let slug: String = normalized
            .to_lowercase()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == ' ' || *c == '_')
            .map(|c| if c == ' ' || c == '_' { '-' } else { c })
            .collect();

        // 3. Remove consecutive hyphens and trim
        let slug = slug
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");

        // 4. Limit length
        let slug: String = slug.chars().take(MAX_LENGTH).collect();

        // 5. Prevent reserved names by adding suffix
        let slug = if RESERVED_NAMES.contains(&slug.as_str()) {
            format!("{}-1", slug)
        } else {
            slug
        };

        // 6. Ensure non-empty
        if slug.is_empty() {
            "untitled".to_string()
        } else {
            slug
        }
    }

    fn generate_variant_title(variant: &entities::product_variant::Model) -> String {
        Self::generate_variant_title_from_inputs(
            variant.option1.as_deref(),
            variant.option2.as_deref(),
            variant.option3.as_deref(),
        )
    }

    fn generate_variant_title_from_inputs(
        option1: Option<&str>,
        option2: Option<&str>,
        option3: Option<&str>,
    ) -> String {
        let options: Vec<&str> = [option1, option2, option3].into_iter().flatten().collect();

        if options.is_empty() {
            "Default".to_string()
        } else {
            options.join(" / ")
        }
    }

    async fn prepare_product_custom_fields_for_create(
        &self,
        tenant_id: Uuid,
        locale: &str,
        payload: Value,
    ) -> CommerceResult<flex::PreparedAttachedValuesWrite> {
        let schema = load_product_custom_fields_schema(&self.db, tenant_id).await?;
        let (reserved_payload, flex_payload) = split_product_metadata_payload(&schema, &payload);
        prepare_attached_values_create(schema, Some(Value::Object(flex_payload)), locale)
            .map(|mut prepared| {
                prepared.metadata = Some(merge_reserved_product_metadata(
                    reserved_payload,
                    prepared.metadata,
                ));
                prepared
            })
            .map_err(|error| CommerceError::Validation(error.to_string()))
    }

    async fn prepare_product_custom_fields_for_update(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: &str,
        existing_metadata: &Value,
        payload: Value,
    ) -> CommerceResult<flex::PreparedAttachedValuesWrite> {
        let schema = load_product_custom_fields_schema(&self.db, tenant_id).await?;
        let (reserved_payload, flex_payload) = split_product_metadata_payload(&schema, &payload);
        let (_, existing_flex_metadata) =
            split_product_metadata_payload(&schema, existing_metadata);
        prepare_attached_values_update(
            &self.db,
            tenant_id,
            "product",
            product_id,
            schema,
            locale,
            &Value::Object(existing_flex_metadata),
            Some(Value::Object(flex_payload)),
        )
        .await
        .map(|mut prepared| {
            prepared.metadata = Some(merge_reserved_product_metadata(
                reserved_payload,
                prepared.metadata,
            ));
            prepared
        })
        .map_err(|error| CommerceError::Validation(error.to_string()))
    }

    async fn resolve_product_metadata(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        metadata: &Value,
        locale: &str,
        fallback_locale: &str,
    ) -> CommerceResult<Value> {
        let shared_metadata = Self::strip_metadata_tags(metadata.clone());
        let schema = load_product_custom_fields_schema(&self.db, tenant_id).await?;
        resolve_attached_payload(
            &self.db,
            tenant_id,
            "product",
            product_id,
            schema,
            &shared_metadata,
            locale,
            fallback_locale,
        )
        .await
        .map(|payload| payload.unwrap_or_else(|| serde_json::json!({})))
        .map_err(|error| CommerceError::Validation(error.to_string()))
    }

    fn preferred_product_locale_from_translations(
        translations: &[ProductTranslationInput],
    ) -> String {
        translations
            .iter()
            .find_map(|translation| {
                let trimmed = translation.locale.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            })
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string())
    }

    fn preferred_product_locale_from_metadata(metadata: &Value) -> String {
        metadata
            .get("locale")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(|value| value.trim().to_string())
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string())
    }

    fn collect_translation_locales(translations: &[ProductTranslationInput]) -> Vec<String> {
        let mut locales = Vec::new();
        for translation in translations {
            if !locales.iter().any(|locale| locale == &translation.locale) {
                locales.push(translation.locale.clone());
            }
        }
        locales
    }

    fn normalize_tag_names(tag_names: &[String]) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut normalized = Vec::new();
        for tag_name in tag_names {
            let trimmed = tag_name.trim();
            if trimmed.is_empty() {
                continue;
            }
            let key = trimmed.to_ascii_lowercase();
            if seen.insert(key) {
                normalized.push(trimmed.to_string());
            }
        }
        normalized
    }

    fn metadata_has_tags_field(metadata: &Value) -> bool {
        metadata
            .as_object()
            .map(|object| object.contains_key("tags"))
            .unwrap_or(false)
    }

    fn extract_metadata_tags(metadata: &Value) -> Vec<String> {
        metadata
            .get("tags")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(ToOwned::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn strip_metadata_tags(mut metadata: Value) -> Value {
        if let Some(object) = metadata.as_object_mut() {
            object.remove("tags");
        }
        metadata
    }

    fn normalize_shipping_profile_slug(value: &str) -> Option<String> {
        let normalized = value.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        }
    }

    fn extract_shipping_profile_slug(metadata: &Value) -> Option<String> {
        metadata
            .get("shipping_profile")
            .and_then(|profile| profile.get("slug"))
            .and_then(Value::as_str)
            .and_then(Self::normalize_shipping_profile_slug)
            .or_else(|| {
                metadata
                    .get("shipping_profile_slug")
                    .and_then(Value::as_str)
                    .and_then(Self::normalize_shipping_profile_slug)
            })
    }

    fn apply_shipping_profile_to_metadata(
        mut metadata: Value,
        shipping_profile_slug: Option<String>,
    ) -> Value {
        let Some(normalized_slug) =
            shipping_profile_slug.and_then(|value| Self::normalize_shipping_profile_slug(&value))
        else {
            return metadata;
        };
        if !metadata.is_object() {
            metadata = Value::Object(Default::default());
        }

        if let Some(object) = metadata.as_object_mut() {
            object.remove("shipping_profile_slug");
            object.insert(
                "shipping_profile".to_string(),
                serde_json::json!({ "slug": normalized_slug }),
            );
        }

        metadata
    }

    fn normalize_create_product_metadata(
        input_tags: Vec<String>,
        shipping_profile_slug: Option<String>,
        metadata: Value,
    ) -> (Value, Option<Vec<String>>) {
        let normalized_input_tags = Self::normalize_tag_names(&input_tags);
        (
            Self::apply_shipping_profile_to_metadata(
                Self::strip_metadata_tags(metadata),
                shipping_profile_slug,
            ),
            Some(normalized_input_tags),
        )
    }

    fn normalize_update_product_metadata(
        input_tags: Option<Vec<String>>,
        shipping_profile_slug: Option<String>,
        metadata: Option<Value>,
        existing_metadata: Value,
    ) -> Option<(Value, Option<Vec<String>>)> {
        match (input_tags, shipping_profile_slug, metadata) {
            (Some(tags), profile_slug, Some(metadata)) => {
                let normalized_tags = Self::normalize_tag_names(&tags);
                Some((
                    Self::apply_shipping_profile_to_metadata(
                        Self::strip_metadata_tags(metadata),
                        profile_slug,
                    ),
                    Some(normalized_tags),
                ))
            }
            (Some(tags), profile_slug, None) => {
                let normalized_tags = Self::normalize_tag_names(&tags);
                Some((
                    Self::apply_shipping_profile_to_metadata(
                        Self::strip_metadata_tags(existing_metadata),
                        profile_slug,
                    ),
                    Some(normalized_tags),
                ))
            }
            (None, Some(profile_slug), Some(metadata)) => Some((
                Self::apply_shipping_profile_to_metadata(
                    Self::strip_metadata_tags(metadata),
                    Some(profile_slug),
                ),
                None,
            )),
            (None, Some(profile_slug), None) => Some((
                Self::apply_shipping_profile_to_metadata(
                    Self::strip_metadata_tags(existing_metadata),
                    Some(profile_slug),
                ),
                None,
            )),
            (None, None, Some(metadata)) => Some((Self::strip_metadata_tags(metadata), None)),
            (None, None, None) => None,
        }
    }

    pub async fn load_product_tag_map(
        &self,
        tenant_id: Uuid,
        products: &[entities::product::Model],
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> CommerceResult<HashMap<Uuid, Vec<String>>> {
        let product_ids = products
            .iter()
            .map(|product| product.id)
            .collect::<Vec<_>>();
        if product_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let relations = product_tag::Entity::find()
            .filter(product_tag::Column::ProductId.is_in(product_ids.clone()))
            .order_by_asc(product_tag::Column::ProductId)
            .order_by_asc(product_tag::Column::CreatedAt)
            .all(&self.db)
            .await?;

        let mut relations_by_product: HashMap<Uuid, Vec<product_tag::Model>> = HashMap::new();
        let mut ordered_term_ids = Vec::new();
        let mut seen_term_ids = HashSet::new();
        for relation in relations {
            if seen_term_ids.insert(relation.term_id) {
                ordered_term_ids.push(relation.term_id);
            }
            relations_by_product
                .entry(relation.product_id)
                .or_default()
                .push(relation);
        }

        let names = if ordered_term_ids.is_empty() {
            HashMap::new()
        } else {
            TaxonomyService::new(self.db.clone())
                .resolve_term_names(tenant_id, &ordered_term_ids, locale, fallback_locale)
                .await
                .map_err(|error| CommerceError::Validation(error.to_string()))?
        };

        let mut tags_by_product = HashMap::new();
        for product in products {
            if let Some(relations) = relations_by_product.get(&product.id) {
                let tags = relations
                    .iter()
                    .filter_map(|relation| names.get(&relation.term_id).cloned())
                    .collect::<Vec<_>>();
                tags_by_product.insert(product.id, tags);
                continue;
            }

            if Self::metadata_has_tags_field(&product.metadata) {
                tags_by_product.insert(
                    product.id,
                    Self::normalize_tag_names(&Self::extract_metadata_tags(&product.metadata)),
                );
            }
        }

        Ok(tags_by_product)
    }

    async fn load_product_tags(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        metadata: &Value,
    ) -> CommerceResult<ProductTagState> {
        let relations = product_tag::Entity::find()
            .filter(product_tag::Column::ProductId.eq(product_id))
            .order_by_asc(product_tag::Column::CreatedAt)
            .all(&self.db)
            .await?;

        if relations.is_empty() {
            if Self::metadata_has_tags_field(metadata) {
                return Ok(ProductTagState {
                    tags: Self::normalize_tag_names(&Self::extract_metadata_tags(metadata)),
                });
            }

            return Ok(ProductTagState { tags: Vec::new() });
        }

        let term_ids = relations
            .iter()
            .map(|relation| relation.term_id)
            .collect::<Vec<_>>();
        let names = TaxonomyService::new(self.db.clone())
            .resolve_term_names(tenant_id, &term_ids, locale, fallback_locale)
            .await
            .map_err(|error| CommerceError::Validation(error.to_string()))?;

        let mut tags = Vec::new();
        for relation in relations {
            if let Some(name) = names.get(&relation.term_id) {
                tags.push(name.clone());
            }
        }

        Ok(ProductTagState { tags })
    }

    async fn sync_product_tags_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: &str,
        tag_names: &[String],
    ) -> CommerceResult<()> {
        let normalized_tags = Self::normalize_tag_names(tag_names);

        product_tag::Entity::delete_many()
            .filter(product_tag::Column::ProductId.eq(product_id))
            .exec(txn)
            .await?;

        if normalized_tags.is_empty() {
            return Ok(());
        }

        let term_ids = TaxonomyService::new(self.db.clone())
            .ensure_terms_for_module_in_tx(
                txn,
                tenant_id,
                TaxonomyTermKind::Tag,
                PRODUCT_SCOPE_VALUE,
                locale,
                &normalized_tags,
            )
            .await
            .map_err(|error| CommerceError::Validation(error.to_string()))?;

        let now = Utc::now();
        for term_id in term_ids {
            product_tag::ActiveModel {
                product_id: Set(product_id),
                term_id: Set(term_id),
                tenant_id: Set(tenant_id),
                created_at: Set(now.into()),
            }
            .insert(txn)
            .await?;
        }

        Ok(())
    }

    async fn resolve_tag_locale_for_update(
        &self,
        txn: &DatabaseTransaction,
        product_id: Uuid,
        translations: Option<&[ProductTranslationInput]>,
    ) -> CommerceResult<String> {
        if let Some(locale) = translations.and_then(|items| {
            items.iter().find_map(|item| {
                let trimmed = item.locale.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            })
        }) {
            return Ok(locale);
        }

        Ok(entities::product_translation::Entity::find()
            .filter(entities::product_translation::Column::ProductId.eq(product_id))
            .order_by_asc(entities::product_translation::Column::Locale)
            .one(txn)
            .await?
            .map(|translation| translation.locale)
            .unwrap_or_else(|| "en".to_string()))
    }

    fn build_option_translations(
        option: &entities::product_option::Model,
        translations: Vec<entities::product_option_translation::Model>,
        option_values: Vec<entities::product_option_value::Model>,
        option_value_translations_by_value: &HashMap<
            Uuid,
            Vec<entities::product_option_value_translation::Model>,
        >,
    ) -> Vec<ProductOptionTranslationResponse> {
        let legacy_values: Vec<String> =
            serde_json::from_value(option.values.clone()).unwrap_or_default();

        translations
            .into_iter()
            .map(|translation| {
                let values = option_values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        option_value_translations_by_value
                            .get(&value.id)
                            .and_then(|items| {
                                items
                                    .iter()
                                    .find(|item| item.locale == translation.locale)
                                    .map(|item| item.value.clone())
                            })
                            .or_else(|| legacy_values.get(index).cloned())
                            .unwrap_or_default()
                    })
                    .collect();

                ProductOptionTranslationResponse {
                    locale: translation.locale,
                    name: translation.title,
                    values,
                }
            })
            .collect()
    }

    fn decimal_to_cents(amount: rust_decimal::Decimal) -> Option<i64> {
        use rust_decimal::prelude::ToPrimitive;

        (amount * rust_decimal::Decimal::from(100))
            .round_dp(0)
            .to_i64()
    }

    async fn ensure_default_stock_location<C>(
        conn: &C,
        tenant_id: Uuid,
    ) -> CommerceResult<entities::stock_location::Model>
    where
        C: ConnectionTrait,
    {
        if let Some(location) = entities::stock_location::Entity::find()
            .filter(entities::stock_location::Column::TenantId.eq(tenant_id))
            .filter(entities::stock_location::Column::DeletedAt.is_null())
            .one(conn)
            .await?
        {
            return Ok(location);
        }

        let now = Utc::now();
        entities::stock_location::ActiveModel {
            id: Set(generate_id()),
            tenant_id: Set(tenant_id),
            name: Set("Default".to_string()),
            code: Set(Some("default".to_string())),
            address_line1: Set(None),
            address_line2: Set(None),
            city: Set(None),
            province: Set(None),
            postal_code: Set(None),
            country_code: Set(None),
            phone: Set(None),
            metadata: Set(serde_json::json!({ "source": "catalog_service" })),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            deleted_at: Set(None),
        }
        .insert(conn)
        .await
        .map_err(CommerceError::from)
    }

    async fn create_initial_inventory_records<C>(
        conn: &C,
        default_stock_location: &entities::stock_location::Model,
        variant_id: Uuid,
        sku: Option<String>,
        quantity: i32,
    ) -> CommerceResult<()>
    where
        C: ConnectionTrait,
    {
        let now = Utc::now();
        let inventory_item = entities::inventory_item::ActiveModel {
            id: Set(generate_id()),
            variant_id: Set(variant_id),
            sku: Set(sku),
            requires_shipping: Set(true),
            metadata: Set(serde_json::json!({ "source": "catalog_service" })),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(conn)
        .await?;

        entities::inventory_level::ActiveModel {
            id: Set(generate_id()),
            inventory_item_id: Set(inventory_item.id),
            location_id: Set(default_stock_location.id),
            stocked_quantity: Set(quantity),
            reserved_quantity: Set(0),
            incoming_quantity: Set(0),
            low_stock_threshold: Set(None),
            updated_at: Set(now.into()),
        }
        .insert(conn)
        .await?;

        Ok(())
    }

    async fn load_available_quantities<C>(
        conn: &C,
        variant_ids: &[Uuid],
    ) -> CommerceResult<HashMap<Uuid, i32>>
    where
        C: ConnectionTrait,
    {
        if variant_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let inventory_items = entities::inventory_item::Entity::find()
            .filter(entities::inventory_item::Column::VariantId.is_in(variant_ids.iter().copied()))
            .all(conn)
            .await?;

        if inventory_items.is_empty() {
            return Ok(HashMap::new());
        }

        let item_to_variant: HashMap<Uuid, Uuid> = inventory_items
            .iter()
            .map(|item| (item.id, item.variant_id))
            .collect();
        let levels = entities::inventory_level::Entity::find()
            .filter(
                entities::inventory_level::Column::InventoryItemId
                    .is_in(item_to_variant.keys().copied()),
            )
            .all(conn)
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
}

async fn load_product_custom_fields_schema(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> CommerceResult<CustomFieldsSchema> {
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

    let rows = product_field_definitions_storage::Entity::find()
        .filter(product_field_definitions_storage::Column::TenantId.eq(tenant_id))
        .filter(product_field_definitions_storage::Column::IsActive.eq(true))
        .order_by_asc(product_field_definitions_storage::Column::Position)
        .all(db)
        .await
        .map_err(CommerceError::from)?;

    let definitions = rows
        .into_iter()
        .filter_map(product_field_definition_from_row)
        .collect();

    Ok(CustomFieldsSchema::new(definitions))
}

fn product_field_definition_from_row(
    row: product_field_definitions_storage::Model,
) -> Option<FieldDefinition> {
    let field_type: FieldType =
        serde_json::from_value(serde_json::Value::String(row.field_type.clone())).ok()?;
    let label = serde_json::from_value(row.label).unwrap_or_default();
    let description = row
        .description
        .and_then(|value| serde_json::from_value(value).ok());
    let validation: Option<ValidationRule> = row
        .validation
        .and_then(|value| serde_json::from_value(value).ok());

    Some(FieldDefinition {
        field_key: row.field_key,
        field_type,
        label,
        description,
        is_localized: row.is_localized,
        is_required: row.is_required,
        default_value: row.default_value,
        validation,
        position: row.position,
        is_active: row.is_active,
    })
}

fn split_product_metadata_payload(
    schema: &CustomFieldsSchema,
    metadata: &Value,
) -> (
    serde_json::Map<String, Value>,
    serde_json::Map<String, Value>,
) {
    let known_keys = schema
        .active_definitions()
        .into_iter()
        .map(|definition| definition.field_key.as_str())
        .collect::<HashSet<_>>();
    let mut reserved = serde_json::Map::new();
    let mut custom_fields = serde_json::Map::new();

    for (key, value) in metadata.as_object().cloned().unwrap_or_default() {
        if known_keys.contains(key.as_str()) {
            custom_fields.insert(key, value);
        } else {
            reserved.insert(key, value);
        }
    }

    (reserved, custom_fields)
}

fn merge_reserved_product_metadata(
    mut reserved: serde_json::Map<String, Value>,
    custom_fields: Option<Value>,
) -> Value {
    if let Some(custom_fields) = custom_fields.and_then(|value| value.as_object().cloned()) {
        for (key, value) in custom_fields {
            reserved.insert(key, value);
        }
    }

    Value::Object(reserved)
}

fn pick_product_translation<'a>(
    translations: &'a [entities::product_translation::Model],
    locale: &str,
    fallback_locale: &str,
) -> Option<&'a entities::product_translation::Model> {
    translations
        .iter()
        .find(|translation| translation.locale == locale)
        .or_else(|| {
            (fallback_locale != locale).then(|| {
                translations
                    .iter()
                    .find(|translation| translation.locale == fallback_locale)
            })?
        })
        .or_else(|| translations.first())
}

fn pick_response_translation<'a>(
    translations: &'a [ProductTranslationResponse],
    locale: &str,
    fallback_locale: &str,
) -> Option<&'a ProductTranslationResponse> {
    translations
        .iter()
        .find(|translation| translation.locale == locale)
        .or_else(|| {
            (fallback_locale != locale).then(|| {
                translations
                    .iter()
                    .find(|translation| translation.locale == fallback_locale)
            })?
        })
        .or_else(|| translations.first())
}

fn localize_product_response(
    mut product: ProductResponse,
    locale: &str,
    fallback_locale: &str,
) -> ProductResponse {
    let selected_translation =
        pick_response_translation(product.translations.as_slice(), locale, fallback_locale)
            .cloned()
            .into_iter()
            .collect::<Vec<_>>();

    if !selected_translation.is_empty() {
        product.translations = selected_translation;
    }

    product
}

fn normalize_public_channel_slug(channel_slug: Option<&str>) -> Option<String> {
    channel_slug
        .map(str::trim)
        .filter(|slug| !slug.is_empty())
        .map(|slug| slug.to_ascii_lowercase())
}

fn extract_allowed_channel_slugs(metadata: &Value) -> Vec<String> {
    let Some(values) = metadata
        .as_object()
        .and_then(|object| object.get("channel_visibility"))
        .and_then(|value| value.as_object())
        .and_then(|object| object.get("allowed_channel_slugs"))
        .and_then(|value| value.as_array())
    else {
        return Vec::new();
    };

    let mut normalized = BTreeSet::new();
    for value in values {
        if let Some(slug) = value
            .as_str()
            .and_then(|value| normalize_public_channel_slug(Some(value)))
        {
            normalized.insert(slug);
        }
    }

    normalized.into_iter().collect()
}

fn is_allowlist_visible_for_public_channel(
    allowed_channel_slugs: &[String],
    public_channel_slug: Option<&str>,
) -> bool {
    if allowed_channel_slugs.is_empty() {
        return true;
    }

    let Some(public_channel_slug) = normalize_public_channel_slug(public_channel_slug) else {
        return false;
    };

    allowed_channel_slugs
        .iter()
        .any(|slug| slug == &public_channel_slug)
}

fn is_metadata_visible_for_public_channel(
    metadata: &Value,
    public_channel_slug: Option<&str>,
) -> bool {
    let allowed_channel_slugs = extract_allowed_channel_slugs(metadata);
    is_allowlist_visible_for_public_channel(&allowed_channel_slugs, public_channel_slug)
}

async fn load_available_inventory_by_variant_for_public_channel(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    variant_ids: &[Uuid],
    public_channel_slug: Option<&str>,
) -> Result<HashMap<Uuid, i32>, sea_orm::DbErr> {
    if variant_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let inventory_items = entities::inventory_item::Entity::find()
        .filter(entities::inventory_item::Column::VariantId.is_in(variant_ids.iter().copied()))
        .all(db)
        .await?;
    if inventory_items.is_empty() {
        return Ok(HashMap::new());
    }

    let item_to_variant: HashMap<Uuid, Uuid> = inventory_items
        .iter()
        .map(|item| (item.id, item.variant_id))
        .collect();
    let levels = entities::inventory_level::Entity::find()
        .filter(
            entities::inventory_level::Column::InventoryItemId
                .is_in(item_to_variant.keys().copied()),
        )
        .all(db)
        .await?;
    if levels.is_empty() {
        return Ok(HashMap::new());
    }

    let locations = entities::stock_location::Entity::find()
        .filter(entities::stock_location::Column::TenantId.eq(tenant_id))
        .filter(entities::stock_location::Column::DeletedAt.is_null())
        .filter(
            entities::stock_location::Column::Id
                .is_in(levels.iter().map(|level| level.location_id)),
        )
        .all(db)
        .await?;
    let visible_location_ids = locations
        .into_iter()
        .filter(|location| {
            is_metadata_visible_for_public_channel(&location.metadata, public_channel_slug)
        })
        .map(|location| location.id)
        .collect::<HashSet<_>>();

    let mut available_by_variant = HashMap::new();
    for level in levels {
        if !visible_location_ids.contains(&level.location_id) {
            continue;
        }

        if let Some(variant_id) = item_to_variant.get(&level.inventory_item_id) {
            *available_by_variant.entry(*variant_id).or_insert(0) +=
                level.stocked_quantity - level.reserved_quantity;
        }
    }

    Ok(available_by_variant)
}

async fn apply_public_channel_inventory_to_product(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    product: &mut ProductResponse,
    public_channel_slug: Option<&str>,
) -> Result<(), sea_orm::DbErr> {
    let variant_ids = product
        .variants
        .iter()
        .map(|variant| variant.id)
        .collect::<Vec<_>>();
    let available_by_variant = load_available_inventory_by_variant_for_public_channel(
        db,
        tenant_id,
        &variant_ids,
        public_channel_slug,
    )
    .await?;

    for variant in &mut product.variants {
        let available_inventory = available_by_variant.get(&variant.id).copied().unwrap_or(0);
        variant.inventory_quantity = available_inventory;
        variant.in_stock = available_inventory > 0 || variant.inventory_policy == "continue";
    }

    Ok(())
}

async fn find_published_product_id_by_handle(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    handle: &str,
    locale: &str,
    fallback_locale: &str,
    public_channel_slug: Option<&str>,
) -> CommerceResult<Option<Uuid>> {
    if let Some(product_id) =
        find_published_product_id_for_locale(db, tenant_id, handle, locale, public_channel_slug)
            .await?
    {
        return Ok(Some(product_id));
    }

    if fallback_locale != locale {
        if let Some(product_id) = find_published_product_id_for_locale(
            db,
            tenant_id,
            handle,
            fallback_locale,
            public_channel_slug,
        )
        .await?
        {
            return Ok(Some(product_id));
        }
    }

    find_published_product_id_any_locale(db, tenant_id, handle, public_channel_slug).await
}

async fn find_published_product_id_for_locale(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    handle: &str,
    locale: &str,
    public_channel_slug: Option<&str>,
) -> CommerceResult<Option<Uuid>> {
    let translations = entities::product_translation::Entity::find()
        .filter(entities::product_translation::Column::Handle.eq(handle))
        .filter(entities::product_translation::Column::Locale.eq(locale))
        .all(db)
        .await?;

    find_first_published_product(db, tenant_id, translations, public_channel_slug).await
}

async fn find_published_product_id_any_locale(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    handle: &str,
    public_channel_slug: Option<&str>,
) -> CommerceResult<Option<Uuid>> {
    let translations = entities::product_translation::Entity::find()
        .filter(entities::product_translation::Column::Handle.eq(handle))
        .all(db)
        .await?;

    find_first_published_product(db, tenant_id, translations, public_channel_slug).await
}

async fn find_first_published_product(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    translations: Vec<entities::product_translation::Model>,
    public_channel_slug: Option<&str>,
) -> CommerceResult<Option<Uuid>> {
    for translation in translations {
        let product = entities::product::Entity::find_by_id(translation.product_id)
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .filter(entities::product::Column::Status.eq(entities::product::ProductStatus::Active))
            .filter(entities::product::Column::PublishedAt.is_not_null())
            .one(db)
            .await?;

        if product.as_ref().is_some_and(|product| {
            is_metadata_visible_for_public_channel(&product.metadata, public_channel_slug)
        }) {
            return Ok(Some(translation.product_id));
        }
    }

    Ok(None)
}
