use super::*;
use sea_orm::FromQueryResult;

#[derive(FromQueryResult)]
struct AxisRow {
    id: Uuid,
    product_id: Uuid,
    attribute_id: Uuid,
    position: i32,
    code: String,
    name: Option<String>,
}

#[derive(FromQueryResult)]
struct AxisValueRow {
    axis_id: Uuid,
    option_id: Uuid,
    position: i32,
    code: String,
    label: Option<String>,
}

#[derive(FromQueryResult)]
struct VariantAxisValueRow {
    variant_id: Uuid,
    attribute_id: Uuid,
    option_id: Uuid,
    attribute_code: String,
    label: Option<String>,
}

pub async fn load_product_variant_axes<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    product_id: Uuid,
    locale: &str,
) -> CommerceResult<Vec<VariantAxisConfigResponse>> {
    let axes = AxisRow::find_by_statement(Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
        SELECT ax.id, ax.product_id, ax.attribute_id, ax.position, a.code, pat.name
        FROM product_variant_axes ax
        JOIN product_attributes a ON a.id = ax.attribute_id AND a.tenant_id = ax.tenant_id
        LEFT JOIN product_attribute_translations pat ON pat.attribute_id = a.id AND pat.locale = $3
        WHERE ax.tenant_id = $1 AND ax.product_id = $2
        ORDER BY ax.position ASC
        "#,
        vec![tenant_id.into(), product_id.into(), locale.into()],
    ))
    .all(db)
    .await?;

    if axes.is_empty() {
        return Ok(Vec::new());
    }

    let values = AxisValueRow::find_by_statement(Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
        SELECT av.axis_id, av.option_id, av.position, pao.code, paot.label
        FROM product_variant_axis_values av
        JOIN product_variant_axes ax ON ax.id = av.axis_id AND ax.tenant_id = av.tenant_id
        JOIN product_attribute_options pao ON pao.id = av.option_id AND pao.tenant_id = av.tenant_id
        LEFT JOIN product_attribute_option_translations paot ON paot.option_id = pao.id AND paot.locale = $3
        WHERE av.tenant_id = $1 AND ax.product_id = $2
        ORDER BY av.position ASC
        "#,
        vec![tenant_id.into(), product_id.into(), locale.into()],
    ))
    .all(db)
    .await?;

    let mut values_by_axis: HashMap<Uuid, Vec<AxisAllowedValueResponse>> = HashMap::new();
    for val in values {
        let label = val.label.unwrap_or(val.code);
        values_by_axis
            .entry(val.axis_id)
            .or_default()
            .push(AxisAllowedValueResponse {
                option_id: val.option_id,
                value: label,
                position: val.position,
            });
    }

    Ok(axes
        .into_iter()
        .map(|ax| {
            let name = ax.name.unwrap_or_else(|| ax.code.clone());
            let allowed_values = values_by_axis.remove(&ax.id).unwrap_or_default();
            VariantAxisConfigResponse {
                id: ax.id,
                product_id: ax.product_id,
                attribute_id: ax.attribute_id,
                code: ax.code,
                name,
                position: ax.position,
                allowed_values,
            }
        })
        .collect())
}

pub async fn load_variant_axis_values<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    variant_ids: &[Uuid],
    locale: &str,
) -> CommerceResult<HashMap<Uuid, Vec<VariantAxisValueResponse>>> {
    if variant_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let placeholders = variant_ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("${}", i + 3))
        .collect::<Vec<_>>()
        .join(",");
    let query = format!(
        r#"
        SELECT pvav.variant_id, pvav.attribute_id, pvao.option_id, a.code as attribute_code, paot.label
        FROM product_variant_attribute_values pvav
        JOIN product_variant_attribute_value_options pvao ON pvao.tenant_id = pvav.tenant_id AND pvao.value_id = pvav.id
        JOIN product_attributes a ON a.id = pvav.attribute_id AND a.tenant_id = pvav.tenant_id
        JOIN product_attribute_options pao ON pao.id = pvao.option_id AND pao.tenant_id = pvao.tenant_id
        LEFT JOIN product_attribute_option_translations paot ON paot.option_id = pao.id AND paot.locale = $2
        WHERE pvav.tenant_id = $1 AND pvav.detached_at IS NULL AND pvav.variant_id IN ({placeholders})
        ORDER BY pvav.attribute_id ASC
        "#
    );

    let mut values = vec![tenant_id.into(), locale.into()];
    for id in variant_ids {
        values.push((*id).into());
    }

    let rows = VariantAxisValueRow::find_by_statement(Statement::from_sql_and_values(
        db.get_database_backend(),
        &query,
        values,
    ))
    .all(db)
    .await?;

    let mut result: HashMap<Uuid, Vec<VariantAxisValueResponse>> = HashMap::new();
    for row in rows {
        result
            .entry(row.variant_id)
            .or_default()
            .push(VariantAxisValueResponse {
                attribute_id: row.attribute_id,
                option_id: row.option_id,
                code: Some(row.attribute_code),
                label: row.label,
            });
    }

    Ok(result)
}

fn resolve_image_alt_text(
    translations: &[entities::product_image_translation::Model],
    locale: &str,
    fallback_locale: Option<&str>,
) -> Option<String> {
    let fallback_locale = fallback_locale.unwrap_or(PLATFORM_FALLBACK_LOCALE);
    let selected = translations
        .iter()
        .find(|translation| rustok_api::locale_tags_match(&translation.locale, locale))
        .or_else(|| {
            translations.iter().find(|translation| {
                rustok_api::locale_tags_match(&translation.locale, fallback_locale)
            })
        })
        .or_else(|| translations.first());

    selected.and_then(|translation| translation.alt_text.clone())
}

