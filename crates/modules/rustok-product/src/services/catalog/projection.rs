use super::*;
use crate::ProductFulfillmentRequirement;
use rustok_api::{PortContext, PortError, PortErrorKind};
use rustok_core::{error::Error as CoreError, field_schema::CustomFieldsSchema};
use sea_orm::{DbBackend, FromQueryResult};

struct ProductProjectionRead<'a> {
    locale: &'a str,
    fallback_locale: Option<&'a str>,
    variant_limit: Option<u64>,
    schema: ProductProjectionSchema<'a>,
}

enum ProductProjectionSchema<'a> {
    Connection,
    CatalogRead(&'a PortContext),
    Pinned(&'a CustomFieldsSchema),
}

async fn catalog_flex_schema(
    port: &dyn flex_ports::FlexAttachedSchemaReadPort,
    context: &PortContext,
    tenant_id: Uuid,
) -> CommerceResult<CustomFieldsSchema> {
    let donor = flex_ports::PRODUCT_ENTITY_TYPE;
    let snapshot = port
        .read_attached_schema(context.clone(), donor)
        .await
        .map_err(flex_schema_error)?;
    snapshot
        .verify_scope(tenant_id, donor)
        .map_err(flex_schema_error)?;
    let definitions = snapshot
        .definitions
        .iter()
        .map(rustok_core::field_schema::field_definition_from_source)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| CoreError::Internal("Flex attached schema is invalid".into()))?;
    Ok(CustomFieldsSchema::new(definitions))
}

