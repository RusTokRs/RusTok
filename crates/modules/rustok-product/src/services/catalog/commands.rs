use super::*;
use rustok_api::TenantLocale;
use rustok_core::error::Error as CoreError;
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseTransaction, FromQueryResult};

async fn find_product_for_update_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
) -> CommerceResult<entities::product::Model> {
    let query = entities::product::Entity::find_by_id(product_id)
        .filter(entities::product::Column::TenantId.eq(tenant_id));
    let product = match txn.get_database_backend() {
        DatabaseBackend::Postgres | DatabaseBackend::MySql => {
            query.lock_exclusive().one(txn).await?
        }
        _ => query.one(txn).await?,
    };
    product.ok_or(CommerceError::ProductNotFound(product_id))
}

async fn find_variant_for_update_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    variant_id: Uuid,
) -> CommerceResult<entities::product_variant::Model> {
    let query = entities::product_variant::Entity::find_by_id(variant_id)
        .filter(entities::product_variant::Column::TenantId.eq(tenant_id));
    let variant = match txn.get_database_backend() {
        DatabaseBackend::Postgres | DatabaseBackend::MySql => {
            query.lock_exclusive().one(txn).await?
        }
        _ => query.one(txn).await?,
    };
    variant.ok_or(CommerceError::VariantNotFound(variant_id))
}

fn validate_variant_prices(prices: &[PriceInput]) -> CommerceResult<()> {
    let mut seen = HashSet::new();
    for price in prices {
        let currency = price.currency_code.trim().to_ascii_uppercase();
        if currency.len() != 3 {
            return Err(CommerceError::Validation(format!(
                "Currency code `{}` must be exactly 3 characters (ISO 4217)",
                price.currency_code
            )));
        }
        let channel_slug = normalize_public_channel_slug(price.channel_slug.as_deref());
        let key = (price.channel_id, channel_slug, currency);
        if !seen.insert(key) {
            return Err(CommerceError::Validation(format!(
                "Duplicate price for currency `{}` and channel",
                price.currency_code
            )));
        }
    }
    Ok(())
}

