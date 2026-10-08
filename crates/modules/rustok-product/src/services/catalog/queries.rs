use super::*;

impl CatalogService {
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
        self.list_published_products_with_query(
            tenant_id,
            locale,
            fallback_locale,
            public_channel_slug,
            StorefrontProductListQuery::default().with_pagination(page, per_page),
        )
        .await
    }

    /// Keyset scan of published product ids for batch readers (SEO bulk): same base
    /// filters as the storefront list, ordered by `id`. Returns the cursor of the last
    /// id when more rows follow.
    pub(crate) async fn scan_published_product_ids(
        &self,
        tenant_id: Uuid,
        public_channel_slug: Option<&str>,
        after: Option<Uuid>,
        limit: u64,
    ) -> CommerceResult<(Vec<Uuid>, Option<Uuid>)> {
        let limit = limit.max(1);
        let mut query = entities::product::Entity::find()
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .filter(entities::product::Column::Status.eq(entities::product::ProductStatus::Active))
            .filter(entities::product::Column::PublishedAt.is_not_null())
            .filter(product_channel_visibility_condition(
                self.db.get_database_backend(),
                public_channel_slug,
            ));
        if let Some(after) = after {
            query = query.filter(entities::product::Column::Id.gt(after));
        }
        let mut rows = query
            .order_by_asc(entities::product::Column::Id)
            .limit(limit + 1)
            .all(&self.db)
            .await?;
        let has_next_page = rows.len() as u64 > limit;
        rows.truncate(limit as usize);
        let next_after = if has_next_page {
            rows.last().map(|product| product.id)
        } else {
            None
        };
        Ok((rows.into_iter().map(|product| product.id).collect(), next_after))
    }

    #[instrument(skip(self))]
    pub async fn list_published_products_with_query(
        &self,
        tenant_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        public_channel_slug: Option<&str>,
        list_query: StorefrontProductListQuery,
    ) -> CommerceResult<StorefrontProductList> {
        let fallback_locale = fallback_locale.unwrap_or(PLATFORM_FALLBACK_LOCALE);
        let page = list_query.page;
        let per_page = list_query.per_page;
        if page == 0 || per_page == 0 || per_page > 48 {
            return Err(CommerceError::Validation(
                "page must be at least 1 and per_page must be between 1 and 48".to_owned(),
            ));
        }
        types::validate_storefront_product_search(list_query.search.as_deref())?;
        let offset = page
            .checked_sub(1)
            .and_then(|page| page.checked_mul(per_page))
            .ok_or_else(|| {
                CommerceError::Validation("page and per_page are too large".to_owned())
            })?;

        let mut query = entities::product::Entity::find()
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .filter(entities::product::Column::Status.eq(entities::product::ProductStatus::Active))
            .filter(entities::product::Column::PublishedAt.is_not_null())
            .filter(product_channel_visibility_condition(
                self.db.get_database_backend(),
                public_channel_slug,
            ));
        if let Some(category_id) = list_query.category_id {
            query = query.filter(entities::product::Column::PrimaryCategoryId.eq(category_id));
        }
        if let Some(search) = list_query
            .search
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            query = query.filter(product_title_search_condition(
                self.db.get_database_backend(),
                search,
            ));
        }
        for condition in attribute_filters::load_catalog_attribute_filter_conditions(
            &self.db,
            tenant_id,
            locale,
            fallback_locale,
            list_query.attribute_filters.as_slice(),
        )
        .await?
        {
            query = query.filter(condition);
        }
        let total = query.clone().count(&self.db).await?;
        let query = match (list_query.sort_by, list_query.sort_direction) {
            (StorefrontProductSortBy::PublishedAt, StorefrontProductSortDirection::Asc) => query
                .order_by_asc(entities::product::Column::PublishedAt)
                .order_by_asc(entities::product::Column::CreatedAt)
                .order_by_asc(entities::product::Column::Id),
            (StorefrontProductSortBy::PublishedAt, StorefrontProductSortDirection::Desc) => query
                .order_by_desc(entities::product::Column::PublishedAt)
                .order_by_desc(entities::product::Column::CreatedAt)
                .order_by_desc(entities::product::Column::Id),
            (StorefrontProductSortBy::CreatedAt, StorefrontProductSortDirection::Asc) => query
                .order_by_asc(entities::product::Column::CreatedAt)
                .order_by_asc(entities::product::Column::PublishedAt)
                .order_by_asc(entities::product::Column::Id),
            (StorefrontProductSortBy::CreatedAt, StorefrontProductSortDirection::Desc) => query
                .order_by_desc(entities::product::Column::CreatedAt)
                .order_by_desc(entities::product::Column::PublishedAt)
                .order_by_desc(entities::product::Column::Id),
        };
        let products = query.offset(offset).limit(per_page).all(&self.db).await?;
        let product_ids = products
            .iter()
            .map(|product| product.id)
            .collect::<Vec<_>>();

        let translations = if product_ids.is_empty() {
            Vec::new()
        } else {
            entities::product_translation::Entity::find()
                .filter(entities::product_translation::Column::TenantId.eq(tenant_id))
                .filter(entities::product_translation::Column::ProductId.is_in(product_ids.clone()))
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
        let media_summaries = self
            .load_storefront_product_list_media(product_ids.as_slice(), locale, fallback_locale)
            .await?;
        let variants = if product_ids.is_empty() {
            Vec::new()
        } else {
            entities::product_variant::Entity::find()
                .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
                .filter(entities::product_variant::Column::ProductId.is_in(product_ids.clone()))
                .all(&self.db)
                .await?
        };
        let variant_ids = variants
            .iter()
            .map(|variant| variant.id)
            .collect::<Vec<_>>();
        let prices = if variant_ids.is_empty() {
            Vec::new()
        } else {
            rustok_pricing_persistence::entities::price::Entity::find()
                .filter(
                    rustok_pricing_persistence::entities::price::Column::VariantId
                        .is_in(variant_ids.clone()),
                )
                .all(&self.db)
                .await?
        };
        let mut variant_to_product = HashMap::<Uuid, Uuid>::new();
        for variant in &variants {
            variant_to_product.insert(variant.id, variant.product_id);
        }
        let price_from_by_product = build_storefront_list_price_from_map(
            &variant_to_product,
            prices.as_slice(),
            list_query.currency_code.as_deref(),
            public_channel_slug,
        );

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
                    primary_image: media_summaries.get(&product.id).cloned(),
                    price_from: price_from_by_product.get(&product.id).cloned(),
                    created_at: product.created_at.into(),
                    published_at: product.published_at.map(Into::into),
                }
            })
            .collect::<Vec<_>>();

        let has_next = offset
            .checked_add(items.len() as u64)
            .is_some_and(|through| through < total);

        Ok(StorefrontProductList {
            items,
            total,
            page,
            per_page,
            has_next,
        })
    }

    #[instrument(skip(self))]
    pub(crate) async fn list_legacy_storefront_products_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        public_channel_slug: Option<&str>,
        vendor: Option<&str>,
        product_type: Option<&str>,
        search: Option<&str>,
        page: u64,
        per_page: u64,
    ) -> CommerceResult<crate::LegacyStorefrontProductList> {
        let fallback_locale = fallback_locale.unwrap_or(PLATFORM_FALLBACK_LOCALE);
        if page == 0 || per_page == 0 || per_page > 48 {
            return Err(CommerceError::Validation(
                "page must be at least 1 and per_page must be between 1 and 48".to_owned(),
            ));
        }
        let offset = page
            .checked_sub(1)
            .and_then(|page| page.checked_mul(per_page))
            .ok_or_else(|| {
                CommerceError::Validation("page and per_page are too large".to_owned())
            })?;

        let mut query = entities::product::Entity::find()
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .filter(entities::product::Column::Status.eq(entities::product::ProductStatus::Active))
            .filter(entities::product::Column::PublishedAt.is_not_null())
            .filter(product_channel_visibility_condition(
                self.db.get_database_backend(),
                public_channel_slug,
            ));
        if let Some(vendor) = vendor {
            query = query.filter(entities::product::Column::Vendor.eq(vendor));
        }
        if let Some(product_type) = product_type {
            query = query.filter(entities::product::Column::ProductType.eq(product_type));
        }
        if let Some(search) = search {
            query = query.filter(product_title_search_condition(
                self.db.get_database_backend(),
                search,
            ));
        }

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
                .filter(entities::product_translation::Column::TenantId.eq(tenant_id))
                .filter(entities::product_translation::Column::ProductId.is_in(product_ids.clone()))
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
                let shipping_profile_slug = product
                    .shipping_profile_slug
                    .as_deref()
                    .and_then(normalize_shipping_profile_slug)
                    .or_else(|| extract_shipping_profile_slug(&product.metadata))
                    .unwrap_or_else(|| "default".to_string());
                crate::LegacyStorefrontProductListItem {
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
                    shipping_profile_slug,
                    tags: product_tags.get(&product.id).cloned().unwrap_or_default(),
                    created_at: product.created_at.into(),
                    published_at: product.published_at.map(Into::into),
                }
            })
            .collect::<Vec<_>>();

        let has_next = offset
            .checked_add(items.len() as u64)
            .is_some_and(|through| through < total);

        Ok(crate::LegacyStorefrontProductList {
            items,
            total,
            page,
            per_page,
            has_next,
        })
    }

    #[instrument(skip(self))]
    pub async fn get_published_product_by_id_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        public_channel_slug: Option<&str>,
    ) -> CommerceResult<Option<ProductResponse>> {
        let fallback_locale = fallback_locale.unwrap_or(PLATFORM_FALLBACK_LOCALE);
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

        self.get_published_product_by_id_with_locale_fallback(
            tenant_id,
            product_id,
            locale,
            Some(fallback_locale),
            public_channel_slug,
        )
        .await
    }
}

