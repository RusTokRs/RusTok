use std::collections::HashSet;

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{
    PLATFORM_FALLBACK_LOCALE, build_locale_candidates, locale_tags_match, normalize_locale_tag,
};
use rustok_product::entities::{product, product_variant};
use sea_orm::prelude::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    DatabaseTransaction, DbErr, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
    Set, Statement, TransactionTrait,
};
use uuid::Uuid;

use crate::dto::{
    BundleDto, BundleFilter, BundleItemDto, BundleItemInput, BundleListResponse,
    BundleTranslationDto, BundleTranslationInput, BundleTranslationUpdateInput, CreateBundleInput,
    UpdateBundleInput,
};
use crate::entities::{
    bundle::{ActiveModel as BundleActiveModel, Column as BundleColumn, Entity as Bundle},
    bundle_item::{
        ActiveModel as BundleItemActiveModel, Column as BundleItemColumn, Entity as BundleItem,
    },
    bundle_translation::{
        ActiveModel as BundleTranslationActiveModel, Column as BundleTranslationColumn,
        Entity as BundleTranslation,
    },
};
use crate::error::{BundleError, BundleResult};
use crate::ports::BundlePort;

async fn lock_bundle_for_update(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    bundle_id: Uuid,
) -> BundleResult<crate::entities::bundle::Model> {
    let query = Bundle::find_by_id(bundle_id).filter(BundleColumn::TenantId.eq(tenant_id));
    let bundle = match txn.get_database_backend() {
        DatabaseBackend::Postgres | DatabaseBackend::MySql => {
            query.lock_exclusive().one(txn).await?
        }
        DatabaseBackend::Sqlite => {
            let statement = Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "UPDATE product_bundles SET updated_at = updated_at WHERE tenant_id = ?1 AND id = ?2",
                [tenant_id.into(), bundle_id.into()],
            );
            txn.execute_raw(statement).await?;
            query.one(txn).await?
        }
        _ => query.one(txn).await?,
    };
    bundle.ok_or(BundleError::NotFound(bundle_id))
}

async fn validate_product_ref_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
) -> BundleResult<()> {
    let exists = product::Entity::find_by_id(product_id)
        .filter(product::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?
        .is_some();
    if !exists {
        return Err(BundleError::InvalidInput(
            "Bundle item product does not belong to the current tenant".into(),
        ));
    }
    Ok(())
}

async fn validate_variant_ref_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
    variant_id: Uuid,
) -> BundleResult<()> {
    let variant = product_variant::Entity::find_by_id(variant_id)
        .filter(product_variant::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?
        .ok_or_else(|| {
            BundleError::InvalidInput(
                "Bundle item variant does not belong to the current tenant".into(),
            )
        })?;
    if variant.product_id != product_id {
        return Err(BundleError::InvalidInput(
            "Bundle item variant does not belong to the selected product".into(),
        ));
    }
    Ok(())
}

async fn validate_bundle_product_ref_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    bundle_product_id: Option<Uuid>,
) -> BundleResult<()> {
    if let Some(product_id) = bundle_product_id {
        validate_product_ref_in_tx(txn, tenant_id, product_id).await?;
    }
    Ok(())
}

async fn validate_bundle_item_refs_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
    variant_id: Option<Uuid>,
) -> BundleResult<()> {
    validate_product_ref_in_tx(txn, tenant_id, product_id).await?;
    if let Some(variant_id) = variant_id {
        validate_variant_ref_in_tx(txn, tenant_id, product_id, variant_id).await?;
    }
    Ok(())
}

fn normalize_choice(value: &str, field: &'static str, allowed: &[&str]) -> BundleResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if allowed.contains(&normalized.as_str()) {
        Ok(normalized)
    } else {
        Err(BundleError::InvalidInput(format!(
            "Bundle {field} must be one of: {}",
            allowed.join(", ")
        )))
    }
}

const MAX_BUNDLE_SLUG_LEN: usize = 255;
const MAX_BUNDLE_TRANSLATION_NAME_LEN: usize = 255;
const BUNDLE_DECIMAL_SCALE: u32 = 4;

fn normalize_bundle_slug(value: String) -> BundleResult<String> {
    // Keep the write/read canonicalization compatible with the original Bundle owner behavior:
    // Unicode case folding must not make a previously stored non-ASCII slug unaddressable.
    let slug = value.trim().to_lowercase();
    if slug.is_empty() || slug.chars().count() > MAX_BUNDLE_SLUG_LEN || slug.contains('\0') {
        return Err(BundleError::InvalidInput(
            "Bundle slug must be 1..255 characters and cannot contain a NUL character".to_owned(),
        ));
    }
    Ok(slug)
}

