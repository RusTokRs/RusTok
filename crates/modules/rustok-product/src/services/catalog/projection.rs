use super::*;

impl CatalogService {
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
                    PricingBootstrapService::load_prices_for_variants(&self.db, &variant_ids)
                        .await
                        .map_err(CommerceError::from)
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
            async {
                BootstrapService::load_available_quantities(&self.db, &variant_ids)
                    .await
                    .map_err(CommerceError::from)
            },
        )?;

        // Group prices by variant_id
        let mut prices_by_variant: HashMap<
            Uuid,
            Vec<rustok_pricing_persistence::entities::price::Model>,
        > = HashMap::new();
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
}