impl CatalogService {
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
            axes_count = input.variant_axes.len(),
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
        validate_variant_axes_and_combinations(&input.variant_axes, &input.variants)?;
        for var_input in &input.variants {
            validate_variant_prices(&var_input.prices)?;
        }
        self.validate_primary_category(tenant_id, input.primary_category_id)
            .await?;
        validate_variant_axis_configuration_in(
            &self.db,
            tenant_id,
            input.primary_category_id,
            input.variant_axes.as_slice(),
        )
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
            flex::persist_localized_values(
                &txn,
                tenant_id,
                flex::PRODUCT_ENTITY_TYPE,
                product_id,
                locale,
                values,
            )
            .await
            .map_err(CommerceError::from)?;
        }

        let translation_locales = collect_translation_locales(&input.translations);

        let mut seen_locales = HashSet::new();
        let mut seen_handles = HashSet::new();
        for trans_input in &input.translations {
            if !seen_locales.insert(trans_input.locale.clone()) {
                return Err(CommerceError::Validation(format!(
                    "Duplicate translation locale `{}` in product input",
                    trans_input.locale
                )));
            }

            let handle = trans_input
                .handle
                .clone()
                .unwrap_or_else(|| slugify(&trans_input.title));

            let key = format!("{}::{}", trans_input.locale, handle.clone());
            if !seen_handles.insert(key) {
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

        for (position, axis_input) in input.variant_axes.iter().enumerate() {
            let axis_id = generate_id();
            let axis = entities::product_variant_axis::ActiveModel {
                id: Set(axis_id),
                tenant_id: Set(tenant_id),
                product_id: Set(product_id),
                attribute_id: Set(axis_input.attribute_id),
                position: Set(resolved_axis_position(axis_input.position, position)?),
                created_at: Set(now.into()),
            };
            axis.insert(&txn).await?;

            for (val_pos, option_id) in axis_input.allowed_option_ids.iter().enumerate() {
                let axis_val = entities::product_variant_axis_value::ActiveModel {
                    id: Set(generate_id()),
                    tenant_id: Set(tenant_id),
                    axis_id: Set(axis_id),
                    option_id: Set(*option_id),
                    position: Set(resolved_axis_position(0, val_pos)?),
                    created_at: Set(now.into()),
                };
                axis_val.insert(&txn).await?;
            }
        }
        debug!(
            axes_count = input.variant_axes.len(),
            "Product variant axes inserted"
        );

        let default_stock_location =
            BootstrapService::ensure_default_location_in_tx(&txn, tenant_id).await?;

        let mut variant_translation_models = Vec::new();
        let mut initial_prices = Vec::new();
        for (position, var_input) in input.variants.iter().enumerate() {
            let variant_position = i32::try_from(position)
                .map_err(|_| CommerceError::Validation("too many Product variants".to_owned()))?;
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
                combination_identity: Set(variant_combination_identity(&var_input.axis_values)),
                position: Set(variant_position),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            };
            variant.insert(&txn).await.map_err(|error| {
                tracing::error!(%error, "variant.insert failed in create_product");
                map_product_unique_violation(error, "", "", var_input.sku.as_deref())
            })?;

            if !var_input.axis_values.is_empty() {
                assign_variant_axis_values_in_tx(&txn, tenant_id, variant_id, &var_input.axis_values).await?;
            }

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

            let variant_title = if !var_input.axis_values.is_empty() {
                let variant_model = entities::product_variant::Entity::find_by_id(variant_id)
                    .one(&txn)
                    .await?
                    .ok_or(CommerceError::VariantNotFound(variant_id))?;
                generate_variant_title(&variant_model)
            } else {
                "Default".to_string()
            };
            for locale in &translation_locales {
                variant_translation_models.push(entities::variant_translation::ActiveModel {
                    id: Set(generate_id()),
                    variant_id: Set(variant_id),
                    locale: Set(locale.clone()),
                    title: Set(Some(variant_title.clone())),
                });
            }

            for price_input in &var_input.prices {
                initial_prices.push(InitialPrice {
                    variant_id,
                    channel_id: price_input.channel_id,
                    channel_slug: normalize_public_channel_slug(
                        price_input.channel_slug.as_deref(),
                    ),
                    currency_code: price_input.currency_code.trim().to_ascii_uppercase(),
                    amount: price_input.amount,
                    compare_at_amount: price_input.compare_at_amount,
                });
            }
        }
        if !variant_translation_models.is_empty() {
            entities::variant_translation::Entity::insert_many(variant_translation_models)
                .exec(&txn)
                .await?;
        }
        PricingBootstrapService::create_initial_prices_in_tx(&txn, initial_prices).await?;
        debug!(
            variants_count = input.variants.len(),
            "Product variants and prices inserted"
        );

        if let Some(tags) = normalized_tags.as_deref() {
            let locale = input
                .translations
                .first()
                .map(|translation| translation.locale.as_str())
                .unwrap_or(PLATFORM_FALLBACK_LOCALE);
            self.sync_product_tags_in_tx(&txn, tenant_id, product_id, locale, tags)
                .await?;
        }

        txn.publish(
            tenant_id,
            Some(actor_id),
            DomainEvent::ProductCreated { product_id },
        )
        .await?;

        if input.publish {
            txn.publish(
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductPublished { product_id },
            )
            .await?;
        }

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

        let product = find_product_for_update_in_tx(&txn, tenant_id, product_id).await
            .map_err(|error| {
                if matches!(error, CommerceError::ProductNotFound(_)) {
                    warn!(product_id = %product_id, "Product not found for update");
                }
                error
            })?;
        let existing_product = product.clone();
        if let Some(primary_category_id) = input.primary_category_id {
            validate_existing_variant_axes_for_category_in(
                &txn,
                tenant_id,
                product_id,
                primary_category_id,
            )
            .await?;
        }
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

        let will_become_active = match input.status.as_ref() {
            Some(entities::product::ProductStatus::Active) => {
                existing_product.status != entities::product::ProductStatus::Active
            }
            _ => false,
        };
        let will_deactivate = match input.status.as_ref() {
            Some(status) => {
                *status != entities::product::ProductStatus::Active
                    && existing_product.status == entities::product::ProductStatus::Active
            }
            _ => false,
        };

        if will_become_active {
            ProductCatalogSchemaService::new(self.db.clone(), self.event_bus.clone())
                .validate_product_publish_requirements_in(&txn, tenant_id, product_id)
                .await?;
            product_active.published_at = Set(Some(Utc::now().into()));
        } else if will_deactivate {
            product_active.published_at = Set(None);
        }

        if let Some(status) = input.status {
            product_active.status = Set(status);
        }

        product_active.update(&txn).await?;

        if let Some(prepared_custom_fields) = prepared_custom_fields.as_ref()
            && let (Some(locale), Some(values)) = (
                prepared_custom_fields.locale.as_deref(),
                prepared_custom_fields.localized_values.as_ref(),
            )
        {
            flex::persist_localized_values(
                &txn,
                tenant_id,
                flex::PRODUCT_ENTITY_TYPE,
                product_id,
                locale,
                values,
            )
            .await
            .map_err(CommerceError::from)?;
        }

        let translation_inputs = input.translations.clone();

        if let Some(translations) = translation_inputs {
            let mut seen_locales = HashSet::new();
            let mut seen_handles = HashSet::new();
            for translation_input in translations {
                if !seen_locales.insert(translation_input.locale.clone()) {
                    return Err(CommerceError::Validation(format!(
                        "Duplicate translation locale `{}` in product input",
                        translation_input.locale
                    )));
                }

                let handle = translation_input
                    .handle
                    .clone()
                    .unwrap_or_else(|| slugify(&translation_input.title));

                let locale = translation_input.locale.clone();
                let key = format!("{}::{}", locale, handle.clone());
                if !seen_handles.insert(key) {
                    return Err(CommerceError::DuplicateHandle { handle, locale });
                }

                let existing = entities::product_translation::Entity::find()
                    .filter(entities::product_translation::Column::TenantId.eq(tenant_id))
                    .filter(entities::product_translation::Column::ProductId.eq(product_id))
                    .filter(entities::product_translation::Column::Locale.eq(&locale))
                    .one(&txn)
                    .await?;

                if let Some(existing_model) = existing {
                    let mut active: entities::product_translation::ActiveModel =
                        existing_model.into();
                    active.title = Set(translation_input.title);
                    active.handle = Set(handle.clone());
                    active.description = Set(translation_input.description);
                    active.meta_title = Set(translation_input.meta_title);
                    active.meta_description = Set(translation_input.meta_description);
                    active.update(&txn).await.map_err(|error| {
                        map_product_unique_violation(error, &handle, &locale, None)
                    })?;
                } else {
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
                    translation.insert(&txn).await.map_err(|error| {
                        map_product_unique_violation(error, &handle, &locale, None)
                    })?;

                    let variants = entities::product_variant::Entity::find()
                        .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
                        .filter(entities::product_variant::Column::ProductId.eq(product_id))
                        .all(&txn)
                        .await?;

                    if !variants.is_empty() {
                        let variant_ids: Vec<Uuid> = variants.iter().map(|v| v.id).collect();
                        let existing_trans_variant_ids: HashSet<Uuid> =
                            entities::variant_translation::Entity::find()
                                .filter(
                                    entities::variant_translation::Column::VariantId
                                        .is_in(variant_ids),
                                )
                                .filter(entities::variant_translation::Column::Locale.eq(&locale))
                                .all(&txn)
                                .await?
                                .into_iter()
                                .map(|vt| vt.variant_id)
                                .collect();

                        let mut new_variant_translations = Vec::new();
                        for variant in variants {
                            if !existing_trans_variant_ids.contains(&variant.id) {
                                let variant_title = generate_variant_title(&variant);
                                new_variant_translations.push(
                                    entities::variant_translation::ActiveModel {
                                        id: Set(generate_id()),
                                        variant_id: Set(variant.id),
                                        locale: Set(locale.clone()),
                                        title: Set(Some(variant_title)),
                                    },
                                );
                            }
                        }
                        if !new_variant_translations.is_empty() {
                            entities::variant_translation::Entity::insert_many(
                                new_variant_translations,
                            )
                            .exec(&txn)
                            .await?;
                        }
                    }
                }
            }
        }

        if let Some((_, Some(tags))) = metadata_update.as_ref() {
            let locale = resolve_tag_locale_for_update(
                &txn,
                tenant_id,
                product_id,
                input.translations.as_deref(),
            )
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
        if will_become_active {
            txn.publish(
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductPublished { product_id },
            )
            .await?;
        }
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
            .query_one_raw(Statement::from_sql_and_values(
                self.db.get_database_backend(),
                "SELECT kind FROM catalog_categories WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
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

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;

        let product = find_product_for_update_in_tx(&txn, tenant_id, product_id)
            .await
            .map_err(|error| {
                if matches!(error, CommerceError::ProductNotFound(_)) {
                    warn!(product_id = %product_id, "Product not found for publishing");
                }
                error
            })?;

        ProductCatalogSchemaService::new(self.db.clone(), self.event_bus.clone())
            .validate_product_publish_requirements_in(&txn, tenant_id, product_id)
            .await?;

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

        let product = find_product_for_update_in_tx(&txn, tenant_id, product_id).await?;

        let mut product_active: entities::product::ActiveModel = product.into();
        product_active.status = Set(entities::product::ProductStatus::Draft);
        product_active.published_at = Set(None);
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

        let product = find_product_for_update_in_tx(&txn, tenant_id, product_id).await?;

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

            PricingBootstrapService::delete_prices_for_variants_in_tx(&txn, &variant_ids).await?;

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

        let axis_ids: Vec<Uuid> = entities::product_variant_axis::Entity::find()
            .filter(entities::product_variant_axis::Column::ProductId.eq(product_id))
            .filter(entities::product_variant_axis::Column::TenantId.eq(tenant_id))
            .all(&txn)
            .await?
            .into_iter()
            .map(|axis| axis.id)
            .collect();

        if !axis_ids.is_empty() {
            entities::product_variant_axis_value::Entity::delete_many()
                .filter(entities::product_variant_axis_value::Column::AxisId.is_in(axis_ids.clone()))
                .exec(&txn)
                .await?;

            entities::product_variant_axis::Entity::delete_many()
                .filter(entities::product_variant_axis::Column::Id.is_in(axis_ids))
                .exec(&txn)
                .await?;
        }

        let image_ids: Vec<Uuid> = entities::product_image::Entity::find()
            .filter(entities::product_image::Column::ProductId.eq(product_id))
            .all(&txn)
            .await?
            .into_iter()
            .map(|image| image.id)
            .collect();
        if !image_ids.is_empty() {
            entities::product_image_translation::Entity::delete_many()
                .filter(
                    entities::product_image_translation::Column::ImageId.is_in(image_ids.clone()),
                )
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

        flex::delete_attached_localized_values(&txn, tenant_id, flex::PRODUCT_ENTITY_TYPE, product_id)
            .await
            .map_err(CommerceError::from)?;

        txn.publish_product_deleted(
            tenant_id,
            Some(actor_id),
            product_id,
            &image_ids,
        )
        .await?;

        txn.commit().await?;
        info!(product_id = %product_id, "Product deleted successfully");

        Ok(())
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, product_id = %product_id))]
    pub async fn create_variant(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        product_id: Uuid,
        input: CreateVariantInput,
    ) -> CommerceResult<VariantResponse> {
        debug!("Creating product variant");

        input
            .validate()
            .map_err(|e| CommerceError::Validation(e.to_string()))?;
        validate_variant_prices(&input.prices)?;

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;

        let _product = find_product_for_update_in_tx(&txn, tenant_id, product_id).await?;
        let configured_axes = load_variant_axis_configuration_in(&txn, tenant_id, product_id).await?;
        validate_variant_axis_values_against_configuration(
            input.axis_values.as_slice(),
            configured_axes.as_slice(),
        )?;

        let existing_locales = entities::product_translation::Entity::find()
            .filter(entities::product_translation::Column::TenantId.eq(tenant_id))
            .filter(entities::product_translation::Column::ProductId.eq(product_id))
            .all(&txn)
            .await?
            .into_iter()
            .map(|t| t.locale)
            .collect::<Vec<_>>();

        let existing_variants = entities::product_variant::Entity::find()
            .filter(entities::product_variant::Column::ProductId.eq(product_id))
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .all(&txn)
            .await?;
        if configured_axes.is_empty() && !existing_variants.is_empty() {
            return Err(CommerceError::Validation(
                "a product without variant axes can have exactly one default variant".to_owned(),
            ));
        }
        let next_position = match existing_variants.iter().map(|variant| variant.position).max() {
            Some(max_position) => max_position.checked_add(1).ok_or_else(|| {
                CommerceError::Validation("cannot append variant: variant ordering is exhausted".to_owned())
            })?,
            None => 0,
        };

        let variant_id = generate_id();
        let now = Utc::now();

        let variant = entities::product_variant::ActiveModel {
            id: Set(variant_id),
            product_id: Set(product_id),
            tenant_id: Set(tenant_id),
            sku: Set(input.sku.clone()),
            barcode: Set(input.barcode.clone()),
            shipping_profile_slug: Set(input
                .shipping_profile_slug
                .as_deref()
                .and_then(normalize_shipping_profile_slug)),
            ean: Set(None),
            upc: Set(None),
            inventory_policy: Set(input.inventory_policy.clone()),
            inventory_management: Set("manual".into()),
            inventory_quantity: Set(0),
            weight: Set(input.weight),
            weight_unit: Set(input.weight_unit.clone()),
            combination_identity: Set(variant_combination_identity(&input.axis_values)),
            position: Set(next_position),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };

        variant
            .insert(&txn)
            .await
            .map_err(|error| map_product_unique_violation(error, "", "", input.sku.as_deref()))?;

        if !input.axis_values.is_empty() {
            assign_variant_axis_values_in_tx(&txn, tenant_id, variant_id, &input.axis_values).await?;
        }

        let default_stock_location =
            BootstrapService::ensure_default_location_in_tx(&txn, tenant_id).await?;

        BootstrapService::create_initial_records_in_tx(
            &txn,
            &default_stock_location,
            InitialInventory {
                variant_id,
                sku: input.sku.clone(),
                available_quantity: input.inventory_quantity,
            },
        )
        .await?;

        let variant_model = entities::product_variant::Entity::find_by_id(variant_id)
            .one(&txn)
            .await?
            .ok_or(CommerceError::VariantNotFound(variant_id))?;
        let variant_title = generate_variant_title(&variant_model);

        let mut variant_translation_models = Vec::new();
        for locale in &existing_locales {
            variant_translation_models.push(entities::variant_translation::ActiveModel {
                id: Set(generate_id()),
                variant_id: Set(variant_id),
                locale: Set(locale.clone()),
                title: Set(Some(variant_title.clone())),
            });
        }
        if !variant_translation_models.is_empty() {
            entities::variant_translation::Entity::insert_many(variant_translation_models)
                .exec(&txn)
                .await?;
        }

        let mut initial_prices = Vec::new();
        for price_input in &input.prices {
            initial_prices.push(InitialPrice {
                variant_id,
                channel_id: price_input.channel_id,
                channel_slug: normalize_public_channel_slug(price_input.channel_slug.as_deref()),
                currency_code: price_input.currency_code.trim().to_ascii_uppercase(),
                amount: price_input.amount,
                compare_at_amount: price_input.compare_at_amount,
            });
        }
        if !initial_prices.is_empty() {
            PricingBootstrapService::create_initial_prices_in_tx(&txn, initial_prices).await?;
        }

        txn.publish(
            tenant_id,
            Some(actor_id),
            DomainEvent::VariantCreated {
                variant_id,
                product_id,
            },
        )
        .await?;

        txn.commit().await?;

        info!(
            variant_id = %variant_id,
            product_id = %product_id,
            "Product variant created successfully"
        );

        self.get_variant(tenant_id, variant_id).await
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, variant_id = %variant_id))]
    pub async fn update_variant(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        input: UpdateVariantInput,
    ) -> CommerceResult<VariantResponse> {
        debug!("Updating product variant");

        input
            .validate()
            .map_err(|e| CommerceError::Validation(e.to_string()))?;
        if let Some(prices) = input.prices.as_deref() {
            validate_variant_prices(prices)?;
        }

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;

        let observed_variant = entities::product_variant::Entity::find_by_id(variant_id)
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(CommerceError::VariantNotFound(variant_id))?;
        let product_id = observed_variant.product_id;
        let _product = find_product_for_update_in_tx(&txn, tenant_id, product_id).await?;
        let variant = find_variant_for_update_in_tx(&txn, tenant_id, variant_id).await?;
        let mut active: entities::product_variant::ActiveModel = variant.into();
        active.updated_at = Set(Utc::now().into());

        if let Some(sku) = input.sku.clone() {
            active.sku = Set(Some(sku));
        }
        if let Some(barcode) = input.barcode {
            active.barcode = Set(Some(barcode));
        }
        if input.shipping_profile_slug.is_some() {
            active.shipping_profile_slug = Set(input
                .shipping_profile_slug
                .as_deref()
                .and_then(normalize_shipping_profile_slug));
        }
        if let Some(inventory_policy) = input.inventory_policy {
            active.inventory_policy = Set(inventory_policy);
        }
        if input.weight.is_some() {
            active.weight = Set(input.weight);
        }
        if let Some(weight_unit) = input.weight_unit {
            active.weight_unit = Set(Some(weight_unit));
        }
        let _updated_variant = active
            .update(&txn)
            .await
            .map_err(|error| map_product_unique_violation(error, "", "", input.sku.as_deref()))?;

        if let Some(ref axis_values) = input.axis_values {
            let configured_axes =
                load_variant_axis_configuration_in(&txn, tenant_id, product_id).await?;
            validate_variant_axis_values_against_configuration(
                axis_values.as_slice(),
                configured_axes.as_slice(),
            )?;
            let configured_axis_attribute_ids = configured_axes
                .iter()
                .map(|axis| axis.attribute_id)
                .collect::<Vec<_>>();

            if !configured_axis_attribute_ids.is_empty() {
                let placeholders = (0..configured_axis_attribute_ids.len())
                    .map(|i| format!("${}", i + 3))
                    .collect::<Vec<_>>()
                    .join(", ");
                let query = format!(
                    "SELECT id FROM product_variant_attribute_values WHERE tenant_id = $1 AND variant_id = $2 AND attribute_id IN ({placeholders})"
                );
                let mut params = vec![tenant_id.into(), variant_id.into()];
                for aid in configured_axis_attribute_ids {
                    params.push(aid.into());
                }
                let existing_vals: Vec<IdRow> = IdRow::find_by_statement(Statement::from_sql_and_values(
                    txn.get_database_backend(),
                    &query,
                    params,
                ))
                .all(&txn)
                .await?;
                for ev in existing_vals {
                    txn.execute_raw(Statement::from_sql_and_values(
                        txn.get_database_backend(),
                        "DELETE FROM product_variant_attribute_value_options WHERE tenant_id = $1 AND value_id = $2",
                        vec![tenant_id.into(), ev.id.into()],
                    ))
                    .await?;
                    txn.execute_raw(Statement::from_sql_and_values(
                        txn.get_database_backend(),
                        "DELETE FROM product_variant_attribute_values WHERE tenant_id = $1 AND id = $2",
                        vec![tenant_id.into(), ev.id.into()],
                    ))
                    .await?;
                }
            }

            assign_variant_axis_values_in_tx(&txn, tenant_id, variant_id, axis_values).await?;

            let variant_model = entities::product_variant::Entity::find_by_id(variant_id)
                .one(&txn)
                .await?
                .ok_or(CommerceError::VariantNotFound(variant_id))?;
            let variant_title = generate_variant_title(&variant_model);
            entities::variant_translation::Entity::update_many()
                .filter(entities::variant_translation::Column::VariantId.eq(variant_id))
                .col_expr(
                    entities::variant_translation::Column::Title,
                    sea_orm::sea_query::Expr::value(Some(variant_title)),
                )
                .exec(&txn)
                .await?;
        }

        if let Some(prices) = input.prices {
            PricingBootstrapService::delete_prices_for_variants_in_tx(&txn, &[variant_id]).await?;

            let mut initial_prices = Vec::new();
            for price_input in &prices {
                initial_prices.push(InitialPrice {
                    variant_id,
                    channel_id: price_input.channel_id,
                    channel_slug: normalize_public_channel_slug(
                        price_input.channel_slug.as_deref(),
                    ),
                    currency_code: price_input.currency_code.trim().to_ascii_uppercase(),
                    amount: price_input.amount,
                    compare_at_amount: price_input.compare_at_amount,
                });
            }
            if !initial_prices.is_empty() {
                PricingBootstrapService::create_initial_prices_in_tx(&txn, initial_prices).await?;
            }
        }

        if let Some(quantity) = input.inventory_quantity {
            BootstrapService::update_initial_quantity_in_tx(&txn, variant_id, quantity).await?;
        }

        txn.publish(
            tenant_id,
            Some(actor_id),
            DomainEvent::VariantUpdated {
                variant_id,
                product_id,
            },
        )
        .await?;

        txn.commit().await?;

        info!(
            variant_id = %variant_id,
            product_id = %product_id,
            "Product variant updated successfully"
        );

        self.get_variant(tenant_id, variant_id).await
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id, variant_id = %variant_id))]
    pub async fn delete_variant(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
    ) -> CommerceResult<()> {
        debug!("Deleting product variant");

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;

        let observed_variant = entities::product_variant::Entity::find_by_id(variant_id)
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(CommerceError::VariantNotFound(variant_id))?;
        let product_id = observed_variant.product_id;
        let _product = find_product_for_update_in_tx(&txn, tenant_id, product_id).await?;
        let _variant = find_variant_for_update_in_tx(&txn, tenant_id, variant_id).await?;

        let count = entities::product_variant::Entity::find()
            .filter(entities::product_variant::Column::ProductId.eq(product_id))
            .count(&txn)
            .await?;
        if count <= 1 {
            return Err(CommerceError::CannotDeleteOnlyVariant);
        }

        BootstrapService::delete_records_for_variants_in_tx(&txn, &[variant_id]).await?;
        PricingBootstrapService::delete_prices_for_variants_in_tx(&txn, &[variant_id]).await?;

        entities::variant_translation::Entity::delete_many()
            .filter(entities::variant_translation::Column::VariantId.eq(variant_id))
            .exec(&txn)
            .await?;

        entities::product_variant::Entity::delete_by_id(variant_id)
            .exec(&txn)
            .await?;

        txn.publish(
            tenant_id,
            Some(actor_id),
            DomainEvent::VariantDeleted {
                variant_id,
                product_id,
            },
        )
        .await?;

        txn.commit().await?;

        info!(
            variant_id = %variant_id,
            product_id = %product_id,
            "Product variant deleted successfully"
        );

        Ok(())
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, product_id = %product_id))]
    pub async fn add_product_image(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        product_id: Uuid,
        input: AddProductImageInput,
    ) -> CommerceResult<ProductImageResponse> {
        debug!(media_id = %input.media_id, "Adding product image");
        validate_product_image_input(input.position, input.alt_text.as_deref())?;
        let image_locale = input
            .alt_text
            .as_deref()
            .map(|_| canonical_product_image_locale(input.locale.as_deref()))
            .transpose()?;

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;

        let _product = find_product_for_update_in_tx(&txn, tenant_id, product_id).await?;

        let existing_images = entities::product_image::Entity::find()
            .filter(entities::product_image::Column::ProductId.eq(product_id))
            .order_by_asc(entities::product_image::Column::Position)
            .order_by_asc(entities::product_image::Column::Id)
            .all(&txn)
            .await?;
        let requested_position = input.position.map(|position| {
            usize::try_from(position).map_err(|_| {
                CommerceError::Validation("Product image position cannot be negative".to_owned())
            })
        }).transpose()?;
        let insertion_index = requested_position.unwrap_or(existing_images.len());
        if insertion_index > existing_images.len() {
            return Err(CommerceError::Validation(
                "Product image position must not exceed the current image count".to_owned(),
            ));
        }
        let position = i32::try_from(insertion_index)
            .map_err(|_| CommerceError::Validation("too many Product images".to_owned()))?;

        let image_id = generate_id();
        let _image = entities::product_image::ActiveModel {
            id: Set(image_id),
            product_id: Set(product_id),
            media_id: Set(input.media_id),
            position: Set(position),
        }
        .insert(&txn)
        .await?;

        let mut ordered_image_ids = existing_images
            .into_iter()
            .map(|image| image.id)
            .collect::<Vec<_>>();
        ordered_image_ids.insert(insertion_index, image_id);
        persist_product_image_order_in_tx(&txn, product_id, ordered_image_ids.as_slice()).await?;

        let mut translations = Vec::new();
        if let Some(alt_text) = input.alt_text.as_deref() {
            let locale = image_locale.clone().ok_or_else(|| {
                CommerceError::Core(CoreError::Internal(
                    "validated Product image translation locale is missing".to_owned(),
                ))
            })?;
            entities::product_image_translation::ActiveModel {
                id: Set(generate_id()),
                image_id: Set(image_id),
                locale: Set(locale.clone()),
                alt_text: Set(Some(alt_text.to_string())),
            }
            .insert(&txn)
            .await?;

            translations.push(ProductImageTranslationResponse {
                locale,
                alt_text: Some(alt_text.to_string()),
            });
        }

        txn.publish(
            tenant_id,
            Some(actor_id),
            DomainEvent::ProductUpdated { product_id },
        )
        .await?;

        txn.commit().await?;

        info!(
            image_id = %image_id,
            product_id = %product_id,
            "Product image added successfully"
        );

        Ok(ProductImageResponse {
            id: image_id,
            media_id: input.media_id,
            url: format_product_media_url(input.media_id),
            alt_text: input.alt_text,
            position,
            translations,
        })
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, product_id = %product_id, image_id = %image_id))]
    pub async fn update_product_image(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        product_id: Uuid,
        image_id: Uuid,
        input: UpdateProductImageInput,
    ) -> CommerceResult<ProductImageResponse> {
        debug!("Updating product image");
        validate_product_image_input(input.position, input.alt_text.as_deref())?;
        let image_locale = input
            .alt_text
            .as_deref()
            .map(|_| canonical_product_image_locale(input.locale.as_deref()))
            .transpose()?;

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;

        let _product = find_product_for_update_in_tx(&txn, tenant_id, product_id).await?;

        let image = entities::product_image::Entity::find_by_id(image_id)
            .filter(entities::product_image::Column::ProductId.eq(product_id))
            .one(&txn)
            .await?
            .ok_or(CommerceError::ImageNotFound(image_id))?;

        let mut position = image.position;
        if let Some(new_position) = input.position {
            let target_index = usize::try_from(new_position).map_err(|_| {
                CommerceError::Validation("Product image position cannot be negative".to_owned())
            })?;
            let mut ordered_image_ids = entities::product_image::Entity::find()
                .filter(entities::product_image::Column::ProductId.eq(product_id))
                .order_by_asc(entities::product_image::Column::Position)
                .order_by_asc(entities::product_image::Column::Id)
                .all(&txn)
                .await?
                .into_iter()
                .map(|image| image.id)
                .collect::<Vec<_>>();
            let current_index = ordered_image_ids
                .iter()
                .position(|id| *id == image_id)
                .ok_or(CommerceError::ImageNotFound(image_id))?;
            if target_index >= ordered_image_ids.len() {
                return Err(CommerceError::Validation(
                    "Product image position must reference an existing image slot".to_owned(),
                ));
            }
            ordered_image_ids.remove(current_index);
            ordered_image_ids.insert(target_index, image_id);
            persist_product_image_order_in_tx(&txn, product_id, ordered_image_ids.as_slice()).await?;
            position = i32::try_from(target_index)
                .map_err(|_| CommerceError::Validation("too many Product images".to_owned()))?;
        }
        let updated_image = entities::product_image::Entity::find_by_id(image_id)
            .filter(entities::product_image::Column::ProductId.eq(product_id))
            .one(&txn)
            .await?
            .ok_or(CommerceError::ImageNotFound(image_id))?;

        if let Some(alt_text) = input.alt_text.as_deref() {
            let locale = image_locale.clone().ok_or_else(|| {
                CommerceError::Core(CoreError::Internal(
                    "validated Product image translation locale is missing".to_owned(),
                ))
            })?;
            let existing_trans = entities::product_image_translation::Entity::find()
                .filter(entities::product_image_translation::Column::ImageId.eq(image_id))
                .filter(entities::product_image_translation::Column::Locale.eq(&locale))
                .one(&txn)
                .await?;

            if let Some(existing) = existing_trans {
                let mut active: entities::product_image_translation::ActiveModel = existing.into();
                active.alt_text = Set(Some(alt_text.to_string()));
                active.update(&txn).await?;
            } else {
                entities::product_image_translation::ActiveModel {
                    id: Set(generate_id()),
                    image_id: Set(image_id),
                    locale: Set(locale),
                    alt_text: Set(Some(alt_text.to_string())),
                }
                .insert(&txn)
                .await?;
            }
        }

        txn.publish(
            tenant_id,
            Some(actor_id),
            DomainEvent::ProductUpdated { product_id },
        )
        .await?;

        txn.commit().await?;

        let all_translations = entities::product_image_translation::Entity::find()
            .filter(entities::product_image_translation::Column::ImageId.eq(image_id))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|t| ProductImageTranslationResponse {
                locale: t.locale,
                alt_text: t.alt_text,
            })
            .collect();

        info!(
            image_id = %image_id,
            product_id = %product_id,
            "Product image updated successfully"
        );

        Ok(ProductImageResponse {
            id: image_id,
            media_id: updated_image.media_id,
            url: format_product_media_url(updated_image.media_id),
            alt_text: input.alt_text,
            position,
            translations: all_translations,
        })
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id, product_id = %product_id, image_id = %image_id))]
    pub async fn delete_product_image(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        product_id: Uuid,
        image_id: Uuid,
    ) -> CommerceResult<()> {
        debug!("Deleting product image");

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;

        let _product = find_product_for_update_in_tx(&txn, tenant_id, product_id).await?;

        let _image = entities::product_image::Entity::find_by_id(image_id)
            .filter(entities::product_image::Column::ProductId.eq(product_id))
            .one(&txn)
            .await?
            .ok_or(CommerceError::ImageNotFound(image_id))?;

        entities::product_image_translation::Entity::delete_many()
            .filter(entities::product_image_translation::Column::ImageId.eq(image_id))
            .exec(&txn)
            .await?;

        entities::product_image::Entity::delete_by_id(image_id)
            .exec(&txn)
            .await?;

        let remaining_image_ids = entities::product_image::Entity::find()
            .filter(entities::product_image::Column::ProductId.eq(product_id))
            .order_by_asc(entities::product_image::Column::Position)
            .order_by_asc(entities::product_image::Column::Id)
            .all(&txn)
            .await?
            .into_iter()
            .map(|image| image.id)
            .collect::<Vec<_>>();
        persist_product_image_order_in_tx(&txn, product_id, remaining_image_ids.as_slice()).await?;

        txn.publish(
            tenant_id,
            Some(actor_id),
            DomainEvent::ProductUpdated { product_id },
        )
        .await?;

        txn.commit().await?;

        info!(
            image_id = %image_id,
            product_id = %product_id,
            "Product image deleted successfully"
        );

        Ok(())
    }

    #[instrument(skip(self, image_ids), fields(tenant_id = %tenant_id, product_id = %product_id))]
    pub async fn reorder_product_images(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        product_id: Uuid,
        image_ids: Vec<Uuid>,
    ) -> CommerceResult<()> {
        debug!(count = image_ids.len(), "Reordering product images");

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;

        let _product = find_product_for_update_in_tx(&txn, tenant_id, product_id).await?;
        let existing_images = entities::product_image::Entity::find()
            .filter(entities::product_image::Column::ProductId.eq(product_id))
            .all(&txn)
            .await?;
        let existing_ids = existing_images
            .iter()
            .map(|image| image.id)
            .collect::<HashSet<_>>();
        let requested_ids = image_ids.iter().copied().collect::<HashSet<_>>();
        if requested_ids.len() != image_ids.len()
            || requested_ids.len() != existing_ids.len()
            || requested_ids != existing_ids
        {
            return Err(CommerceError::Validation(
                "image reorder must contain every product image exactly once".to_owned(),
            ));
        }

        persist_product_image_order_in_tx(&txn, product_id, image_ids.as_slice()).await?;

        txn.publish(
            tenant_id,
            Some(actor_id),
            DomainEvent::ProductUpdated { product_id },
        )
        .await?;

        txn.commit().await?;

        info!(product_id = %product_id, "Product images reordered successfully");

        Ok(())
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, product_id = %product_id))]
    pub async fn set_variant_axes(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        product_id: Uuid,
        input: SetVariantAxesInput,
    ) -> CommerceResult<Vec<VariantAxisConfigResponse>> {
        debug!("Setting variant axes for product");

        input
            .validate()
            .map_err(|e| CommerceError::Validation(e.to_string()))?;

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;

        let product = find_product_for_update_in_tx(&txn, tenant_id, product_id).await?;
        validate_variant_axis_configuration_in(
            &txn,
            tenant_id,
            product.primary_category_id,
            input.axes.as_slice(),
        )
        .await?;

        let existing_variants = entities::product_variant::Entity::find()
            .filter(entities::product_variant::Column::ProductId.eq(product_id))
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .all(&txn)
            .await?;
        let next_identities = validate_existing_variant_axis_assignments_in(
            &txn,
            tenant_id,
            existing_variants.as_slice(),
            input.axes.as_slice(),
        )
        .await?;

        let existing_axis_ids: Vec<Uuid> = entities::product_variant_axis::Entity::find()
            .filter(entities::product_variant_axis::Column::ProductId.eq(product_id))
            .filter(entities::product_variant_axis::Column::TenantId.eq(tenant_id))
            .all(&txn)
            .await?
            .into_iter()
            .map(|axis| axis.id)
            .collect();

        if !existing_axis_ids.is_empty() {
            // The value rows are actual Variant EAV data and must survive a configuration
            // change. Only configuration rows are replaced after the proposed configuration
            // was proven compatible with every existing Variant above.
            entities::product_variant_axis_value::Entity::delete_many()
                .filter(
                    entities::product_variant_axis_value::Column::AxisId
                        .is_in(existing_axis_ids.clone()),
                )
                .exec(&txn)
                .await?;
            entities::product_variant_axis::Entity::delete_many()
                .filter(entities::product_variant_axis::Column::Id.is_in(existing_axis_ids))
                .exec(&txn)
                .await?;
        }

        let now = Utc::now();
        for (index, axis_input) in input.axes.iter().enumerate() {
            let axis_id = generate_id();
            entities::product_variant_axis::ActiveModel {
                id: Set(axis_id),
                tenant_id: Set(tenant_id),
                product_id: Set(product_id),
                attribute_id: Set(axis_input.attribute_id),
                position: Set(resolved_axis_position(axis_input.position, index)?),
                created_at: Set(now.into()),
            }
            .insert(&txn)
            .await?;

            for (value_index, option_id) in axis_input.allowed_option_ids.iter().enumerate() {
                entities::product_variant_axis_value::ActiveModel {
                    id: Set(generate_id()),
                    tenant_id: Set(tenant_id),
                    axis_id: Set(axis_id),
                    option_id: Set(*option_id),
                    position: Set(resolved_axis_position(0, value_index)?),
                    created_at: Set(now.into()),
                }
                .insert(&txn)
                .await?;
            }
        }

        replace_variant_combination_identities_in_tx(&txn, tenant_id, next_identities.as_slice())
            .await?;

        txn.publish(
            tenant_id,
            Some(actor_id),
            DomainEvent::ProductUpdated { product_id },
        )
        .await?;

        txn.commit().await?;

        self.get_product_variant_axes(tenant_id, product_id, PLATFORM_FALLBACK_LOCALE)
            .await
    }
}