fn normalize_bundle_filter(filter: BundleFilter) -> BundleResult<BundleFilter> {
    let search = filter
        .search
        .map(|value| {
            let value = value.trim().to_owned();
            if value.contains('\0') {
                Err(BundleError::InvalidInput(
                    "Bundle search cannot contain a NUL character".to_owned(),
                ))
            } else {
                Ok(value)
            }
        })
        .transpose()?
        .filter(|value| !value.is_empty());
    let status = filter
        .status
        .map(|value| normalize_bundle_status(Some(value)))
        .transpose()?;
    let bundle_type = filter
        .bundle_type
        .map(|value| normalize_bundle_type(Some(value)))
        .transpose()?;

    Ok(BundleFilter {
        search,
        status,
        bundle_type,
        owner_product_id: filter.owner_product_id,
    })
}

fn normalize_bundle_translation_name(name: String) -> BundleResult<String> {
    let name = name.trim().to_owned();
    if name.is_empty()
        || name.chars().count() > MAX_BUNDLE_TRANSLATION_NAME_LEN
        || name.contains('\0')
    {
        return Err(BundleError::InvalidInput(
            "Bundle translation name must be 1..255 characters and cannot contain a NUL character"
                .to_owned(),
        ));
    }
    Ok(name)
}

fn validate_bundle_translation_description(description: &Option<String>) -> BundleResult<()> {
    if description
        .as_deref()
        .is_some_and(|description| description.contains('\0'))
    {
        return Err(BundleError::InvalidInput(
            "Bundle translation description cannot contain a NUL character".to_owned(),
        ));
    }
    Ok(())
}

fn normalize_bundle_translation_updates(
    translations: Vec<BundleTranslationUpdateInput>,
) -> BundleResult<Vec<BundleTranslationUpdateInput>> {
    let mut seen_locales = HashSet::with_capacity(translations.len());
    translations
        .into_iter()
        .map(|translation| {
            let locale = normalize_locale_tag(&translation.locale).ok_or_else(|| {
                BundleError::InvalidInput("Bundle translation locale is invalid".to_owned())
            })?;
            if !seen_locales.insert(locale.clone()) {
                return Err(BundleError::InvalidInput(format!(
                    "Bundle translation locale {locale} occurs more than once after normalization"
                )));
            }
            if translation.name.is_none() && translation.description.is_none() {
                return Err(BundleError::InvalidInput(
                    "Bundle translation update must change a name or description".to_owned(),
                ));
            }
            let name = translation
                .name
                .map(normalize_bundle_translation_name)
                .transpose()?;
            if let Some(description) = &translation.description {
                validate_bundle_translation_description(description)?;
            }

            Ok(BundleTranslationUpdateInput {
                locale,
                name,
                description: translation.description,
            })
        })
        .collect()
}

fn normalize_bundle_translations(
    translations: Vec<BundleTranslationInput>,
) -> BundleResult<Vec<BundleTranslationInput>> {
    let mut seen_locales = HashSet::with_capacity(translations.len());
    translations
        .into_iter()
        .map(|translation| {
            let locale = normalize_locale_tag(&translation.locale).ok_or_else(|| {
                BundleError::InvalidInput("Bundle translation locale is invalid".to_owned())
            })?;
            if !seen_locales.insert(locale.clone()) {
                return Err(BundleError::InvalidInput(format!(
                    "Bundle translation locale {locale} occurs more than once after normalization"
                )));
            }

            let name = normalize_bundle_translation_name(translation.name)?;
            validate_bundle_translation_description(&translation.description)?;

            Ok(BundleTranslationInput {
                locale,
                name,
                description: translation.description,
            })
        })
        .collect()
}

fn validate_decimal_storage(
    value: &Decimal,
    max_scale: u32,
    max_integral_digits: u32,
    field: &'static str,
) -> BundleResult<()> {
    let max_absolute = Decimal::new(10_i64.pow(max_integral_digits), 0);
    if value.scale() > max_scale || value.abs() >= max_absolute {
        return Err(BundleError::InvalidInput(format!(
            "Bundle {field} exceeds its supported decimal precision"
        )));
    }
    Ok(())
}

fn normalize_bundle_type(value: Option<String>) -> BundleResult<String> {
    normalize_choice(
        value.as_deref().unwrap_or("fixed"),
        "type",
        &["fixed", "flexible"],
    )
}

fn normalize_bundle_status(value: Option<String>) -> BundleResult<String> {
    normalize_choice(
        value.as_deref().unwrap_or("active"),
        "status",
        &["draft", "active", "archived"],
    )
}

