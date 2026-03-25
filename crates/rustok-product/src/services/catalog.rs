use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait,
};
use std::collections::{HashMap, HashSet};
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;
use validator::Validate;

use rustok_core::generate_id;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use rustok_commerce_foundation::dto::*;
use rustok_commerce_foundation::entities;
use rustok_commerce_foundation::error::{CommerceError, CommerceResult};

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
            metadata: Set(input.metadata.clone()),
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
                ean: Set(None),
                upc: Set(None),
                inventory_policy: Set(var_input.inventory_policy.clone()),
                inventory_management: Set("manual".into()),
                inventory_quantity: Set(var_input.inventory_quantity),
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
                    legacy_compare_at_amount: Set(
                        price_input.compare_at_amount.and_then(Self::decimal_to_cents),
                    ),
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

        self.get_product(tenant_id, product_id).await
    }

    #[instrument(skip(self))]
    pub async fn get_product(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
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

                VariantResponse {
                    id: variant.id,
                    product_id: variant.product_id,
                    sku: variant.sku,
                    barcode: variant.barcode,
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
                    inventory_quantity: variant.inventory_quantity,
                    inventory_policy: variant.inventory_policy.clone(),
                    in_stock: variant.inventory_quantity > 0
                        || variant.inventory_policy == "continue",
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

        let response = ProductResponse {
            id: product.id,
            tenant_id: product.tenant_id,
            status: product.status,
            vendor: product.vendor,
            product_type: product.product_type,
            metadata: product.metadata,
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

        let mut product_active: entities::product::ActiveModel = product.into();
        product_active.updated_at = Set(Utc::now().into());

        if let Some(vendor) = input.vendor {
            product_active.vendor = Set(Some(vendor));
        }
        if let Some(product_type) = input.product_type {
            product_active.product_type = Set(Some(product_type));
        }
        if let Some(metadata) = input.metadata {
            product_active.metadata = Set(metadata);
        }
        if let Some(status) = input.status {
            product_active.status = Set(status);
        }

        product_active.update(&txn).await?;

        if let Some(translations) = input.translations {
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

        self.get_product(tenant_id, product_id).await
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

    fn collect_translation_locales(translations: &[ProductTranslationInput]) -> Vec<String> {
        let mut locales = Vec::new();
        for translation in translations {
            if !locales.iter().any(|locale| locale == &translation.locale) {
                locales.push(translation.locale.clone());
            }
        }
        locales
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
}
