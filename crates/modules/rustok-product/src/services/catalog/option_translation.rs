use super::*;

use rustok_api::TenantLocale;
use sea_orm::DbBackend;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProductOptionTranslationExactLocaleError {
    #[error(transparent)]
    Commerce(#[from] CommerceError),

    #[error("Product option not found: {0}")]
    OptionNotFound(Uuid),

    #[error("Product option translation source locale not found: {locale} for option {option_id}")]
    SourceLocaleNotFound { option_id: Uuid, locale: String },

    #[error("Product option translation locale is incomplete: {locale} for option {option_id}")]
    IncompleteLocale { option_id: Uuid, locale: String },

    #[error("Product option translation target locale missing after apply: {locale} for option {option_id}")]
    TargetLocaleMissingAfterApply { option_id: Uuid, locale: String },

    #[error("Product option translation value set does not match the current option values")]
    ValueSetMismatch,

    #[error("Product option translation {revision} revision conflict")]
    RevisionConflict { revision: &'static str },
}

impl From<sea_orm::DbErr> for ProductOptionTranslationExactLocaleError {
    fn from(error: sea_orm::DbErr) -> Self {
        Self::Commerce(CommerceError::Database(error))
    }
}

pub type ProductOptionTranslationExactLocaleResult<T> =
    Result<T, ProductOptionTranslationExactLocaleError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductOptionTranslationExactLocaleValueRecord {
    pub value_id: Uuid,
    pub position: i32,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductOptionTranslationExactLocaleRecord {
    pub locale: String,
    pub title: String,
    pub values: Vec<ProductOptionTranslationExactLocaleValueRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductOptionTranslationExactLocaleSnapshot {
    pub product_id: Uuid,
    pub option_id: Uuid,
    pub product_status: entities::product::ProductStatus,
    pub source_locale: String,
    pub target_locale: String,
    pub resource_revision: String,
    pub source_revision: String,
    pub target_revision: Option<String>,
    pub exact_locales: Vec<String>,
    pub source: ProductOptionTranslationExactLocaleRecord,
    pub target: Option<ProductOptionTranslationExactLocaleRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductOptionTranslationExactLocaleValueApply {
    pub value_id: Uuid,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductOptionTranslationExactLocaleApply {
    pub source_locale: String,
    pub target_locale: String,
    pub title: String,
    pub values: Vec<ProductOptionTranslationExactLocaleValueApply>,
    pub expected_resource_revision: String,
    pub expected_source_revision: String,
    pub expected_target_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductOptionTranslationExactLocaleApplyReceipt {
    pub operation_id: Option<Uuid>,
    pub product_id: Uuid,
    pub option_id: Uuid,
    pub resource_revision: String,
    pub target_revision: String,
    pub target: ProductOptionTranslationExactLocaleRecord,
}

impl CatalogService {
    /// Lists non-archived Product Options that own one complete exact source aggregate.
    ///
    /// The SQL candidate window admits an Option only when its exact source title exists
    /// and every current Option Value has the exact source locale. The bounded candidate
    /// set is then hydrated in bulk and assembled with the same semantic revision builder
    /// used by exact read/apply, avoiding provider-owned SQL and per-resource reads.
    pub async fn list_product_option_translation_exact_resources(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
        after: Option<Uuid>,
        limit: u16,
    ) -> ProductOptionTranslationExactLocaleResult<(
        Vec<ProductOptionTranslationExactLocaleSnapshot>,
        Option<Uuid>,
    )> {
        let source_locale = canonical_option_translation_locale(source_locale)?;
        let target_locale = canonical_option_translation_locale(target_locale)?;
        validate_option_locale_pair(&source_locale, &target_locale)?;
        if limit == 0 {
            return Err(CommerceError::Validation(
                "Product option translation resource page limit must be positive".to_string(),
            )
            .into());
        }
        if self.db.get_database_backend() != DbBackend::Postgres {
            return Err(CommerceError::Validation(
                "Product option translation inventory requires PostgreSQL".to_string(),
            )
            .into());
        }

        let candidate_limit = i64::from(limit) + 1;
        let statement = match after {
            Some(after) => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
SELECT option_row.id
FROM product_options AS option_row
INNER JOIN products AS product
    ON product.id = option_row.product_id
INNER JOIN product_option_translations AS source_title
    ON source_title.option_id = option_row.id
   AND source_title.locale = $2
WHERE product.tenant_id = $1
  AND product.status <> 'archived'
  AND option_row.id > $3
  AND NOT EXISTS (
      SELECT 1
      FROM product_option_values AS option_value
      WHERE option_value.option_id = option_row.id
        AND NOT EXISTS (
            SELECT 1
            FROM product_option_value_translations AS source_value
            WHERE source_value.value_id = option_value.id
              AND source_value.locale = $2
        )
  )
ORDER BY option_row.id ASC
LIMIT $4
"#,
                vec![
                    tenant_id.into(),
                    source_locale.clone().into(),
                    after.into(),
                    candidate_limit.into(),
                ],
            ),
            None => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
SELECT option_row.id
FROM product_options AS option_row
INNER JOIN products AS product
    ON product.id = option_row.product_id
INNER JOIN product_option_translations AS source_title
    ON source_title.option_id = option_row.id
   AND source_title.locale = $2
WHERE product.tenant_id = $1
  AND product.status <> 'archived'
  AND NOT EXISTS (
      SELECT 1
      FROM product_option_values AS option_value
      WHERE option_value.option_id = option_row.id
        AND NOT EXISTS (
            SELECT 1
            FROM product_option_value_translations AS source_value
            WHERE source_value.value_id = option_value.id
              AND source_value.locale = $2
        )
  )
ORDER BY option_row.id ASC
LIMIT $3
"#,
                vec![
                    tenant_id.into(),
                    source_locale.clone().into(),
                    candidate_limit.into(),
                ],
            ),
        };
        let rows = self.db.query_all_raw(statement).await?;
        let mut option_ids = rows
            .into_iter()
            .map(|row| row.try_get::<Uuid>("", "id"))
            .collect::<Result<Vec<_>, _>>()?;
        let has_more = option_ids.len() > usize::from(limit);
        if has_more {
            option_ids.truncate(usize::from(limit));
        }
        let next_after = has_more.then(|| option_ids.last().copied()).flatten();
        if option_ids.is_empty() {
            return Ok((Vec::new(), None));
        }

        let options = entities::product_option::Entity::find()
            .filter(entities::product_option::Column::Id.is_in(option_ids.clone()))
            .order_by_asc(entities::product_option::Column::Id)
            .all(&self.db)
            .await?;
        let product_ids = options
            .iter()
            .map(|option| option.product_id)
            .collect::<HashSet<_>>();
        let products = entities::product::Entity::find()
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .filter(entities::product::Column::Id.is_in(product_ids))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|product| (product.id, product))
            .collect::<HashMap<_, _>>();
        let values = entities::product_option_value::Entity::find()
            .filter(entities::product_option_value::Column::OptionId.is_in(option_ids.clone()))
            .order_by_asc(entities::product_option_value::Column::OptionId)
            .order_by_asc(entities::product_option_value::Column::Position)
            .order_by_asc(entities::product_option_value::Column::Id)
            .all(&self.db)
            .await?;
        let mut values_by_option = values.into_iter().fold(
            HashMap::<Uuid, Vec<entities::product_option_value::Model>>::new(),
            |mut grouped, value| {
                grouped.entry(value.option_id).or_default().push(value);
                grouped
            },
        );
        let option_translations = entities::product_option_translation::Entity::find()
            .filter(entities::product_option_translation::Column::OptionId.is_in(option_ids.clone()))
            .order_by_asc(entities::product_option_translation::Column::OptionId)
            .order_by_asc(entities::product_option_translation::Column::Locale)
            .all(&self.db)
            .await?;
        let mut option_translations_by_option = option_translations.into_iter().fold(
            HashMap::<Uuid, Vec<entities::product_option_translation::Model>>::new(),
            |mut grouped, translation| {
                grouped
                    .entry(translation.option_id)
                    .or_default()
                    .push(translation);
                grouped
            },
        );
        let all_value_ids = values_by_option
            .values()
            .flatten()
            .map(|value| value.id)
            .collect::<Vec<_>>();
        let value_translations = if all_value_ids.is_empty() {
            Vec::new()
        } else {
            entities::product_option_value_translation::Entity::find()
                .filter(
                    entities::product_option_value_translation::Column::ValueId
                        .is_in(all_value_ids),
                )
                .order_by_asc(entities::product_option_value_translation::Column::ValueId)
                .order_by_asc(entities::product_option_value_translation::Column::Locale)
                .all(&self.db)
                .await?
        };
        let value_to_option = values_by_option
            .iter()
            .flat_map(|(option_id, values)| {
                values.iter().map(|value| (value.id, *option_id))
            })
            .collect::<HashMap<_, _>>();
        let mut value_translations_by_option = value_translations.into_iter().try_fold(
            HashMap::<Uuid, Vec<entities::product_option_value_translation::Model>>::new(),
            |mut grouped, translation| {
                let option_id = value_to_option.get(&translation.value_id).copied().ok_or_else(|| {
                    CommerceError::Validation(
                        "Product option translation value escaped its owner inventory"
                            .to_string(),
                    )
                })?;
                grouped.entry(option_id).or_default().push(translation);
                Ok::<_, CommerceError>(grouped)
            },
        )?;

        let mut resources = Vec::with_capacity(options.len());
        for option in options {
            let product = products
                .get(&option.product_id)
                .cloned()
                .ok_or(CommerceError::ProductNotFound(option.product_id))?;
            let values = values_by_option.remove(&option.id).unwrap_or_default();
            let option_translations = option_translations_by_option
                .remove(&option.id)
                .unwrap_or_default();
            let value_translations = value_translations_by_option
                .remove(&option.id)
                .unwrap_or_default();
            resources.push(build_option_exact_locale_snapshot(
                product,
                option,
                values,
                option_translations,
                value_translations,
                source_locale.clone(),
                target_locale.clone(),
            )?);
        }

        Ok((resources, next_after))
    }

    /// Reads one exact Product Option aggregate without storefront fallback.
    ///
    /// The aggregate owns the option title plus the ordered values identified by
    /// stable Product-owned value UUIDs. Resource revision covers parent Product
    /// lifecycle, Option/value identity and ordering, and every exact locale row.
    pub async fn read_product_option_translation_exact_locale(
        &self,
        tenant_id: Uuid,
        option_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> ProductOptionTranslationExactLocaleResult<ProductOptionTranslationExactLocaleSnapshot>
    {
        let source_locale = canonical_option_translation_locale(source_locale)?;
        let target_locale = canonical_option_translation_locale(target_locale)?;
        validate_option_locale_pair(&source_locale, &target_locale)?;

        let option = load_option(&self.db, tenant_id, option_id).await?;
        let product = load_option_product(&self.db, tenant_id, option.product_id).await?;
        let values = load_option_values(&self.db, option_id).await?;
        let option_translations = load_option_translations(&self.db, option_id).await?;
        let value_translations = load_option_value_translations(&self.db, &values).await?;

        build_option_exact_locale_snapshot(
            product,
            option,
            values,
            option_translations,
            value_translations,
            source_locale,
            target_locale,
        )
    }

    /// Applies one exact Product Option locale under aggregate resource/source/target CAS.
    ///
    /// Product is locked before Option and Option values. The write updates or
    /// inserts only the requested locale rows while preserving sibling locales
    /// and stable value identities. ProductUpdated publication and any active
    /// Product owner-operation receipt complete in the same transaction.
    pub async fn apply_product_option_translation_exact_locale(
        &self,
        tenant_id: Uuid,
        actor_user_id: Option<Uuid>,
        option_id: Uuid,
        request: ProductOptionTranslationExactLocaleApply,
    ) -> ProductOptionTranslationExactLocaleResult<
        ProductOptionTranslationExactLocaleApplyReceipt,
    > {
        validate_option_translation_title(&request.title)?;
        validate_option_translation_values(&request.values)?;

        let source_locale = canonical_option_translation_locale(&request.source_locale)?;
        let target_locale = canonical_option_translation_locale(&request.target_locale)?;
        validate_option_locale_pair(&source_locale, &target_locale)?;

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;

        let discovered_option = load_option(&txn, tenant_id, option_id).await?;
        let product = entities::product::Entity::find_by_id(discovered_option.product_id)
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .lock_exclusive()
            .one(&txn)
            .await?
            .ok_or(CommerceError::ProductNotFound(discovered_option.product_id))?;
        let option = entities::product_option::Entity::find_by_id(option_id)
            .filter(entities::product_option::Column::ProductId.eq(product.id))
            .lock_exclusive()
            .one(&txn)
            .await?
            .ok_or(ProductOptionTranslationExactLocaleError::OptionNotFound(option_id))?;
        let values = entities::product_option_value::Entity::find()
            .filter(entities::product_option_value::Column::OptionId.eq(option_id))
            .order_by_asc(entities::product_option_value::Column::Position)
            .order_by_asc(entities::product_option_value::Column::Id)
            .lock_exclusive()
            .all(&txn)
            .await?;

        let option_translations = load_option_translations(&txn, option_id).await?;
        let value_translations = load_option_value_translations(&txn, &values).await?;
        let source = exact_option_locale_record(
            option_id,
            &source_locale,
            &values,
            &option_translations,
            &value_translations,
        )?
        .ok_or_else(|| ProductOptionTranslationExactLocaleError::SourceLocaleNotFound {
            option_id,
            locale: source_locale.clone(),
        })?;
        let target = exact_option_locale_record(
            option_id,
            &target_locale,
            &values,
            &option_translations,
            &value_translations,
        )?;

        let current_resource_revision = product_option_translation_resource_revision(
            &product,
            &option,
            &values,
            &option_translations,
            &value_translations,
        );
        let current_source_revision = product_option_translation_locale_revision(&source);
        let current_target_revision = target
            .as_ref()
            .map(product_option_translation_locale_revision);

        ensure_option_revision(
            "resource",
            &request.expected_resource_revision,
            &current_resource_revision,
        )?;
        ensure_option_revision(
            "source",
            &request.expected_source_revision,
            &current_source_revision,
        )?;
        if request.expected_target_revision != current_target_revision {
            return Err(ProductOptionTranslationExactLocaleError::RevisionConflict {
                revision: "target",
            });
        }

        let requested_values = request
            .values
            .iter()
            .map(|value| (value.value_id, value.value.clone()))
            .collect::<HashMap<_, _>>();
        if requested_values.len() != values.len()
            || values
                .iter()
                .any(|value| !requested_values.contains_key(&value.id))
        {
            return Err(ProductOptionTranslationExactLocaleError::ValueSetMismatch);
        }

        if let Some(existing) = option_translations
            .iter()
            .find(|translation| translation.locale == target_locale)
            .cloned()
        {
            let mut active: entities::product_option_translation::ActiveModel = existing.into();
            active.title = Set(request.title.clone());
            active.update(&txn).await?;
        } else {
            entities::product_option_translation::ActiveModel {
                id: Set(generate_id()),
                option_id: Set(option_id),
                locale: Set(target_locale.clone()),
                title: Set(request.title.clone()),
            }
            .insert(&txn)
            .await?;
        }

        for value in &values {
            let translated = requested_values
                .get(&value.id)
                .expect("validated Product Option value set must contain every current value")
                .clone();
            if let Some(existing) = value_translations
                .iter()
                .find(|translation| {
                    translation.value_id == value.id && translation.locale == target_locale
                })
                .cloned()
            {
                let mut active: entities::product_option_value_translation::ActiveModel =
                    existing.into();
                active.value = Set(translated);
                active.update(&txn).await?;
            } else {
                entities::product_option_value_translation::ActiveModel {
                    id: Set(generate_id()),
                    value_id: Set(value.id),
                    locale: Set(target_locale.clone()),
                    value: Set(translated),
                }
                .insert(&txn)
                .await?;
            }
        }

        let option_translations_after = load_option_translations(&txn, option_id).await?;
        let value_translations_after = load_option_value_translations(&txn, &values).await?;
        let target_after = exact_option_locale_record(
            option_id,
            &target_locale,
            &values,
            &option_translations_after,
            &value_translations_after,
        )?
        .ok_or_else(|| {
            ProductOptionTranslationExactLocaleError::TargetLocaleMissingAfterApply {
                option_id,
                locale: target_locale.clone(),
            }
        })?;
        let resource_revision = product_option_translation_resource_revision(
            &product,
            &option,
            &values,
            &option_translations_after,
            &value_translations_after,
        );
        let target_revision = product_option_translation_locale_revision(&target_after);

        txn.publish(
            tenant_id,
            actor_user_id,
            DomainEvent::ProductUpdated {
                product_id: product.id,
            },
        )
        .await?;
        let receipt = ProductOptionTranslationExactLocaleApplyReceipt {
            operation_id: current_product_operation_id(),
            product_id: product.id,
            option_id,
            resource_revision,
            target_revision,
            target: target_after,
        };
        record_product_operation_result(&receipt)?;
        txn.commit().await?;

        Ok(receipt)
    }
}

async fn load_option<C>(
    db: &C,
    tenant_id: Uuid,
    option_id: Uuid,
) -> ProductOptionTranslationExactLocaleResult<entities::product_option::Model>
where
    C: ConnectionTrait,
{
    entities::product_option::Entity::find_by_id(option_id)
        .inner_join(entities::product::Entity)
        .filter(entities::product::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or(ProductOptionTranslationExactLocaleError::OptionNotFound(option_id))
}

async fn load_option_product<C>(
    db: &C,
    tenant_id: Uuid,
    product_id: Uuid,
) -> ProductOptionTranslationExactLocaleResult<entities::product::Model>
where
    C: ConnectionTrait,
{
    entities::product::Entity::find_by_id(product_id)
        .filter(entities::product::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| CommerceError::ProductNotFound(product_id).into())
}

async fn load_option_values<C>(
    db: &C,
    option_id: Uuid,
) -> ProductOptionTranslationExactLocaleResult<Vec<entities::product_option_value::Model>>
where
    C: ConnectionTrait,
{
    Ok(entities::product_option_value::Entity::find()
        .filter(entities::product_option_value::Column::OptionId.eq(option_id))
        .order_by_asc(entities::product_option_value::Column::Position)
        .order_by_asc(entities::product_option_value::Column::Id)
        .all(db)
        .await?)
}

async fn load_option_translations<C>(
    db: &C,
    option_id: Uuid,
) -> ProductOptionTranslationExactLocaleResult<Vec<entities::product_option_translation::Model>>
where
    C: ConnectionTrait,
{
    Ok(entities::product_option_translation::Entity::find()
        .filter(entities::product_option_translation::Column::OptionId.eq(option_id))
        .order_by_asc(entities::product_option_translation::Column::Locale)
        .all(db)
        .await?)
}

async fn load_option_value_translations<C>(
    db: &C,
    values: &[entities::product_option_value::Model],
) -> ProductOptionTranslationExactLocaleResult<Vec<entities::product_option_value_translation::Model>>
where
    C: ConnectionTrait,
{
    if values.is_empty() {
        return Ok(Vec::new());
    }
    let value_ids = values.iter().map(|value| value.id).collect::<Vec<_>>();
    Ok(entities::product_option_value_translation::Entity::find()
        .filter(entities::product_option_value_translation::Column::ValueId.is_in(value_ids))
        .order_by_asc(entities::product_option_value_translation::Column::ValueId)
        .order_by_asc(entities::product_option_value_translation::Column::Locale)
        .all(db)
        .await?)
}

fn build_option_exact_locale_snapshot(
    product: entities::product::Model,
    option: entities::product_option::Model,
    values: Vec<entities::product_option_value::Model>,
    option_translations: Vec<entities::product_option_translation::Model>,
    value_translations: Vec<entities::product_option_value_translation::Model>,
    source_locale: String,
    target_locale: String,
) -> ProductOptionTranslationExactLocaleResult<ProductOptionTranslationExactLocaleSnapshot> {
    let source = exact_option_locale_record(
        option.id,
        &source_locale,
        &values,
        &option_translations,
        &value_translations,
    )?
    .ok_or_else(|| ProductOptionTranslationExactLocaleError::SourceLocaleNotFound {
        option_id: option.id,
        locale: source_locale.clone(),
    })?;
    let target = exact_option_locale_record(
        option.id,
        &target_locale,
        &values,
        &option_translations,
        &value_translations,
    )?;

    let resource_revision = product_option_translation_resource_revision(
        &product,
        &option,
        &values,
        &option_translations,
        &value_translations,
    );
    let source_revision = product_option_translation_locale_revision(&source);
    let target_revision = target
        .as_ref()
        .map(product_option_translation_locale_revision);
    let exact_locales = complete_option_locales(
        option.id,
        &values,
        &option_translations,
        &value_translations,
    )?;

    Ok(ProductOptionTranslationExactLocaleSnapshot {
        product_id: product.id,
        option_id: option.id,
        product_status: product.status,
        source_locale,
        target_locale,
        resource_revision,
        source_revision,
        target_revision,
        exact_locales,
        source,
        target,
    })
}

fn exact_option_locale_record(
    option_id: Uuid,
    locale: &str,
    values: &[entities::product_option_value::Model],
    option_translations: &[entities::product_option_translation::Model],
    value_translations: &[entities::product_option_value_translation::Model],
) -> ProductOptionTranslationExactLocaleResult<Option<ProductOptionTranslationExactLocaleRecord>> {
    let title = option_translations
        .iter()
        .find(|translation| translation.locale == locale);
    let translated_values = value_translations
        .iter()
        .filter(|translation| translation.locale == locale)
        .collect::<Vec<_>>();

    if title.is_none() && translated_values.is_empty() {
        return Ok(None);
    }
    if title.is_none() || translated_values.len() != values.len() {
        return Err(ProductOptionTranslationExactLocaleError::IncompleteLocale {
            option_id,
            locale: locale.to_string(),
        });
    }

    let mut records = Vec::with_capacity(values.len());
    for value in values {
        let translation = translated_values
            .iter()
            .find(|translation| translation.value_id == value.id)
            .ok_or_else(|| ProductOptionTranslationExactLocaleError::IncompleteLocale {
                option_id,
                locale: locale.to_string(),
            })?;
        records.push(ProductOptionTranslationExactLocaleValueRecord {
            value_id: value.id,
            position: value.position,
            value: translation.value.clone(),
        });
    }

    Ok(Some(ProductOptionTranslationExactLocaleRecord {
        locale: locale.to_string(),
        title: title.expect("validated exact title row must exist").title.clone(),
        values: records,
    }))
}

fn complete_option_locales(
    option_id: Uuid,
    values: &[entities::product_option_value::Model],
    option_translations: &[entities::product_option_translation::Model],
    value_translations: &[entities::product_option_value_translation::Model],
) -> ProductOptionTranslationExactLocaleResult<Vec<String>> {
    let mut locales = option_translations
        .iter()
        .map(|translation| translation.locale.clone())
        .collect::<Vec<_>>();
    locales.sort();
    locales.dedup();
    for translation in value_translations {
        if !locales.contains(&translation.locale) {
            locales.push(translation.locale.clone());
        }
    }
    locales.sort();

    let mut complete = Vec::with_capacity(locales.len());
    for locale in locales {
        match exact_option_locale_record(
            option_id,
            &locale,
            values,
            option_translations,
            value_translations,
        )? {
            Some(_) => complete.push(locale),
            None => {}
        }
    }
    Ok(complete)
}

fn canonical_option_translation_locale(
    locale: &str,
) -> ProductOptionTranslationExactLocaleResult<String> {
    TenantLocale::new(locale)
        .map(TenantLocale::into_inner)
        .map_err(|error| CommerceError::Validation(error.to_string()).into())
}

fn validate_option_locale_pair(
    source_locale: &str,
    target_locale: &str,
) -> ProductOptionTranslationExactLocaleResult<()> {
    if source_locale == target_locale {
        return Err(CommerceError::Validation(
            "Product option translation source and target locale must differ".to_string(),
        )
        .into());
    }
    Ok(())
}

fn validate_option_translation_title(
    title: &str,
) -> ProductOptionTranslationExactLocaleResult<()> {
    let length = title.chars().count();
    if length == 0 || length > 100 {
        return Err(CommerceError::Validation(
            "Product option translation title must be 1-100 characters".to_string(),
        )
        .into());
    }
    Ok(())
}

fn validate_option_translation_values(
    values: &[ProductOptionTranslationExactLocaleValueApply],
) -> ProductOptionTranslationExactLocaleResult<()> {
    let mut ids = HashSet::with_capacity(values.len());
    for value in values {
        if value.value_id.is_nil() || !ids.insert(value.value_id) {
            return Err(ProductOptionTranslationExactLocaleError::ValueSetMismatch);
        }
        if value.value.chars().count() > 100 {
            return Err(CommerceError::Validation(
                "Product option translation value must be at most 100 characters".to_string(),
            )
            .into());
        }
    }
    Ok(())
}

fn ensure_option_revision(
    revision: &'static str,
    expected: &str,
    current: &str,
) -> ProductOptionTranslationExactLocaleResult<()> {
    if expected != current {
        return Err(ProductOptionTranslationExactLocaleError::RevisionConflict { revision });
    }
    Ok(())
}

fn product_option_translation_resource_revision(
    product: &entities::product::Model,
    option: &entities::product_option::Model,
    values: &[entities::product_option_value::Model],
    option_translations: &[entities::product_option_translation::Model],
    value_translations: &[entities::product_option_value_translation::Model],
) -> String {
    let mut hasher = Sha256::new();
    digest_option_text(&mut hasher, "rustok-product/option-translation-resource/v1");
    digest_option_text(&mut hasher, &product.id.to_string());
    digest_option_text(&mut hasher, &product.tenant_id.to_string());
    digest_option_text(&mut hasher, &product.status.to_string());
    digest_option_text(&mut hasher, &option.id.to_string());
    digest_option_text(&mut hasher, &option.product_id.to_string());
    digest_option_i32(&mut hasher, option.position);

    let mut ordered_values = values.iter().collect::<Vec<_>>();
    ordered_values.sort_by_key(|value| (value.position, value.id));
    for value in ordered_values {
        digest_option_text(&mut hasher, &value.id.to_string());
        digest_option_text(&mut hasher, &value.option_id.to_string());
        digest_option_i32(&mut hasher, value.position);
    }

    let mut titles = option_translations.iter().collect::<Vec<_>>();
    titles.sort_by(|left, right| left.locale.cmp(&right.locale));
    for translation in titles {
        digest_option_text(&mut hasher, &translation.option_id.to_string());
        digest_option_text(&mut hasher, &translation.locale);
        digest_option_text(&mut hasher, &translation.title);
    }

    let mut translated_values = value_translations.iter().collect::<Vec<_>>();
    translated_values.sort_by(|left, right| {
        left.value_id
            .cmp(&right.value_id)
            .then_with(|| left.locale.cmp(&right.locale))
    });
    for translation in translated_values {
        digest_option_text(&mut hasher, &translation.value_id.to_string());
        digest_option_text(&mut hasher, &translation.locale);
        digest_option_text(&mut hasher, &translation.value);
    }

    finish_option_revision(hasher)
}

fn product_option_translation_locale_revision(
    record: &ProductOptionTranslationExactLocaleRecord,
) -> String {
    let mut hasher = Sha256::new();
    digest_option_text(&mut hasher, "rustok-product/option-translation-locale/v1");
    digest_option_text(&mut hasher, &record.locale);
    digest_option_text(&mut hasher, &record.title);
    for value in &record.values {
        digest_option_text(&mut hasher, &value.value_id.to_string());
        digest_option_i32(&mut hasher, value.position);
        digest_option_text(&mut hasher, &value.value);
    }
    finish_option_revision(hasher)
}

fn digest_option_text(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

fn digest_option_i32(hasher: &mut Sha256, value: i32) {
    hasher.update(value.to_be_bytes());
}

fn finish_option_revision(hasher: Sha256) -> String {
    format!("sha256:{}", hex::encode(hasher.finalize()))
}