fn normalize_discount(
    discount_type: Option<String>,
    discount_value: Option<Decimal>,
) -> BundleResult<(String, Decimal)> {
    let discount_type = normalize_choice(
        discount_type.as_deref().unwrap_or("none"),
        "discount type",
        &["none", "percentage", "fixed_amount"],
    )?;
    let discount_value = discount_value.unwrap_or_default();
    validate_decimal_storage(&discount_value, BUNDLE_DECIMAL_SCALE, 8, "discount value")?;

    if discount_value < Decimal::ZERO {
        return Err(BundleError::InvalidInput(
            "Bundle discount value cannot be negative".to_owned(),
        ));
    }
    if discount_type == "none" && !discount_value.is_zero() {
        return Err(BundleError::InvalidInput(
            "Bundle discount value must be zero when discount type is none".to_owned(),
        ));
    }
    if discount_type == "percentage" && discount_value > Decimal::new(100, 0) {
        return Err(BundleError::InvalidInput(
            "Bundle percentage discount cannot exceed 100".to_owned(),
        ));
    }

    Ok((discount_type, discount_value))
}

fn json_contains_nul(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(text) => text.contains('\0'),
        serde_json::Value::Array(values) => values.iter().any(json_contains_nul),
        serde_json::Value::Object(values) => values
            .iter()
            .any(|(key, value)| key.contains('\0') || json_contains_nul(value)),
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
            false
        }
    }
}

fn normalize_metadata(metadata: Option<serde_json::Value>) -> BundleResult<serde_json::Value> {
    let metadata = metadata.unwrap_or_else(|| serde_json::json!({}));
    if !metadata.is_object() {
        return Err(BundleError::InvalidInput(
            "Bundle metadata must be a JSON object".to_owned(),
        ));
    }
    if json_contains_nul(&metadata) {
        return Err(BundleError::InvalidInput(
            "Bundle metadata cannot contain a NUL character".to_owned(),
        ));
    }
    Ok(metadata)
}

fn validate_bundle_item_input(item: &BundleItemInput) -> BundleResult<()> {
    if item.quantity < 1 {
        return Err(BundleError::InvalidInput(
            "Bundle item quantity must be at least 1".to_owned(),
        ));
    }
    if item.position.is_some_and(|position| position < 0) {
        return Err(BundleError::InvalidInput(
            "Bundle item position cannot be negative".to_owned(),
        ));
    }
    if let Some(discount_rate) = &item.discount_rate {
        validate_decimal_storage(discount_rate, BUNDLE_DECIMAL_SCALE, 1, "item discount rate")?;
    }
    if item
        .discount_rate
        .is_some_and(|rate| rate < Decimal::ZERO || rate > Decimal::new(1, 0))
    {
        return Err(BundleError::InvalidInput(
            "Bundle item discount rate must be between 0 and 1".to_owned(),
        ));
    }
    Ok(())
}

/// Converts a database-enforced uniqueness race into one stable domain conflict.
///
/// Preflight reads make ordinary duplicate requests actionable, but they cannot replace the
/// database invariant when independently admitted commands commit concurrently. Do not inspect
/// backend-specific constraint names here: all unique violations at this write boundary mean the
/// command lost a concurrent claim and must not leak as a storage outage.
fn map_unique_constraint_conflict(error: DbErr) -> BundleError {
    if matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    ) {
        BundleError::ConcurrentWriteConflict
    } else {
        BundleError::Database(error)
    }
}

async fn ensure_owner_product_is_available_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    bundle_product_id: Option<Uuid>,
    excluding_bundle_id: Option<Uuid>,
) -> BundleResult<()> {
    let Some(product_id) = bundle_product_id else {
        return Ok(());
    };

    let mut query = Bundle::find()
        .filter(BundleColumn::TenantId.eq(tenant_id))
        .filter(BundleColumn::BundleProductId.eq(product_id));
    if let Some(excluding_bundle_id) = excluding_bundle_id {
        query = query.filter(BundleColumn::Id.ne(excluding_bundle_id));
    }
    if query.one(txn).await?.is_some() {
        return Err(BundleError::OwnerProductAlreadyHasBundle(product_id));
    }
    Ok(())
}

#[derive(Clone)]
pub struct BundleService {
    db: DatabaseConnection,
}

