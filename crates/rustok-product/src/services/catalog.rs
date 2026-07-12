pub mod helpers;
mod tags;
pub mod types;

pub use types::{ProductTagState, StorefrontProductList, StorefrontProductListItem};

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, Statement,
};
use std::collections::{HashMap, HashSet};
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;
use validator::Validate;

use rustok_core::generate_id;

use rustok_api::PLATFORM_FALLBACK_LOCALE;
use rustok_commerce_foundation::dto::*;
use rustok_commerce_foundation::entities;
use rustok_commerce_foundation::error::{CommerceError, CommerceResult};
use rustok_events::DomainEvent;
use rustok_inventory::{BootstrapService, InitialInventory};
use rustok_outbox::TransactionalEventBus;

use crate::ProductCatalogSchemaService;

use super::write_transaction::ProductWriteTransaction;
use helpers::*;

const PRODUCT_SCOPE_VALUE: &str = "product";

fn map_product_unique_violation(
    error: sea_orm::DbErr,
    handle: &str,
    locale: &str,
    sku: Option<&str>,
) -> CommerceError {
    let message = error.to_string();
    if message.contains("uq_product_variants_tenant_sku") {
        return CommerceError::DuplicateSku(sku.unwrap_or_default().to_owned());
    }
    if message.contains("uq_product_translations_tenant_locale_handle") {
        return CommerceError::DuplicateHandle {
            handle: handle.to_owned(),
            locale: locale.to_owned(),
        };
    }
    CommerceError::Database(error)
}

pub struct CatalogService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl CatalogService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    pub(crate) fn database(&self) -> &DatabaseConnection {
        &self.db
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
        self.validate_primary_category(tenant_id, input.primary_category_id)
            .await?;
        if input.publish {
            ProductCatalogSchemaService::new(self.db.clone(), self.event_bus.clone())
                .validate_new_product_publish_requirements(tenant_id, input.primary_category_id)
                .await?;
        }

        let product_id = generate_id();
        let now = Utc::now();
        debug!(product_id = %product_id, "Generated product ID");

        let preferred_locale = preferred_product_locale_from_translations(&input.translations);
        let prepared_custom_fields = prepare_product_custom_fields_for_create(
            &self.db,
            tenant_id,
            preferred_locale.as_str(),
            input.metadata.clone(),
        )
        .await?;
        let product_metadata = prepared_custom_fields
            .metadata
            .clone()
            .unwrap_or_else(|| serde_json::json!({}));
        let (normalized_metadata, normalized_tags) = normalize_create_product_metadata(
            input.tags.clone(),
            input.shipping_profile_slug.clone(),
            product_metadata,
        );

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;

        let product = entities::product::ActiveModel {
            id: Set(product_id),
            tenant_id: Set(tenant_id),
            status: Set(if input.publish {
                entities::product::ProductStatus::Active
            } else {
                entities::product::ProductStatus::Draft
            }),
            seller_id: Set(normalize_seller_id(input.seller_id.as_deref())),
            vendor: Set(input.vendor.clone()),
            product_type: Set(input.product_type.clone()),
            shipping_profile_slug: Set(input
                .shipping_profile_slug
                .as_deref()
                .and_then(normalize_shipping_profile_slug)),
            primary_category_id: Set(input.primary_category_id),
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
            flex::persist_localized_values(&txn, tenant_id, "product", product_id, locale, values)
                .await
                .map_err(|error| CommerceError::Validation(error.to_string()))?;
        }

        let translation_locales = collect_translation_locales(&input.translations);

        let mut seen = HashSet::new();
        for trans_input in &input.translations {
            let handle = trans_input
                .handle
                .clone()
                .unwrap_or_else(|| slugify(&trans_input.title));

            let key = format!("{}::{}", trans_input.locale, handle.clone());
            if !seen.insert(key) {
                warn!(handle = %handle, locale = %trans_input.locale, "Duplicate handle detected");
                return Err(CommerceError::DuplicateHandle {
                    handle,
                    locale: trans_input.locale.clone(),
                });
            }

            let translation = entities::product_translation::ActiveModel {
                id: Set(generate_id()),
                product_id: Set(product_id),
                tenant_id: Set(tenant_id),
                locale: Set(trans_input.locale.clone()),
                title: Set(trans_input.title.clone()),
                handle: Set(handle.clone()),
                description: Set(trans_input.description.clone()),
                meta_title: Set(trans_input.meta_title.clone()),
                meta_description: Set(trans_input.meta_description.clone()),
            };
            translation.insert(&txn).await.map_err(|error| {
                map_product_unique_violation(error, &handle, &trans_input.locale, None)
            })?;
        }
        debug!(
            translations_count = input.translations.len(),
            "Product translations inserted"
        );

