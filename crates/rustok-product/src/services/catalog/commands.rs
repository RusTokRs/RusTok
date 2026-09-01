use super::*;

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
        let mut initial_prices = Vec::new();
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
                initial_prices.push(InitialPrice {
                    variant_id,
                    channel_id: price_input.channel_id,
                    channel_slug: normalize_public_channel_slug(
                        price_input.channel_slug.as_deref(),
                    ),
                    currency_code: price_input.currency_code.clone(),
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

        if let Some(prepared_custom_fields) = prepared_custom_fields.as_ref()
            && let (Some(locale), Some(values)) = (
                prepared_custom_fields.locale.as_deref(),
                prepared_custom_fields.localized_values.as_ref(),
            )
        {
            flex::persist_localized_values(&txn, tenant_id, "product", product_id, locale, values)
                .await
                .map_err(|error| CommerceError::Validation(error.to_string()))?;
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
            .query_one_raw(Statement::from_sql_and_values(
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