impl BundleService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub(crate) fn database(&self) -> &DatabaseConnection {
        &self.db
    }

    fn assemble_bundle_dto(
        model: crate::entities::bundle::Model,
        mut translations: Vec<crate::entities::bundle_translation::Model>,
        items: Vec<crate::entities::bundle_item::Model>,
        locale: Option<&str>,
    ) -> BundleDto {
        // Queries do not promise insertion order. Sort before both selecting a fallback and
        // exposing the translation collection so an otherwise identical read is deterministic.
        translations.sort_by(|left, right| {
            left.locale
                .cmp(&right.locale)
                .then_with(|| left.id.cmp(&right.id))
        });
        let translation_dtos: Vec<BundleTranslationDto> = translations
            .iter()
            .map(|t| BundleTranslationDto {
                id: t.id,
                locale: t.locale.clone(),
                name: t.name.clone(),
                description: t.description.clone(),
            })
            .collect();

        let item_dtos: Vec<BundleItemDto> = items
            .iter()
            .map(|i| BundleItemDto {
                id: i.id,
                bundle_id: i.bundle_id,
                product_id: i.product_id,
                variant_id: i.variant_id,
                quantity: i.quantity,
                is_optional: i.is_optional,
                discount_rate: i.discount_rate,
                position: i.position,
                created_at: i.created_at.into(),
            })
            .collect();

        // Use the platform locale candidate chain so a request such as `ru_RU` can resolve a
        // `ru` translation before falling back to English. This also keeps Bundle reads aligned
        // with the catalog's canonical locale matching rules.
        let locale_candidates =
            build_locale_candidates([locale, Some(PLATFORM_FALLBACK_LOCALE)], true);
        let effective_trans = locale_candidates
            .iter()
            .find_map(|candidate| {
                translations
                    .iter()
                    .find(|translation| locale_tags_match(&translation.locale, candidate))
            })
            .or_else(|| translations.first());

        let name = effective_trans
            .map(|t| t.name.clone())
            .unwrap_or_else(|| model.slug.clone());
        let description = effective_trans.and_then(|t| t.description.clone());

        BundleDto {
            id: model.id,
            tenant_id: model.tenant_id,
            bundle_product_id: model.bundle_product_id,
            slug: model.slug,
            name,
            description,
            bundle_type: model.bundle_type,
            status: model.status,
            discount_type: model.discount_type,
            discount_value: model.discount_value,
            metadata: model.metadata,
            translations: translation_dtos,
            items: item_dtos,
            created_at: model.created_at.into(),
            updated_at: model.updated_at.into(),
        }
    }
}

/// Creates one bundle inside the caller transaction.
///
/// The receipt-bound owner command and the plain owner port method share this body, so an
/// idempotent retry can never diverge from the non-idempotent write path. Slug uniqueness is
/// checked inside the transaction instead of a pre-transaction read.
pub(crate) async fn create_bundle_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    input: CreateBundleInput,
) -> BundleResult<BundleDto> {
    let CreateBundleInput {
        bundle_product_id,
        slug,
        bundle_type,
        status,
        discount_type,
        discount_value,
        metadata,
        translations,
        items,
    } = input;

    let slug = normalize_bundle_slug(slug)?;
    let translations = normalize_bundle_translations(translations)?;
    let bundle_type = normalize_bundle_type(bundle_type)?;
    let status = normalize_bundle_status(status)?;
    let (discount_type, discount_value) = normalize_discount(discount_type, discount_value)?;
    let metadata = normalize_metadata(metadata)?;

    let existing = Bundle::find()
        .filter(BundleColumn::TenantId.eq(tenant_id))
        .filter(BundleColumn::Slug.eq(&slug))
        .one(txn)
        .await?;
    if existing.is_some() {
        return Err(BundleError::SlugAlreadyExists(slug));
    }

    validate_bundle_product_ref_in_tx(txn, tenant_id, bundle_product_id).await?;
    ensure_owner_product_is_available_in_tx(txn, tenant_id, bundle_product_id, None).await?;
    let mut positions = HashSet::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        validate_bundle_item_input(item)?;
        validate_bundle_item_refs_in_tx(txn, tenant_id, item.product_id, item.variant_id).await?;
        let default_position = i32::try_from(index)
            .map_err(|_| BundleError::InvalidInput("Bundle contains too many items".to_owned()))?;
        let position = item.position.unwrap_or(default_position);
        if !positions.insert(position) {
            return Err(BundleError::InvalidInput(
                "Bundle item positions must be unique".to_owned(),
            ));
        }
    }

    let bundle_id = Uuid::new_v4();
    let now = Utc::now();
    let bundle_active = BundleActiveModel {
        id: Set(bundle_id),
        tenant_id: Set(tenant_id),
        bundle_product_id: Set(bundle_product_id),
        slug: Set(slug),
        bundle_type: Set(bundle_type),
        status: Set(status),
        discount_type: Set(discount_type),
        discount_value: Set(discount_value),
        metadata: Set(metadata),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    };
    let inserted_bundle = bundle_active
        .insert(txn)
        .await
        .map_err(map_unique_constraint_conflict)?;

    let mut inserted_translations = Vec::with_capacity(translations.len());
    for trans_input in translations {
        let trans_active = BundleTranslationActiveModel {
            id: Set(Uuid::new_v4()),
            bundle_id: Set(bundle_id),
            locale: Set(trans_input.locale),
            name: Set(trans_input.name),
            description: Set(trans_input.description),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };
        inserted_translations.push(
            trans_active
                .insert(txn)
                .await
                .map_err(map_unique_constraint_conflict)?,
        );
    }

    let mut inserted_items = Vec::with_capacity(items.len());
    for (index, item_input) in items.into_iter().enumerate() {
        let default_position = i32::try_from(index)
            .map_err(|_| BundleError::InvalidInput("Bundle contains too many items".to_owned()))?;
        let item_active = BundleItemActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            bundle_id: Set(bundle_id),
            product_id: Set(item_input.product_id),
            variant_id: Set(item_input.variant_id),
            quantity: Set(item_input.quantity),
            is_optional: Set(item_input.is_optional.unwrap_or(false)),
            discount_rate: Set(item_input.discount_rate),
            position: Set(item_input.position.unwrap_or(default_position)),
            created_at: Set(now.into()),
        };
        inserted_items.push(
            item_active
                .insert(txn)
                .await
                .map_err(map_unique_constraint_conflict)?,
        );
    }

    Ok(BundleService::assemble_bundle_dto(
        inserted_bundle,
        inserted_translations,
        inserted_items,
        None,
    ))
}