        let mut option_models = Vec::with_capacity(input.options.len());
        let mut option_translation_models = Vec::new();
        let mut option_value_models = Vec::new();
        let mut option_value_translation_models = Vec::new();
        for (position, opt_input) in input.options.iter().enumerate() {
            let option_id = generate_id();
            let option_translations = normalize_option_translations(&opt_input.translations)?;
            let option_translations = expand_option_translations_for_product_locales(
                option_translations,
                &translation_locales,
            );
            let base_values = option_translations
                .first()
                .map(|item| item.values.clone())
                .unwrap_or_default();
            ensure_option_values_consistent(&option_translations, &base_values)?;
            option_models.push(entities::product_option::ActiveModel {
                id: Set(option_id),
                product_id: Set(product_id),
                position: Set(position as i32),
            });

            for translation in &option_translations {
                option_translation_models.push(entities::product_option_translation::ActiveModel {
                    id: Set(generate_id()),
                    option_id: Set(option_id),
                    locale: Set(translation.locale.clone()),
                    title: Set(translation.name.clone()),
                });
            }

            let mut option_value_ids = Vec::with_capacity(base_values.len());
            for (value_position, _) in base_values.iter().enumerate() {
                let option_value_id = generate_id();
                option_value_models.push(entities::product_option_value::ActiveModel {
                    id: Set(option_value_id),
                    option_id: Set(option_id),
                    position: Set(value_position as i32),
                    metadata: Set(serde_json::json!({})),
                });
                option_value_ids.push(option_value_id);
            }

            for translation in &option_translations {
                for (value_position, value_id) in option_value_ids.iter().enumerate() {
                    let value = translation
                        .values
                        .get(value_position)
                        .cloned()
                        .unwrap_or_default();
                    option_value_translation_models.push(
                        entities::product_option_value_translation::ActiveModel {
                            id: Set(generate_id()),
                            value_id: Set(*value_id),
                            locale: Set(translation.locale.clone()),
                            value: Set(value),
                        },
                    );
                }
            }
        }
        if !option_models.is_empty() {
            entities::product_option::Entity::insert_many(option_models)
                .exec(&txn)
                .await?;
        }
        if !option_translation_models.is_empty() {
            entities::product_option_translation::Entity::insert_many(option_translation_models)
                .exec(&txn)
                .await?;
        }
        if !option_value_models.is_empty() {
            entities::product_option_value::Entity::insert_many(option_value_models)
                .exec(&txn)
                .await?;
        }
        if !option_value_translation_models.is_empty() {
            entities::product_option_value_translation::Entity::insert_many(
                option_value_translation_models,
            )
            .exec(&txn)
            .await?;
        }
        debug!(
            options_count = input.options.len(),
            "Product options inserted"
        );

        let default_stock_location =
            BootstrapService::ensure_default_location_in_tx(&txn, tenant_id).await?;

