use std::fmt::Write as _;

use rustok_api::{TenantLocale, sha256_digest};
use rustok_commerce_foundation::entities::{stock_location, stock_location_translation};
use rustok_core::generate_id;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum StockLocationTranslationExactLocaleError {
    #[error("Stock location not found: {0}")]
    StockLocationNotFound(Uuid),

    #[error(
        "Stock location translation source locale not found: {locale} for stock location {stock_location_id}"
    )]
    SourceLocaleNotFound {
        stock_location_id: Uuid,
        locale: String,
    },

    #[error(
        "Stock location translation target locale missing after apply: {locale} for stock location {stock_location_id}"
    )]
    TargetLocaleMissingAfterApply {
        stock_location_id: Uuid,
        locale: String,
    },

    #[error("Stock location translation {revision} revision conflict")]
    RevisionConflict { revision: &'static str },

    #[error("Stock location translation validation failed: {0}")]
    Validation(String),

    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

pub type StockLocationTranslationExactLocaleResult<T> =
    Result<T, StockLocationTranslationExactLocaleError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StockLocationTranslationExactLocaleRecord {
    pub locale: String,
    pub name: String,
}

impl From<stock_location_translation::Model> for StockLocationTranslationExactLocaleRecord {
    fn from(value: stock_location_translation::Model) -> Self {
        Self {
            locale: value.locale,
            name: value.name,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StockLocationTranslationExactLocaleSnapshot {
    pub stock_location_id: Uuid,
    pub source_locale: String,
    pub target_locale: String,
    pub resource_revision: String,
    pub source_revision: String,
    pub target_revision: Option<String>,
    pub exact_locales: Vec<String>,
    pub source: StockLocationTranslationExactLocaleRecord,
    pub target: Option<StockLocationTranslationExactLocaleRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StockLocationTranslationExactLocaleApply {
    pub source_locale: String,
    pub target_locale: String,
    pub name: String,
    pub expected_resource_revision: String,
    pub expected_source_revision: String,
    pub expected_target_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StockLocationTranslationExactLocaleApplyReceipt {
    pub stock_location_id: Uuid,
    pub resource_revision: String,
    pub target_revision: String,
    pub target: StockLocationTranslationExactLocaleRecord,
}

/// Inventory-owned exact-locale mutation boundary for Stock Location presentation copy.
///
/// The parent Stock Location row is locked for the complete compare-and-swap mutation so
/// canonical owner writes and future Translation callers share one serialization boundary.
/// Revisions intentionally cover only owner identity plus localized presentation copy; codes,
/// addresses, contact data, inventory quantities, reservations, and other operational state are
/// not Translation resource state.
pub struct StockLocationTranslationService {
    db: DatabaseConnection,
}

impl StockLocationTranslationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn read_exact_locale(
        &self,
        tenant_id: Uuid,
        stock_location_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> StockLocationTranslationExactLocaleResult<StockLocationTranslationExactLocaleSnapshot>
    {
        validate_tenant(tenant_id)?;
        let source_locale = canonical_locale(source_locale)?;
        let target_locale = canonical_locale(target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;

        let stock_location = load_stock_location(&self.db, tenant_id, stock_location_id).await?;
        let translations = load_translations(&self.db, stock_location_id).await?;
        build_snapshot(stock_location, translations, source_locale, target_locale)
    }

    pub async fn apply_exact_locale(
        &self,
        tenant_id: Uuid,
        stock_location_id: Uuid,
        request: StockLocationTranslationExactLocaleApply,
    ) -> StockLocationTranslationExactLocaleResult<StockLocationTranslationExactLocaleApplyReceipt>
    {
        validate_tenant(tenant_id)?;
        let source_locale = canonical_locale(&request.source_locale)?;
        let target_locale = canonical_locale(&request.target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;
        let target_name = normalize_name(&request.name)?;

        let txn = self.db.begin().await?;
        let stock_location = stock_location::Entity::find_by_id(stock_location_id)
            .filter(stock_location::Column::TenantId.eq(tenant_id))
            .filter(stock_location::Column::DeletedAt.is_null())
            .lock_exclusive()
            .one(&txn)
            .await?
            .ok_or(
                StockLocationTranslationExactLocaleError::StockLocationNotFound(stock_location_id),
            )?;
        let translations = load_translations(&txn, stock_location_id).await?;
        let source = exact_locale_row(&translations, &source_locale).ok_or_else(|| {
            StockLocationTranslationExactLocaleError::SourceLocaleNotFound {
                stock_location_id,
                locale: source_locale.clone(),
            }
        })?;
        let target = exact_locale_row(&translations, &target_locale);

        ensure_revision(
            "resource",
            &request.expected_resource_revision,
            &resource_revision(&stock_location, &translations),
        )?;
        ensure_revision(
            "source",
            &request.expected_source_revision,
            &locale_revision(source),
        )?;
        let current_target_revision = target.map(locale_revision);
        if request.expected_target_revision != current_target_revision {
            return Err(StockLocationTranslationExactLocaleError::RevisionConflict {
                revision: "target",
            });
        }

        let unchanged = target.is_some_and(|existing| existing.name == target_name);
        if !unchanged {
            if let Some(existing) = target.cloned() {
                let mut active: stock_location_translation::ActiveModel = existing.into();
                active.name = Set(target_name);
                active.update(&txn).await?;
            } else {
                stock_location_translation::ActiveModel {
                    id: Set(generate_id()),
                    stock_location_id: Set(stock_location_id),
                    locale: Set(target_locale.clone()),
                    name: Set(target_name),
                }
                .insert(&txn)
                .await?;
            }
        }

        let translations_after = load_translations(&txn, stock_location_id).await?;
        let target_after = exact_locale_row(&translations_after, &target_locale)
            .cloned()
            .ok_or_else(|| {
                StockLocationTranslationExactLocaleError::TargetLocaleMissingAfterApply {
                    stock_location_id,
                    locale: target_locale.clone(),
                }
            })?;
        let receipt = StockLocationTranslationExactLocaleApplyReceipt {
            stock_location_id,
            resource_revision: resource_revision(&stock_location, &translations_after),
            target_revision: locale_revision(&target_after),
            target: StockLocationTranslationExactLocaleRecord::from(target_after),
        };
        txn.commit().await?;
        Ok(receipt)
    }
}

async fn load_stock_location<C>(
    db: &C,
    tenant_id: Uuid,
    stock_location_id: Uuid,
) -> StockLocationTranslationExactLocaleResult<stock_location::Model>
where
    C: ConnectionTrait,
{
    stock_location::Entity::find_by_id(stock_location_id)
        .filter(stock_location::Column::TenantId.eq(tenant_id))
        .filter(stock_location::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(StockLocationTranslationExactLocaleError::StockLocationNotFound(stock_location_id))
}

async fn load_translations<C>(
    db: &C,
    stock_location_id: Uuid,
) -> StockLocationTranslationExactLocaleResult<Vec<stock_location_translation::Model>>
where
    C: ConnectionTrait,
{
    Ok(stock_location_translation::Entity::find()
        .filter(stock_location_translation::Column::StockLocationId.eq(stock_location_id))
        .order_by_asc(stock_location_translation::Column::Locale)
        .all(db)
        .await?)
}

fn build_snapshot(
    stock_location: stock_location::Model,
    translations: Vec<stock_location_translation::Model>,
    source_locale: String,
    target_locale: String,
) -> StockLocationTranslationExactLocaleResult<StockLocationTranslationExactLocaleSnapshot> {
    let source = exact_locale_row(&translations, &source_locale)
        .cloned()
        .ok_or_else(
            || StockLocationTranslationExactLocaleError::SourceLocaleNotFound {
                stock_location_id: stock_location.id,
                locale: source_locale.clone(),
            },
        )?;
    let target = exact_locale_row(&translations, &target_locale).cloned();
    let resource_revision = resource_revision(&stock_location, &translations);
    let source_revision = locale_revision(&source);
    let target_revision = target.as_ref().map(locale_revision);
    let exact_locales = translations
        .iter()
        .filter_map(|translation| {
            TenantLocale::new(&translation.locale)
                .ok()
                .map(TenantLocale::into_inner)
        })
        .collect();

    Ok(StockLocationTranslationExactLocaleSnapshot {
        stock_location_id: stock_location.id,
        source_locale,
        target_locale,
        resource_revision,
        source_revision,
        target_revision,
        exact_locales,
        source: StockLocationTranslationExactLocaleRecord::from(source),
        target: target.map(StockLocationTranslationExactLocaleRecord::from),
    })
}

fn exact_locale_row<'a>(
    translations: &'a [stock_location_translation::Model],
    locale: &str,
) -> Option<&'a stock_location_translation::Model> {
    translations
        .iter()
        .find(|translation| translation.locale == locale)
}

fn validate_tenant(tenant_id: Uuid) -> StockLocationTranslationExactLocaleResult<()> {
    if tenant_id.is_nil() {
        return Err(StockLocationTranslationExactLocaleError::Validation(
            "Inventory translation tenant_id must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn canonical_locale(locale: &str) -> StockLocationTranslationExactLocaleResult<String> {
    TenantLocale::new(locale)
        .map(TenantLocale::into_inner)
        .map_err(|error| StockLocationTranslationExactLocaleError::Validation(error.to_string()))
}

fn validate_locale_pair(
    source_locale: &str,
    target_locale: &str,
) -> StockLocationTranslationExactLocaleResult<()> {
    if source_locale == target_locale {
        return Err(StockLocationTranslationExactLocaleError::Validation(
            "Stock location translation source and target locale must differ".to_string(),
        ));
    }
    Ok(())
}

fn normalize_name(name: &str) -> StockLocationTranslationExactLocaleResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(StockLocationTranslationExactLocaleError::Validation(
            "Stock location name cannot be empty".to_string(),
        ));
    }
    if name.chars().count() > 100 {
        return Err(StockLocationTranslationExactLocaleError::Validation(
            "Stock location name must be at most 100 characters".to_string(),
        ));
    }
    Ok(name.to_string())
}

fn ensure_revision(
    revision: &'static str,
    expected: &str,
    current: &str,
) -> StockLocationTranslationExactLocaleResult<()> {
    if expected != current {
        return Err(StockLocationTranslationExactLocaleError::RevisionConflict { revision });
    }
    Ok(())
}

fn resource_revision(
    stock_location: &stock_location::Model,
    translations: &[stock_location_translation::Model],
) -> String {
    let mut payload = Vec::new();
    append_text(
        &mut payload,
        "rustok-inventory/stock-location-translation-resource/v1",
    );
    append_text(&mut payload, &stock_location.id.to_string());
    append_text(&mut payload, &stock_location.tenant_id.to_string());
    for translation in translations {
        append_translation(&mut payload, translation);
    }
    digest_revision(&payload)
}

fn locale_revision(translation: &stock_location_translation::Model) -> String {
    let mut payload = Vec::new();
    append_text(
        &mut payload,
        "rustok-inventory/stock-location-translation-locale/v1",
    );
    append_translation(&mut payload, translation);
    digest_revision(&payload)
}

fn append_translation(payload: &mut Vec<u8>, translation: &stock_location_translation::Model) {
    append_text(payload, &translation.stock_location_id.to_string());
    append_text(payload, &translation.locale);
    append_text(payload, &translation.name);
}

fn append_text(payload: &mut Vec<u8>, value: &str) {
    payload.extend_from_slice(&(value.len() as u64).to_be_bytes());
    payload.extend_from_slice(value.as_bytes());
}

fn digest_revision(payload: &[u8]) -> String {
    let digest = sha256_digest(&[payload]);
    let mut revision = String::with_capacity(71);
    revision.push_str("sha256:");
    for byte in digest {
        write!(&mut revision, "{byte:02x}").expect("writing to String cannot fail");
    }
    revision
}