/// Adds one item to an existing bundle inside the caller transaction.
pub(crate) async fn add_bundle_item_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    bundle_id: Uuid,
    item: BundleItemInput,
) -> BundleResult<BundleItemDto> {
    let _bundle = lock_bundle_for_update(txn, tenant_id, bundle_id).await?;
    validate_bundle_item_input(&item)?;
    validate_bundle_item_refs_in_tx(txn, tenant_id, item.product_id, item.variant_id).await?;

    let position = match item.position {
        Some(position) => position,
        None => BundleItem::find()
            .filter(BundleItemColumn::TenantId.eq(tenant_id))
            .filter(BundleItemColumn::BundleId.eq(bundle_id))
            .order_by_desc(BundleItemColumn::Position)
            .one(txn)
            .await?
            .map(|item| {
                item.position.checked_add(1).ok_or_else(|| {
                    BundleError::InvalidInput(
                        "Bundle item positions exceed the supported range".to_owned(),
                    )
                })
            })
            .transpose()?
            .unwrap_or_default(),
    };
    let existing_at_position = BundleItem::find()
        .filter(BundleItemColumn::TenantId.eq(tenant_id))
        .filter(BundleItemColumn::BundleId.eq(bundle_id))
        .filter(BundleItemColumn::Position.eq(position))
        .one(txn)
        .await?;
    if existing_at_position.is_some() {
        return Err(BundleError::InvalidInput(
            "Bundle item position is already occupied".to_owned(),
        ));
    }

    let now = Utc::now();
    let inserted = BundleItemActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        bundle_id: Set(bundle_id),
        product_id: Set(item.product_id),
        variant_id: Set(item.variant_id),
        quantity: Set(item.quantity),
        is_optional: Set(item.is_optional.unwrap_or(false)),
        discount_rate: Set(item.discount_rate),
        position: Set(position),
        created_at: Set(now.into()),
    }
    .insert(txn)
    .await
    .map_err(map_unique_constraint_conflict)?;

    Ok(BundleItemDto {
        id: inserted.id,
        bundle_id: inserted.bundle_id,
        product_id: inserted.product_id,
        variant_id: inserted.variant_id,
        quantity: inserted.quantity,
        is_optional: inserted.is_optional,
        discount_rate: inserted.discount_rate,
        position: inserted.position,
        created_at: inserted.created_at.into(),
    })
}