pub(super) fn product_title_search_condition(
    backend: sea_orm::DbBackend,
    search: &str,
) -> sea_orm::Condition {
    let pattern = format!("%{search}%");
    let exists_sql = match backend {
        sea_orm::DbBackend::Sqlite => {
            "EXISTS (
                SELECT 1
                FROM product_translations pt
                WHERE pt.product_id = products.id
                  AND pt.title LIKE ?
            )"
        }
        _ => {
            "EXISTS (
                SELECT 1
                FROM product_translations pt
                WHERE pt.product_id = products.id
                  AND pt.title LIKE $1
            )"
        }
    };

    sea_orm::Condition::all().add(sea_orm::sea_query::Expr::cust_with_values(
        exists_sql,
        vec![sea_orm::Value::from(pattern)],
    ))
}

impl CatalogService {
    /// Loads the lowest-position image of every listed product together with
    /// its locale-resolved alt text.
    ///
    /// The storefront list contract owns this projection so catalog cards can
    /// render real media instead of placeholders. One batched image query and
    /// one batched translation query cover the whole page, so the page cost does
    /// not grow with `per_page`.
    async fn load_storefront_product_list_media(
        &self,
        product_ids: &[Uuid],
        locale: &str,
        fallback_locale: &str,
    ) -> CommerceResult<HashMap<Uuid, StorefrontProductListImage>> {
        if product_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let images = entities::product_image::Entity::find()
            .filter(entities::product_image::Column::ProductId.is_in(product_ids.to_vec()))
            .order_by_asc(entities::product_image::Column::Position)
            .order_by_asc(entities::product_image::Column::Id)
            .all(&self.db)
            .await?;

        let mut primary_by_product = HashMap::<Uuid, entities::product_image::Model>::new();
        for image in images {
            primary_by_product.entry(image.product_id).or_insert(image);
        }
        let image_ids = primary_by_product
            .values()
            .map(|image| image.id)
            .collect::<Vec<_>>();
        let translations = if image_ids.is_empty() {
            Vec::new()
        } else {
            entities::product_image_translation::Entity::find()
                .filter(
                    entities::product_image_translation::Column::ImageId.is_in(image_ids.clone()),
                )
                .all(&self.db)
                .await?
        };
        let mut translations_by_image: HashMap<
            Uuid,
            Vec<entities::product_image_translation::Model>,
        > = HashMap::new();
        for translation in translations {
            translations_by_image
                .entry(translation.image_id)
                .or_default()
                .push(translation);
        }

        Ok(primary_by_product
            .into_iter()
            .map(|(product_id, image)| {
                let alt_text = translations_by_image
                    .get(&image.id)
                    .map(|translations| {
                        projection::resolve_image_alt_text(
                            translations.as_slice(),
                            locale,
                            Some(fallback_locale),
                        )
                    })
                    .unwrap_or(None);
                (
                    product_id,
                    StorefrontProductListImage {
                        media_id: image.media_id,
                        url: helpers::format_product_media_url(image.media_id),
                        alt_text,
                        position: image.position,
                    },
                )
            })
            .collect())
    }
}

