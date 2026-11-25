#[path = "variant_axes.rs"]
pub(super) mod variant_axes;

use super::attached_localized_storage;
use super::concurrency::{
    ensure_expected_revision, ensure_expected_variant_revision, next_revision,
    validate_patch_text_length,
};
use super::*;
use crate::services::catalog_schema_service::policy::lock_schema_for_product_write;
use rustok_api::{Patch, TenantLocale};
use rustok_core::error::Error as CoreError;
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseTransaction, FromQueryResult};

pub(super) async fn find_product_for_update_in_tx(
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

fn validate_product_variant_count(
    current_count: usize,
    incoming_count: usize,
) -> CommerceResult<()> {
    let total = current_count.checked_add(incoming_count).ok_or_else(|| {
        CommerceError::Validation(
            "Product variant count exceeds the supported limit".to_owned(),
        )
    })?;
    if total > crate::dto::MAX_PRODUCT_VARIANTS {
        return Err(CommerceError::Validation(format!(
            "A Product can contain at most {} variants",
            crate::dto::MAX_PRODUCT_VARIANTS
        )));
    }
    Ok(())
}

fn validate_variant_inventory_quantity(quantity: i32) -> CommerceResult<()> {
    if quantity < 0 {
        return Err(CommerceError::Validation(
            "Inventory quantity cannot be negative".to_owned(),
        ));
    }
    Ok(())
}

fn normalize_variant_inventory_policy(value: &str) -> CommerceResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if !matches!(normalized.as_str(), "deny" | "continue") {
        return Err(CommerceError::Validation(
            "Inventory policy must be either `deny` or `continue`".to_owned(),
        ));
    }
    Ok(normalized)
}