#[derive(sea_orm::FromQueryResult)]
struct IdRow {
    id: Uuid,
}

async fn persist_product_image_order_in_tx(
    txn: &DatabaseTransaction,
    product_id: Uuid,
    image_ids: &[Uuid],
) -> CommerceResult<()> {
    for (index, image_id) in image_ids.iter().enumerate() {
        let position = i32::try_from(index)
            .map_err(|_| CommerceError::Validation("too many Product images".to_owned()))?;
        let result = entities::product_image::Entity::update_many()
            .filter(entities::product_image::Column::Id.eq(*image_id))
            .filter(entities::product_image::Column::ProductId.eq(product_id))
            .col_expr(
                entities::product_image::Column::Position,
                sea_orm::sea_query::Expr::value(position),
            )
            .exec(txn)
            .await?;
        if result.rows_affected != 1 {
            return Err(CommerceError::ImageNotFound(*image_id));
        }
    }
    Ok(())
}

fn canonical_product_image_locale(locale: Option<&str>) -> CommerceResult<String> {
    TenantLocale::new(locale.unwrap_or(PLATFORM_FALLBACK_LOCALE))
        .map(TenantLocale::into_inner)
        .map_err(|error| CommerceError::Validation(format!("invalid Product image locale: {error}")))
}

fn validate_product_image_input(position: Option<i32>, alt_text: Option<&str>) -> CommerceResult<()> {
    if position.is_some_and(|position| position < 0) {
        return Err(CommerceError::Validation(
            "Product image position cannot be negative".to_owned(),
        ));
    }
    if alt_text.is_some_and(|alt_text| alt_text.chars().count() > 255) {
        return Err(CommerceError::Validation(
            "Product image alt text must contain at most 255 characters".to_owned(),
        ));
    }
    Ok(())
}