/// Updates one bundle inside the caller transaction.
pub(crate) async fn update_bundle_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    id: Uuid,
    input: UpdateBundleInput,
) -> BundleResult<BundleDto> {
    let bundle = lock_bundle_for_update(txn, tenant_id, id).await?;
    let UpdateBundleInput {
        bundle_product_id,
        slug,
        bundle_type,
        status,
        discount_type,
        discount_value,
        metadata,
        translations,
    } = input;

    // Validate and resolve every translation before mutating the parent row. In particular, a
    // description-only patch for an unknown locale is rejected before any write is issued.
    // The locked parent Bundle serializes these translation lookups with competing updates.
    let translations = if let Some(translations) = translations {
        let translations = normalize_bundle_translation_updates(translations)?;
        let mut resolved = Vec::with_capacity(translations.len());
        for translation in translations {
            let existing = BundleTranslation::find()
                .filter(BundleTranslationColumn::BundleId.eq(id))
                .filter(BundleTranslationColumn::Locale.eq(&translation.locale))
                .one(txn)
                .await?;
            if existing.is_none() && translation.name.is_none() {
                return Err(BundleError::InvalidInput(
                    "A new Bundle translation must provide a name".to_owned(),
                ));
            }
            resolved.push((translation, existing));
        }
        Some(resolved)
    } else {
        None
    };

    let mut active: BundleActiveModel = bundle.clone().into();
    if let Some(slug) = slug {
        let slug = normalize_bundle_slug(slug)?;
        let existing = Bundle::find()
            .filter(BundleColumn::TenantId.eq(tenant_id))
            .filter(BundleColumn::Slug.eq(&slug))
            .filter(BundleColumn::Id.ne(id))
            .one(txn)
            .await?;
        if existing.is_some() {
            return Err(BundleError::SlugAlreadyExists(slug));
        }
        active.slug = Set(slug);
    }

    if let Some(bundle_product_id) = bundle_product_id {
        validate_bundle_product_ref_in_tx(txn, tenant_id, bundle_product_id).await?;
        ensure_owner_product_is_available_in_tx(txn, tenant_id, bundle_product_id, Some(id))
            .await?;
        active.bundle_product_id = Set(bundle_product_id);
    }

    if let Some(bundle_type) = bundle_type {
        active.bundle_type = Set(normalize_bundle_type(Some(bundle_type))?);
    }
    if let Some(status) = status {
        active.status = Set(normalize_bundle_status(Some(status))?);
    }

    let next_discount_type = discount_type.or_else(|| Some(bundle.discount_type.clone()));
    let next_discount_value = discount_value.or(Some(bundle.discount_value.clone()));
    let (next_discount_type, next_discount_value) =
        normalize_discount(next_discount_type, next_discount_value)?;
    active.discount_type = Set(next_discount_type);
    active.discount_value = Set(next_discount_value);

    if let Some(metadata) = metadata {
        active.metadata = Set(normalize_metadata(metadata)?);
    }
    active.updated_at = Set(Utc::now().into());
    let updated_bundle = active
        .update(txn)
        .await
        .map_err(map_unique_constraint_conflict)?;

    if let Some(translations) = translations {
        for (translation, existing) in translations {
            if let Some(existing) = existing {
                let mut active: BundleTranslationActiveModel = existing.into();
                if let Some(name) = translation.name {
                    active.name = Set(name);
                }
                if let Some(description) = translation.description {
                    active.description = Set(description);
                }
                active.updated_at = Set(Utc::now().into());
                active.update(txn).await?;
            } else {
                let name = translation.name.ok_or_else(|| {
                    BundleError::InvalidInput(
                        "A new Bundle translation must provide a name".to_owned(),
                    )
                })?;
                BundleTranslationActiveModel {
                    id: Set(Uuid::new_v4()),
                    bundle_id: Set(id),
                    locale: Set(translation.locale),
                    name: Set(name),
                    description: Set(translation.description.unwrap_or(None)),
                    created_at: Set(Utc::now().into()),
                    updated_at: Set(Utc::now().into()),
                }
                .insert(txn)
                .await
                .map_err(map_unique_constraint_conflict)?;
            }
        }
    }

    let translations = BundleTranslation::find()
        .filter(BundleTranslationColumn::BundleId.eq(id))
        .all(txn)
        .await?;
    let items = BundleItem::find()
        .filter(BundleItemColumn::TenantId.eq(tenant_id))
        .filter(BundleItemColumn::BundleId.eq(id))
        .order_by_asc(BundleItemColumn::Position)
        .all(txn)
        .await?;

    Ok(BundleService::assemble_bundle_dto(
        updated_bundle,
        translations,
        items,
        None,
    ))
}

/// Deletes one bundle inside the caller transaction.
pub(crate) async fn delete_bundle_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    id: Uuid,
) -> BundleResult<()> {
    let bundle = lock_bundle_for_update(txn, tenant_id, id).await?;
    BundleTranslation::delete_many()
        .filter(BundleTranslationColumn::BundleId.eq(id))
        .exec(txn)
        .await?;

    BundleItem::delete_many()
        .filter(BundleItemColumn::TenantId.eq(tenant_id))
        .filter(BundleItemColumn::BundleId.eq(id))
        .exec(txn)
        .await?;

    let delete = Bundle::delete_by_id(bundle.id).exec(txn).await?;
    if delete.rows_affected != 1 {
        return Err(BundleError::NotFound(id));
    }
    Ok(())
}

