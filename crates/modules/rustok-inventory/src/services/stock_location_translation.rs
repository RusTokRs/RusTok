use std::{collections::HashMap, fmt::Write as _};

use rustok_api::{PortError, TenantLocale, sha256_digest};
use rustok_commerce_foundation::entities::{stock_location, stock_location_translation};
use rustok_core::generate_id;
use rustok_outbox::idempotency;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait, sea_query::ExprTrait,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::translation_changes::{
    StockLocationTranslationChangeLifecycle, record_stock_location_translation_change_in_tx,
};

pub const MAX_STOCK_LOCATION_TRANSLATION_RESOURCE_PAGE: u16 = 200;

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

    #[error("Stock location translation owner receipt failed: {0}")]
    OperationReceipt(PortError),

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
    pub operation_id: Option<Uuid>,
    pub stock_location_id: Uuid,
    pub resource_revision: String,
    pub target_revision: String,
    pub target: StockLocationTranslationExactLocaleRecord,
}

/// Inventory-owned exact-locale mutation boundary for Stock Location presentation copy.
///
/// The parent Stock Location row is locked for the complete compare-and-swap mutation so
/// canonical owner writes and Translation callers share one serialization boundary. Revisions
/// intentionally cover only owner identity plus localized presentation copy; codes, addresses,
/// contact data, inventory quantities, reservations, and other operational state are not
/// Translation resource state.
pub struct StockLocationTranslationService {
    db: DatabaseConnection,
}

impl StockLocationTranslationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub(crate) fn database(&self) -> &DatabaseConnection {
        &self.db
    }

    pub async fn list_exact_resources(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
        after: Option<Uuid>,
        limit: u16,
    ) -> StockLocationTranslationExactLocaleResult<(
        Vec<StockLocationTranslationExactLocaleSnapshot>,
        Option<Uuid>,
    )> {
        validate_tenant(tenant_id)?;
        let source_locale = canonical_locale(source_locale)?;
        let target_locale = canonical_locale(target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;
        if limit == 0 || limit > MAX_STOCK_LOCATION_TRANSLATION_RESOURCE_PAGE {
            return Err(StockLocationTranslationExactLocaleError::Validation(format!(
                "Inventory translation resource page size must be between 1 and {MAX_STOCK_LOCATION_TRANSLATION_RESOURCE_PAGE}"
            )));
        }

        let source_stock_location_ids = sea_orm::sea_query::Query::select()
            .column(stock_location_translation::Column::StockLocationId)
            .from(stock_location_translation::Entity)
            .and_where(
                sea_orm::sea_query::Expr::col(stock_location_translation::Column::Locale)
                    .eq(source_locale.clone()),
            )
            .to_owned();
        let mut query = stock_location::Entity::find()
            .filter(stock_location::Column::TenantId.eq(tenant_id))
            .filter(stock_location::Column::DeletedAt.is_null())
            .filter(stock_location::Column::Id.in_subquery(source_stock_location_ids))
            .order_by_asc(stock_location::Column::Id);
        if let Some(after) = after {
            query = query.filter(stock_location::Column::Id.gt(after));
        }

        let mut stock_locations = query.limit(u64::from(limit) + 1).all(&self.db).await?;
        let has_more = stock_locations.len() > usize::from(limit);
        if has_more {
            stock_locations.truncate(usize::from(limit));
        }
        let next_after = has_more
            .then(|| stock_locations.last().map(|stock_location| stock_location.id))
            .flatten();
        if stock_locations.is_empty() {
            return Ok((Vec::new(), None));
        }

        let stock_location_ids = stock_locations
            .iter()
            .map(|stock_location| stock_location.id)
            .collect::<Vec<_>>();
        let mut translations = load_translations_for_stock_locations(&self.db, &stock_location_ids)
            .await?
            .into_iter()
            .fold(
                HashMap::<Uuid, Vec<stock_location_translation::Model>>::new(),
                |mut grouped, translation| {
                    grouped
                        .entry(translation.stock_location_id)
                        .or_default()
                        .push(translation);
                    grouped
                },
            );

        let mut snapshots = Vec::with_capacity(stock_locations.len());
        for stock_location in stock_locations {
            let exact = translations.remove(&stock_location.id).ok_or_else(|| {
                StockLocationTranslationExactLocaleError::SourceLocaleNotFound {
                    stock_location_id: stock_location.id,
                    locale: source_locale.clone(),
                }
            })?;
            snapshots.push(build_snapshot(
                stock_location,
                exact,
                source_locale.clone(),
                target_locale.clone(),
            )?);
        }
        Ok((snapshots, next_after))
    }

    pub async fn read_exact_locale(
        &self,
        tenant_id: Uuid,
        stock_location_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> StockLocationTranslationExactLocaleResult<StockLocationTranslationExactLocaleSnapshot> {
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
        self.apply_exact_locale_inner(tenant_id, stock_location_id, request, None)
            .await
    }

    pub(crate) async fn apply_exact_locale_with_operation(
        &self,
        tenant_id: Uuid,
        stock_location_id: Uuid,
        request: StockLocationTranslationExactLocaleApply,
        operation_lease: idempotency::Lease,
    ) -> StockLocationTranslationExactLocaleResult<StockLocationTranslationExactLocaleApplyReceipt>
    {
        self.apply_exact_locale_inner(
            tenant_id,
            stock_location_id,
            request,
            Some(operation_lease),
        )
        .await
    }

    async fn apply_exact_locale_inner(
        &self,
        tenant_id: Uuid,
        stock_location_id: Uuid,
        request: StockLocationTranslationExactLocaleApply,
        operation_lease: Option<idempotency::Lease>,
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
            .ok_or(StockLocationTranslationExactLocaleError::StockLocationNotFound(
                stock_location_id,
            ))?;
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
            .ok_or_else(
                || StockLocationTranslationExactLocaleError::TargetLocaleMissingAfterApply {
                    stock_location_id,
                    locale: target_locale.clone(),
                },
            )?;
        let resource_revision = resource_revision(&stock_location, &translations_after);
        let operation_id = operation_lease
            .map(|lease| lease.operation_id)
            .or_else(|| (!unchanged).then(generate_id));

        if !unchanged {
            record_stock_location_translation_change_in_tx(
                &txn,
                tenant_id,
                stock_location_id,
                operation_id.expect("changed Inventory translation apply must have operation id"),
                &resource_revision,
                StockLocationTranslationChangeLifecycle::Active,
            )
            .await?;
        }

        let receipt = StockLocationTranslationExactLocaleApplyReceipt {
            operation_id,
            stock_location_id,
            resource_revision,
            target_revision: locale_revision(&target_after),
            target: StockLocationTranslationExactLocaleRecord::from(target_after),
        };
        if let Some(lease) = operation_lease {
            idempotency::complete(&txn, lease, &receipt)
                .await
                .map_err(StockLocationTranslationExactLocaleError::OperationReceipt)?;
        }
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
        .ok_or(StockLocationTranslationExactLocaleError::StockLocationNotFound(
            stock_location_id,
        ))
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

async fn load_translations_for_stock_locations<C>(
    db: &C,
    stock_location_ids: &[Uuid],
) -> StockLocationTranslationExactLocaleResult<Vec<stock_location_translation::Model>>
where
    C: ConnectionTrait,
{
    if stock_location_ids.is_empty() {
        return Ok(Vec::new());
    }
    Ok(stock_location_translation::Entity::find()
        .filter(stock_location_translation::Column::StockLocationId.is_in(stock_location_ids.to_vec()))
        .order_by_asc(stock_location_translation::Column::StockLocationId)
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