const MAX_VARIANT_AXES: usize = 32;
const MAX_VARIANT_AXIS_VALUES: usize = 256;
const VARIANT_AXIS_POSITION_GAP: i32 = 100;

#[derive(Clone, Debug)]
struct ConfiguredVariantAxis {
    attribute_id: Uuid,
    allowed_option_ids: HashSet<Uuid>,
}

#[derive(FromQueryResult)]
struct VariantAxisDefinitionRow {
    id: Uuid,
    value_type: String,
    scope: String,
}

#[derive(FromQueryResult)]
struct VariantAxisOptionRow {
    id: Uuid,
    attribute_id: Uuid,
}

#[derive(FromQueryResult)]
struct ExistingVariantAxisAssignmentRow {
    attribute_id: Uuid,
    option_id: Uuid,
}

async fn load_variant_axis_configuration_in<C>(
    conn: &C,
    tenant_id: Uuid,
    product_id: Uuid,
) -> CommerceResult<Vec<ConfiguredVariantAxis>>
where
    C: ConnectionTrait,
{
    let axes = entities::product_variant_axis::Entity::find()
        .filter(entities::product_variant_axis::Column::TenantId.eq(tenant_id))
        .filter(entities::product_variant_axis::Column::ProductId.eq(product_id))
        .order_by_asc(entities::product_variant_axis::Column::Position)
        .all(conn)
        .await?;

    let mut configured = Vec::with_capacity(axes.len());
    for axis in axes {
        let allowed_option_ids = entities::product_variant_axis_value::Entity::find()
            .filter(entities::product_variant_axis_value::Column::TenantId.eq(tenant_id))
            .filter(entities::product_variant_axis_value::Column::AxisId.eq(axis.id))
            .all(conn)
            .await?
            .into_iter()
            .map(|value| value.option_id)
            .collect::<HashSet<_>>();
        if allowed_option_ids.is_empty() {
            return Err(CommerceError::Core(CoreError::Internal(format!(
                "configured variant axis {} has no allowed options",
                axis.attribute_id
            ))));
        }
        configured.push(ConfiguredVariantAxis {
            attribute_id: axis.attribute_id,
            allowed_option_ids,
        });
    }
    Ok(configured)
}

