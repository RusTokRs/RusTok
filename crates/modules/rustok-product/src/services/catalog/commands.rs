use super::*;
use sea_orm::{DatabaseTransaction, FromQueryResult};

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

        for (position, axis_input) in input.variant_axes.iter().enumerate() {
            let axis_id = generate_id();
            let axis = entities::product_variant_axis::ActiveModel {
                id: Set(axis_id),
                tenant_id: Set(tenant_id),
                product_id: Set(product_id),
                attribute_id: Set(axis_input.attribute_id),
                position: Set(if axis_input.position != 0 { axis_input.position } else { position as i32 }),
                created_at: Set(now.into()),
            };
            axis.insert(&txn).await?;

            for (val_pos, option_id) in axis_input.allowed_option_ids.iter().enumerate() {
                let axis_val = entities::product_variant_axis_value::ActiveModel {
                    id: Set(generate_id()),
                    tenant_id: Set(tenant_id),
                    axis_id: Set(axis_id),
                    option_id: Set(*option_id),
                    position: Set(val_pos as i32),
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
                combination_identity: Set(None),
                position: Set(position as i32),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            };
            variant.insert(&txn).await.map_err(|error| {
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

            let variant_title = "Default".to_string();
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
                }
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

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;

        let _product = entities::product::Entity::find_by_id(product_id)
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(CommerceError::ProductNotFound(product_id))?;

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
            .all(&txn)
            .await?;
        let next_position = existing_variants
            .iter()
            .map(|v| v.position)
            .max()
            .map_or(0, |max_pos| max_pos + 1);

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
            combination_identity: Set(None),
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
                currency_code: price_input.currency_code.clone(),
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

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;

        let variant = entities::product_variant::Entity::find_by_id(variant_id)
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(CommerceError::VariantNotFound(variant_id))?;

        let product_id = variant.product_id;
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
            let configured_axis_attribute_ids: Vec<Uuid> =
                entities::product_variant_axis::Entity::find()
                    .filter(entities::product_variant_axis::Column::ProductId.eq(product_id))
                    .filter(entities::product_variant_axis::Column::TenantId.eq(tenant_id))
                    .all(&txn)
                    .await?
                    .into_iter()
                    .map(|a| a.attribute_id)
                    .collect();

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
                .await
                .unwrap_or_default();
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
                    currency_code: price_input.currency_code.clone(),
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

        let variant = entities::product_variant::Entity::find_by_id(variant_id)
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(CommerceError::VariantNotFound(variant_id))?;

        let product_id = variant.product_id;

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

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;

        let _product = entities::product::Entity::find_by_id(product_id)
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(CommerceError::ProductNotFound(product_id))?;

        let position = match input.position {
            Some(pos) => pos,
            None => {
                let max_pos: Option<i32> = entities::product_image::Entity::find()
                    .filter(entities::product_image::Column::ProductId.eq(product_id))
                    .order_by_desc(entities::product_image::Column::Position)
                    .one(&txn)
                    .await?
                    .map(|img| img.position + 1);
                max_pos.unwrap_or(0)
            }
        };

        let image_id = generate_id();
        let _image = entities::product_image::ActiveModel {
            id: Set(image_id),
            product_id: Set(product_id),
            media_id: Set(input.media_id),
            position: Set(position),
        }
        .insert(&txn)
        .await?;

        let mut translations = Vec::new();
        if let Some(alt_text) = input.alt_text.as_deref() {
            let locale = input
                .locale
                .clone()
                .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
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
            url: format!("/api/v1/media/{}", input.media_id),
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

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;

        let _product = entities::product::Entity::find_by_id(product_id)
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(CommerceError::ProductNotFound(product_id))?;

        let image = entities::product_image::Entity::find_by_id(image_id)
            .filter(entities::product_image::Column::ProductId.eq(product_id))
            .one(&txn)
            .await?
            .ok_or(CommerceError::ImageNotFound(image_id))?;

        let mut active_image: entities::product_image::ActiveModel = image.clone().into();
        let mut position = image.position;
        if let Some(new_pos) = input.position {
            active_image.position = Set(new_pos);
            position = new_pos;
        }
        let updated_image = active_image.update(&txn).await?;

        if let Some(alt_text) = input.alt_text.as_deref() {
            let locale = input
                .locale
                .clone()
                .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
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
            url: format!("/api/v1/media/{}", updated_image.media_id),
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

        let _product = entities::product::Entity::find_by_id(product_id)
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(CommerceError::ProductNotFound(product_id))?;

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

        let _product = entities::product::Entity::find_by_id(product_id)
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(CommerceError::ProductNotFound(product_id))?;

        for (pos, image_id) in image_ids.into_iter().enumerate() {
            let image = entities::product_image::Entity::find_by_id(image_id)
                .filter(entities::product_image::Column::ProductId.eq(product_id))
                .one(&txn)
                .await?;
            if let Some(image) = image {
                let mut active: entities::product_image::ActiveModel = image.into();
                active.position = Set(pos as i32);
                active.update(&txn).await?;
            }
        }

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

        let _product = entities::product::Entity::find_by_id(product_id)
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(CommerceError::ProductNotFound(product_id))?;

        let existing_variants = entities::product_variant::Entity::find()
            .filter(entities::product_variant::Column::ProductId.eq(product_id))
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .all(&txn)
            .await?;

        if input.axes.is_empty() && existing_variants.len() > 1 {
            return Err(CommerceError::Validation(
                "Cannot remove all variant axes from a product with multiple variants. Consolidate to a single variant first.".into(),
            ));
        }

        let mut seen_attrs = HashSet::new();
        for axis in &input.axes {
            if !seen_attrs.insert(axis.attribute_id) {
                return Err(CommerceError::Validation(format!(
                    "Duplicate attribute_id `{}` in variant axes",
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

        let existing_axis_ids: Vec<Uuid> = entities::product_variant_axis::Entity::find()
            .filter(entities::product_variant_axis::Column::ProductId.eq(product_id))
            .filter(entities::product_variant_axis::Column::TenantId.eq(tenant_id))
            .all(&txn)
            .await?
            .into_iter()
            .map(|a| a.id)
            .collect();

        if !existing_axis_ids.is_empty() {
            entities::product_variant_axis_value::Entity::delete_many()
                .filter(entities::product_variant_axis_value::Column::AxisId.is_in(existing_axis_ids.clone()))
                .exec(&txn)
                .await?;

            entities::product_variant_axis::Entity::delete_many()
                .filter(entities::product_variant_axis::Column::Id.is_in(existing_axis_ids))
                .exec(&txn)
                .await?;
        }

        let now = Utc::now();
        for (pos, axis_input) in input.axes.iter().enumerate() {
            let axis_id = generate_id();
            let axis = entities::product_variant_axis::ActiveModel {
                id: Set(axis_id),
                tenant_id: Set(tenant_id),
                product_id: Set(product_id),
                attribute_id: Set(axis_input.attribute_id),
                position: Set(if axis_input.position != 0 { axis_input.position } else { pos as i32 }),
                created_at: Set(now.into()),
            };
            axis.insert(&txn).await?;

            for (val_pos, option_id) in axis_input.allowed_option_ids.iter().enumerate() {
                let axis_val = entities::product_variant_axis_value::ActiveModel {
                    id: Set(generate_id()),
                    tenant_id: Set(tenant_id),
                    axis_id: Set(axis_id),
                    option_id: Set(*option_id),
                    position: Set(val_pos as i32),
                    created_at: Set(now.into()),
                };
                axis_val.insert(&txn).await?;
            }
        }

        txn.publish(
            tenant_id,
            Some(actor_id),
            DomainEvent::ProductUpdated { product_id },
        )
        .await?;

        txn.commit().await?;

        self.get_product_variant_axes(tenant_id, product_id, PLATFORM_FALLBACK_LOCALE).await
    }
}

#[derive(sea_orm::FromQueryResult)]
struct IdRow {
    id: Uuid,
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
            None => {
                IdRow::find_by_statement(Statement::from_sql_and_values(
                    txn.get_database_backend(),
                    "SELECT id FROM product_variant_attribute_values WHERE tenant_id = $1 AND variant_id = $2 AND attribute_id = $3",
                    vec![tenant_id.into(), variant_id.into(), val.attribute_id.into()],
                ))
                .one(txn)
                .await?
                .map(|r| r.id)
                .unwrap_or_else(generate_id)
            }
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

    let combination_identity = if axis_values.is_empty() {
        None
    } else {
        let mut sorted = axis_values.to_vec();
        sorted.sort_by_key(|v| v.attribute_id);
        Some(
            sorted
                .iter()
                .map(|v| format!("{}:{}", v.attribute_id, v.option_id))
                .collect::<Vec<_>>()
                .join(";"),
        )
    };
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