/// Removes one bundle item inside the caller transaction.
pub(crate) async fn remove_bundle_item_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    bundle_id: Uuid,
    item_id: Uuid,
) -> BundleResult<()> {
    let _bundle = lock_bundle_for_update(txn, tenant_id, bundle_id).await?;
    let item = BundleItem::find_by_id(item_id)
        .filter(BundleItemColumn::TenantId.eq(tenant_id))
        .filter(BundleItemColumn::BundleId.eq(bundle_id))
        .one(txn)
        .await?
        .ok_or(BundleError::ItemNotFound(item_id))?;

    let delete = BundleItem::delete_by_id(item.id).exec(txn).await?;
    if delete.rows_affected != 1 {
        return Err(BundleError::ItemNotFound(item_id));
    }
    Ok(())
}

#[async_trait]
impl BundlePort for BundleService {
    async fn get_bundle(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        locale: Option<&str>,
    ) -> BundleResult<BundleDto> {
        let bundle = Bundle::find_by_id(id)
            .filter(BundleColumn::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(BundleError::NotFound(id))?;

        let translations = BundleTranslation::find()
            .filter(BundleTranslationColumn::BundleId.eq(id))
            .all(&self.db)
            .await?;

        let items = BundleItem::find()
            .filter(BundleItemColumn::TenantId.eq(tenant_id))
            .filter(BundleItemColumn::BundleId.eq(id))
            .order_by_asc(BundleItemColumn::Position)
            .all(&self.db)
            .await?;

        Ok(Self::assemble_bundle_dto(
            bundle,
            translations,
            items,
            locale,
        ))
    }

    async fn get_bundle_by_slug(
        &self,
        tenant_id: Uuid,
        slug: &str,
        locale: Option<&str>,
    ) -> BundleResult<BundleDto> {
        let slug = normalize_bundle_slug(slug.to_owned())?;
        let bundle = Bundle::find()
            .filter(BundleColumn::TenantId.eq(tenant_id))
            .filter(BundleColumn::Slug.eq(&slug))
            .one(&self.db)
            .await?
            .ok_or_else(|| BundleError::SlugNotFound(slug))?;

        let bundle_id = bundle.id;
        let translations = BundleTranslation::find()
            .filter(BundleTranslationColumn::BundleId.eq(bundle_id))
            .all(&self.db)
            .await?;

        let items = BundleItem::find()
            .filter(BundleItemColumn::TenantId.eq(tenant_id))
            .filter(BundleItemColumn::BundleId.eq(bundle_id))
            .order_by_asc(BundleItemColumn::Position)
            .all(&self.db)
            .await?;

        Ok(Self::assemble_bundle_dto(
            bundle,
            translations,
            items,
            locale,
        ))
    }

    async fn list_bundles(
        &self,
        tenant_id: Uuid,
        filter: BundleFilter,
        page: u64,
        per_page: u64,
        locale: Option<&str>,
    ) -> BundleResult<BundleListResponse> {
        let filter = normalize_bundle_filter(filter)?;
        let page = page.max(1);
        let per_page = per_page.clamp(1, 100);

        let mut query = Bundle::find().filter(BundleColumn::TenantId.eq(tenant_id));

        if let Some(search) = &filter.search {
            let search_trim = search.trim();
            if !search_trim.is_empty() {
                query = query.filter(BundleColumn::Slug.contains(search_trim));
            }
        }

        if let Some(status) = &filter.status {
            query = query.filter(BundleColumn::Status.eq(status));
        }

        if let Some(btype) = &filter.bundle_type {
            query = query.filter(BundleColumn::BundleType.eq(btype));
        }

        if let Some(owner_product_id) = filter.owner_product_id {
            query = query.filter(BundleColumn::BundleProductId.eq(owner_product_id));
        }

        let paginator = query
            .order_by_desc(BundleColumn::CreatedAt)
            .paginate(&self.db, per_page);

        let total = paginator.num_items().await?;
        let bundles = paginator.fetch_page(page - 1).await?;

        let bundle_ids: Vec<Uuid> = bundles.iter().map(|b| b.id).collect();

        let all_translations = if !bundle_ids.is_empty() {
            BundleTranslation::find()
                .filter(BundleTranslationColumn::BundleId.is_in(bundle_ids.clone()))
                .all(&self.db)
                .await?
        } else {
            vec![]
        };

        let all_items = if !bundle_ids.is_empty() {
            BundleItem::find()
                .filter(BundleItemColumn::TenantId.eq(tenant_id))
                .filter(BundleItemColumn::BundleId.is_in(bundle_ids))
                .order_by_asc(BundleItemColumn::Position)
                .all(&self.db)
                .await?
        } else {
            vec![]
        };

        let dtos = bundles
            .into_iter()
            .map(|b| {
                let trans = all_translations
                    .iter()
                    .filter(|t| t.bundle_id == b.id)
                    .cloned()
                    .collect();
                let items = all_items
                    .iter()
                    .filter(|i| i.bundle_id == b.id)
                    .cloned()
                    .collect();
                Self::assemble_bundle_dto(b, trans, items, locale)
            })
            .collect();

        Ok(BundleListResponse {
            items: dtos,
            total,
            page,
            per_page,
        })
    }

    async fn create_bundle(
        &self,
        tenant_id: Uuid,
        input: CreateBundleInput,
    ) -> BundleResult<BundleDto> {
        let txn = self.db.begin().await?;
        let bundle = create_bundle_in_tx(&txn, tenant_id, input).await?;
        txn.commit().await?;
        Ok(bundle)
    }

    async fn update_bundle(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        input: UpdateBundleInput,
    ) -> BundleResult<BundleDto> {
        let txn = self.db.begin().await?;
        let bundle = update_bundle_in_tx(&txn, tenant_id, id, input).await?;
        txn.commit().await?;
        Ok(bundle)
    }

    async fn delete_bundle(&self, tenant_id: Uuid, id: Uuid) -> BundleResult<()> {
        let txn = self.db.begin().await?;
        delete_bundle_in_tx(&txn, tenant_id, id).await?;
        txn.commit().await?;
        Ok(())
    }

    async fn add_bundle_item(
        &self,
        tenant_id: Uuid,
        bundle_id: Uuid,
        item: BundleItemInput,
    ) -> BundleResult<BundleItemDto> {
        let txn = self.db.begin().await?;
        let created_item = add_bundle_item_in_tx(&txn, tenant_id, bundle_id, item).await?;
        txn.commit().await?;
        Ok(created_item)
    }

    async fn remove_bundle_item(
        &self,
        tenant_id: Uuid,
        bundle_id: Uuid,
        item_id: Uuid,
    ) -> BundleResult<()> {
        let txn = self.db.begin().await?;
        remove_bundle_item_in_tx(&txn, tenant_id, bundle_id, item_id).await?;
        txn.commit().await?;
        Ok(())
    }

    async fn get_owned_bundle_for_product(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: Option<&str>,
    ) -> BundleResult<Option<BundleDto>> {
        let bundle = Bundle::find()
            .filter(BundleColumn::TenantId.eq(tenant_id))
            .filter(BundleColumn::BundleProductId.eq(product_id))
            .one(&self.db)
            .await?;

        match bundle {
            Some(bundle) => self
                .get_bundle(tenant_id, bundle.id, locale)
                .await
                .map(Some),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::prelude::Decimal;

    use super::{
        BUNDLE_DECIMAL_SCALE, BundleTranslationInput, MAX_BUNDLE_SLUG_LEN, json_contains_nul,
        normalize_bundle_slug, normalize_bundle_translations, validate_decimal_storage,
    };

    fn translation(locale: &str, name: &str) -> BundleTranslationInput {
        BundleTranslationInput {
            locale: locale.to_owned(),
            name: name.to_owned(),
            description: None,
        }
    }

    #[test]
    fn bundle_translations_canonicalize_locales_before_duplicate_detection() {
        let normalized = normalize_bundle_translations(vec![translation(" EN_us ", " Name ")])
            .expect("one valid canonical translation");
        assert_eq!(normalized[0].locale, "en-US");
        assert_eq!(normalized[0].name, "Name");

        assert!(
            normalize_bundle_translations(vec![
                translation("en_us", "English"),
                translation("en-US", "English duplicate"),
            ])
            .is_err()
        );
    }

    #[test]
    fn bundle_metadata_rejects_nul_in_values_and_keys() {
        assert!(json_contains_nul(&serde_json::json!({"key": "\0"})));
        assert!(json_contains_nul(&serde_json::json!({"\0": "value"})));
        assert!(!json_contains_nul(&serde_json::json!({"key": ["safe", 1]})));
    }

    #[test]
    fn bundle_text_and_decimal_values_respect_storage_bounds() {
        assert_eq!(
            normalize_bundle_slug(" Ä ".to_owned()).expect("canonical Unicode slug"),
            "ä"
        );
        assert!(normalize_bundle_slug("x".repeat(MAX_BUNDLE_SLUG_LEN + 1)).is_err());
        assert!(normalize_bundle_translations(vec![translation("en", " ")]).is_err());
        assert!(normalize_bundle_translations(vec![translation("en", &"n".repeat(256))]).is_err());
        assert!(
            validate_decimal_storage(
                &Decimal::new(1, BUNDLE_DECIMAL_SCALE + 1),
                BUNDLE_DECIMAL_SCALE,
                8,
                "discount value",
            )
            .is_err()
        );
        assert!(
            validate_decimal_storage(
                &Decimal::new(100_000_000, 0),
                BUNDLE_DECIMAL_SCALE,
                8,
                "discount value",
            )
            .is_err()
        );
    }
}