/// Derives the storefront "from" price of every listed product from its base
/// variant prices.
///
/// Only base rows participate: price-list rows are resolved by the pricing
/// owner against a price-list/channel/quantity context, and quantity tiers
/// (`min_quantity`) are intentionally excluded from a catalog snapshot. Channel
/// scoping is honoured by preferring rows of the requested public channel and
/// falling back to channel-neutral rows, then to any channel when the tenant
/// only maintains channel-scoped prices.
fn build_storefront_list_price_from_map(
    variant_to_product: &HashMap<Uuid, Uuid>,
    prices: &[rustok_pricing_persistence::entities::price::Model],
    currency_code: Option<&str>,
    public_channel_slug: Option<&str>,
) -> HashMap<Uuid, StorefrontProductListPrice> {
    let mut candidates_by_product =
        HashMap::<Uuid, Vec<&rustok_pricing_persistence::entities::price::Model>>::new();
    for price in prices {
        if price.price_list_id.is_some() || price.min_quantity.is_some() {
            continue;
        }
        if let Some(currency_code) = currency_code {
            if !price.currency_code.eq_ignore_ascii_case(currency_code) {
                continue;
            }
        }
        let Some(product_id) = variant_to_product.get(&price.variant_id) else {
            continue;
        };
        candidates_by_product
            .entry(*product_id)
            .or_default()
            .push(price);
    }

    candidates_by_product
        .into_iter()
        .filter_map(|(product_id, candidates)| {
            let selected = select_storefront_list_price_candidates(candidates, public_channel_slug);
            selected
                .into_iter()
                .min_by(|left, right| left.amount.cmp(&right.amount))
                .map(|price| {
                    let on_sale = price
                        .compare_at_amount
                        .is_some_and(|compare_at| compare_at > price.amount);
                    (
                        product_id,
                        StorefrontProductListPrice {
                            currency_code: price.currency_code.to_ascii_uppercase(),
                            amount: price.amount,
                            compare_at_amount: price.compare_at_amount,
                            on_sale,
                        },
                    )
                })
        })
        .collect()
}

fn select_storefront_list_price_candidates<'a>(
    candidates: Vec<&'a rustok_pricing_persistence::entities::price::Model>,
    public_channel_slug: Option<&str>,
) -> Vec<&'a rustok_pricing_persistence::entities::price::Model> {
    if let Some(channel_slug) = public_channel_slug {
        let channel_scoped = candidates
            .iter()
            .copied()
            .filter(|price| {
                price
                    .channel_slug
                    .as_deref()
                    .is_some_and(|slug| slug.eq_ignore_ascii_case(channel_slug))
            })
            .collect::<Vec<_>>();
        if !channel_scoped.is_empty() {
            return channel_scoped;
        }
    }

    let channel_neutral = candidates
        .iter()
        .copied()
        .filter(|price| price.channel_slug.is_none())
        .collect::<Vec<_>>();
    if !channel_neutral.is_empty() {
        return channel_neutral;
    }

    candidates
}