impl CatalogService {
    pub async fn get_product(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
    ) -> CommerceResult<ProductResponse> {
        self.get_product_with_locale_fallback(tenant_id, product_id, PLATFORM_FALLBACK_LOCALE, None)
            .await
    }

    pub async fn get_product_variant_axes(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: &str,
    ) -> CommerceResult<Vec<VariantAxisConfigResponse>> {
        load_product_variant_axes(&self.db, tenant_id, product_id, locale).await
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
        let (translations, variant_axes, variants, images, product_tags, resolved_metadata) = tokio::try_join!(
            async {
                Ok::<_, CommerceError>(
                    entities::product_translation::Entity::find()
                        .filter(entities::product_translation::Column::ProductId.eq(product_id))
                        .all(&self.db)
                        .await?,
                )
            },
            load_product_variant_axes(&self.db, tenant_id, product_id, locale),
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

        let variant_ids: Vec<Uuid> = variants.iter().map(|v| v.id).collect();
        let (all_prices, variant_translations, available_inventory_by_variant, mut axis_values_by_variant) = tokio::try_join!(
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
            load_variant_axis_values(&self.db, tenant_id, &variant_ids, locale),
        )?;

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
                    combination_identity: variant.combination_identity,
                    axis_values: axis_values_by_variant
                        .remove(&variant.id)
                        .unwrap_or_default(),
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
            variant_axes,
            variants: variant_responses,
            images: images
                .into_iter()
                .map(|image| {
                    let translations = image_translations_by_image
                        .remove(&image.id)
                        .unwrap_or_default();
                    let alt_text = resolve_image_alt_text(&translations, locale, fallback_locale);
                    ProductImageResponse {
                        id: image.id,
                        media_id: image.media_id,
                        url: format!("/api/v1/media/{}", image.media_id),
                        alt_text,
                        position: image.position,
                        translations: translations
                            .into_iter()
                            .map(|translation| ProductImageTranslationResponse {
                                locale: translation.locale,
                                alt_text: translation.alt_text,
                            })
                            .collect(),
                    }
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
    pub async fn get_variant(
        &self,
        tenant_id: Uuid,
        variant_id: Uuid,
    ) -> CommerceResult<VariantResponse> {
        let variant = entities::product_variant::Entity::find_by_id(variant_id)
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(CommerceError::VariantNotFound(variant_id))?;

        let variant_ids = [variant_id];
        let (prices, translations, available_inventory_by_variant, mut axis_values_by_variant) = tokio::try_join!(
            async {
                PricingBootstrapService::load_prices_for_variants(&self.db, &variant_ids)
                    .await
                    .map_err(CommerceError::from)
            },
            async {
                Ok::<_, CommerceError>(
                    entities::variant_translation::Entity::find()
                        .filter(entities::variant_translation::Column::VariantId.eq(variant_id))
                        .order_by_asc(entities::variant_translation::Column::Locale)
                        .all(&self.db)
                        .await?,
                )
            },
            async {
                BootstrapService::load_available_quantities(&self.db, &variant_ids)
                    .await
                    .map_err(CommerceError::from)
            },
            load_variant_axis_values(&self.db, tenant_id, &variant_ids, PLATFORM_FALLBACK_LOCALE),
        )?;

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

        Ok(VariantResponse {
            id: variant.id,
            product_id: variant.product_id,
            sku: variant.sku,
            barcode: variant.barcode,
            shipping_profile_slug: variant.shipping_profile_slug,
            title,
            translations: translations
                .into_iter()
                .map(|translation| VariantTranslationResponse {
                    locale: translation.locale,
                    title: translation.title,
                })
                .collect(),
            combination_identity: variant.combination_identity,
            axis_values: axis_values_by_variant
                .remove(&variant_id)
                .unwrap_or_default(),
            prices: price_responses,
            inventory_quantity: available_inventory,
            inventory_policy: variant.inventory_policy.clone(),
            in_stock: available_inventory > 0 || variant.inventory_policy == "continue",
            weight: variant.weight,
            weight_unit: variant.weight_unit,
            position: variant.position,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image_translation(
        image_id: Uuid,
        locale: &str,
        alt_text: Option<&str>,
    ) -> entities::product_image_translation::Model {
        entities::product_image_translation::Model {
            id: Uuid::new_v4(),
            image_id,
            locale: locale.to_owned(),
            alt_text: alt_text.map(str::to_owned),
        }
    }

    #[test]
    fn image_alt_text_prefers_requested_locale_then_fallback() {
        let image_id = Uuid::new_v4();
        let translations = vec![
            image_translation(image_id, "en", Some("English alt")),
            image_translation(image_id, "fr", Some("Texte alternatif")),
        ];

        assert_eq!(
            resolve_image_alt_text(&translations, "fr", Some("en")).as_deref(),
            Some("Texte alternatif")
        );
        assert_eq!(
            resolve_image_alt_text(&translations, "de", Some("en")).as_deref(),
            Some("English alt")
        );
        assert_eq!(
            resolve_image_alt_text(&translations, "de", None).as_deref(),
            Some("English alt")
        );
    }

    #[test]
    fn image_alt_text_preserves_explicit_null_for_requested_locale() {
        let image_id = Uuid::new_v4();
        let translations = vec![
            image_translation(image_id, "en", Some("English alt")),
            image_translation(image_id, "fr", None),
        ];

        assert_eq!(
            resolve_image_alt_text(&translations, "fr", Some("en")),
            None
        );
    }
}