        let mut variant_translation_models = Vec::new();
        let mut price_models = Vec::new();
        for (position, var_input) in input.variants.iter().enumerate() {
            let variant_id = generate_id();

            let variant = entities::product_variant::ActiveModel {
                id: Set(variant_id),
                product_id: Set(product_id),
                tenant_id: Set(tenant_id),
                sku: Set(var_input.sku.clone()),
                barcode: Set(var_input.barcode.clone()),
                shipping_profile_slug: Set(var_input
                    .shipping_profile_slug
                    .as_deref()
                    .and_then(normalize_shipping_profile_slug)),
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
            variant.insert(&txn).await.map_err(|error| {
                map_product_unique_violation(error, "", "", var_input.sku.as_deref())
            })?;

            BootstrapService::create_initial_records_in_tx(
                &txn,
                &default_stock_location,
                InitialInventory {
                    variant_id,
                    sku: var_input.sku.clone(),
                    available_quantity: var_input.inventory_quantity,
                },
            )
            .await?;

            let variant_title = generate_variant_title_from_inputs(
                var_input.option1.as_deref(),
                var_input.option2.as_deref(),
                var_input.option3.as_deref(),
            );
            for locale in &translation_locales {
                variant_translation_models.push(entities::variant_translation::ActiveModel {
                    id: Set(generate_id()),
                    variant_id: Set(variant_id),
                    locale: Set(locale.clone()),
                    title: Set(Some(variant_title.clone())),
                });
            }

            for price_input in &var_input.prices {
                price_models.push(entities::price::ActiveModel {
                    id: Set(generate_id()),
                    variant_id: Set(variant_id),
                    price_list_id: Set(None),
                    channel_id: Set(price_input.channel_id),
                    channel_slug: Set(normalize_public_channel_slug(
                        price_input.channel_slug.as_deref(),
                    )),
                    currency_code: Set(price_input.currency_code.clone()),
                    region_id: Set(None),
                    amount: Set(price_input.amount),
                    compare_at_amount: Set(price_input.compare_at_amount),
                    legacy_amount: Set(decimal_to_cents(price_input.amount)),
                    legacy_compare_at_amount: Set(price_input
                        .compare_at_amount
                        .and_then(decimal_to_cents)),
                    min_quantity: Set(None),
                    max_quantity: Set(None),
                });
            }
        }
        if !variant_translation_models.is_empty() {
            entities::variant_translation::Entity::insert_many(variant_translation_models)
                .exec(&txn)
                .await?;
        }
        if !price_models.is_empty() {
            entities::price::Entity::insert_many(price_models)
                .exec(&txn)
                .await?;
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

        txn.publish(
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

        let tag_locale = locale;
        let (translations, options, variants, images, product_tags, resolved_metadata) = tokio::try_join!(
            async {
                Ok::<_, CommerceError>(
                    entities::product_translation::Entity::find()
                        .filter(entities::product_translation::Column::ProductId.eq(product_id))
                        .all(&self.db)
                        .await?,
                )
            },
            async {
                Ok::<_, CommerceError>(
                    entities::product_option::Entity::find()
                        .filter(entities::product_option::Column::ProductId.eq(product_id))
                        .order_by_asc(entities::product_option::Column::Position)
                        .all(&self.db)
                        .await?,
                )
            },
            async {
                Ok::<_, CommerceError>(
                    entities::product_variant::Entity::find()
                        .filter(entities::product_variant::Column::ProductId.eq(product_id))
                        .order_by_asc(entities::product_variant::Column::Position)
                        .all(&self.db)
                        .await?,
                )
            },
            async {
                Ok::<_, CommerceError>(
                    entities::product_image::Entity::find()
                        .filter(entities::product_image::Column::ProductId.eq(product_id))
                        .order_by_asc(entities::product_image::Column::Position)
                        .all(&self.db)
                        .await?,
                )
            },
            self.load_product_tags(
                tenant_id,
                product_id,
                tag_locale,
                fallback_locale.or(Some(PLATFORM_FALLBACK_LOCALE)),
                &product.metadata,
            ),
            resolve_product_metadata(
                &self.db,
                tenant_id,
                product_id,
                &product.metadata,
                locale,
                fallback_locale.unwrap_or(PLATFORM_FALLBACK_LOCALE),
            ),
        )?;

        let option_ids: Vec<Uuid> = options.iter().map(|option| option.id).collect();
        let (option_translations, option_values) = tokio::try_join!(
            async {
                if option_ids.is_empty() {
                    Ok::<_, CommerceError>(Vec::new())
                } else {
                    Ok::<_, CommerceError>(
                        entities::product_option_translation::Entity::find()
                            .filter(
                                entities::product_option_translation::Column::OptionId
                                    .is_in(option_ids.clone()),
                            )
                            .order_by_asc(entities::product_option_translation::Column::Locale)
                            .all(&self.db)
                            .await?,
                    )
                }
            },
            async {
                if option_ids.is_empty() {
                    Ok::<_, CommerceError>(Vec::new())
                } else {
                    Ok::<_, CommerceError>(
                        entities::product_option_value::Entity::find()
                            .filter(
                                entities::product_option_value::Column::OptionId
                                    .is_in(option_ids.clone()),
                            )
                            .order_by_asc(entities::product_option_value::Column::Position)
                            .all(&self.db)
                            .await?,
                    )
                }
            },
        )?;
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

        let variant_ids: Vec<Uuid> = variants.iter().map(|v| v.id).collect();
        let (all_prices, variant_translations, available_inventory_by_variant) = tokio::try_join!(
            async {
                if variant_ids.is_empty() {
                    Ok::<_, CommerceError>(Vec::new())
                } else {
                    Ok::<_, CommerceError>(
                        entities::price::Entity::find()
                            .filter(entities::price::Column::VariantId.is_in(variant_ids.clone()))
                            .all(&self.db)
                            .await?,
                    )
                }
            },
            async {
                if variant_ids.is_empty() {
                    Ok::<_, CommerceError>(Vec::new())
                } else {
                    Ok::<_, CommerceError>(
                        entities::variant_translation::Entity::find()
                            .filter(
                                entities::variant_translation::Column::VariantId
                                    .is_in(variant_ids.clone()),
                            )
                            .order_by_asc(entities::variant_translation::Column::Locale)
                            .all(&self.db)
                            .await?,
                    )
                }
            },
            BootstrapService::load_available_quantities(&self.db, &variant_ids),
        )?;

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

                let title = generate_variant_title(&variant);
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

        let response = ProductResponse {
            id: product.id,
            tenant_id: product.tenant_id,
            status: product.status,
            seller_id: product.seller_id,
            vendor: product.vendor,
            product_type: product.product_type,
            shipping_profile_slug: product
                .shipping_profile_slug
                .clone()
                .or_else(|| extract_shipping_profile_slug(&product.metadata)),
            primary_category_id: product.primary_category_id,
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
                    let translations = build_option_translations(
                        option_translations_by_option
                            .remove(&option_id)
                            .unwrap_or_default(),
                        option_values_by_option
                            .remove(&option_id)
                            .unwrap_or_default(),
                        &option_value_translations_by_value,
                    );

                    let (name, values) =
                        resolve_option_display(&translations, locale, fallback_locale);

                    ProductOptionResponse {
                        id: option_id,
                        name,
                        values,
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
        if page == 0 || per_page == 0 || per_page > 48 {
            return Err(CommerceError::Validation(
                "page must be at least 1 and per_page must be between 1 and 48".to_owned(),
            ));
        }
        let offset = (page.saturating_sub(1)) * per_page;

        let query = entities::product::Entity::find()
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .filter(entities::product::Column::Status.eq(entities::product::ProductStatus::Active))
            .filter(entities::product::Column::PublishedAt.is_not_null())
            .filter(product_channel_visibility_condition(
                self.db.get_database_backend(),
                public_channel_slug,
            ));
        let total = query.clone().count(&self.db).await?;
        let products = query
            .order_by_desc(entities::product::Column::PublishedAt)
            .order_by_desc(entities::product::Column::CreatedAt)
            .offset(offset)
            .limit(per_page)
            .all(&self.db)
            .await?;
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
                    seller_id: product.seller_id,
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
        if input.primary_category_id.is_some() {
            self.validate_primary_category(tenant_id, input.primary_category_id)
                .await?;
        }

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;

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
            .map(preferred_product_locale_from_translations)
            .unwrap_or_else(|| preferred_product_locale_from_metadata(&existing_product.metadata));
        let prepared_custom_fields = if let Some(metadata) = input.metadata.clone() {
            Some(
                prepare_product_custom_fields_for_update(
                    &txn,
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
        let metadata_update = normalize_update_product_metadata(
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
        if input.seller_id.is_some() {
            product_active.seller_id = Set(normalize_seller_id(input.seller_id.as_deref()));
        }
        if let Some(product_type) = input.product_type {
            product_active.product_type = Set(Some(product_type));
        }
        if shipping_profile_input.is_some() {
            product_active.shipping_profile_slug = Set(shipping_profile_input
                .as_deref()
                .and_then(normalize_shipping_profile_slug));
        }
        let primary_category_changed = input.primary_category_id.is_some()
            && input.primary_category_id != existing_product.primary_category_id;
        if input.primary_category_id.is_some() {
            product_active.primary_category_id = Set(input.primary_category_id);
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
                flex::persist_localized_values(
                    &txn, tenant_id, "product", product_id, locale, values,
                )
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
                    .unwrap_or_else(|| slugify(&translation_input.title));

                let locale = translation_input.locale.clone();
                let key = format!("{}::{}", locale, handle.clone());
                if !seen.insert(key) {
                    return Err(CommerceError::DuplicateHandle { handle, locale });
                }

                let translation = entities::product_translation::ActiveModel {
                    id: Set(generate_id()),
                    product_id: Set(product_id),
                    tenant_id: Set(tenant_id),
                    locale: Set(translation_input.locale),
                    title: Set(translation_input.title),
                    handle: Set(handle.clone()),
                    description: Set(translation_input.description),
                    meta_title: Set(translation_input.meta_title),
                    meta_description: Set(translation_input.meta_description),
                };
                translation
                    .insert(&txn)
                    .await
                    .map_err(|error| map_product_unique_violation(error, &handle, &locale, None))?;
            }
        }

        if let Some((_, Some(tags))) = metadata_update.as_ref() {
            let locale =
                resolve_tag_locale_for_update(&txn, product_id, input.translations.as_deref())
                    .await?;
            self.sync_product_tags_in_tx(&txn, tenant_id, product_id, &locale, tags)
                .await?;
        }

        txn.publish(
            tenant_id,
            Some(actor_id),
            DomainEvent::ProductUpdated { product_id },
        )
        .await?;
        if primary_category_changed {
            txn.publish(
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductPrimaryCategoryChanged {
                    product_id,
                    old_category_id: existing_product.primary_category_id,
                    new_category_id: input.primary_category_id,
                },
            )
            .await?;
        }

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

    async fn validate_primary_category(
        &self,
        tenant_id: Uuid,
        category_id: Option<Uuid>,
    ) -> CommerceResult<()> {
        let Some(category_id) = category_id else {
            return Ok(());
        };
        let row = self
            .db
            .query_one(Statement::from_sql_and_values(
                self.db.get_database_backend(),
                "SELECT kind FROM catalog_categories WHERE tenant_id = $1 AND id = $2",
                [tenant_id.into(), category_id.into()],
            ))
            .await?;
        let kind = row
            .and_then(|row| row.try_get::<String>("", "kind").ok())
            .ok_or_else(|| {
                CommerceError::Validation(
                    "Primary category must reference an existing tenant category".to_string(),
                )
            })?;
        if kind != "structural" {
            return Err(CommerceError::Validation(
                "Primary category must be structural".to_string(),
            ));
        }
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn publish_product(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        product_id: Uuid,
    ) -> CommerceResult<ProductResponse> {
        debug!(product_id = %product_id, "Publishing product");

        ProductCatalogSchemaService::new(self.db.clone(), self.event_bus.clone())
            .validate_product_publish_requirements(tenant_id, product_id)
            .await?;

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;

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

        txn.publish(
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

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;

        let product = entities::product::Entity::find_by_id(product_id)
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(CommerceError::ProductNotFound(product_id))?;

        let mut product_active: entities::product::ActiveModel = product.into();
        product_active.status = Set(entities::product::ProductStatus::Draft);
        product_active.updated_at = Set(Utc::now().into());
        product_active.update(&txn).await?;

        txn.publish(
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

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;

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
            BootstrapService::delete_records_for_variants_in_tx(&txn, &variant_ids).await?;

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

        flex::delete_attached_localized_values(&txn, tenant_id, "product", product_id)
            .await
            .map_err(map_flex_cleanup_error)?;

        txn.publish(
            tenant_id,
            Some(actor_id),
            DomainEvent::ProductDeleted { product_id },
        )
        .await?;

        txn.commit().await?;
        info!(product_id = %product_id, "Product deleted successfully");

        Ok(())
    }
}
