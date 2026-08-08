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
        let offset = (page.saturating_sub(1)) * per_page;

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
        let offset = (page.saturating_sub(1)) * per_page;

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

        Ok(crate::LegacyStorefrontProductList {
            items,
            total,
            page,
            per_page,
            has_next: page * per_page < total,
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

fn product_title_search_condition(backend: sea_orm::DbBackend, search: &str) -> sea_orm::Condition {
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