fn flex_schema_error(error: PortError) -> CommerceError {
    match error.kind {
        PortErrorKind::Forbidden => {
            CoreError::Forbidden("Flex attached schema access denied".into())
        }
        PortErrorKind::InvariantViolation => {
            CoreError::Internal("Flex attached schema is invalid".into())
        }
        _ => CoreError::External("Flex attached schema is unavailable".into()),
    }
    .into()
}

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
    fallback_locale: Option<&str>,
    bounded: bool,
) -> CommerceResult<Vec<VariantAxisConfigResponse>> {
    let fallback_locale = fallback_locale.unwrap_or(PLATFORM_FALLBACK_LOCALE);
    let axis_limit = if bounded {
        format!(" LIMIT {}", MAX_VARIANT_AXES + 1)
    } else {
        String::new()
    };
    let value_limit = if bounded {
        format!(" LIMIT {}", MAX_VARIANT_AXES * MAX_VARIANT_AXIS_VALUES + 1)
    } else {
        String::new()
    };
    let axes = AxisRow::find_by_statement(Statement::from_sql_and_values(
        db.get_database_backend(),
        format!(r#"
        SELECT ax.id, ax.product_id, ax.attribute_id, ax.position, a.code, COALESCE(pat.label, fallback_pat.label) AS name
        FROM product_variant_axes ax
        JOIN product_attributes a ON a.id = ax.attribute_id AND a.tenant_id = ax.tenant_id
        LEFT JOIN product_attribute_translations pat ON pat.attribute_id = a.id AND pat.locale = $3
        LEFT JOIN product_attribute_translations fallback_pat ON fallback_pat.attribute_id = a.id AND fallback_pat.locale = $4
        WHERE ax.tenant_id = $1 AND ax.product_id = $2
        ORDER BY ax.position ASC, ax.id ASC{axis_limit}
        "#),
        vec![tenant_id.into(), product_id.into(), locale.into(), fallback_locale.into()],
    ))
    .all(db)
    .await?;

    if bounded && axes.len() > MAX_VARIANT_AXES {
        return Err(CommerceError::Validation(
            "storefront axis selection exceeds its read bound".into(),
        ));
    }
    if axes.is_empty() {
        return Ok(Vec::new());
    }

    let values = AxisValueRow::find_by_statement(Statement::from_sql_and_values(
        db.get_database_backend(),
        format!(r#"
        SELECT av.axis_id, av.option_id, av.position, pao.code, COALESCE(paot.label, fallback_paot.label) AS label
        FROM product_variant_axis_values av
        JOIN product_variant_axes ax ON ax.id = av.axis_id AND ax.tenant_id = av.tenant_id
        JOIN product_attribute_options pao ON pao.id = av.option_id AND pao.tenant_id = av.tenant_id
        LEFT JOIN product_attribute_option_translations paot ON paot.option_id = pao.id AND paot.locale = $3
        LEFT JOIN product_attribute_option_translations fallback_paot ON fallback_paot.option_id = pao.id AND fallback_paot.locale = $4
        WHERE av.tenant_id = $1 AND ax.product_id = $2
        ORDER BY av.position ASC, av.option_id ASC{value_limit}
        "#),
        vec![tenant_id.into(), product_id.into(), locale.into(), fallback_locale.into()],
    ))
    .all(db)
    .await?;

    if bounded && values.len() > MAX_VARIANT_AXES * MAX_VARIANT_AXIS_VALUES {
        return Err(CommerceError::Validation(
            "storefront axis values exceed their read bound".into(),
        ));
    }
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

    if bounded
        && values_by_axis
            .values()
            .any(|values| values.len() > MAX_VARIANT_AXIS_VALUES)
    {
        return Err(CommerceError::Validation(
            "storefront axis values exceed their per-axis read bound".into(),
        ));
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
    fallback_locale: &str,
    bounded: bool,
) -> CommerceResult<HashMap<Uuid, Vec<VariantAxisValueResponse>>> {
    if variant_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let backend = db.get_database_backend();
    let placeholders = variant_ids
        .iter()
        .enumerate()
        .map(|(index, _)| sql_placeholder(backend, index + 4))
        .collect::<Vec<_>>()
        .join(",");
    let assignment_bound = MAX_STOREFRONT_PRODUCT_VARIANTS as usize * MAX_VARIANT_AXES;
    let assignment_limit = if bounded {
        format!(" LIMIT {}", assignment_bound + 1)
    } else {
        String::new()
    };
    let query = format!(
        r#"
        SELECT pvav.variant_id, pvav.attribute_id, pvao.option_id, a.code as attribute_code,
               COALESCE(paot.label, fallback_paot.label, pao.code) AS label
        FROM product_variant_attribute_values pvav
        JOIN product_variants pv
          ON pv.id = pvav.variant_id AND pv.tenant_id = pvav.tenant_id
        JOIN product_variant_axes axis
          ON axis.tenant_id = pv.tenant_id
         AND axis.product_id = pv.product_id
         AND axis.attribute_id = pvav.attribute_id
        JOIN product_variant_attribute_value_options pvao
          ON pvao.tenant_id = pvav.tenant_id AND pvao.value_id = pvav.id
        JOIN product_attributes a ON a.id = pvav.attribute_id AND a.tenant_id = pvav.tenant_id
        JOIN product_attribute_options pao ON pao.id = pvao.option_id AND pao.tenant_id = pvao.tenant_id
        LEFT JOIN product_attribute_option_translations paot ON paot.option_id = pao.id AND paot.locale = {locale_ph}
        LEFT JOIN product_attribute_option_translations fallback_paot ON fallback_paot.option_id = pao.id AND fallback_paot.locale = {fallback_ph}
        WHERE pvav.tenant_id = {tenant_ph} AND pvav.detached_at IS NULL AND pvav.variant_id IN ({placeholders})
        ORDER BY pvav.variant_id ASC, pvav.attribute_id ASC, pvao.option_id ASC{assignment_limit}
        "#,
        locale_ph = sql_placeholder(backend, 2),
        fallback_ph = sql_placeholder(backend, 3),
        tenant_ph = sql_placeholder(backend, 1),
    );

    // SQLite binds anonymous `?` markers in textual order and the locale joins appear
    // before the tenant predicate; PostgreSQL binds its numbered placeholders instead.
    let mut values = match backend {
        DbBackend::Sqlite => vec![locale.into(), fallback_locale.into(), tenant_id.into()],
        _ => vec![tenant_id.into(), locale.into(), fallback_locale.into()],
    };
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

    if bounded && rows.len() > assignment_bound {
        return Err(CommerceError::Validation(
            "storefront SKU assignments exceed their read bound".into(),
        ));
    }
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

#[derive(Debug, FromQueryResult)]
struct VariantTitlePartRow {
    variant_id: Uuid,
    label: String,
}

/// Loads one display title per variant from the localized option labels of its
/// axis assignments, ordered by the product's axis positions.
///
/// This is the write-side source of the seeded `variant_translations.title` and the
/// fallback for read-side titles: a human-readable projection of the sparse tuple
/// (`"Red / M"`), never the machine `combination_identity`. An option without a
/// translation in the requested or fallback locale falls back to its dictionary
/// code; a variant without assignments is titled `Default`.
pub async fn load_variant_display_titles<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    product_id: Uuid,
    variant_ids: &[Uuid],
    locale: &str,
    fallback_locale: &str,
) -> CommerceResult<HashMap<Uuid, String>> {
    if variant_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let backend = db.get_database_backend();
    let variant_placeholders = variant_ids
        .iter()
        .enumerate()
        .map(|(index, _)| sql_placeholder(backend, index + 5))
        .collect::<Vec<_>>()
        .join(", ");
    // SQLite binds anonymous `?` markers in textual order and the locale joins
    // appear before the tenant predicate; PostgreSQL binds numbered placeholders.
    let mut values = match backend {
        DbBackend::Sqlite => vec![
            locale.into(),
            fallback_locale.into(),
            tenant_id.into(),
            product_id.into(),
        ],
        _ => vec![
            tenant_id.into(),
            locale.into(),
            fallback_locale.into(),
            product_id.into(),
        ],
    };
    for id in variant_ids {
        values.push((*id).into());
    }
    let rows = VariantTitlePartRow::find_by_statement(Statement::from_sql_and_values(
        backend,
        format!(
            r#"
            SELECT pvav.variant_id,
                   COALESCE(paot.label, fallback_paot.label, pao.code) AS label
            FROM product_variant_attribute_values pvav
            JOIN product_variants pv
              ON pv.id = pvav.variant_id AND pv.tenant_id = pvav.tenant_id
            JOIN product_variant_axes axis
              ON axis.tenant_id = pvav.tenant_id
             AND axis.product_id = pv.product_id
             AND axis.attribute_id = pvav.attribute_id
            JOIN product_variant_attribute_value_options pvao
              ON pvao.tenant_id = pvav.tenant_id AND pvao.value_id = pvav.id
            JOIN product_attribute_options pao
              ON pao.id = pvao.option_id AND pao.tenant_id = pvav.tenant_id
            LEFT JOIN product_attribute_option_translations paot
              ON paot.option_id = pao.id AND paot.locale = {locale_ph}
            LEFT JOIN product_attribute_option_translations fallback_paot
              ON fallback_paot.option_id = pao.id AND fallback_paot.locale = {fallback_ph}
            WHERE pvav.tenant_id = {tenant_ph}
              AND pvav.detached_at IS NULL
              AND axis.product_id = {product_ph}
              AND pvav.variant_id IN ({variant_placeholders})
            ORDER BY pvav.variant_id ASC, axis.position ASC, axis.id ASC, pvao.option_id ASC
            "#,
            locale_ph = sql_placeholder(backend, 2),
            fallback_ph = sql_placeholder(backend, 3),
            tenant_ph = sql_placeholder(backend, 1),
            product_ph = sql_placeholder(backend, 4),
        ),
        values,
    ))
    .all(db)
    .await?;

    let mut parts_by_variant: HashMap<Uuid, Vec<String>> = HashMap::new();
    for row in rows {
        parts_by_variant
            .entry(row.variant_id)
            .or_default()
            .push(row.label);
    }
    Ok(parts_by_variant
        .into_iter()
        .map(|(variant_id, labels)| {
            let references = labels.iter().map(String::as_str).collect::<Vec<_>>();
            (
                variant_id,
                generate_variant_title_from_axis_values(references.as_slice()),
            )
        })
        .collect())
}

pub(crate) fn resolve_image_alt_text(
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
    /// SEO projection for one product: product row + translations + images only.
    /// Four point queries instead of the full detail projection (variants, prices,
    /// inventory, tags, attributes are invisible to SEO mapping).
    pub async fn get_product_seo_projection(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: &str,
        fallback_locale: &str,
    ) -> CommerceResult<Option<crate::dto::ProductSeoProjection>> {
        let Some(product) = entities::product::Entity::find_by_id(product_id)
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
        else {
            return Ok(None);
        };
        let translations = entities::product_translation::Entity::find()
            .filter(entities::product_translation::Column::TenantId.eq(tenant_id))
            .filter(entities::product_translation::Column::ProductId.eq(product_id))
            .order_by_asc(entities::product_translation::Column::Locale)
            .all(&self.db)
            .await?;
        let images = entities::product_image::Entity::find()
            .filter(entities::product_image::Column::ProductId.eq(product_id))
            .order_by_asc(entities::product_image::Column::Position)
            .all(&self.db)
            .await?;
        let mut image_translations =
            Self::load_product_image_translation_map(&self.db, &images).await?;

        Ok(Some(Self::assemble_product_seo_projection(
            product,
            translations,
            images,
            &mut image_translations,
            locale,
            fallback_locale,
        )))
    }

    /// SEO projections for a page of products: four set-based queries regardless of
    /// page size, so scans (diagnostics state loads, bulk summaries, sitemap
    /// candidates) stop paying per-product query fan-out.
    pub async fn get_product_seo_projections(
        &self,
        tenant_id: Uuid,
        product_ids: &[Uuid],
        locale: &str,
        fallback_locale: &str,
    ) -> CommerceResult<Vec<crate::dto::ProductSeoProjection>> {
        if product_ids.is_empty() {
            return Ok(Vec::new());
        }
        let products = entities::product::Entity::find()
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .filter(entities::product::Column::Id.is_in(product_ids.to_vec()))
            .order_by_asc(entities::product::Column::Id)
            .all(&self.db)
            .await?;
        if products.is_empty() {
            return Ok(Vec::new());
        }
        let loaded_ids = products
            .iter()
            .map(|product| product.id)
            .collect::<Vec<_>>();
        let translations = entities::product_translation::Entity::find()
            .filter(entities::product_translation::Column::TenantId.eq(tenant_id))
            .filter(entities::product_translation::Column::ProductId.is_in(loaded_ids.clone()))
            .order_by_asc(entities::product_translation::Column::Locale)
            .all(&self.db)
            .await?;
        let images = entities::product_image::Entity::find()
            .filter(entities::product_image::Column::ProductId.is_in(loaded_ids))
            .order_by_asc(entities::product_image::Column::Position)
            .all(&self.db)
            .await?;
        let mut image_translations =
            Self::load_product_image_translation_map(&self.db, &images).await?;

        let mut translations_by_product: HashMap<Uuid, Vec<entities::product_translation::Model>> =
            HashMap::new();
        for translation in translations {
            translations_by_product
                .entry(translation.product_id)
                .or_default()
                .push(translation);
        }
        let mut images_by_product: HashMap<Uuid, Vec<entities::product_image::Model>> =
            HashMap::new();
        for image in images {
            images_by_product
                .entry(image.product_id)
                .or_default()
                .push(image);
        }

        let mut projections = Vec::with_capacity(products.len());
        for product in products {
            let product_translations = translations_by_product
                .remove(&product.id)
                .unwrap_or_default();
            let product_images = images_by_product.remove(&product.id).unwrap_or_default();
            projections.push(Self::assemble_product_seo_projection(
                product,
                product_translations,
                product_images,
                &mut image_translations,
                locale,
                fallback_locale,
            ));
        }
        Ok(projections)
    }

    async fn load_product_image_translation_map(
        db: &sea_orm::DatabaseConnection,
        images: &[entities::product_image::Model],
    ) -> CommerceResult<HashMap<Uuid, Vec<entities::product_image_translation::Model>>> {
        if images.is_empty() {
            return Ok(HashMap::new());
        }
        let image_ids = images.iter().map(|image| image.id).collect::<Vec<_>>();
        let translations = entities::product_image_translation::Entity::find()
            .filter(entities::product_image_translation::Column::ImageId.is_in(image_ids))
            .order_by_asc(entities::product_image_translation::Column::Locale)
            .all(db)
            .await?;
        let mut by_image: HashMap<Uuid, Vec<entities::product_image_translation::Model>> =
            HashMap::new();
        for translation in translations {
            by_image
                .entry(translation.image_id)
                .or_default()
                .push(translation);
        }
        Ok(by_image)
    }

    #[allow(clippy::too_many_arguments)]
    fn assemble_product_seo_projection(
        product: entities::product::Model,
        translations: Vec<entities::product_translation::Model>,
        images: Vec<entities::product_image::Model>,
        image_translations: &mut HashMap<Uuid, Vec<entities::product_image_translation::Model>>,
        locale: &str,
        fallback_locale: &str,
    ) -> crate::dto::ProductSeoProjection {
        crate::dto::ProductSeoProjection {
            id: product.id,
            status: product.status.clone().into(),
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
            images: images
                .into_iter()
                .map(|image| {
                    let translations = image_translations.remove(&image.id).unwrap_or_default();
                    let alt_text =
                        resolve_image_alt_text(&translations, locale, Some(fallback_locale));
                    ProductImageResponse {
                        id: image.id,
                        media_id: image.media_id,
                        url: format_product_media_url(image.media_id),
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
        }
    }

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
        self.get_product_in(
            &self.db,
            tenant_id,
            product_id,
            locale,
            fallback_locale,
            None,
        )
        .await
    }

    pub(crate) async fn get_product_for_catalog_read(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        context: &PortContext,
    ) -> CommerceResult<ProductResponse> {
        self.get_product_projection_in(
            &self.db,
            tenant_id,
            product_id,
            ProductProjectionRead {
                locale,
                fallback_locale,
                variant_limit: None,
                schema: ProductProjectionSchema::CatalogRead(context),
            },
        )
        .await
    }

    /// Build the public snapshot on the write connection before its commit.
    pub(super) async fn get_product_in<
        C: ConnectionTrait
            + crate::ports::pricing::AsPricingConnection
            + crate::ports::AsInventoryConnection
            + crate::ports::AsTaxonomyConnection,
    >(
        &self,
        conn: &C,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        variant_limit: Option<u64>,
    ) -> CommerceResult<ProductResponse> {
        self.get_product_projection_in(
            conn,
            tenant_id,
            product_id,
            ProductProjectionRead {
                locale,
                fallback_locale,
                variant_limit,
                schema: ProductProjectionSchema::Connection,
            },
        )
        .await
    }

    /// Project the command result, reusing the definitions from preparation when available.
    pub(super) async fn get_product_write_response(
        &self,
        txn: &ProductWriteTransaction,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: &str,
        schema: Option<&CustomFieldsSchema>,
    ) -> CommerceResult<ProductResponse> {
        self.get_product_projection_in(
            txn,
            tenant_id,
            product_id,
            ProductProjectionRead {
                locale,
                fallback_locale: None,
                variant_limit: None,
                schema: schema.map_or(
                    ProductProjectionSchema::Connection,
                    ProductProjectionSchema::Pinned,
                ),
            },
        )
        .await
    }

    async fn get_product_projection_in<
        C: ConnectionTrait
            + crate::ports::pricing::AsPricingConnection
            + crate::ports::AsInventoryConnection
            + crate::ports::AsTaxonomyConnection,
    >(
        &self,
        conn: &C,
        tenant_id: Uuid,
        product_id: Uuid,
        read: ProductProjectionRead<'_>,
    ) -> CommerceResult<ProductResponse> {
        let ProductProjectionRead {
            locale,
            fallback_locale,
            variant_limit,
            schema,
        } = read;
        debug!(product_id = %product_id, "Fetching product");

        let product = entities::product::Entity::find_by_id(product_id)
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .one(conn)
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
                        .filter(entities::product_translation::Column::TenantId.eq(tenant_id))
                        .filter(entities::product_translation::Column::ProductId.eq(product_id))
                        .all(conn)
                        .await?,
                )
            },
            load_product_variant_axes(
                conn,
                tenant_id,
                product_id,
                locale,
                fallback_locale,
                variant_limit.is_some()
            ),
            async {
                Ok::<_, CommerceError>({
                    let mut query = entities::product_variant::Entity::find()
                        .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
                        .filter(entities::product_variant::Column::ProductId.eq(product_id))
                        .order_by_asc(entities::product_variant::Column::Position)
                        .order_by_asc(entities::product_variant::Column::Id);
                    if let Some(limit) = variant_limit {
                        let probe = limit.checked_add(1).ok_or_else(|| {
                            CommerceError::Validation(
                                "public SKU read bound is invalid".to_string(),
                            )
                        })?;
                        query = query.limit(probe);
                    }
                    let rows = query.all(conn).await?;
                    if variant_limit.is_some_and(|limit| rows.len() as u64 > limit) {
                        return Err(CommerceError::Validation(format!(
                            "storefront selection supports at most {MAX_STOREFRONT_PRODUCT_VARIANTS} real SKUs; reconfigure the product before publishing",
                        )));
                    }
                    rows
                })
            },
            async {
                Ok::<_, CommerceError>(
                    entities::product_image::Entity::find()
                        .filter(entities::product_image::Column::ProductId.eq(product_id))
                        .order_by_asc(entities::product_image::Column::Position)
                        .all(conn)
                        .await?,
                )
            },
            self.load_product_tags_in(
                conn,
                tenant_id,
                product_id,
                tag_locale,
                fallback_locale.or(Some(PLATFORM_FALLBACK_LOCALE)),
                match schema {
                    ProductProjectionSchema::CatalogRead(context) => Some(context),
                    ProductProjectionSchema::Connection | ProductProjectionSchema::Pinned(_) => None,
                },
            ),
            async {
                let fallback = fallback_locale.unwrap_or(PLATFORM_FALLBACK_LOCALE);
                let loaded;
                let schema = match schema {
                    ProductProjectionSchema::Pinned(schema) => schema,
                    ProductProjectionSchema::CatalogRead(context)
                        if self.flex_schema_source == ProductFlexSchemaSource::Shared
                            && let Some(port) = &self.flex_schema_read_port =>
                    {
                        loaded = catalog_flex_schema(port.as_ref(), context, tenant_id).await?;
                        &loaded
                    }
                    ProductProjectionSchema::Connection
                    | ProductProjectionSchema::CatalogRead(_) => {
                        loaded = self.flex_schema_source.load(conn, tenant_id).await?;
                        &loaded
                    }
                };
                resolve_product_metadata_with_schema(
                    conn,
                    tenant_id,
                    product_id,
                    &product.metadata,
                    locale,
                    fallback,
                    schema,
                )
                .await
            },
        )?;

        let variant_ids: Vec<Uuid> = variants.iter().map(|v| v.id).collect();
        let (
            all_prices,
            variant_translations,
            available_inventory_by_variant,
            mut axis_values_by_variant,
        ) = tokio::try_join!(
            async {
                if variant_ids.is_empty() {
                    Ok::<_, CommerceError>(Vec::new())
                } else if let Some(port) = &self.pricing_port {
                    port.load_catalog_base_prices_for_variants(
                        conn.as_pricing_connection(),
                        &variant_ids,
                    )
                    .await
                } else {
                    Ok(Vec::new())
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
                            .all(conn)
                            .await?,
                    )
                }
            },
            async {
                if variant_ids.is_empty() {
                    Ok::<_, CommerceError>(HashMap::new())
                } else if let Some(port) = &self.inventory_port {
                    port.load_available_quantities(conn.as_inventory_connection(), tenant_id, &variant_ids)
                        .await
                } else {
                    Ok(HashMap::new())
                }
            },
            load_variant_axis_values(
                conn,
                tenant_id,
                &variant_ids,
                locale,
                fallback_locale.unwrap_or(PLATFORM_FALLBACK_LOCALE),
                variant_limit.is_some()
            ),
        )?;

        let mut prices_by_variant: HashMap<Uuid, Vec<CatalogBasePriceSnapshot>> = HashMap::new();
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

        let variant_title_fallback_locale = fallback_locale.unwrap_or(PLATFORM_FALLBACK_LOCALE);
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

                let translations = variant_translations_by_variant
                    .remove(&variant.id)
                    .unwrap_or_default();
                let axis_values = axis_values_by_variant
                    .remove(&variant.id)
                    .unwrap_or_default();
                let title = variant_display_title(
                    &translations,
                    &axis_values,
                    locale,
                    variant_title_fallback_locale,
                );
                let available_inventory = resolve_variant_inventory_quantity(
                    variant.id,
                    variant.inventory_quantity,
                    &available_inventory_by_variant,
                );

                VariantResponse {
                    id: variant.id,
                    revision: variant.revision.to_string(),
                    product_id: variant.product_id,
                    sku: variant.sku,
                    barcode: variant.barcode,
                    shipping_profile_slug: variant.shipping_profile_slug.clone(),
                    title,
                    translations: translations
                        .into_iter()
                        .map(|translation| VariantTranslationResponse {
                            locale: translation.locale,
                            title: translation.title,
                        })
                        .collect(),
                    combination_identity: variant.combination_identity,
                    axis_values,
                    prices: price_responses,
                    inventory_quantity: available_inventory,
                    inventory_policy: variant.inventory_policy.clone(),
                    in_stock: available_inventory > 0
                        || inventory_policy_allows_backorder(&variant.inventory_policy),
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
                .all(conn)
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

        let fulfillment_requirement =
            ProductFulfillmentRequirement::from_product_type(product.product_type.as_deref());

        let product_brands = entities::product_brand::Entity::find()
            .filter(entities::product_brand::Column::TenantId.eq(tenant_id))
            .filter(entities::product_brand::Column::ProductId.eq(product_id))
            .order_by_desc(entities::product_brand::Column::IsPrimary)
            .order_by_asc(entities::product_brand::Column::Position)
            .order_by_asc(entities::product_brand::Column::CreatedAt)
            .all(conn)
            .await?;

        let brand_responses: Vec<crate::dto::ProductBrandResponse> = product_brands
            .into_iter()
            .map(|brand| crate::dto::ProductBrandResponse {
                id: brand.id,
                brand_id: brand.brand_id,
                is_primary: brand.is_primary,
                role: brand.role,
                position: brand.position,
            })
            .collect();

        let response = ProductResponse {
            id: product.id,
            tenant_id: product.tenant_id,
            status: product.status.into(),
            seller_id: product.seller_id,
            vendor: product.vendor,
            product_type: product.product_type,
            fulfillment_requirement,
            shipping_profile_slug: product
                .shipping_profile_slug
                .clone()
                .or_else(|| extract_shipping_profile_slug(&product.metadata)),
            primary_category_id: product.primary_category_id,
            brands: brand_responses,
            tags: product_tags.tags,
            metadata: resolved_metadata,
            created_at: product.created_at.into(),
            updated_at: product.updated_at.into(),
            published_at: product.published_at.map(Into::into),
            revision: product.revision,
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
                        url: format_product_media_url(image.media_id),
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
            // Admin reads resolve attribute values through the catalog schema service; the
            // storefront projection fills this block for published detail reads.
            storefront_attributes: Vec::new(),
        };

        debug!(
            product_id = %product_id,
            variants_count = response.variants.len(),
            "Product fetched successfully"
        );

        Ok(response)
    }

    pub(super) async fn get_variant_in<
        C: ConnectionTrait
            + crate::ports::pricing::AsPricingConnection
            + crate::ports::AsInventoryConnection
            + crate::ports::AsTaxonomyConnection,
    >(
        &self,
        conn: &C,
        tenant_id: Uuid,
        variant_id: Uuid,
    ) -> CommerceResult<VariantResponse> {
        let variant = entities::product_variant::Entity::find_by_id(variant_id)
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .one(conn)
            .await?
            .ok_or(CommerceError::VariantNotFound(variant_id))?;

        let variant_ids = [variant_id];
        let (prices, translations, available_inventory_by_variant, mut axis_values_by_variant) = tokio::try_join!(
            async {
                if let Some(port) = &self.pricing_port {
                    port.load_catalog_base_prices_for_variants(
                        conn.as_pricing_connection(),
                        &variant_ids,
                    )
                    .await
                } else {
                    Ok(Vec::new())
                }
            },
            async {
                Ok::<_, CommerceError>(
                    entities::variant_translation::Entity::find()
                        .filter(entities::variant_translation::Column::VariantId.eq(variant_id))
                        .order_by_asc(entities::variant_translation::Column::Locale)
                        .all(conn)
                        .await?,
                )
            },
            async {
                if variant_ids.is_empty() {
                    Ok::<_, CommerceError>(HashMap::new())
                } else if let Some(port) = &self.inventory_port {
                    port.load_available_quantities(conn.as_inventory_connection(), tenant_id, &variant_ids)
                        .await
                } else {
                    Ok(HashMap::new())
                }
            },
            load_variant_axis_values(
                conn,
                tenant_id,
                &variant_ids,
                PLATFORM_FALLBACK_LOCALE,
                PLATFORM_FALLBACK_LOCALE,
                false
            ),
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

        let axis_values = axis_values_by_variant
            .remove(&variant_id)
            .unwrap_or_default();
        let title = variant_display_title(
            &translations,
            &axis_values,
            PLATFORM_FALLBACK_LOCALE,
            PLATFORM_FALLBACK_LOCALE,
        );
        let available_inventory = resolve_variant_inventory_quantity(
            variant.id,
            variant.inventory_quantity,
            &available_inventory_by_variant,
        );

        Ok(VariantResponse {
            id: variant.id,
            revision: variant.revision.to_string(),
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
            axis_values,
            prices: price_responses,
            inventory_quantity: available_inventory,
            inventory_policy: variant.inventory_policy.clone(),
            in_stock: available_inventory > 0
                || inventory_policy_allows_backorder(&variant.inventory_policy),
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
    fn legacy_inventory_is_used_only_when_inventory_has_no_item_identity() {
        let variant_id = Uuid::new_v4();
        let missing_inventory = HashMap::new();
        assert_eq!(
            resolve_variant_inventory_quantity(variant_id, 7, &missing_inventory),
            7,
        );

        let authoritative_zero = HashMap::from([(variant_id, 0)]);
        assert_eq!(
            resolve_variant_inventory_quantity(variant_id, 7, &authoritative_zero),
            0,
        );

        let authoritative_stock = HashMap::from([(variant_id, 12)]);
        assert_eq!(
            resolve_variant_inventory_quantity(variant_id, 7, &authoritative_stock),
            12,
        );
    }

    #[test]
    fn backorder_policy_is_case_insensitive_and_trimmed_in_catalog_projection() {
        assert!(inventory_policy_allows_backorder("continue"));
        assert!(inventory_policy_allows_backorder(" CONTINUE "));
        assert!(inventory_policy_allows_backorder("Continue"));
        assert!(!inventory_policy_allows_backorder("deny"));
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
    async fn variant_projection_db() -> sea_orm::DatabaseConnection {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("sqlite in-memory db");
        db.execute_raw(sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            r#"
            CREATE TABLE product_variants (
                id TEXT PRIMARY KEY,
                tenant_id TEXT NOT NULL,
                product_id TEXT NOT NULL,
                sku TEXT,
                position INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE product_variant_axes (
                id TEXT PRIMARY KEY,
                tenant_id TEXT NOT NULL,
                product_id TEXT NOT NULL,
                attribute_id TEXT NOT NULL,
                position INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE product_variant_attribute_values (
                id TEXT PRIMARY KEY,
                tenant_id TEXT NOT NULL,
                variant_id TEXT NOT NULL,
                attribute_id TEXT NOT NULL,
                detached_at TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE product_variant_attribute_value_options (
                tenant_id TEXT NOT NULL,
                value_id TEXT NOT NULL,
                option_id TEXT NOT NULL,
                PRIMARY KEY (value_id, option_id)
            );
            CREATE TABLE product_attributes (
                id TEXT PRIMARY KEY,
                tenant_id TEXT NOT NULL,
                code TEXT NOT NULL
            );
            CREATE TABLE product_attribute_options (
                id TEXT PRIMARY KEY,
                tenant_id TEXT NOT NULL,
                code TEXT NOT NULL
            );
            CREATE TABLE product_attribute_option_translations (
                id TEXT PRIMARY KEY,
                option_id TEXT NOT NULL,
                locale TEXT NOT NULL,
                label TEXT NOT NULL
            );
            "#
            .to_string(),
        ))
        .await
        .expect("create variant projection tables");
        db
    }

    async fn seed_variant_projection_fixture(
        db: &sea_orm::DatabaseConnection,
    ) -> (Uuid, Uuid, Uuid) {
        let tenant_id = Uuid::new_v4();
        let product_id = Uuid::new_v4();
        let variant_id = Uuid::new_v4();
        let size_attribute = Uuid::new_v4();
        let color_attribute = Uuid::new_v4();
        let size_axis = Uuid::new_v4();
        let color_axis = Uuid::new_v4();
        let medium_option = Uuid::new_v4();
        let red_option = Uuid::new_v4();
        let size_value = Uuid::new_v4();
        let color_value = Uuid::new_v4();

        let backend = sea_orm::DatabaseBackend::Sqlite;
        for (sql, values) in [
            (
                "INSERT INTO product_variants (id, tenant_id, product_id) VALUES (?, ?, ?)",
                vec![variant_id.into(), tenant_id.into(), product_id.into()],
            ),
            (
                "INSERT INTO product_variant_axes (id, tenant_id, product_id, attribute_id, position) VALUES (?, ?, ?, ?, ?)",
                vec![
                    color_axis.into(),
                    tenant_id.into(),
                    product_id.into(),
                    color_attribute.into(),
                    1_i32.into(),
                ],
            ),
            (
                "INSERT INTO product_variant_axes (id, tenant_id, product_id, attribute_id, position) VALUES (?, ?, ?, ?, ?)",
                vec![
                    size_axis.into(),
                    tenant_id.into(),
                    product_id.into(),
                    size_attribute.into(),
                    0_i32.into(),
                ],
            ),
            (
                "INSERT INTO product_attributes (id, tenant_id, code) VALUES (?, ?, ?)",
                vec![size_attribute.into(), tenant_id.into(), "size".into()],
            ),
            (
                "INSERT INTO product_attributes (id, tenant_id, code) VALUES (?, ?, ?)",
                vec![color_attribute.into(), tenant_id.into(), "color".into()],
            ),
            (
                "INSERT INTO product_attribute_options (id, tenant_id, code) VALUES (?, ?, ?)",
                vec![medium_option.into(), tenant_id.into(), "m".into()],
            ),
            (
                "INSERT INTO product_attribute_options (id, tenant_id, code) VALUES (?, ?, ?)",
                vec![red_option.into(), tenant_id.into(), "red".into()],
            ),
            (
                "INSERT INTO product_attribute_option_translations (id, option_id, locale, label) VALUES (?, ?, ?, ?)",
                vec![Uuid::new_v4().into(), medium_option.into(), "en".into(), "M".into()],
            ),
            (
                "INSERT INTO product_attribute_option_translations (id, option_id, locale, label) VALUES (?, ?, ?, ?)",
                vec![Uuid::new_v4().into(), red_option.into(), "en".into(), "Red".into()],
            ),
            (
                "INSERT INTO product_attribute_option_translations (id, option_id, locale, label) VALUES (?, ?, ?, ?)",
                vec![
                    Uuid::new_v4().into(),
                    red_option.into(),
                    "ru".into(),
                    "Красный".into(),
                ],
            ),
            (
                "INSERT INTO product_variant_attribute_values (id, tenant_id, variant_id, attribute_id) VALUES (?, ?, ?, ?)",
                vec![color_value.into(), tenant_id.into(), variant_id.into(), color_attribute.into()],
            ),
            (
                "INSERT INTO product_variant_attribute_values (id, tenant_id, variant_id, attribute_id) VALUES (?, ?, ?, ?)",
                vec![size_value.into(), tenant_id.into(), variant_id.into(), size_attribute.into()],
            ),
            (
                "INSERT INTO product_variant_attribute_value_options (tenant_id, value_id, option_id) VALUES (?, ?, ?)",
                vec![tenant_id.into(), color_value.into(), red_option.into()],
            ),
            (
                "INSERT INTO product_variant_attribute_value_options (tenant_id, value_id, option_id) VALUES (?, ?, ?)",
                vec![tenant_id.into(), size_value.into(), medium_option.into()],
            ),
        ] {
            db.execute_raw(sea_orm::Statement::from_sql_and_values(backend, sql, values))
                .await
                .expect("seed fixture row");
        }

        (tenant_id, product_id, variant_id)
    }

    #[tokio::test]
    async fn display_titles_order_parts_by_axis_position_and_localize() {
        let db = variant_projection_db().await;
        let (tenant_id, product_id, variant_id) = seed_variant_projection_fixture(&db).await;

        let titles = load_variant_display_titles(
            &db,
            tenant_id,
            product_id,
            &[variant_id],
            "ru",
            "en",
        )
        .await
        .expect("load ru titles");
        assert_eq!(
            titles.get(&variant_id).map(String::as_str),
            Some("M / Красный"),
            "parts follow axis positions, labels resolve in the requested locale"
        );

        let titles = load_variant_display_titles(
            &db,
            tenant_id,
            product_id,
            &[variant_id],
            "de",
            "en",
        )
        .await
        .expect("load de titles with en fallback");
        assert_eq!(
            titles.get(&variant_id).map(String::as_str),
            Some("M / Red"),
            "labels resolve through the fallback locale"
        );
    }

    #[tokio::test]
    async fn axis_order_refresh_advances_seeded_titles_without_overwriting_authored_locales() {
        use super::super::commands::VariantAutoTitleSnapshot;
        use sea_orm::TransactionTrait;

        let db = variant_projection_db().await;
        let (tenant_id, product_id, variant_id) = seed_variant_projection_fixture(&db).await;
        let backend = sea_orm::DatabaseBackend::Sqlite;
        db.execute_raw(Statement::from_string(
            backend,
            "CREATE TABLE product_translations (tenant_id TEXT, product_id TEXT, locale TEXT);
             CREATE TABLE product_variant_translations (id TEXT PRIMARY KEY, variant_id TEXT, locale TEXT, title TEXT);",
        ))
        .await
        .unwrap();
        for locale in ["en", "de"] {
            db.execute_raw(Statement::from_sql_and_values(
                backend,
                "INSERT INTO product_translations (tenant_id, product_id, locale) VALUES (?, ?, ?)",
                [tenant_id.into(), product_id.into(), locale.into()],
            ))
            .await
            .unwrap();
        }
        for (locale, title) in [("en", "M / Red"), ("ru", "Авторское название"), ("fr", "M / Red")] {
            entities::variant_translation::ActiveModel {
                id: Set(Uuid::new_v4()),
                variant_id: Set(variant_id),
                locale: Set(locale.to_owned()),
                title: Set(Some(title.to_owned())),
            }
            .insert(&db)
            .await
            .unwrap();
        }
        let txn = db.begin().await.unwrap();
        let titles = VariantAutoTitleSnapshot::capture(&txn, tenant_id, product_id, &[variant_id])
            .await
            .unwrap();
        txn.execute_raw(Statement::from_string(
            backend,
            "UPDATE product_variant_axes SET position = 1 - position",
        ))
        .await
        .unwrap();
        titles.refresh(&txn, tenant_id, product_id).await.unwrap();
        txn.commit().await.unwrap();

        let saved = entities::variant_translation::Entity::find()
            .filter(entities::variant_translation::Column::VariantId.eq(variant_id))
            .all(&db)
            .await
            .unwrap()
            .into_iter()
            .map(|row| (row.locale, row.title))
            .collect::<HashMap<_, _>>();
        assert_eq!(saved.len(), 4);
        assert_eq!(saved["en"].as_deref(), Some("Red / M"));
        assert_eq!(saved["fr"].as_deref(), Some("Red / M"));
        assert_eq!(saved["de"].as_deref(), Some("Red / M"));
        assert_eq!(saved["ru"].as_deref(), Some("Авторское название"));
    }

    #[tokio::test]
    async fn display_titles_fall_back_to_option_codes_and_default() {
        let db = variant_projection_db().await;
        let (tenant_id, product_id, variant_id) = seed_variant_projection_fixture(&db).await;
        let unassigned_variant = Uuid::new_v4();

        db.execute_raw(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO product_variants (id, tenant_id, product_id) VALUES (?, ?, ?)",
            vec![
                unassigned_variant.into(),
                tenant_id.into(),
                product_id.into(),
            ],
        ))
        .await
        .expect("insert unassigned variant");
        db.execute_raw(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "DELETE FROM product_attribute_option_translations",
            vec![],
        ))
        .await
        .expect("remove option translations");

        let titles = load_variant_display_titles(
            &db,
            tenant_id,
            product_id,
            &[variant_id, unassigned_variant],
            "en",
            "en",
        )
        .await
        .expect("load titles without translations");
        assert_eq!(
            titles.get(&variant_id).map(String::as_str),
            Some("m / red"),
            "dictionary codes back the labels when no locale resolves"
        );
        assert!(
            !titles.contains_key(&unassigned_variant),
            "a variant without assignments yields no parts; callers title it Default"
        );
    }

    #[tokio::test]
    async fn axis_values_resolve_labels_and_codes_with_fallback() {
        let db = variant_projection_db().await;
        let (tenant_id, _product_id, variant_id) = seed_variant_projection_fixture(&db).await;

        let values = load_variant_axis_values(
            &db,
            tenant_id,
            &[variant_id],
            "ru",
            "en",
            false,
        )
        .await
        .expect("load axis values");
        let mut values = values.get(&variant_id).cloned().unwrap_or_default();
        values.sort_by_key(|value| value.attribute_id);

        assert_eq!(values.len(), 2);
        assert!(values.iter().any(|value| {
            value.code.as_deref() == Some("color") && value.label.as_deref() == Some("Красный")
        }));
        assert!(values.iter().any(|value| {
            value.code.as_deref() == Some("size") && value.label.as_deref() == Some("M")
        }));

        let values = load_variant_axis_values(
            &db,
            tenant_id,
            &[variant_id],
            "de",
            "en",
            false,
        )
        .await
        .expect("load axis values with fallback");
        assert!(
            values
                .get(&variant_id)
                .is_some_and(|values| values.iter().all(|value| value
                    .label
                    .as_deref()
                    .is_some_and(|label| label == "Red" || label == "M"))),
            "labels resolve through the fallback locale"
        );
    }

    #[tokio::test]
    async fn axis_values_exclude_detached_assignments() {
        let db = variant_projection_db().await;
        let (tenant_id, _product_id, variant_id) = seed_variant_projection_fixture(&db).await;

        db.execute_raw(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "UPDATE product_variant_attribute_values SET detached_at = ? WHERE tenant_id = ? AND variant_id = ?",
            vec!["detached".into(), tenant_id.into(), variant_id.into()],
        ))
        .await
        .expect("detach assignments");

        let values = load_variant_axis_values(
            &db,
            tenant_id,
            &[variant_id],
            "en",
            "en",
            false,
        )
        .await
        .expect("load axis values");
        assert!(
            values.get(&variant_id).is_none_or(|values| values.is_empty()),
            "detached assignments never surface"
        );
    }
}