fn resolved_axis_position(requested_position: i32, index: usize) -> CommerceResult<i32> {
    if requested_position < 0 {
        return Err(CommerceError::Validation(
            "variant axis positions cannot be negative".to_owned(),
        ));
    }
    if requested_position != 0 || index == 0 {
        return Ok(requested_position);
    }
    i32::try_from(index)
        .ok()
        .and_then(|index| index.checked_mul(VARIANT_AXIS_POSITION_GAP))
        .ok_or_else(|| CommerceError::Validation("too many variant axes or axis values".to_owned()))
}

async fn validate_variant_axis_configuration_in<C>(
    conn: &C,
    tenant_id: Uuid,
    primary_category_id: Option<Uuid>,
    axes: &[VariantAxisInput],
) -> CommerceResult<()>
where
    C: ConnectionTrait,
{
    if axes.len() > MAX_VARIANT_AXES {
        return Err(CommerceError::Validation(format!(
            "at most {MAX_VARIANT_AXES} variant axes are supported"
        )));
    }

    let form = match primary_category_id {
        Some(category_id) => Some(
            ProductCatalogSchemaService::load_effective_form_for_category_in(
                conn,
                tenant_id,
                category_id,
                &[],
            )
            .await?,
        ),
        None if axes.is_empty() => None,
        None => {
            return Err(CommerceError::Validation(
                "a product must have a primary structural category before variant axes can be configured"
                    .to_owned(),
            ));
        }
    };
    let policies = form
        .into_iter()
        .flat_map(|form| form.attributes)
        .filter(|binding| !binding.is_disabled)
        .map(|binding| (binding.attribute_id, binding.variant_axis_policy))
        .collect::<HashMap<_, _>>();

    let selected_attribute_ids = axes
        .iter()
        .map(|axis| axis.attribute_id)
        .collect::<HashSet<_>>();
    let missing_required_axes = policies
        .iter()
        .filter(|(_, policy)| policy.as_str() == "required")
        .map(|(attribute_id, _)| *attribute_id)
        .filter(|attribute_id| !selected_attribute_ids.contains(attribute_id))
        .collect::<Vec<_>>();
    if !missing_required_axes.is_empty() {
        return Err(CommerceError::Validation(format!(
            "required variant axes are missing from the configuration: {}",
            missing_required_axes
                .iter()
                .map(Uuid::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    if axes.is_empty() {
        return Ok(());
    }

    let mut seen_attributes = HashSet::new();
    let mut seen_positions = HashSet::new();
    let mut requested_option_ids = Vec::new();
    for (index, axis) in axes.iter().enumerate() {
        if !seen_attributes.insert(axis.attribute_id) {
            return Err(CommerceError::Validation(format!(
                "duplicate attribute_id `{}` in variant axes",
                axis.attribute_id
            )));
        }
        let position = resolved_axis_position(axis.position, index)?;
        if !seen_positions.insert(position) {
            return Err(CommerceError::Validation(format!(
                "duplicate variant axis position `{position}`"
            )));
        }
        if axis.allowed_option_ids.is_empty() || axis.allowed_option_ids.len() > MAX_VARIANT_AXIS_VALUES {
            return Err(CommerceError::Validation(format!(
                "variant axis `{}` must contain 1..={MAX_VARIANT_AXIS_VALUES} allowed options",
                axis.attribute_id
            )));
        }
        let mut seen_options = HashSet::new();
        for option_id in &axis.allowed_option_ids {
            if !seen_options.insert(*option_id) {
                return Err(CommerceError::Validation(format!(
                    "duplicate option_id `{option_id}` in variant axis `{}`",
                    axis.attribute_id
                )));
            }
            requested_option_ids.push(*option_id);
        }
        match policies.get(&axis.attribute_id).map(String::as_str) {
            Some("allowed" | "required") => {}
            Some("forbidden") => {
                return Err(CommerceError::Validation(format!(
                    "attribute {} is forbidden as a variant axis in the product category",
                    axis.attribute_id
                )));
            }
            _ => {
                return Err(CommerceError::Validation(format!(
                    "attribute {} is outside the product effective schema",
                    axis.attribute_id
                )));
            }
        }
    }

    let attribute_ids = axes.iter().map(|axis| axis.attribute_id).collect::<Vec<_>>();
    let attribute_placeholders = (0..attribute_ids.len())
        .map(|index| format!("${}", index + 2))
        .collect::<Vec<_>>()
        .join(", ");
    let mut attribute_values = vec![tenant_id.into()];
    attribute_values.extend(attribute_ids.iter().copied().map(Into::into));
    let definitions = VariantAxisDefinitionRow::find_by_statement(Statement::from_sql_and_values(
        conn.get_database_backend(),
        format!(
            "SELECT id, value_type, scope FROM product_attributes WHERE tenant_id = $1 AND archived_at IS NULL AND id IN ({attribute_placeholders})"
        ),
        attribute_values,
    ))
    .all(conn)
    .await?
    .into_iter()
    .map(|definition| (definition.id, definition))
    .collect::<HashMap<_, _>>();

    for attribute_id in &attribute_ids {
        let definition = definitions.get(attribute_id).ok_or_else(|| {
            CommerceError::Validation(format!(
                "variant axis attribute {attribute_id} is unavailable or archived"
            ))
        })?;
        if definition.value_type != "select" || !matches!(definition.scope.as_str(), "variant" | "both") {
            return Err(CommerceError::Validation(format!(
                "variant axis attribute {attribute_id} must be an active variant-scoped select attribute"
            )));
        }
    }

    let option_placeholders = (0..requested_option_ids.len())
        .map(|index| format!("${}", index + 2))
        .collect::<Vec<_>>()
        .join(", ");
    let mut option_values = vec![tenant_id.into()];
    option_values.extend(requested_option_ids.iter().copied().map(Into::into));
    let option_owners = VariantAxisOptionRow::find_by_statement(Statement::from_sql_and_values(
        conn.get_database_backend(),
        format!(
            "SELECT id, attribute_id FROM product_attribute_options WHERE tenant_id = $1 AND archived_at IS NULL AND id IN ({option_placeholders})"
        ),
        option_values,
    ))
    .all(conn)
    .await?
    .into_iter()
    .map(|option| (option.id, option.attribute_id))
    .collect::<HashMap<_, _>>();
    for axis in axes {
        for option_id in &axis.allowed_option_ids {
            if option_owners.get(option_id) != Some(&axis.attribute_id) {
                return Err(CommerceError::Validation(format!(
                    "option {option_id} does not belong to active variant axis attribute {}",
                    axis.attribute_id
                )));
            }
        }
    }

    Ok(())
}

async fn validate_existing_variant_axes_for_category_in<C>(
    conn: &C,
    tenant_id: Uuid,
    product_id: Uuid,
    primary_category_id: Uuid,
) -> CommerceResult<()>
where
    C: ConnectionTrait,
{
    let existing_axes = load_variant_axis_configuration_in(conn, tenant_id, product_id).await?;
    let mut proposed_axes = Vec::with_capacity(existing_axes.len());
    for (index, axis) in existing_axes.into_iter().enumerate() {
        proposed_axes.push(VariantAxisInput {
            attribute_id: axis.attribute_id,
            position: resolved_axis_position(0, index)?,
            allowed_option_ids: axis.allowed_option_ids.into_iter().collect(),
        });
    }
    validate_variant_axis_configuration_in(conn, tenant_id, Some(primary_category_id), &proposed_axes)
        .await
}

fn validate_variant_axis_values_against_configuration(
    values: &[VariantAxisValueInput],
    configured_axes: &[ConfiguredVariantAxis],
) -> CommerceResult<()> {
    if configured_axes.is_empty() {
        if values.is_empty() {
            return Ok(());
        }
        return Err(CommerceError::Validation(
            "a product without variant axes cannot receive axis assignments".to_owned(),
        ));
    }
    if values.len() != configured_axes.len() {
        return Err(CommerceError::Validation(format!(
            "variant has {} axis assignments, expected {}",
            values.len(),
            configured_axes.len()
        )));
    }

    let configurations = configured_axes
        .iter()
        .map(|axis| (axis.attribute_id, &axis.allowed_option_ids))
        .collect::<HashMap<_, _>>();
    let mut seen_attributes = HashSet::new();
    for value in values {
        if !seen_attributes.insert(value.attribute_id) {
            return Err(CommerceError::Validation(format!(
                "variant has duplicate assignment for axis `{}`",
                value.attribute_id
            )));
        }
        let allowed_options = configurations.get(&value.attribute_id).ok_or_else(|| {
            CommerceError::Validation(format!(
                "variant assigns attribute {} that is not a configured axis",
                value.attribute_id
            ))
        })?;
        if !allowed_options.contains(&value.option_id) {
            return Err(CommerceError::Validation(format!(
                "option {} is not allowed for variant axis {}",
                value.option_id, value.attribute_id
            )));
        }
    }
    Ok(())
}

async fn validate_existing_variant_axis_assignments_in(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    variants: &[entities::product_variant::Model],
    proposed_axes: &[VariantAxisInput],
) -> CommerceResult<Vec<(Uuid, Option<String>)>> {
    if proposed_axes.is_empty() {
        if variants.len() > 1 {
            return Err(CommerceError::Validation(
                "cannot remove all variant axes while multiple variants exist; consolidate to one variant first"
                    .to_owned(),
            ));
        }
        return Ok(variants.iter().map(|variant| (variant.id, None)).collect());
    }

    let proposed_configuration = proposed_axes
        .iter()
        .map(|axis| ConfiguredVariantAxis {
            attribute_id: axis.attribute_id,
            allowed_option_ids: axis.allowed_option_ids.iter().copied().collect(),
        })
        .collect::<Vec<_>>();
    let attribute_placeholders = (0..proposed_configuration.len())
        .map(|index| format!("${}", index + 3))
        .collect::<Vec<_>>()
        .join(", ");
    let mut identities = Vec::with_capacity(variants.len());
    let mut seen_identities = HashSet::new();
    for variant in variants {
        let mut values = vec![tenant_id.into(), variant.id.into()];
        values.extend(
            proposed_configuration
                .iter()
                .map(|axis| axis.attribute_id.into()),
        );
        let assignments = ExistingVariantAxisAssignmentRow::find_by_statement(
            Statement::from_sql_and_values(
                txn.get_database_backend(),
                format!(
                    "SELECT pvav.attribute_id, pvavo.option_id FROM product_variant_attribute_values pvav JOIN product_variant_attribute_value_options pvavo ON pvavo.tenant_id = pvav.tenant_id AND pvavo.value_id = pvav.id WHERE pvav.tenant_id = $1 AND pvav.variant_id = $2 AND pvav.detached_at IS NULL AND pvav.attribute_id IN ({attribute_placeholders})"
                ),
                values,
            ),
        )
        .all(txn)
        .await?
        .into_iter()
        .map(|assignment| VariantAxisValueInput {
            attribute_id: assignment.attribute_id,
            option_id: assignment.option_id,
        })
        .collect::<Vec<_>>();
        validate_variant_axis_values_against_configuration(
            assignments.as_slice(),
            proposed_configuration.as_slice(),
        )?;
        let identity = variant_combination_identity(assignments.as_slice()).ok_or_else(|| {
            CommerceError::Core(CoreError::Internal(
                "configured variant axes produced no combination identity".to_owned(),
            ))
        })?;
        if !seen_identities.insert(identity.clone()) {
            return Err(CommerceError::Validation(
                "the proposed axes would create duplicate variant combinations".to_owned(),
            ));
        }
        identities.push((variant.id, Some(identity)));
    }
    Ok(identities)
}

async fn replace_variant_combination_identities_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    identities: &[(Uuid, Option<String>)],
) -> CommerceResult<()> {
    if identities.len() > 1 {
        // Combination identity is indexed immediately. First move every affected Variant to a
        // transaction-unique sentinel so swapping two otherwise-valid combinations cannot
        // transiently collide with the unique Product combination index.
        let reconfiguration_id = generate_id();
        for (variant_id, _) in identities {
            txn.execute_raw(Statement::from_sql_and_values(
                txn.get_database_backend(),
                "UPDATE product_variants SET combination_identity = $1 WHERE tenant_id = $2 AND id = $3",
                vec![format!("__axis-reconfiguration:{reconfiguration_id}:{variant_id}").into(), tenant_id.into(), (*variant_id).into()],
            ))
            .await?;
        }
    }
    for (variant_id, identity) in identities {
        txn.execute_raw(Statement::from_sql_and_values(
            txn.get_database_backend(),
            "UPDATE product_variants SET combination_identity = $1 WHERE tenant_id = $2 AND id = $3",
            vec![identity.clone().into(), tenant_id.into(), (*variant_id).into()],
        ))
        .await?;
    }
    Ok(())
}

fn variant_combination_identity(axis_values: &[VariantAxisValueInput]) -> Option<String> {
    if axis_values.is_empty() {
        return None;
    }
    let mut values = axis_values.to_vec();
    values.sort_by_key(|value| value.attribute_id);
    Some(
        values
            .iter()
            .map(|value| format!("{}:{}", value.attribute_id, value.option_id))
            .collect::<Vec<_>>()
            .join(";"),
    )
}

pub(crate) async fn assign_variant_axis_values_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    variant_id: Uuid,
    axis_values: &[VariantAxisValueInput],
) -> CommerceResult<()> {
    for val in axis_values {
        let row = IdRow::find_by_statement(Statement::from_sql_and_values(
            txn.get_database_backend(),
            r#"
            INSERT INTO product_variant_attribute_values (
                id, tenant_id, variant_id, attribute_id, detached_at
            ) VALUES ($1, $2, $3, $4, NULL)
            ON CONFLICT (tenant_id, variant_id, attribute_id) DO UPDATE SET
                detached_at = NULL,
                updated_at = CURRENT_TIMESTAMP
            RETURNING id
            "#,
            vec![
                generate_id().into(),
                tenant_id.into(),
                variant_id.into(),
                val.attribute_id.into(),
            ],
        ))
        .one(txn)
        .await?;

        let value_id = match row {
            Some(r) => r.id,
            None => IdRow::find_by_statement(Statement::from_sql_and_values(
                txn.get_database_backend(),
                "SELECT id FROM product_variant_attribute_values WHERE tenant_id = $1 AND variant_id = $2 AND attribute_id = $3",
                vec![tenant_id.into(), variant_id.into(), val.attribute_id.into()],
            ))
            .one(txn)
            .await?
            .map(|r| r.id)
            .ok_or_else(|| {
                CommerceError::Core(CoreError::Internal(
                    "failed to resolve variant attribute value identity after upsert".to_string(),
                ))
            })?,
        };

        txn.execute_raw(Statement::from_sql_and_values(
            txn.get_database_backend(),
            "DELETE FROM product_variant_attribute_value_options WHERE tenant_id = $1 AND value_id = $2",
            vec![tenant_id.into(), value_id.into()],
        ))
        .await?;

        txn.execute_raw(Statement::from_sql_and_values(
            txn.get_database_backend(),
            "INSERT INTO product_variant_attribute_value_options (tenant_id, value_id, option_id) VALUES ($1, $2, $3)",
            vec![tenant_id.into(), value_id.into(), val.option_id.into()],
        ))
        .await?;
    }

    let combination_identity = variant_combination_identity(axis_values);
    txn.execute_raw(Statement::from_sql_and_values(
        txn.get_database_backend(),
        "UPDATE product_variants SET combination_identity = $1 WHERE tenant_id = $2 AND id = $3",
        vec![combination_identity.into(), tenant_id.into(), variant_id.into()],
    ))
    .await?;

    Ok(())
}

pub(crate) fn validate_variant_axes_and_combinations(
    variant_axes: &[VariantAxisInput],
    variants: &[CreateVariantInput],
) -> CommerceResult<()> {
    if variant_axes.is_empty() {
        if variants.len() != 1 {
            return Err(CommerceError::Validation(
                "a product without variant axes must contain exactly one default variant".to_owned(),
            ));
        }
        if !variants[0].axis_values.is_empty() {
            return Err(CommerceError::Validation(
                "a default variant cannot include axis assignments".to_owned(),
            ));
        }
        return Ok(());
    }

    let mut seen_attrs = HashSet::new();
    for axis in variant_axes {
        if !seen_attrs.insert(axis.attribute_id) {
            return Err(CommerceError::Validation(format!(
                "Duplicate attribute_id `{}` in variant axes configuration",
                axis.attribute_id
            )));
        }
        if axis.allowed_option_ids.is_empty() {
            return Err(CommerceError::Validation(format!(
                "Variant axis `{}` must have at least one allowed option",
                axis.attribute_id
            )));
        }
        let mut seen_opts = HashSet::new();
        for opt in &axis.allowed_option_ids {
            if !seen_opts.insert(*opt) {
                return Err(CommerceError::Validation(format!(
                    "Duplicate option_id `{}` in allowed options for axis `{}`",
                    opt, axis.attribute_id
                )));
            }
        }
    }

    let mut seen_combinations = HashSet::new();
    for (idx, variant) in variants.iter().enumerate() {
        if variant.axis_values.is_empty() {
            return Err(CommerceError::Validation(format!(
                "Variant at index {} is missing axis values for configured axes",
                idx
            )));
        }
        if variant.axis_values.len() != variant_axes.len() {
            return Err(CommerceError::Validation(format!(
                "Variant at index {} has {} axis assignments, expected {}",
                idx,
                variant.axis_values.len(),
                variant_axes.len()
            )));
        }

        let mut variant_axis_attrs = HashSet::new();
        let mut sorted_pairs = Vec::new();
        for val in &variant.axis_values {
            if !variant_axis_attrs.insert(val.attribute_id) {
                return Err(CommerceError::Validation(format!(
                    "Variant at index {} has duplicate assignment for axis `{}`",
                    idx, val.attribute_id
                )));
            }
            let axis = variant_axes
                .iter()
                .find(|a| a.attribute_id == val.attribute_id)
                .ok_or_else(|| {
                    CommerceError::Validation(format!(
                        "Variant at index {} assigns unknown axis `{}`",
                        idx, val.attribute_id
                    ))
                })?;
            if !axis.allowed_option_ids.contains(&val.option_id) {
                return Err(CommerceError::Validation(format!(
                    "Option `{}` is not allowed for axis `{}` in variant at index {}",
                    val.option_id, val.attribute_id, idx
                )));
            }
            sorted_pairs.push((val.attribute_id, val.option_id));
        }

        sorted_pairs.sort_by_key(|(attr, _)| *attr);
        let combination_key: String = sorted_pairs
            .into_iter()
            .map(|(a, o)| format!("{a}:{o}"))
            .collect::<Vec<_>>()
            .join(";");

        if !seen_combinations.insert(combination_key.clone()) {
            return Err(CommerceError::Validation(format!(
                "Duplicate variant combination `{combination_key}` in product variants"
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn validate_variant_prices_accepts_valid_prices() {
        let prices = vec![
            PriceInput {
                currency_code: "USD".to_string(),
                channel_id: None,
                channel_slug: Some("web".to_string()),
                amount: Decimal::new(1000, 2),
                compare_at_amount: None,
            },
            PriceInput {
                currency_code: "EUR".to_string(),
                channel_id: None,
                channel_slug: Some("web".to_string()),
                amount: Decimal::new(950, 2),
                compare_at_amount: None,
            },
            PriceInput {
                currency_code: "usd".to_string(),
                channel_id: None,
                channel_slug: Some("pos".to_string()),
                amount: Decimal::new(1000, 2),
                compare_at_amount: None,
            },
        ];
        assert!(validate_variant_prices(&prices).is_ok());
    }

    #[test]
    fn validate_variant_prices_rejects_duplicate_currency_for_same_channel() {
        let prices = vec![
            PriceInput {
                currency_code: "USD".to_string(),
                channel_id: None,
                channel_slug: Some("web".to_string()),
                amount: Decimal::new(1000, 2),
                compare_at_amount: None,
            },
            PriceInput {
                currency_code: "usd".to_string(),
                channel_id: None,
                channel_slug: Some("web".to_string()),
                amount: Decimal::new(1200, 2),
                compare_at_amount: None,
            },
        ];
        let err = validate_variant_prices(&prices).unwrap_err();
        assert!(matches!(err, CommerceError::Validation(_)));
    }

    #[test]
    fn validate_variant_prices_rejects_invalid_currency_code_length() {
        let prices = vec![PriceInput {
            currency_code: "US".to_string(),
            channel_id: None,
            channel_slug: None,
            amount: Decimal::new(1000, 2),
            compare_at_amount: None,
        }];
        let err = validate_variant_prices(&prices).unwrap_err();
        assert!(matches!(err, CommerceError::Validation(_)));
    }
}

