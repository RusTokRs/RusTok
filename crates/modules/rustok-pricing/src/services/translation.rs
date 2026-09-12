use std::{collections::HashMap, fmt::Write as _};

use rustok_api::{PortError, TenantLocale, sha256_digest};
use rustok_core::generate_id;
use rustok_outbox::idempotency;
use rustok_pricing_persistence::entities::{price_list, price_list_translation};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait, sea_query::ExprTrait,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::translation_changes::{
    PriceListTranslationChangeLifecycle, record_price_list_translation_change_in_tx,
};

pub const MAX_PRICE_LIST_TRANSLATION_RESOURCE_PAGE: u16 = 200;

#[derive(Debug, Error)]
pub enum PriceListTranslationExactLocaleError {
    #[error("Price list not found: {0}")]
    PriceListNotFound(Uuid),

    #[error(
        "Price list translation source locale not found: {locale} for price list {price_list_id}"
    )]
    SourceLocaleNotFound { price_list_id: Uuid, locale: String },

    #[error(
        "Price list translation target locale missing after apply: {locale} for price list {price_list_id}"
    )]
    TargetLocaleMissingAfterApply { price_list_id: Uuid, locale: String },

    #[error("Price list translation {revision} revision conflict")]
    RevisionConflict { revision: &'static str },

    #[error("Price list translation validation failed: {0}")]
    Validation(String),

    #[error("Price list translation owner receipt failed: {0}")]
    OperationReceipt(PortError),

    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

pub type PriceListTranslationExactLocaleResult<T> = Result<T, PriceListTranslationExactLocaleError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceListTranslationExactLocaleRecord {
    pub locale: String,
    pub name: String,
    pub description: Option<String>,
}

impl From<price_list_translation::Model> for PriceListTranslationExactLocaleRecord {
    fn from(value: price_list_translation::Model) -> Self {
        Self {
            locale: value.locale,
            name: value.name,
            description: value.description,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceListTranslationExactLocaleSnapshot {
    pub price_list_id: Uuid,
    pub source_locale: String,
    pub target_locale: String,
    pub resource_revision: String,
    pub source_revision: String,
    pub target_revision: Option<String>,
    pub exact_locales: Vec<String>,
    pub source: PriceListTranslationExactLocaleRecord,
    pub target: Option<PriceListTranslationExactLocaleRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceListTranslationExactLocaleApply {
    pub source_locale: String,
    pub target_locale: String,
    pub name: String,
    pub description: Option<String>,
    pub expected_resource_revision: String,
    pub expected_source_revision: String,
    pub expected_target_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceListTranslationExactLocaleApplyReceipt {
    pub operation_id: Option<Uuid>,
    pub price_list_id: Uuid,
    pub resource_revision: String,
    pub target_revision: String,
    pub target: PriceListTranslationExactLocaleRecord,
}

/// Pricing-owned exact-locale mutation boundary for price-list presentation copy.
///
/// The parent price-list row is locked for the entire compare-and-swap mutation,
/// giving canonical Pricing writes and Translation callers one serialization
/// boundary. Revisions cover only owner identity plus localized presentation copy;
/// pricing rules, amounts, channels, schedules, and other calculation inputs are
/// deliberately outside the Translation resource revision.
pub struct PriceListTranslationService {
    db: DatabaseConnection,
}

impl PriceListTranslationService {
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
    ) -> PriceListTranslationExactLocaleResult<(
        Vec<PriceListTranslationExactLocaleSnapshot>,
        Option<Uuid>,
    )> {
        validate_tenant(tenant_id)?;
        let source_locale = canonical_locale(source_locale)?;
        let target_locale = canonical_locale(target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;
        if limit == 0 || limit > MAX_PRICE_LIST_TRANSLATION_RESOURCE_PAGE {
            return Err(PriceListTranslationExactLocaleError::Validation(format!(
                "Pricing translation resource page size must be between 1 and {MAX_PRICE_LIST_TRANSLATION_RESOURCE_PAGE}"
            )));
        }

        let source_price_list_ids = sea_orm::sea_query::Query::select()
            .column(price_list_translation::Column::PriceListId)
            .from(price_list_translation::Entity)
            .and_where(
                sea_orm::sea_query::Expr::col(price_list_translation::Column::Locale)
                    .eq(source_locale.clone()),
            )
            .to_owned();
        let mut query = price_list::Entity::find()
            .filter(price_list::Column::TenantId.eq(tenant_id))
            .filter(price_list::Column::Id.in_subquery(source_price_list_ids))
            .order_by_asc(price_list::Column::Id);
        if let Some(after) = after {
            query = query.filter(price_list::Column::Id.gt(after));
        }

        let mut price_lists = query.limit(u64::from(limit) + 1).all(&self.db).await?;
        let has_more = price_lists.len() > usize::from(limit);
        if has_more {
            price_lists.truncate(usize::from(limit));
        }
        let next_after = has_more
            .then(|| price_lists.last().map(|price_list| price_list.id))
            .flatten();
        if price_lists.is_empty() {
            return Ok((Vec::new(), None));
        }

        let price_list_ids = price_lists
            .iter()
            .map(|price_list| price_list.id)
            .collect::<Vec<_>>();
        let mut translations = load_translations_for_price_lists(&self.db, &price_list_ids)
            .await?
            .into_iter()
            .fold(
                HashMap::<Uuid, Vec<price_list_translation::Model>>::new(),
                |mut grouped, translation| {
                    grouped
                        .entry(translation.price_list_id)
                        .or_default()
                        .push(translation);
                    grouped
                },
            );

        let mut snapshots = Vec::with_capacity(price_lists.len());
        for price_list in price_lists {
            let exact = translations.remove(&price_list.id).ok_or_else(|| {
                PriceListTranslationExactLocaleError::SourceLocaleNotFound {
                    price_list_id: price_list.id,
                    locale: source_locale.clone(),
                }
            })?;
            snapshots.push(build_snapshot(
                price_list,
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
        price_list_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> PriceListTranslationExactLocaleResult<PriceListTranslationExactLocaleSnapshot> {
        validate_tenant(tenant_id)?;
        let source_locale = canonical_locale(source_locale)?;
        let target_locale = canonical_locale(target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;

        let price_list = load_price_list(&self.db, tenant_id, price_list_id).await?;
        let translations = load_translations(&self.db, price_list_id).await?;
        build_snapshot(price_list, translations, source_locale, target_locale)
    }

    pub async fn apply_exact_locale(
        &self,
        tenant_id: Uuid,
        price_list_id: Uuid,
        request: PriceListTranslationExactLocaleApply,
    ) -> PriceListTranslationExactLocaleResult<PriceListTranslationExactLocaleApplyReceipt> {
        self.apply_exact_locale_inner(tenant_id, price_list_id, request, None)
            .await
    }

    pub(crate) async fn apply_exact_locale_with_operation(
        &self,
        tenant_id: Uuid,
        price_list_id: Uuid,
        request: PriceListTranslationExactLocaleApply,
        operation_lease: idempotency::Lease,
    ) -> PriceListTranslationExactLocaleResult<PriceListTranslationExactLocaleApplyReceipt> {
        self.apply_exact_locale_inner(tenant_id, price_list_id, request, Some(operation_lease))
            .await
    }

    async fn apply_exact_locale_inner(
        &self,
        tenant_id: Uuid,
        price_list_id: Uuid,
        request: PriceListTranslationExactLocaleApply,
        operation_lease: Option<idempotency::Lease>,
    ) -> PriceListTranslationExactLocaleResult<PriceListTranslationExactLocaleApplyReceipt> {
        validate_tenant(tenant_id)?;
        let source_locale = canonical_locale(&request.source_locale)?;
        let target_locale = canonical_locale(&request.target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;
        let target_name = normalize_name(&request.name)?;

        let txn = self.db.begin().await?;
        let price_list = price_list::Entity::find_by_id(price_list_id)
            .filter(price_list::Column::TenantId.eq(tenant_id))
            .lock_exclusive()
            .one(&txn)
            .await?
            .ok_or(PriceListTranslationExactLocaleError::PriceListNotFound(
                price_list_id,
            ))?;
        let translations = load_translations(&txn, price_list_id).await?;
        let source = exact_locale_row(&translations, &source_locale).ok_or_else(|| {
            PriceListTranslationExactLocaleError::SourceLocaleNotFound {
                price_list_id,
                locale: source_locale.clone(),
            }
        })?;
        let target = exact_locale_row(&translations, &target_locale);

        ensure_revision(
            "resource",
            &request.expected_resource_revision,
            &resource_revision(&price_list, &translations),
        )?;
        ensure_revision(
            "source",
            &request.expected_source_revision,
            &locale_revision(source),
        )?;
        let current_target_revision = target.map(locale_revision);
        if request.expected_target_revision != current_target_revision {
            return Err(PriceListTranslationExactLocaleError::RevisionConflict {
                revision: "target",
            });
        }

        let unchanged = target.is_some_and(|existing| {
            existing.name == target_name && existing.description == request.description
        });
        if !unchanged {
            if let Some(existing) = target.cloned() {
                let mut active: price_list_translation::ActiveModel = existing.into();
                active.name = Set(target_name);
                active.description = Set(request.description);
                active.update(&txn).await?;
            } else {
                price_list_translation::ActiveModel {
                    id: Set(generate_id()),
                    price_list_id: Set(price_list_id),
                    locale: Set(target_locale.clone()),
                    name: Set(target_name),
                    description: Set(request.description),
                }
                .insert(&txn)
                .await?;
            }
        }

        let translations_after = load_translations(&txn, price_list_id).await?;
        let target_after = exact_locale_row(&translations_after, &target_locale)
            .cloned()
            .ok_or_else(
                || PriceListTranslationExactLocaleError::TargetLocaleMissingAfterApply {
                    price_list_id,
                    locale: target_locale.clone(),
                },
            )?;
        let resource_revision = resource_revision(&price_list, &translations_after);
        let operation_id = operation_lease
            .map(|lease| lease.operation_id)
            .or_else(|| (!unchanged).then(generate_id));

        if !unchanged {
            record_price_list_translation_change_in_tx(
                &txn,
                tenant_id,
                price_list_id,
                operation_id.expect("changed Pricing translation apply must have operation id"),
                &resource_revision,
                PriceListTranslationChangeLifecycle::Active,
            )
            .await?;
        }

        let receipt = PriceListTranslationExactLocaleApplyReceipt {
            operation_id,
            price_list_id,
            resource_revision,
            target_revision: locale_revision(&target_after),
            target: PriceListTranslationExactLocaleRecord::from(target_after),
        };
        if let Some(lease) = operation_lease {
            idempotency::complete(&txn, lease, &receipt)
                .await
                .map_err(PriceListTranslationExactLocaleError::OperationReceipt)?;
        }
        txn.commit().await?;
        Ok(receipt)
    }
}

async fn load_price_list<C>(
    db: &C,
    tenant_id: Uuid,
    price_list_id: Uuid,
) -> PriceListTranslationExactLocaleResult<price_list::Model>
where
    C: ConnectionTrait,
{
    price_list::Entity::find_by_id(price_list_id)
        .filter(price_list::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or(PriceListTranslationExactLocaleError::PriceListNotFound(
            price_list_id,
        ))
}

async fn load_translations<C>(
    db: &C,
    price_list_id: Uuid,
) -> PriceListTranslationExactLocaleResult<Vec<price_list_translation::Model>>
where
    C: ConnectionTrait,
{
    Ok(price_list_translation::Entity::find()
        .filter(price_list_translation::Column::PriceListId.eq(price_list_id))
        .order_by_asc(price_list_translation::Column::Locale)
        .all(db)
        .await?)
}

async fn load_translations_for_price_lists<C>(
    db: &C,
    price_list_ids: &[Uuid],
) -> PriceListTranslationExactLocaleResult<Vec<price_list_translation::Model>>
where
    C: ConnectionTrait,
{
    if price_list_ids.is_empty() {
        return Ok(Vec::new());
    }
    Ok(price_list_translation::Entity::find()
        .filter(price_list_translation::Column::PriceListId.is_in(price_list_ids.to_vec()))
        .order_by_asc(price_list_translation::Column::PriceListId)
        .order_by_asc(price_list_translation::Column::Locale)
        .all(db)
        .await?)
}

fn build_snapshot(
    price_list: price_list::Model,
    translations: Vec<price_list_translation::Model>,
    source_locale: String,
    target_locale: String,
) -> PriceListTranslationExactLocaleResult<PriceListTranslationExactLocaleSnapshot> {
    let source = exact_locale_row(&translations, &source_locale)
        .cloned()
        .ok_or_else(
            || PriceListTranslationExactLocaleError::SourceLocaleNotFound {
                price_list_id: price_list.id,
                locale: source_locale.clone(),
            },
        )?;
    let target = exact_locale_row(&translations, &target_locale).cloned();
    let resource_revision = resource_revision(&price_list, &translations);
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

    Ok(PriceListTranslationExactLocaleSnapshot {
        price_list_id: price_list.id,
        source_locale,
        target_locale,
        resource_revision,
        source_revision,
        target_revision,
        exact_locales,
        source: PriceListTranslationExactLocaleRecord::from(source),
        target: target.map(PriceListTranslationExactLocaleRecord::from),
    })
}

fn exact_locale_row<'a>(
    translations: &'a [price_list_translation::Model],
    locale: &str,
) -> Option<&'a price_list_translation::Model> {
    translations
        .iter()
        .find(|translation| translation.locale == locale)
}

fn validate_tenant(tenant_id: Uuid) -> PriceListTranslationExactLocaleResult<()> {
    if tenant_id.is_nil() {
        return Err(PriceListTranslationExactLocaleError::Validation(
            "Pricing translation tenant_id must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn canonical_locale(locale: &str) -> PriceListTranslationExactLocaleResult<String> {
    TenantLocale::new(locale)
        .map(TenantLocale::into_inner)
        .map_err(|error| PriceListTranslationExactLocaleError::Validation(error.to_string()))
}

fn validate_locale_pair(
    source_locale: &str,
    target_locale: &str,
) -> PriceListTranslationExactLocaleResult<()> {
    if source_locale == target_locale {
        return Err(PriceListTranslationExactLocaleError::Validation(
            "Price list translation source and target locale must differ".to_string(),
        ));
    }
    Ok(())
}

fn normalize_name(name: &str) -> PriceListTranslationExactLocaleResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(PriceListTranslationExactLocaleError::Validation(
            "Price list name cannot be empty".to_string(),
        ));
    }
    if name.chars().count() > 100 {
        return Err(PriceListTranslationExactLocaleError::Validation(
            "Price list name must be at most 100 characters".to_string(),
        ));
    }
    Ok(name.to_string())
}

fn ensure_revision(
    revision: &'static str,
    expected: &str,
    current: &str,
) -> PriceListTranslationExactLocaleResult<()> {
    if expected != current {
        return Err(PriceListTranslationExactLocaleError::RevisionConflict { revision });
    }
    Ok(())
}

fn resource_revision(
    price_list: &price_list::Model,
    translations: &[price_list_translation::Model],
) -> String {
    let mut payload = Vec::new();
    append_text(
        &mut payload,
        "rustok-pricing/price-list-translation-resource/v1",
    );
    append_text(&mut payload, &price_list.id.to_string());
    append_text(&mut payload, &price_list.tenant_id.to_string());
    for translation in translations {
        append_translation(&mut payload, translation);
    }
    digest_revision(&payload)
}

fn locale_revision(translation: &price_list_translation::Model) -> String {
    let mut payload = Vec::new();
    append_text(
        &mut payload,
        "rustok-pricing/price-list-translation-locale/v1",
    );
    append_translation(&mut payload, translation);
    digest_revision(&payload)
}

fn append_translation(payload: &mut Vec<u8>, translation: &price_list_translation::Model) {
    append_text(payload, &translation.price_list_id.to_string());
    append_text(payload, &translation.locale);
    append_text(payload, &translation.name);
    append_optional_text(payload, translation.description.as_deref());
}

fn append_optional_text(payload: &mut Vec<u8>, value: Option<&str>) {
    match value {
        Some(value) => {
            payload.push(1);
            append_text(payload, value);
        }
        None => payload.push(0),
    }
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