fn validate_variant_prices(prices: &[PriceInput]) -> CommerceResult<()> {
    let mut seen = HashSet::new();
    for price in prices {
        let currency = price.currency_code.trim().to_ascii_uppercase();
        if currency.len() != 3 || !currency.bytes().all(|byte| byte.is_ascii_uppercase()) {
            return Err(CommerceError::Validation(format!(
                "Currency code `{}` must contain exactly 3 ASCII letters (ISO 4217)",
                price.currency_code
            )));
        }
        if price.channel_id.is_some_and(|channel_id| channel_id.is_nil()) {
            return Err(CommerceError::Validation(
                "Price channel ID must not be nil".to_owned(),
            ));
        }
        if price.amount < rust_decimal::Decimal::ZERO
            || price
                .compare_at_amount
                .is_some_and(|amount| amount < rust_decimal::Decimal::ZERO)
        {
            return Err(CommerceError::Validation(
                "Variant prices and compare-at amounts cannot be negative".to_owned(),
            ));
        }
        if price
            .compare_at_amount
            .is_some_and(|compare_at| compare_at < price.amount)
        {
            return Err(CommerceError::Validation(
                "Compare-at price cannot be lower than the base price".to_owned(),
            ));
        }
        // The Pricing resolver requires exact currency minor units.
        for amount in [Some(price.amount), price.compare_at_amount].into_iter().flatten() {
            rustok_core::money::to_minor_units_exact(amount, &currency).map_err(|_| {
                CommerceError::Validation(
                    "Variant prices must be exactly representable in their currency minor units".into(),
                )
            })?;
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

fn validate_variant_weight(weight: Option<&rust_decimal::Decimal>) -> CommerceResult<()> {
    if weight.is_some_and(|value| *value < rust_decimal::Decimal::ZERO) {
        return Err(CommerceError::Validation(
            "Product variant weight cannot be negative".to_owned(),
        ));
    }
    Ok(())
}

fn pricing_port_unavailable() -> CommerceError {
    CommerceError::Core(CoreError::External(
        "Product Pricing capability is unavailable".to_owned(),
    ))
}

fn validate_product_brands(brands: &[ProductBrandInput]) -> CommerceResult<()> {
    let mut seen = HashSet::new();
    let mut primary_count = 0;
    for brand in brands {
        if brand.brand_id.is_nil() {
            return Err(CommerceError::Validation(
                "Brand ID cannot be nil".to_string(),
            ));
        }
        if !seen.insert(brand.brand_id) {
            return Err(CommerceError::Validation(format!(
                "Duplicate brand ID `{}` in product brands",
                brand.brand_id
            )));
        }
        if brand.is_primary {
            primary_count += 1;
        }
    }
    if brands.len() > 1 && primary_count == 0 {
        return Err(CommerceError::Validation(
            "A product with multiple associated brands must specify exactly one primary brand"
                .to_string(),
        ));
    }
    if primary_count > 1 {
        return Err(CommerceError::Validation(
            "A product cannot have multiple primary brands".to_string(),
        ));
    }
    Ok(())
}

async fn sync_product_brands_in_tx<C: ConnectionTrait>(
    conn: &C,
    tenant_id: Uuid,
    product_id: Uuid,
    brand_inputs: &[ProductBrandInput],
) -> CommerceResult<()> {
    validate_product_brands(brand_inputs)?;

    entities::product_brand::Entity::delete_many()
        .filter(entities::product_brand::Column::TenantId.eq(tenant_id))
        .filter(entities::product_brand::Column::ProductId.eq(product_id))
        .exec(conn)
        .await?;

    if brand_inputs.is_empty() {
        return Ok(());
    }

    let default_single_primary = brand_inputs.len() == 1 && !brand_inputs[0].is_primary;
    let now = Utc::now();
    let mut models = Vec::with_capacity(brand_inputs.len());

    for (pos, brand_input) in brand_inputs.iter().enumerate() {
        let is_primary = if default_single_primary {
            true
        } else {
            brand_input.is_primary
        };
        let position = if brand_input.position != 0 {
            brand_input.position
        } else {
            pos as i32
        };

        models.push(entities::product_brand::ActiveModel {
            id: Set(generate_id()),
            tenant_id: Set(tenant_id),
            product_id: Set(product_id),
            brand_id: Set(brand_input.brand_id),
            is_primary: Set(is_primary),
            role: Set(if brand_input.role.is_empty() {
                "manufacturer".to_string()
            } else {
                brand_input.role.clone()
            }),
            position: Set(position),
            created_at: Set(now.into()),
        });
    }

    entities::product_brand::Entity::insert_many(models)
        .exec(conn)
        .await?;

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
        validate_product_brands(&input.brands)?;
        self.validate_shipping_profiles(
            tenant_id,
            actor_id,
            input
                .shipping_profile_slug
                .iter()
                .map(String::as_str)
                .chain(
                    input
                        .variants
                        .iter()
                        .filter_map(|variant| variant.shipping_profile_slug.as_deref()),
                ),
        )
        .await?;

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
            validate_variant_inventory_quantity(var_input.inventory_quantity)?;
            normalize_variant_inventory_policy(&var_input.inventory_policy)?;
            validate_variant_prices(&var_input.prices)?;
            validate_variant_weight(var_input.weight.as_ref())?;
        }
        if input
            .variants
            .iter()
            .any(|variant| !variant.prices.is_empty())
            && self.pricing_port.is_none()
        {
            return Err(pricing_port_unavailable());
        }
        let product_id = current_product_operation_id().unwrap_or_else(generate_id);
        let now = Utc::now();
        debug!(product_id = %product_id, "Generated product ID");

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
        lock_schema_for_product_write(&txn, tenant_id).await?;
        // Validate the category against the owner transaction, not a pre-tx snapshot:
        // a category deleted between check and insert must surface as this typed
        // refusal instead of a raw FK failure.
        validate_primary_category(&txn, tenant_id, input.primary_category_id).await?;
        let preferred_locale = preferred_product_locale_from_translations(&input.translations);
        let (prepared_custom_fields, custom_fields_schema) =
            prepare_product_custom_fields_and_schema_for_create(
                &txn,
                self.flex_schema_source,
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

        validate_variant_axis_configuration_in(
            &txn,
            tenant_id,
            input.primary_category_id,
            input.variant_axes.as_slice(),
        )
        .await?;

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
            revision: Set(1),
        };
        product.insert(&txn).await?;
        debug!("Product entity inserted");

        if let (Some(locale), Some(values)) = (
            prepared_custom_fields.locale.as_deref(),
            prepared_custom_fields.localized_values.as_ref(),
        ) {
            attached_localized_storage::replace_for_locale(
                &txn, tenant_id, product_id, locale, values,
            )
            .await?;
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

            let handle =
                product_translation_handle(trans_input.handle.as_deref(), &trans_input.title);

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

        let mut initial_prices = Vec::new();
        let mut initial_inventories = Vec::new();
        let mut created_variant_ids = Vec::new();
        for (position, var_input) in input.variants.iter().enumerate() {
            let variant_position = i32::try_from(position)
                .map_err(|_| CommerceError::Validation("too many Product variants".to_owned()))?;
            let variant_id = generate_id();
            created_variant_ids.push(variant_id);
            let normalized_sku =
                super::helpers::normalize_variant_sku(var_input.sku.as_deref());

            let variant = entities::product_variant::ActiveModel {
                id: Set(variant_id),
                product_id: Set(product_id),
                tenant_id: Set(tenant_id),
                sku: Set(normalized_sku.clone()),
                barcode: Set(var_input.barcode.clone()),
                shipping_profile_slug: Set(var_input
                    .shipping_profile_slug
                    .as_deref()
                    .and_then(normalize_shipping_profile_slug)),
                ean: Set(None),
                upc: Set(None),
                inventory_policy: Set(normalize_variant_inventory_policy(
                    &var_input.inventory_policy,
                )?),
                inventory_management: Set("manual".into()),
                inventory_quantity: Set(0),
                weight: Set(var_input.weight),
                weight_unit: Set(var_input.weight_unit.clone()),
                combination_identity: Set(variant_combination_identity(&var_input.axis_values)),
                position: Set(variant_position),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
                index_revision: sea_orm::ActiveValue::NotSet,
            };
            variant.insert(&txn).await.map_err(|error| {
                tracing::error!(%error, "variant.insert failed in create_product");
                map_product_unique_violation(error, "", "", normalized_sku.as_deref())
            })?;

            if !var_input.axis_values.is_empty() {
                assign_variant_axis_values_in_tx(
                    &txn,
                    tenant_id,
                    variant_id,
                    &var_input.axis_values,
                )
                .await?;
            }

            initial_inventories.push(crate::ports::ProductInitialInventory {
                variant_id,
                sku: normalized_sku,
                available_quantity: var_input.inventory_quantity,
            });

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
        // Seed one translation row per product locale with that locale's readable
        // title, resolved from the localized option labels of each sparse tuple.
        let mut variant_translation_models = Vec::new();
        for locale in &translation_locales {
            let titles_by_variant = super::projection::load_variant_display_titles(
                &txn,
                tenant_id,
                product_id,
                &created_variant_ids,
                locale,
                PLATFORM_FALLBACK_LOCALE,
            )
            .await?;
            for seeded_variant_id in &created_variant_ids {
                variant_translation_models.push(entities::variant_translation::ActiveModel {
                    id: Set(generate_id()),
                    variant_id: Set(*seeded_variant_id),
                    locale: Set(locale.clone()),
                    title: Set(Some(
                        titles_by_variant
                            .get(seeded_variant_id)
                            .cloned()
                            .unwrap_or_else(|| "Default".to_string()),
                    )),
                });
            }
        }
        if !variant_translation_models.is_empty() {
            entities::variant_translation::Entity::insert_many(variant_translation_models)
                .exec(&txn)
                .await?;
        }
        if !initial_inventories.is_empty() {
            self.inventory()
                .bootstrap_variants_in_tx(&txn, tenant_id, actor_id, initial_inventories)
                .await?;
        }
        if !initial_prices.is_empty() {
            self.pricing_port
                .as_ref()
                .ok_or_else(pricing_port_unavailable)?
                .create_initial_prices_in_tx(&txn, tenant_id, initial_prices)
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
                .unwrap_or(PLATFORM_FALLBACK_LOCALE);
            self.sync_product_tags_in_tx(&txn, tenant_id, product_id, locale, tags)
                .await?;
        }

        if !input.brands.is_empty() {
            sync_product_brands_in_tx(&txn, tenant_id, product_id, &input.brands).await?;
        }

        let schema = ProductCatalogSchemaService::new(self.db.clone(), self.event_bus.clone());
        let attributes_changed = schema
            .apply_product_attribute_batches_in_tx(
                &txn,
                tenant_id,
                product_id,
                &input.attribute_values,
            )
            .await?;
        if input.publish {
            schema
                .validate_product_publish_requirements_in(&txn, tenant_id, product_id)
                .await?;
        }

        txn.publish_in_tx(
            tenant_id,
            Some(actor_id),
            DomainEvent::ProductCreated { product_id },
        )
        .await?;
        if attributes_changed {
            txn.publish_in_tx(
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductAttributeValuesChanged { product_id },
            )
            .await?;
        }

        if input.publish {
            txn.publish_in_tx(
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductPublished { product_id },
            )
            .await?;
        }

        let result = self
            .get_product_write_response(
                &txn,
                tenant_id,
                product_id,
                preferred_locale.as_str(),
                Some(&custom_fields_schema),
            )
            .await?;
        record_product_operation_result(&result);
        txn.commit().await?;
        debug!("Transaction committed");

        info!(
            product_id = %product_id,
            translations_count = input.translations.len(),
            variants_count = input.variants.len(),
            status = if input.publish { "active" } else { "draft" },
            "Product created successfully"
        );

        Ok(result)
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
        if let Some(brands) = input.brands.as_deref() {
            validate_product_brands(brands)?;
        }
        let shipping_slug = match &input.shipping_profile_slug {
            Patch::Set(slug) => Some(slug.as_str()),
            Patch::Keep | Patch::Clear => None,
        };
        self.validate_shipping_profiles(tenant_id, actor_id, shipping_slug)
            .await?;

        validate_patch_text_length(&input.seller_id, 100, "Seller ID")?;
        validate_patch_text_length(&input.vendor, 255, "Vendor")?;
        validate_patch_text_length(&input.product_type, 255, "Product type")?;
        validate_patch_text_length(&input.shipping_profile_slug, 64, "Shipping profile slug")?;

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
        lock_schema_for_product_write(&txn, tenant_id).await?;
        if let Patch::Set(primary_category_id) = &input.primary_category_id {
            // Validate the category against the owner transaction, not a pre-tx
            // snapshot: a concurrently deleted category must surface as this typed
            // refusal instead of a raw FK failure.
            validate_primary_category(&txn, tenant_id, Some(*primary_category_id)).await?;
        }

        let product = find_product_for_update_in_tx(&txn, tenant_id, product_id)
            .await
            .map_err(|error| {
                if matches!(error, CommerceError::ProductNotFound(_)) {
                    warn!(product_id = %product_id, "Product not found for update");
                }
                error
            })?;
        let existing_product = product.clone();
        ensure_expected_revision(&existing_product, input.expected_revision)?;
        let existing_status: rustok_product_ports::ProductStatus =
            existing_product.status.clone().into();
        // An input that changes neither the document nor the lifecycle state is a
        // no-op: it must not burn a revision or emit a ProductUpdated event.
        if !input.changes_document()
            && input
                .status
                .as_ref()
                .is_none_or(|status| status == &existing_status)
        {
            let result = self
                .get_product_in(
                    &txn,
                    tenant_id,
                    product_id,
                    PLATFORM_FALLBACK_LOCALE,
                    None,
                    None,
                )
                .await?;
            record_product_operation_result(&result);
            txn.commit().await?;
            return Ok(result);
        }
        match &input.primary_category_id {
            Patch::Set(primary_category_id) => {
                validate_existing_variant_axes_for_category_in(
                    &txn,
                    tenant_id,
                    product_id,
                    Some(*primary_category_id),
                )
                .await?
            }
            Patch::Clear => {
                validate_existing_variant_axes_for_category_in(&txn, tenant_id, product_id, None)
                    .await?
            }
            Patch::Keep => {}
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
                prepare_product_custom_fields_and_schema_for_update(
                    &txn,
                    self.flex_schema_source,
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
        let shipping_profile_patch = input.shipping_profile_slug.clone();
        let shipping_profile_input = match &shipping_profile_patch {
            Patch::Set(slug) => Some(slug.clone()),
            Patch::Keep | Patch::Clear => None,
        };
        let mut metadata_update = normalize_update_product_metadata(
            input.tags.clone(),
            shipping_profile_input,
            prepared_custom_fields
                .as_ref()
                .and_then(|(prepared, _)| prepared.metadata.clone()),
            existing_product.metadata.clone(),
        );
        if matches!(shipping_profile_patch, Patch::Clear) {
            // Clearing the typed binding must also drop its legacy metadata shadow; otherwise the
            // projection would resurrect a shipping profile the operator removed.
            let base = match metadata_update.take() {
                Some((metadata, tags)) => (clear_shipping_profile_metadata_shadow(metadata), tags),
                None => (
                    clear_shipping_profile_metadata_shadow(existing_product.metadata.clone()),
                    None,
                ),
            };
            metadata_update = Some(base);
        }

        match input.vendor {
            Patch::Keep => {}
            Patch::Set(vendor) => product_active.vendor = Set(Some(vendor)),
            Patch::Clear => product_active.vendor = Set(None),
        }
        if !input.seller_id.is_keep() {
            product_active.seller_id = match &input.seller_id {
                Patch::Set(seller_id) => Set(normalize_seller_id(Some(seller_id.as_str()))),
                Patch::Keep | Patch::Clear => Set(None),
            };
        }
        match input.product_type {
            Patch::Keep => {}
            Patch::Set(product_type) => product_active.product_type = Set(Some(product_type)),
            Patch::Clear => product_active.product_type = Set(None),
        }
        if !shipping_profile_patch.is_keep() {
            product_active.shipping_profile_slug = match &shipping_profile_patch {
                Patch::Set(slug) => Set(normalize_shipping_profile_slug(slug)),
                Patch::Keep | Patch::Clear => Set(None),
            };
        }
        let primary_category_changed = match &input.primary_category_id {
            Patch::Keep => false,
            Patch::Set(category_id) => Some(*category_id) != existing_product.primary_category_id,
            Patch::Clear => existing_product.primary_category_id.is_some(),
        };
        match &input.primary_category_id {
            Patch::Keep => {}
            Patch::Set(category_id) => product_active.primary_category_id = Set(Some(*category_id)),
            Patch::Clear => product_active.primary_category_id = Set(None),
        }
        if let Some((metadata, _)) = metadata_update.as_ref() {
            product_active.metadata = Set(metadata.clone());
        }
        product_active.revision = Set(next_revision(existing_product.revision)?);

        let input_status = input.status.map(entities::product::ProductStatus::from);
        let will_become_active = match input_status.as_ref() {
            Some(entities::product::ProductStatus::Active) => {
                existing_product.status != entities::product::ProductStatus::Active
            }
            _ => false,
        };
        let will_archive = match input_status.as_ref() {
            Some(status) => {
                *status == entities::product::ProductStatus::Archived
                    && existing_product.status != entities::product::ProductStatus::Archived
            }
            _ => false,
        };
        let will_deactivate = match input_status.as_ref() {
            Some(status) => {
                *status != entities::product::ProductStatus::Active
                    && existing_product.status == entities::product::ProductStatus::Active
            }
            _ => false,
        };

        let remains_active = input_status.as_ref().unwrap_or(&existing_product.status)
            == &entities::product::ProductStatus::Active;
        if will_become_active {
            product_active.published_at = Set(Some(Utc::now().into()));
        } else if will_deactivate {
            product_active.published_at = Set(None);
        }

        if let Some(status) = input_status {
            product_active.status = Set(status);
        }

        product_active.update(&txn).await?;

        if let Some((prepared_custom_fields, _)) = prepared_custom_fields.as_ref()
            && let (Some(locale), Some(values)) = (
                prepared_custom_fields.locale.as_deref(),
                prepared_custom_fields.localized_values.as_ref(),
            )
        {
            attached_localized_storage::replace_for_locale(
                &txn, tenant_id, product_id, locale, values,
            )
            .await?;
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

                let handle = product_translation_handle(
                    translation_input.handle.as_deref(),
                    &translation_input.title,
                );

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
                                        .is_in(variant_ids.clone()),
                                )
                                .filter(entities::variant_translation::Column::Locale.eq(&locale))
                                .all(&txn)
                                .await?
                                .into_iter()
                                .map(|vt| vt.variant_id)
                                .collect();

                        // Seed the new locale's rows with that locale's readable
                        // tuple labels instead of the machine identity.
                        let titles_by_variant = super::projection::load_variant_display_titles(
                            &txn,
                            tenant_id,
                            product_id,
                            &variant_ids,
                            &locale,
                            PLATFORM_FALLBACK_LOCALE,
                        )
                        .await?;
                        let mut new_variant_translations = Vec::new();
                        for variant in variants {
                            if !existing_trans_variant_ids.contains(&variant.id) {
                                let variant_title = titles_by_variant
                                    .get(&variant.id)
                                    .cloned()
                                    .unwrap_or_else(|| "Default".to_string());
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

        if let Some(brand_inputs) = input.brands.as_deref() {
            sync_product_brands_in_tx(&txn, tenant_id, product_id, brand_inputs).await?;
        }

        let attributes_changed =
            ProductCatalogSchemaService::new(self.db.clone(), self.event_bus.clone())
                .apply_product_attribute_batches_in_tx(
                    &txn,
                    tenant_id,
                    product_id,
                    &input.attribute_values,
                )
                .await?;
        if attributes_changed {
            txn.publish_in_tx(
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductAttributeValuesChanged { product_id },
            )
            .await?;
        }

        // Validate the persisted candidate, including its new category and translations.
        // Failure rolls the entire write back before any lifecycle event is committed.
        if remains_active {
            ProductCatalogSchemaService::new(self.db.clone(), self.event_bus.clone())
                .validate_product_publish_requirements_in(&txn, tenant_id, product_id)
                .await?;
        }

        txn.publish_in_tx(
            tenant_id,
            Some(actor_id),
            DomainEvent::ProductUpdated { product_id },
        )
        .await?;
        if will_become_active {
            txn.publish_in_tx(
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductPublished { product_id },
            )
            .await?;
        } else if will_archive {
            txn.publish_in_tx(
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductArchived { product_id },
            )
            .await?;
        } else if will_deactivate {
            txn.publish_in_tx(
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductUnpublished { product_id },
            )
            .await?;
        }
        if primary_category_changed {
            let new_category_id = match input.primary_category_id {
                Patch::Set(id) => Some(id),
                Patch::Clear => None,
                Patch::Keep => existing_product.primary_category_id,
            };
            txn.publish_in_tx(
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductPrimaryCategoryChanged {
                    product_id,
                    old_category_id: existing_product.primary_category_id,
                    new_category_id,
                },
            )
            .await?;
        }

        let result = self
            .get_product_write_response(
                &txn,
                tenant_id,
                product_id,
                preferred_locale.as_str(),
                prepared_custom_fields.as_ref().map(|(_, schema)| schema),
            )
            .await?;
        record_product_operation_result(&result);
        txn.commit().await?;
        info!(product_id = %product_id, "Product updated successfully");

        Ok(result)
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
        lock_schema_for_product_write(&txn, tenant_id).await?;

        let product = find_product_for_update_in_tx(&txn, tenant_id, product_id)
            .await
            .map_err(|error| {
                if matches!(error, CommerceError::ProductNotFound(_)) {
                    warn!(product_id = %product_id, "Product not found for publishing");
                }
                error
            })?;

        // Retries of a completed transition must not change timestamps or emit events.
        if product.status == entities::product::ProductStatus::Active {
            let result = self
                .get_product_in(
                    &txn,
                    tenant_id,
                    product_id,
                    PLATFORM_FALLBACK_LOCALE,
                    None,
                    None,
                )
                .await?;
            record_product_operation_result(&result);
            txn.commit().await?;
            return Ok(result);
        }

        ProductCatalogSchemaService::new(self.db.clone(), self.event_bus.clone())
            .validate_product_publish_requirements_in(&txn, tenant_id, product_id)
            .await?;

        let next_revision = next_revision(product.revision)?;
        let mut product_active: entities::product::ActiveModel = product.into();
        product_active.revision = Set(next_revision);
        product_active.status = Set(entities::product::ProductStatus::Active);
        product_active.published_at = Set(Some(Utc::now().into()));
        product_active.updated_at = Set(Utc::now().into());
        product_active.update(&txn).await?;

        txn.publish_in_tx(
            tenant_id,
            Some(actor_id),
            DomainEvent::ProductPublished { product_id },
        )
        .await?;

        let result = self
            .get_product_in(
                &txn,
                tenant_id,
                product_id,
                PLATFORM_FALLBACK_LOCALE,
                None,
                None,
            )
            .await?;
        record_product_operation_result(&result);
        txn.commit().await?;
        info!(product_id = %product_id, "Product published successfully");

        Ok(result)
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
        lock_schema_for_product_write(&txn, tenant_id).await?;

        let product = find_product_for_update_in_tx(&txn, tenant_id, product_id).await?;

        // Retries of a completed transition must not change timestamps or emit events.
        if product.status == entities::product::ProductStatus::Draft {
            let result = self
                .get_product_in(
                    &txn,
                    tenant_id,
                    product_id,
                    PLATFORM_FALLBACK_LOCALE,
                    None,
                    None,
                )
                .await?;
            record_product_operation_result(&result);
            txn.commit().await?;
            return Ok(result);
        }

        let next_revision = next_revision(product.revision)?;
        let mut product_active: entities::product::ActiveModel = product.into();
        product_active.revision = Set(next_revision);
        product_active.status = Set(entities::product::ProductStatus::Draft);
        product_active.published_at = Set(None);
        product_active.updated_at = Set(Utc::now().into());
        product_active.update(&txn).await?;

        txn.publish_in_tx(
            tenant_id,
            Some(actor_id),
            DomainEvent::ProductUnpublished { product_id },
        )
        .await?;

        let result = self
            .get_product_in(
                &txn,
                tenant_id,
                product_id,
                PLATFORM_FALLBACK_LOCALE,
                None,
                None,
            )
            .await?;
        record_product_operation_result(&result);
        txn.commit().await?;
        info!(product_id = %product_id, "Product unpublished successfully");

        Ok(result)
    }

    #[instrument(skip(self))]
    pub async fn archive_product(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        product_id: Uuid,
    ) -> CommerceResult<ProductResponse> {
        debug!(product_id = %product_id, "Archiving product");

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
        lock_schema_for_product_write(&txn, tenant_id).await?;

        let product = find_product_for_update_in_tx(&txn, tenant_id, product_id).await?;

        // Retries of a completed transition must not change timestamps or emit events.
        if product.status == entities::product::ProductStatus::Archived {
            let result = self
                .get_product_in(
                    &txn,
                    tenant_id,
                    product_id,
                    PLATFORM_FALLBACK_LOCALE,
                    None,
                    None,
                )
                .await?;
            record_product_operation_result(&result);
            txn.commit().await?;
            return Ok(result);
        }

        let next_revision = next_revision(product.revision)?;
        let mut product_active: entities::product::ActiveModel = product.into();
        product_active.revision = Set(next_revision);
        product_active.status = Set(entities::product::ProductStatus::Archived);
        product_active.published_at = Set(None);
        product_active.updated_at = Set(Utc::now().into());
        product_active.update(&txn).await?;

        txn.publish_in_tx(
            tenant_id,
            Some(actor_id),
            DomainEvent::ProductArchived { product_id },
        )
        .await?;

        let result = self
            .get_product_in(
                &txn,
                tenant_id,
                product_id,
                PLATFORM_FALLBACK_LOCALE,
                None,
                None,
            )
            .await?;
        record_product_operation_result(&result);
        txn.commit().await?;
        info!(product_id = %product_id, "Product archived successfully");

        Ok(result)
    }

    #[instrument(skip(self))]
    pub async fn delete_product(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        product_id: Uuid,
    ) -> CommerceResult<()> {
        debug!(product_id = %product_id, "Deleting product");

        // Pricing owns variant price rows. Without its transactional port Product cannot
        // prove that deleting these variants will not leave orphaned authoritative prices.
        let pricing_port = self
            .pricing_port
            .as_ref()
            .ok_or_else(pricing_port_unavailable)?;
        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
        lock_schema_for_product_write(&txn, tenant_id).await?;

        let product = find_product_for_update_in_tx(&txn, tenant_id, product_id).await?;

        if product.status == entities::product::ProductStatus::Active {
            warn!(product_id = %product_id, "Cannot delete published product");
            return Err(CommerceError::CannotDeletePublished);
        }

        // INVARIANT: the parent was tenant-proved and locked above. Load the complete
        // bounded child identity set, including a contradictory child tenant, before any
        // owner cleanup. Do not pass a truncated page as a complete deletion command.
        let variants = entities::product_variant::Entity::find()
            .select_only()
            .column(entities::product_variant::Column::Id)
            .column(entities::product_variant::Column::TenantId)
            .filter(entities::product_variant::Column::ProductId.eq(product_id))
            .order_by_asc(entities::product_variant::Column::Id)
            .limit((crate::ports::MAX_CATALOG_DELETE_VARIANTS + 1) as u64)
            .into_tuple::<(Uuid, Uuid)>()
            .all(&txn)
            .await?;
        if variants.len() > crate::ports::MAX_CATALOG_DELETE_VARIANTS {
            return Err(CommerceError::InventoryDeletionBlocked(
                crate::ports::ProductInventoryDeletionBlocker::LimitExceeded,
            ));
        }
        if variants.iter().any(|(_, owner)| *owner != tenant_id) {
            return Err(CommerceError::InventoryDeletionBlocked(
                crate::ports::ProductInventoryDeletionBlocker::IdentityUnavailable,
            ));
        }
        if variants.is_empty() {
            return Err(CommerceError::NoVariants);
        }
        let variant_ids = variants.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        self.inventory()
            .delete_records_for_variants_in_tx(&txn, tenant_id, actor_id, &variant_ids)
            .await?;
        pricing_port
            .delete_prices_for_variants_in_tx(&txn, tenant_id, &variant_ids)
            .await?;

        entities::variant_translation::Entity::delete_many()
            .filter(
                entities::variant_translation::Column::VariantId.is_in(variant_ids.iter().copied()),
            )
            .exec(&txn)
            .await?;

        let deleted_variants = entities::product_variant::Entity::delete_many()
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .filter(entities::product_variant::Column::ProductId.eq(product_id))
            .filter(entities::product_variant::Column::Id.is_in(variant_ids.iter().copied()))
            .exec(&txn)
            .await?;
        if deleted_variants.rows_affected != variant_ids.len() as u64 {
            return Err(CommerceError::InventoryDeletionBlocked(
                crate::ports::ProductInventoryDeletionBlocker::ConcurrentChange,
            ));
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
                .filter(
                    entities::product_variant_axis_value::Column::AxisId.is_in(axis_ids.clone()),
                )
                .exec(&txn)
                .await?;

            entities::product_variant_axis::Entity::delete_many()
                .filter(entities::product_variant_axis::Column::Id.is_in(axis_ids))
                .exec(&txn)
                .await?;
        }

        let images = entities::product_image::Entity::find()
            .filter(entities::product_image::Column::ProductId.eq(product_id))
            .all(&txn)
            .await?;
        for image in &images {
            self.enqueue_product_media_release(&*txn, tenant_id, actor_id, image)
                .await?;
        }
        let image_ids: Vec<Uuid> = images.iter().map(|image| image.id).collect();
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

        let deleted_product = entities::product::Entity::delete_by_id(product_id)
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .exec(&txn)
            .await?;
        if deleted_product.rows_affected != 1 {
            return Err(CommerceError::InventoryDeletionBlocked(
                crate::ports::ProductInventoryDeletionBlocker::ConcurrentChange,
            ));
        }

        txn.publish_product_deleted_in_tx(tenant_id, Some(actor_id), product_id, &image_ids)
            .await?;

        record_product_operation_result(&());
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
        validate_variant_inventory_quantity(input.inventory_quantity)?;
        let inventory_policy = normalize_variant_inventory_policy(&input.inventory_policy)?;
        self.validate_shipping_profiles(
            tenant_id,
            actor_id,
            input.shipping_profile_slug.as_deref(),
        )
        .await?;

        validate_variant_prices(&input.prices)?;
        validate_variant_weight(input.weight.as_ref())?;
        if !input.prices.is_empty() && self.pricing_port.is_none() {
            return Err(pricing_port_unavailable());
        }

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
        lock_schema_for_product_write(&txn, tenant_id).await?;

        let product = find_product_for_update_in_tx(&txn, tenant_id, product_id).await?;
        let configured_axes =
            load_variant_axis_configuration_in(&txn, tenant_id, product_id).await?;
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
            .limit((crate::dto::MAX_PRODUCT_VARIANTS + 1) as u64)
            .all(&txn)
            .await?;
        validate_product_variant_count(existing_variants.len(), 1)?;
        if configured_axes.is_empty() && !existing_variants.is_empty() {
            return Err(CommerceError::Validation(
                "a product without variant axes can have exactly one default variant".to_owned(),
            ));
        }
        let next_position = match existing_variants
            .iter()
            .map(|variant| variant.position)
            .max()
        {
            Some(max_position) => max_position.checked_add(1).ok_or_else(|| {
                CommerceError::Validation(
                    "cannot append variant: variant ordering is exhausted".to_owned(),
                )
            })?,
            None => 0,
        };

        let variant_id = current_product_operation_id().unwrap_or_else(generate_id);
        let now = Utc::now();
        let normalized_sku = super::helpers::normalize_variant_sku(input.sku.as_deref());

        let variant = entities::product_variant::ActiveModel {
            id: Set(variant_id),
            product_id: Set(product_id),
            tenant_id: Set(tenant_id),
            sku: Set(normalized_sku.clone()),
            barcode: Set(input.barcode.clone()),
            shipping_profile_slug: Set(input
                .shipping_profile_slug
                .as_deref()
                .and_then(normalize_shipping_profile_slug)),
            ean: Set(None),
            upc: Set(None),
            inventory_policy: Set(inventory_policy),
            inventory_management: Set("manual".into()),
            inventory_quantity: Set(0),
            weight: Set(input.weight),
            weight_unit: Set(input.weight_unit.clone()),
            combination_identity: Set(variant_combination_identity(&input.axis_values)),
            position: Set(next_position),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            index_revision: sea_orm::ActiveValue::NotSet,
        };

        variant
            .insert(&txn)
            .await
            .map_err(|error| map_product_unique_violation(error, "", "", normalized_sku.as_deref()))?;

        if !input.axis_values.is_empty() {
            assign_variant_axis_values_in_tx(&txn, tenant_id, variant_id, &input.axis_values)
                .await?;
        }

        if product.status == entities::product::ProductStatus::Active {
            // A newly-created SKU is part of the live Product immediately. Do not commit it
            // unless every required Variant EAV value/localization is present on every SKU.
            ProductCatalogSchemaService::new(self.db.clone(), self.event_bus.clone())
                .validate_product_publish_requirements_in(&txn, tenant_id, product_id)
                .await?;
        }

        self.inventory()
            .bootstrap_variants_in_tx(
                &txn,
                tenant_id,
                actor_id,
                vec![crate::ports::ProductInitialInventory {
                    variant_id,
                    sku: normalized_sku,
                    available_quantity: input.inventory_quantity,
                }],
            )
            .await?;

        // Seed one translation row per existing product locale with that locale's
        // readable title, resolved from the localized option labels of the tuple.
        let seeded_variant_ids = [variant_id];
        let mut variant_translation_models = Vec::new();
        for locale in &existing_locales {
            let titles_by_variant = super::projection::load_variant_display_titles(
                &txn,
                tenant_id,
                product_id,
                &seeded_variant_ids,
                locale,
                PLATFORM_FALLBACK_LOCALE,
            )
            .await?;
            variant_translation_models.push(entities::variant_translation::ActiveModel {
                id: Set(generate_id()),
                variant_id: Set(variant_id),
                locale: Set(locale.clone()),
                title: Set(Some(
                    titles_by_variant
                        .get(&variant_id)
                        .cloned()
                        .unwrap_or_else(|| "Default".to_string()),
                )),
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
            self.pricing_port
                .as_ref()
                .ok_or_else(pricing_port_unavailable)?
                .create_initial_prices_in_tx(&txn, tenant_id, initial_prices)
                .await?;
        }

        txn.publish_in_tx(
            tenant_id,
            Some(actor_id),
            DomainEvent::VariantCreated {
                variant_id,
                product_id,
            },
        )
        .await?;

        let result = self.get_variant_in(&txn, tenant_id, variant_id).await?;
        record_product_operation_result(&result);
        txn.commit().await?;

        info!(
            variant_id = %variant_id,
            product_id = %product_id,
            "Product variant created successfully"
        );

        Ok(result)
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
        if input.inventory_quantity.is_some() != input.expected_inventory_quantity.is_some() {
            return Err(CommerceError::Validation(
                "inventory quantity changes require the quantity snapshot that was read".into(),
            ));
        }
        validate_patch_text_length(&input.sku, 100, "SKU")?;
        validate_patch_text_length(&input.barcode, 100, "Barcode")?;
        validate_patch_text_length(&input.shipping_profile_slug, 100, "Shipping profile slug")?;
        validate_patch_text_length(&input.weight_unit, 16, "Weight unit")?;
        if let Patch::Set(weight) = &input.weight {
            validate_variant_weight(Some(weight))?;
        }
        if let Some(quantity) = input.inventory_quantity {
            validate_variant_inventory_quantity(quantity)?;
        }
        if let Some(policy) = input.inventory_policy.as_deref() {
            normalize_variant_inventory_policy(policy)?;
        }
        if let Patch::Set(slug) = &input.shipping_profile_slug {
            self.validate_shipping_profiles(tenant_id, actor_id, Some(slug.as_str()))
                .await?;
        }

        if let Some(prices) = input.prices.as_deref() {
            validate_variant_prices(prices)?;
            if prices.iter().any(|price| {
                price.channel_id.is_some()
                    || normalize_public_channel_slug(price.channel_slug.as_deref()).is_some()
            }) {
                return Err(CommerceError::Validation(
                    "Catalog edits accept only unscoped base prices; use Pricing for contextual prices"
                        .into(),
                ));
            }
        }
        if input.prices.is_some() && self.pricing_port.is_none() {
            return Err(pricing_port_unavailable());
        }

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
        lock_schema_for_product_write(&txn, tenant_id).await?;

        let observed_variant = entities::product_variant::Entity::find_by_id(variant_id)
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(CommerceError::VariantNotFound(variant_id))?;
        let product_id = observed_variant.product_id;
        let _product = find_product_for_update_in_tx(&txn, tenant_id, product_id).await?;
        let variant = find_variant_for_update_in_tx(&txn, tenant_id, variant_id).await?;
        if input.has_changes() {
            ensure_expected_variant_revision(&variant, input.expected_revision.as_deref())?;
        }
        if !input.has_changes() {
            let result = self.get_variant_in(&txn, tenant_id, variant_id).await?;
            record_product_operation_result(&result);
            txn.commit().await?;
            return Ok(result);
        }
        let mut active: entities::product_variant::ActiveModel = variant.into();
        active.updated_at = Set(Utc::now().into());

        // Keep leaves the loaded ActiveValue untouched; Clear and blank Set write SQL NULL.
        let requested_sku = match &input.sku {
            Patch::Keep => None,
            Patch::Clear => Some(None),
            Patch::Set(value) => Some(super::helpers::normalize_variant_sku(Some(value.as_str()))),
        };
        if let Some(sku) = &requested_sku {
            active.sku = Set(sku.clone());
        }
        match input.barcode {
            Patch::Keep => {}
            Patch::Clear => active.barcode = Set(None),
            Patch::Set(value) => active.barcode = Set(Some(value)),
        }
        match input.shipping_profile_slug {
            Patch::Keep => {}
            Patch::Clear => active.shipping_profile_slug = Set(None),
            Patch::Set(value) => {
                active.shipping_profile_slug = Set(normalize_shipping_profile_slug(&value))
            }
        }
        if let Some(inventory_policy) = input.inventory_policy {
            active.inventory_policy = Set(normalize_variant_inventory_policy(
                &inventory_policy,
            )?);
        }
        match input.weight {
            Patch::Keep => {}
            Patch::Clear => active.weight = Set(None),
            Patch::Set(value) => active.weight = Set(Some(value)),
        }
        match input.weight_unit {
            Patch::Keep => {}
            Patch::Clear => active.weight_unit = Set(None),
            Patch::Set(value) => active.weight_unit = Set(Some(value)),
        }
        let requested_sku_for_unique_error =
            requested_sku.as_ref().and_then(|sku| sku.as_deref());
        let _updated_variant = active
            .update(&txn)
            .await
            .map_err(|error| {
                map_product_unique_violation(error, "", "", requested_sku_for_unique_error)
            })?;

        if let Some(sku) = requested_sku {
            self.inventory()
                .sync_variant_sku_in_tx(&txn, tenant_id, actor_id, variant_id, sku)
                .await?;
        }

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

            let title_snapshot = VariantAutoTitleSnapshot::capture(
                &txn, tenant_id, product_id, &[variant_id],
            )
            .await?;

            if !configured_axis_attribute_ids.is_empty() {
                let backend = txn.get_database_backend();
                let placeholders = (0..configured_axis_attribute_ids.len())
                    .map(|index| sql_placeholder(backend, index + 3))
                    .collect::<Vec<_>>()
                    .join(", ");
                let query = format!(
                    "SELECT id FROM product_variant_attribute_values WHERE tenant_id = {tenant_ph} AND variant_id = {variant_ph} AND attribute_id IN ({placeholders})",
                    tenant_ph = sql_placeholder(backend, 1),
                    variant_ph = sql_placeholder(backend, 2),
                );
                let mut params = vec![tenant_id.into(), variant_id.into()];
                for aid in configured_axis_attribute_ids {
                    params.push(aid.into());
                }
                let existing_vals: Vec<IdRow> = IdRow::find_by_statement(
                    Statement::from_sql_and_values(backend, &query, params),
                )
                .all(&txn)
                .await?;
                for ev in existing_vals {
                    txn.execute_raw(Statement::from_sql_and_values(
                        backend,
                        format!(
                            "DELETE FROM product_variant_attribute_value_options WHERE tenant_id = {tenant_ph} AND value_id = {value_ph}",
                            tenant_ph = sql_placeholder(backend, 1),
                            value_ph = sql_placeholder(backend, 2),
                        ),
                        vec![tenant_id.into(), ev.id.into()],
                    ))
                    .await?;
                    txn.execute_raw(Statement::from_sql_and_values(
                        backend,
                        format!(
                            "DELETE FROM product_variant_attribute_values WHERE tenant_id = {tenant_ph} AND id = {value_ph}",
                            tenant_ph = sql_placeholder(backend, 1),
                            value_ph = sql_placeholder(backend, 2),
                        ),
                        vec![tenant_id.into(), ev.id.into()],
                    ))
                    .await?;
                }
            }

            assign_variant_axis_values_in_tx(&txn, tenant_id, variant_id, axis_values).await?;

            title_snapshot.refresh(&txn, tenant_id, product_id).await?;
        }

        if let Some(prices) = input.prices {
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
            self.pricing_port
                .as_ref()
                .ok_or_else(pricing_port_unavailable)?
                .replace_catalog_base_prices_in_tx(&txn, tenant_id, variant_id, initial_prices)
                .await?;
        }

        if let Some(quantity) = input.inventory_quantity {
            self.inventory()
                .update_initial_quantity_in_tx(
                    &txn,
                    tenant_id,
                    actor_id,
                    variant_id,
                    quantity,
                    input.expected_inventory_quantity.ok_or_else(|| CommerceError::Validation(
                        "inventory quantity changes require the quantity snapshot that was read".into(),
                    ))?,
                )
                .await?;
        }

        txn.publish_in_tx(
            tenant_id,
            Some(actor_id),
            DomainEvent::VariantUpdated {
                variant_id,
                product_id,
            },
        )
        .await?;

        let result = self.get_variant_in(&txn, tenant_id, variant_id).await?;
        record_product_operation_result(&result);
        txn.commit().await?;

        info!(
            variant_id = %variant_id,
            product_id = %product_id,
            "Product variant updated successfully"
        );

        Ok(result)
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id, variant_id = %variant_id))]
    pub async fn delete_variant(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        expected_revision: String,
    ) -> CommerceResult<()> {
        debug!("Deleting product variant");

        // Refuse when Pricing cannot participate in the same transaction; otherwise
        // authoritative price-list/contextual rows could outlive their ProductVariant.
        let pricing_port = self
            .pricing_port
            .as_ref()
            .ok_or_else(pricing_port_unavailable)?;
        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
        lock_schema_for_product_write(&txn, tenant_id).await?;

        let observed_variant = entities::product_variant::Entity::find_by_id(variant_id)
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(CommerceError::VariantNotFound(variant_id))?;
        let product_id = observed_variant.product_id;
        let _product = find_product_for_update_in_tx(&txn, tenant_id, product_id).await?;
        let variant = find_variant_for_update_in_tx(&txn, tenant_id, variant_id).await?;
        ensure_expected_variant_revision(&variant, Some(&expected_revision))?;

        let count = entities::product_variant::Entity::find()
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .filter(entities::product_variant::Column::ProductId.eq(product_id))
            .count(&txn)
            .await?;
        if count <= 1 {
            return Err(CommerceError::CannotDeleteOnlyVariant);
        }

        self.inventory()
            .delete_records_for_variants_in_tx(&txn, tenant_id, actor_id, &[variant_id])
            .await?;
        pricing_port
            .delete_prices_for_variants_in_tx(&txn, tenant_id, &[variant_id])
            .await?;

        entities::variant_translation::Entity::delete_many()
            .filter(entities::variant_translation::Column::VariantId.eq(variant_id))
            .exec(&txn)
            .await?;

        let deleted_variant = entities::product_variant::Entity::delete_by_id(variant_id)
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .filter(entities::product_variant::Column::ProductId.eq(product_id))
            .exec(&txn)
            .await?;
        if deleted_variant.rows_affected != 1 {
            return Err(CommerceError::InventoryDeletionBlocked(
                crate::ports::ProductInventoryDeletionBlocker::ConcurrentChange,
            ));
        }

        txn.publish_in_tx(
            tenant_id,
            Some(actor_id),
            DomainEvent::VariantDeleted {
                variant_id,
                product_id,
            },
        )
        .await?;

        record_product_operation_result(&());
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
        if input.media_id.is_nil() {
            return Err(CommerceError::Validation(
                "Product image Media UUID must not be nil".into(),
            ));
        }
        let image_locale = input
            .alt_text
            .as_deref()
            .map(|_| canonical_product_image_locale(input.locale.as_deref()))
            .transpose()?;

        // Product owns the UUID before Media IO; receipt replay returns before
        // this future is polled. Retries of an admitted operation reuse this UUID.
        let image_id = current_product_operation_id().unwrap_or_else(generate_id);
        // Avoid retaining against an obviously missing/cross-tenant target. The
        // parent is rechecked under its write lock after Media retain.
        entities::product::Entity::find_by_id(product_id)
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(CommerceError::ProductNotFound(product_id))?;
        if self.has_unverified_media_images(tenant_id).await? {
            return Err(CommerceError::Database(sea_orm::DbErr::Custom(
                "existing Product images require Media hold backfill before image admission".into(),
            )));
        }
        let media_id = input.media_id;
        self.retain_product_image(tenant_id, actor_id, image_id, media_id)
            .await?;

        let (txn, result) = self
            .stage_product_image_addition_in_tx(
                tenant_id,
                actor_id,
                product_id,
                image_id,
                input,
                image_locale,
            )
            .await?;
        record_product_operation_result(&result);
        // A commit failure is ambiguous (the commit may have landed) and is
        // deliberately not compensated: a leaked hold is recoverable, releasing
        // the hold of a landed image is not.
        txn.commit().await?;

        info!(
            image_id = %image_id,
            product_id = %product_id,
            "Product image added successfully"
        );

        Ok(result)
    }

    /// Stages a retained Product image inside its owner transaction without
    /// committing it.
    ///
    /// Split from `add_product_image` so a definite rollback can compensate the
    /// Media hold that was taken before the transaction opened.
    async fn stage_product_image_addition_in_tx(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        product_id: Uuid,
        image_id: Uuid,
        input: AddProductImageInput,
        image_locale: Option<String>,
    ) -> CommerceResult<(ProductWriteTransaction, ProductImageResponse)> {
        // A retained Media hold remains conservative after local failure or an
        // ambiguous commit. Retries reuse the same Media idempotency identity;
        // only committed Product removal rows authorize automatic release.
        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
        lock_schema_for_product_write(&txn, tenant_id).await?;

        let _product = find_product_for_update_in_tx(&txn, tenant_id, product_id).await?;

        let existing_images = entities::product_image::Entity::find()
            .filter(entities::product_image::Column::ProductId.eq(product_id))
            .order_by_asc(entities::product_image::Column::Position)
            .order_by_asc(entities::product_image::Column::Id)
            .all(&txn)
            .await?;
        let requested_position = input
            .position
            .map(|position| {
                usize::try_from(position).map_err(|_| {
                    CommerceError::Validation(
                        "Product image position cannot be negative".to_owned(),
                    )
                })
            })
            .transpose()?;
        let insertion_index = requested_position.unwrap_or(existing_images.len());
        if insertion_index > existing_images.len() {
            return Err(CommerceError::Validation(
                "Product image position must not exceed the current image count".to_owned(),
            ));
        }
        let position = i32::try_from(insertion_index)
            .map_err(|_| CommerceError::Validation("too many Product images".to_owned()))?;

        let _image = entities::product_image::ActiveModel {
            id: Set(image_id),
            product_id: Set(product_id),
            media_id: Set(input.media_id),
            position: Set(position),
        }
        .insert(&txn)
        .await?;
        self.mark_image_media_provisioned(&*txn, tenant_id, image_id)
            .await?;
        self.mark_media_retention_ready(&*txn, tenant_id).await?;

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

        txn.publish_in_tx(
            tenant_id,
            Some(actor_id),
            DomainEvent::ProductUpdated { product_id },
        )
        .await?;

        let result = ProductImageResponse {
            id: image_id,
            media_id: input.media_id,
            url: format_product_media_url(input.media_id),
            alt_text: input.alt_text,
            position,
            translations,
        };

        Ok((txn, result))
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
        let response_locale = canonical_product_image_locale(input.locale.as_deref())?;
        validate_product_image_input(input.position, input.alt_text.as_deref())?;
        let image_locale = input
            .alt_text
            .as_deref()
            .map(|_| canonical_product_image_locale(input.locale.as_deref()))
            .transpose()?;

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
        lock_schema_for_product_write(&txn, tenant_id).await?;

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
            persist_product_image_order_in_tx(&txn, product_id, ordered_image_ids.as_slice())
                .await?;
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

        txn.publish_in_tx(
            tenant_id,
            Some(actor_id),
            DomainEvent::ProductUpdated { product_id },
        )
        .await?;

        let all_translations = entities::product_image_translation::Entity::find()
            .filter(entities::product_image_translation::Column::ImageId.eq(image_id))
            .order_by_asc(entities::product_image_translation::Column::Locale)
            .all(&txn)
            .await?;
        let alt_text =
            super::projection::resolve_image_alt_text(&all_translations, &response_locale, None);
        let result = ProductImageResponse {
            id: image_id,
            media_id: updated_image.media_id,
            url: format_product_media_url(updated_image.media_id),
            alt_text,
            position,
            translations: all_translations
                .into_iter()
                .map(|translation| ProductImageTranslationResponse {
                    locale: translation.locale,
                    alt_text: translation.alt_text,
                })
                .collect(),
        };
        record_product_operation_result(&result);
        txn.commit().await?;

        info!(
            image_id = %image_id,
            product_id = %product_id,
            "Product image updated successfully"
        );

        Ok(result)
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
        lock_schema_for_product_write(&txn, tenant_id).await?;

        let _product = find_product_for_update_in_tx(&txn, tenant_id, product_id).await?;

        let image = entities::product_image::Entity::find_by_id(image_id)
            .filter(entities::product_image::Column::ProductId.eq(product_id))
            .one(&txn)
            .await?
            .ok_or(CommerceError::ImageNotFound(image_id))?;

        self.enqueue_product_media_release(&*txn, tenant_id, actor_id, &image)
            .await?;

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

        txn.publish_in_tx(
            tenant_id,
            Some(actor_id),
            DomainEvent::ProductUpdated { product_id },
        )
        .await?;

        record_product_operation_result(&());
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
        lock_schema_for_product_write(&txn, tenant_id).await?;

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

        txn.publish_in_tx(
            tenant_id,
            Some(actor_id),
            DomainEvent::ProductUpdated { product_id },
        )
        .await?;

        record_product_operation_result(&());
        txn.commit().await?;

        info!(product_id = %product_id, "Product images reordered successfully");

        Ok(())
    }
}

#[derive(sea_orm::FromQueryResult)]
struct IdRow {
    id: Uuid,
}

/// Validates a primary category binding against the caller's connection, inside the
/// owner transaction: a concurrently deleted category must surface as this typed
/// refusal instead of a raw FK failure at insert time.
async fn validate_primary_category<C: ConnectionTrait>(
    conn: &C,
    tenant_id: Uuid,
    category_id: Option<Uuid>,
) -> CommerceResult<()> {
    let Some(category_id) = category_id else {
        return Ok(());
    };
    let backend = conn.get_database_backend();
    let row = conn
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT kind FROM catalog_categories WHERE tenant_id = {tenant_ph} AND id = {category_ph} AND deleted_at IS NULL",
                tenant_ph = sql_placeholder(backend, 1),
                category_ph = sql_placeholder(backend, 2),
            ),
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

/// Auto-generated variant titles per product locale, resolved from the currently
/// persisted sparse tuple.
/// Captures generated titles before a tuple or axis-order edit. Product locales
/// apply to every affected SKU; an extra Translation locale belongs only to its SKU.
pub(super) struct VariantAutoTitleSnapshot {
    variants_by_locale: std::collections::BTreeMap<String, Vec<Uuid>>,
    previous: HashMap<Uuid, HashMap<String, String>>,
}

impl VariantAutoTitleSnapshot {
    pub(super) async fn capture(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        product_id: Uuid,
        variant_ids: &[Uuid],
    ) -> CommerceResult<Self> {
        let mut variants_by_locale = std::collections::BTreeMap::new();
        if !variant_ids.is_empty() {
            let product_locales = entities::product_translation::Entity::find()
                .filter(entities::product_translation::Column::TenantId.eq(tenant_id))
                .filter(entities::product_translation::Column::ProductId.eq(product_id))
                .select_only()
                .column(entities::product_translation::Column::Locale)
                .into_tuple::<String>()
                .all(txn)
                .await?;
            for locale in product_locales {
                variants_by_locale.insert(locale, variant_ids.to_vec());
            }
            let variant_locales = entities::variant_translation::Entity::find()
                .filter(entities::variant_translation::Column::VariantId.is_in(variant_ids.iter().copied()))
                .select_only()
                .column(entities::variant_translation::Column::VariantId)
                .column(entities::variant_translation::Column::Locale)
                .into_tuple::<(Uuid, String)>()
                .all(txn)
                .await?;
            for (variant_id, locale) in variant_locales {
                let ids = variants_by_locale.entry(locale).or_default();
                if !ids.contains(&variant_id) {
                    ids.push(variant_id);
                }
            }
        }
        let previous = Self::load_titles(txn, tenant_id, product_id, &variants_by_locale).await?;
        Ok(Self { variants_by_locale, previous })
    }

    async fn load_titles(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        product_id: Uuid,
        variants_by_locale: &std::collections::BTreeMap<String, Vec<Uuid>>,
    ) -> CommerceResult<HashMap<Uuid, HashMap<String, String>>> {
        let mut titles = HashMap::<Uuid, HashMap<String, String>>::new();
        // Batch the tuple projection once per locale, not once per SKU and locale.
        for (locale, variant_ids) in variants_by_locale {
            let mut generated = super::projection::load_variant_display_titles(
                txn, tenant_id, product_id, variant_ids, locale, PLATFORM_FALLBACK_LOCALE,
            )
            .await?;
            for &variant_id in variant_ids {
                titles.entry(variant_id).or_default().insert(
                    locale.clone(),
                    generated.remove(&variant_id).unwrap_or_else(|| "Default".to_owned()),
                );
            }
        }
        Ok(titles)
    }

    pub(super) async fn refresh(
        self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        product_id: Uuid,
    ) -> CommerceResult<()> {
        let next = Self::load_titles(txn, tenant_id, product_id, &self.variants_by_locale).await?;
        for (variant_id, previous) in self.previous {
            let locales = previous.keys().cloned().collect::<Vec<_>>();
            let next_titles = next.get(&variant_id).ok_or_else(|| {
                CommerceError::Validation("Variant title refresh lost its locale snapshot".to_owned())
            })?;
            sync_variant_auto_titles_in_tx(txn, variant_id, &locales, &previous, next_titles).await?;
        }
        Ok(())
    }
}

/// Refreshes variant translation titles after a tuple change without destroying
/// authored copy.
///
/// Missing locales and empty rows are seeded with the next auto-generated title, and
/// a row that still holds the previous auto-generated value is advanced to it. Any
/// other row was authored through the Translation boundary and is left untouched.
async fn sync_variant_auto_titles_in_tx(
    txn: &DatabaseTransaction,
    variant_id: Uuid,
    locales: &[String],
    previous_auto_titles: &HashMap<String, String>,
    next_auto_titles: &HashMap<String, String>,
) -> CommerceResult<()> {
    let existing: HashMap<String, entities::variant_translation::Model> =
        entities::variant_translation::Entity::find()
            .filter(entities::variant_translation::Column::VariantId.eq(variant_id))
            .all(txn)
            .await?
            .into_iter()
            .map(|row| (row.locale.clone(), row))
            .collect();

    let mut inserts = Vec::new();
    for locale in locales {
        let next_title = next_auto_titles
            .get(locale)
            .cloned()
            .unwrap_or_else(|| "Default".to_string());
        match existing.get(locale) {
            None => {
                inserts.push(entities::variant_translation::ActiveModel {
                    id: Set(generate_id()),
                    variant_id: Set(variant_id),
                    locale: Set(locale.clone()),
                    title: Set(Some(next_title)),
                });
            }
            Some(row) => {
                let current = row.title.as_deref().unwrap_or("");
                let is_auto = current.trim().is_empty()
                    || previous_auto_titles
                        .get(locale)
                        .is_some_and(|previous| previous == current);
                if is_auto && current != next_title {
                    let mut active: entities::variant_translation::ActiveModel =
                        row.clone().into();
                    active.title = Set(Some(next_title));
                    active.update(txn).await?;
                }
            }
        }
    }
    if !inserts.is_empty() {
        entities::variant_translation::Entity::insert_many(inserts)
            .exec(txn)
            .await?;
    }
    Ok(())
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
        .map_err(|error| {
            CommerceError::Validation(format!("invalid Product image locale: {error}"))
        })
}

fn validate_product_image_input(
    position: Option<i32>,
    alt_text: Option<&str>,
) -> CommerceResult<()> {
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
        if axis.allowed_option_ids.is_empty()
            || axis.allowed_option_ids.len() > MAX_VARIANT_AXIS_VALUES
        {
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

    let attribute_ids = axes
        .iter()
        .map(|axis| axis.attribute_id)
        .collect::<Vec<_>>();
    let backend = conn.get_database_backend();
    let attribute_placeholders = (0..attribute_ids.len())
        .map(|index| sql_placeholder(backend, index + 2))
        .collect::<Vec<_>>()
        .join(", ");
    let mut attribute_values = vec![tenant_id.into()];
    attribute_values.extend(attribute_ids.iter().copied().map(Into::into));
    let definitions = VariantAxisDefinitionRow::find_by_statement(Statement::from_sql_and_values(
        backend,
        format!(
            "SELECT id, value_type, scope FROM product_attributes WHERE tenant_id = {tenant_ph} AND archived_at IS NULL AND id IN ({attribute_placeholders})",
            tenant_ph = sql_placeholder(backend, 1),
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
        if definition.value_type != "select"
            || !matches!(definition.scope.as_str(), "variant" | "both")
        {
            return Err(CommerceError::Validation(format!(
                "variant axis attribute {attribute_id} must be an active variant-scoped select attribute"
            )));
        }
    }

    let mut option_owners = HashMap::new();
    for option_ids in requested_option_ids.chunks(128) {
        let placeholders = (0..option_ids.len())
            .map(|index| sql_placeholder(backend, index + 2))
            .collect::<Vec<_>>()
            .join(", ");
        let mut values = vec![tenant_id.into()];
        values.extend(option_ids.iter().copied().map(Into::into));
        let options = VariantAxisOptionRow::find_by_statement(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT id, attribute_id FROM product_attribute_options WHERE tenant_id = {tenant_ph} AND archived_at IS NULL AND id IN ({placeholders})",
                tenant_ph = sql_placeholder(backend, 1),
            ),
            values,
        ))
        .all(conn)
        .await?;
        option_owners.extend(
            options
                .into_iter()
                .map(|option| (option.id, option.attribute_id)),
        );
    }
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
    primary_category_id: Option<Uuid>,
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
    validate_variant_axis_configuration_in(conn, tenant_id, primary_category_id, &proposed_axes)
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

async fn assign_variant_axis_value_rows_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    variant_id: Uuid,
    axis_values: &[VariantAxisValueInput],
) -> CommerceResult<()> {
    for val in axis_values {
        let backend = txn.get_database_backend();
        let row = IdRow::find_by_statement(Statement::from_sql_and_values(
            backend,
            format!(
                r#"
            INSERT INTO product_variant_attribute_values (
                id, tenant_id, variant_id, attribute_id, detached_at
            ) VALUES ({p1}, {p2}, {p3}, {p4}, NULL)
            ON CONFLICT (tenant_id, variant_id, attribute_id) DO UPDATE SET
                detached_at = NULL,
                updated_at = CURRENT_TIMESTAMP
            RETURNING id
            "#,
                p1 = sql_placeholder(backend, 1),
                p2 = sql_placeholder(backend, 2),
                p3 = sql_placeholder(backend, 3),
                p4 = sql_placeholder(backend, 4),
            ),
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
                backend,
                format!(
                    "SELECT id FROM product_variant_attribute_values WHERE tenant_id = {tenant_ph} AND variant_id = {variant_ph} AND attribute_id = {attribute_ph}",
                    tenant_ph = sql_placeholder(backend, 1),
                    variant_ph = sql_placeholder(backend, 2),
                    attribute_ph = sql_placeholder(backend, 3),
                ),
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
            backend,
            format!(
                "DELETE FROM product_variant_attribute_value_options WHERE tenant_id = {tenant_ph} AND value_id = {value_ph}",
                tenant_ph = sql_placeholder(backend, 1),
                value_ph = sql_placeholder(backend, 2),
            ),
            vec![tenant_id.into(), value_id.into()],
        ))
        .await?;

        txn.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO product_variant_attribute_value_options (tenant_id, value_id, option_id) VALUES ({tenant_ph}, {value_ph}, {option_ph})",
                tenant_ph = sql_placeholder(backend, 1),
                value_ph = sql_placeholder(backend, 2),
                option_ph = sql_placeholder(backend, 3),
            ),
            vec![tenant_id.into(), value_id.into(), val.option_id.into()],
        ))
        .await?;
    }

    Ok(())
}

pub(crate) async fn assign_variant_axis_values_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    variant_id: Uuid,
    axis_values: &[VariantAxisValueInput],
) -> CommerceResult<()> {
    assign_variant_axis_value_rows_in_tx(txn, tenant_id, variant_id, axis_values).await?;

    let backend = txn.get_database_backend();
    let combination_identity = variant_combination_identity(axis_values);
    txn.execute_raw(Statement::from_sql_and_values(
        backend,
        format!(
            "UPDATE product_variants SET combination_identity = {identity_ph} WHERE tenant_id = {tenant_ph} AND id = {variant_ph}",
            identity_ph = sql_placeholder(backend, 1),
            tenant_ph = sql_placeholder(backend, 2),
            variant_ph = sql_placeholder(backend, 3),
        ),
        vec![
            combination_identity.into(),
            tenant_id.into(),
            variant_id.into(),
        ],
    ))
    .await?;

    Ok(())
}

pub(crate) fn validate_variant_axes_and_combinations(
    variant_axes: &[VariantAxisInput],
    variants: &[CreateVariantInput],
) -> CommerceResult<()> {
    let tuples = variants
        .iter()
        .map(|variant| variant.axis_values.clone())
        .collect::<Vec<_>>();
    validate_variant_axis_tuples(variant_axes, &tuples)
}

/// One identity validator for authored and public-read tuples; no price/stock input is fabricated.
pub(super) fn validate_variant_axis_tuples(
    variant_axes: &[VariantAxisInput],
    variants: &[Vec<VariantAxisValueInput>],
) -> CommerceResult<()> {
    validate_product_variant_count(0, variants.len())?;
    if variant_axes.is_empty() {
        if variants.len() != 1 {
            return Err(CommerceError::Validation(
                "a product without variant axes must contain exactly one default variant"
                    .to_owned(),
            ));
        }
        if !variants[0].is_empty() {
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
        if variant.is_empty() {
            return Err(CommerceError::Validation(format!(
                "Variant at index {} is missing axis values for configured axes",
                idx
            )));
        }
        if variant.len() != variant_axes.len() {
            return Err(CommerceError::Validation(format!(
                "Variant at index {} has {} axis assignments, expected {}",
                idx,
                variant.len(),
                variant_axes.len()
            )));
        }

        let mut variant_axis_attrs = HashSet::new();
        let mut sorted_pairs = Vec::new();
        for val in variant {
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
    fn negative_variant_weight_is_rejected_for_all_write_inputs() {
        assert!(validate_variant_weight(None).is_ok());
        assert!(validate_variant_weight(Some(&Decimal::ZERO)).is_ok());
        assert!(validate_variant_weight(Some(&Decimal::new(100, 2))).is_ok());
        assert!(matches!(
            validate_variant_weight(Some(&Decimal::new(-1, 2))),
            Err(CommerceError::Validation(_))
        ));
    }

    #[test]
    fn product_variant_capacity_is_enforced_without_overflow() {
        assert!(validate_product_variant_count(0, 1).is_ok());
        assert!(validate_product_variant_count(0, crate::dto::MAX_PRODUCT_VARIANTS).is_ok());
        assert!(validate_product_variant_count(crate::dto::MAX_PRODUCT_VARIANTS - 1, 1).is_ok());
        assert!(validate_product_variant_count(0, crate::dto::MAX_PRODUCT_VARIANTS + 1).is_err());
        assert!(validate_product_variant_count(crate::dto::MAX_PRODUCT_VARIANTS, 1).is_err());
        assert!(validate_product_variant_count(usize::MAX, 1).is_err());
    }

    #[test]
    fn variant_inventory_quantity_rejects_negative_values() {
        assert!(validate_variant_inventory_quantity(0).is_ok());
        assert!(validate_variant_inventory_quantity(i32::MAX).is_ok());
        assert!(matches!(
            validate_variant_inventory_quantity(-1),
            Err(CommerceError::Validation(_))
        ));
    }

    #[test]
    fn variant_inventory_policy_is_canonical_and_closed() {
        assert_eq!(
            normalize_variant_inventory_policy(" deny ").unwrap(),
            "deny"
        );
        assert_eq!(
            normalize_variant_inventory_policy("CONTINUE").unwrap(),
            "continue"
        );
        for invalid in ["", "allow", "backorder", "deny-stockout"] {
            assert!(matches!(
                normalize_variant_inventory_policy(invalid),
                Err(CommerceError::Validation(_))
            ));
        }
    }

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
    #[test]
    fn validate_variant_prices_rejects_nil_channel_identity() {
        let prices = vec![PriceInput {
            currency_code: "USD".to_owned(),
            channel_id: Some(Uuid::nil()),
            channel_slug: None,
            amount: Decimal::new(1000, 2),
            compare_at_amount: None,
        }];
        assert!(validate_variant_prices(&prices).is_err());
    }

    #[test]
    fn validate_variant_prices_rejects_compare_at_below_base_amount() {
        let prices = vec![PriceInput {
            currency_code: "USD".to_owned(),
            channel_id: None,
            channel_slug: None,
            amount: Decimal::new(1200, 2),
            compare_at_amount: Some(Decimal::new(1199, 2)),
        }];
        assert!(validate_variant_prices(&prices).is_err());

        let equal_compare_at = vec![PriceInput {
            currency_code: "USD".to_owned(),
            channel_id: None,
            channel_slug: None,
            amount: Decimal::new(1200, 2),
            compare_at_amount: Some(Decimal::new(1200, 2)),
        }];
        assert!(validate_variant_prices(&equal_compare_at).is_ok());
    }

    #[test]
    fn validate_variant_prices_rejects_negative_amounts_before_pricing_io() {
        for (amount, compare_at_amount) in [
            (Decimal::new(-1, 0), None),
            (Decimal::ZERO, Some(Decimal::new(-1, 0))),
        ] {
            let prices = vec![PriceInput {
                currency_code: "USD".to_owned(),
                channel_id: None,
                channel_slug: None,
                amount,
                compare_at_amount,
            }];
            assert!(validate_variant_prices(&prices).is_err());
        }
    }

    #[test]
    fn validate_variant_prices_requires_exact_currency_minor_units() {
        for (currency, amount, compare_at, valid) in [
            ("USD", Decimal::new(1999, 3), None, false),
            ("USD", Decimal::new(199, 2), Some(Decimal::new(2001, 3)), false),
            ("JPY", Decimal::new(101, 1), None, false),
            ("JPY", Decimal::new(10, 0), None, true),
            ("KWD", Decimal::new(1999, 3), Some(Decimal::new(2001, 3)), true),
            ("USD", Decimal::MAX, None, false),
        ] {
            let price = PriceInput {
                currency_code: currency.to_owned(),
                channel_id: None,
                channel_slug: None,
                amount,
                compare_at_amount: compare_at,
            };
            assert_eq!(validate_variant_prices(&[price]).is_ok(), valid, "{currency}");
        }
    }

    #[test]
    fn validate_variant_prices_rejects_non_ascii_or_non_alphabetic_currency_codes() {
        for currency_code in ["1$!", "US1", "UŠD", ""] {
            let prices = vec![PriceInput {
                currency_code: currency_code.to_owned(),
                channel_id: None,
                channel_slug: None,
                amount: Decimal::new(1000, 2),
                compare_at_amount: None,
            }];
            assert!(
                validate_variant_prices(&prices).is_err(),
                "currency code {currency_code:?} must be rejected"
            );
        }

        let prices = vec![PriceInput {
            currency_code: " usd ".to_owned(),
            channel_id: None,
            channel_slug: None,
            amount: Decimal::new(1000, 2),
            compare_at_amount: None,
        }];
        assert!(validate_variant_prices(&prices).is_ok());
    }

    #[tokio::test]
    async fn sync_variant_auto_titles_seeds_missing_advances_auto_and_preserves_authored() {
        use sea_orm::TransactionTrait;

        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("sqlite in-memory db");
        db.execute_raw(sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            r#"
            CREATE TABLE product_variant_translations (
                id TEXT PRIMARY KEY,
                variant_id TEXT NOT NULL,
                locale TEXT NOT NULL,
                title TEXT
            )
            "#
            .to_string(),
        ))
        .await
        .expect("create variant translations");

        let variant_id = Uuid::new_v4();
        let seeded = |locale: &str, title: Option<&str>| {
            entities::variant_translation::ActiveModel {
                id: Set(generate_id()),
                variant_id: Set(variant_id),
                locale: Set(locale.to_owned()),
                title: Set(title.map(str::to_owned)),
            }
        };

        // ru is operator-authored; de is an empty row; fr still holds the
        // previous auto-generated value; en has no row at all.
        for model in [
            seeded("ru", Some("Ручное название")),
            seeded("de", Some("")),
            seeded("fr", Some("Red / M")),
        ] {
            model.insert(&db).await.expect("seed translation row");
        }

        let locales = vec![
            "en".to_owned(),
            "ru".to_owned(),
            "de".to_owned(),
            "fr".to_owned(),
        ];
        let previous_auto = HashMap::from([
            ("en".to_owned(), "Red / M".to_owned()),
            ("fr".to_owned(), "Red / M".to_owned()),
        ]);
        let next_auto = HashMap::from([
            ("en".to_owned(), "Blue / M".to_owned()),
            ("fr".to_owned(), "Blue / M".to_owned()),
            ("de".to_owned(), "Blue / M".to_owned()),
        ]);

        let txn = db.begin().await.expect("begin transaction");
        sync_variant_auto_titles_in_tx(
            &txn,
            variant_id,
            &locales,
            &previous_auto,
            &next_auto,
        )
        .await
        .expect("sync auto titles");
        txn.commit().await.expect("commit");

        let mut rows = entities::variant_translation::Entity::find()
            .filter(entities::variant_translation::Column::VariantId.eq(variant_id))
            .all(&db)
            .await
            .expect("read translations");
        rows.sort_by_key(|row| row.locale.clone());
        let titles: Vec<(&str, Option<&str>)> = rows
            .iter()
            .map(|row| (row.locale.as_str(), row.title.as_deref()))
            .collect();

        assert_eq!(
            titles,
            vec![
                ("de", Some("Blue / M")), // empty rows are seeded with the next auto title
                ("en", Some("Blue / M")), // missing locales are seeded
                ("fr", Some("Blue / M")), // the previous auto value advances
                ("ru", Some("Ручное название")), // authored copy survives
            ]
        );
    }
}
