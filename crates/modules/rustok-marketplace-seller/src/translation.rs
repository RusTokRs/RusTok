use std::{collections::HashMap, sync::Arc};

use chrono::Utc;
use rustok_api::PortError;
use rustok_events::MarketplaceSellerEvent;
use rustok_outbox::{OutboxTransport, TransactionalEventBus, idempotency};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait,
    sea_query::{Expr, Query},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::entities::{seller, seller_translation};
use crate::localized_sellers::normalize_seller_locale;
use crate::translation_changes::{
    record_seller_translation_change_in_tx, translation_lifecycle_for_status,
};

pub const MAX_MARKETPLACE_SELLER_TRANSLATION_RESOURCE_PAGE: u16 = 200;

#[derive(Debug, Error)]
pub enum MarketplaceSellerTranslationExactLocaleError {
    #[error("marketplace seller {0} not found")]
    SellerNotFound(Uuid),

    #[error(
        "marketplace seller translation source locale not found: {locale} for seller {seller_id}"
    )]
    SourceLocaleNotFound { seller_id: Uuid, locale: String },

    #[error(
        "marketplace seller translation target locale missing after apply: {locale} for seller {seller_id}"
    )]
    TargetLocaleMissingAfterApply { seller_id: Uuid, locale: String },

    #[error("marketplace seller translation {revision} revision conflict")]
    RevisionConflict { revision: &'static str },

    #[error("marketplace seller translation validation failed: {0}")]
    Validation(String),

    #[error("marketplace seller translation owner receipt failed: {0}")]
    OperationReceipt(PortError),

    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

pub type MarketplaceSellerTranslationExactLocaleResult<T> =
    Result<T, MarketplaceSellerTranslationExactLocaleError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketplaceSellerTranslationExactLocaleRecord {
    pub locale: String,
    pub display_name: String,
}

impl From<seller_translation::Model> for MarketplaceSellerTranslationExactLocaleRecord {
    fn from(value: seller_translation::Model) -> Self {
        Self {
            locale: value.locale,
            display_name: value.display_name,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketplaceSellerTranslationExactLocaleSnapshot {
    pub seller_id: Uuid,
    pub status: String,
    pub source_locale: String,
    pub target_locale: String,
    pub resource_revision: String,
    pub source_revision: String,
    pub target_revision: Option<String>,
    pub exact_locales: Vec<String>,
    pub source: MarketplaceSellerTranslationExactLocaleRecord,
    pub target: Option<MarketplaceSellerTranslationExactLocaleRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketplaceSellerTranslationExactLocaleApply {
    pub source_locale: String,
    pub target_locale: String,
    pub display_name: String,
    pub expected_resource_revision: String,
    pub expected_source_revision: String,
    pub expected_target_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketplaceSellerTranslationExactLocaleApplyReceipt {
    pub operation_id: Option<Uuid>,
    pub seller_id: Uuid,
    pub resource_revision: String,
    pub target_revision: String,
    pub target: MarketplaceSellerTranslationExactLocaleRecord,
}

/// Marketplace Seller-owned exact-locale mutation boundary for public seller presentation copy.
///
/// Only localized `display_name` participates in Translation revisions. Legal identity,
/// onboarding/suspension prose, membership, metadata, handle and operational lifecycle remain
/// Marketplace Seller state. The parent Seller row is the serialization lock for every localized
/// presentation write. A real localized mutation publishes the canonical Seller profile-updated
/// contract in the same transaction as the copy row, owner change journal and operation receipt.
pub struct MarketplaceSellerTranslationService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl MarketplaceSellerTranslationService {
    pub fn new(db: DatabaseConnection) -> Self {
        let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
        Self { db, event_bus }
    }

    pub fn database(&self) -> &DatabaseConnection {
        &self.db
    }

    pub async fn list_exact_resources(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
        after: Option<Uuid>,
        limit: u16,
    ) -> MarketplaceSellerTranslationExactLocaleResult<(
        Vec<MarketplaceSellerTranslationExactLocaleSnapshot>,
        Option<Uuid>,
    )> {
        if tenant_id.is_nil() {
            return Err(MarketplaceSellerTranslationExactLocaleError::Validation(
                "marketplace seller Translation tenant_id must not be nil".to_string(),
            ));
        }
        let source_locale = canonical_locale(source_locale)?;
        let target_locale = canonical_locale(target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;
        if limit == 0 || limit > MAX_MARKETPLACE_SELLER_TRANSLATION_RESOURCE_PAGE {
            return Err(MarketplaceSellerTranslationExactLocaleError::Validation(
                format!(
                    "marketplace seller Translation resource page size must be between 1 and {MAX_MARKETPLACE_SELLER_TRANSLATION_RESOURCE_PAGE}"
                ),
            ));
        }

        let source_seller_ids = Query::select()
            .column(seller_translation::Column::SellerId)
            .from(seller_translation::Entity)
            .and_where(Expr::col(seller_translation::Column::TenantId).eq(tenant_id))
            .and_where(Expr::col(seller_translation::Column::Locale).eq(source_locale.clone()))
            .to_owned();
        let mut query = seller::Entity::find()
            .filter(seller::Column::TenantId.eq(tenant_id))
            .filter(seller::Column::Id.in_subquery(source_seller_ids))
            .order_by_asc(seller::Column::Id);
        if let Some(after) = after {
            query = query.filter(seller::Column::Id.gt(after));
        }

        let mut sellers = query.limit(u64::from(limit) + 1).all(&self.db).await?;
        let has_more = sellers.len() > usize::from(limit);
        if has_more {
            sellers.truncate(usize::from(limit));
        }
        let next_after = has_more
            .then(|| sellers.last().map(|seller| seller.id))
            .flatten();
        if sellers.is_empty() {
            return Ok((Vec::new(), None));
        }

        let seller_ids = sellers.iter().map(|seller| seller.id).collect::<Vec<_>>();
        let mut translations = load_translations_for_sellers(&self.db, tenant_id, &seller_ids)
            .await?
            .into_iter()
            .fold(
                HashMap::<Uuid, Vec<seller_translation::Model>>::new(),
                |mut grouped, translation| {
                    grouped
                        .entry(translation.seller_id)
                        .or_default()
                        .push(translation);
                    grouped
                },
            );

        let mut snapshots = Vec::with_capacity(sellers.len());
        for seller in sellers {
            let exact = translations.remove(&seller.id).ok_or_else(|| {
                MarketplaceSellerTranslationExactLocaleError::SourceLocaleNotFound {
                    seller_id: seller.id,
                    locale: source_locale.clone(),
                }
            })?;
            snapshots.push(build_snapshot(
                seller,
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
        seller_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> MarketplaceSellerTranslationExactLocaleResult<
        MarketplaceSellerTranslationExactLocaleSnapshot,
    > {
        validate_identity(tenant_id, seller_id)?;
        let source_locale = canonical_locale(source_locale)?;
        let target_locale = canonical_locale(target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;

        let seller = load_seller(&self.db, tenant_id, seller_id).await?;
        let translations = load_translations(&self.db, tenant_id, seller_id).await?;
        build_snapshot(seller, translations, source_locale, target_locale)
    }

    pub async fn apply_exact_locale(
        &self,
        tenant_id: Uuid,
        seller_id: Uuid,
        request: MarketplaceSellerTranslationExactLocaleApply,
    ) -> MarketplaceSellerTranslationExactLocaleResult<
        MarketplaceSellerTranslationExactLocaleApplyReceipt,
    > {
        self.apply_exact_locale_inner(tenant_id, None, seller_id, request, None)
            .await
    }

    pub(crate) async fn apply_exact_locale_with_operation(
        &self,
        tenant_id: Uuid,
        actor_user_id: Option<Uuid>,
        seller_id: Uuid,
        request: MarketplaceSellerTranslationExactLocaleApply,
        operation_lease: idempotency::Lease,
    ) -> MarketplaceSellerTranslationExactLocaleResult<
        MarketplaceSellerTranslationExactLocaleApplyReceipt,
    > {
        self.apply_exact_locale_inner(
            tenant_id,
            actor_user_id,
            seller_id,
            request,
            Some(operation_lease),
        )
        .await
    }

    async fn apply_exact_locale_inner(
        &self,
        tenant_id: Uuid,
        actor_user_id: Option<Uuid>,
        seller_id: Uuid,
        request: MarketplaceSellerTranslationExactLocaleApply,
        operation_lease: Option<idempotency::Lease>,
    ) -> MarketplaceSellerTranslationExactLocaleResult<
        MarketplaceSellerTranslationExactLocaleApplyReceipt,
    > {
        validate_identity(tenant_id, seller_id)?;
        let source_locale = canonical_locale(&request.source_locale)?;
        let target_locale = canonical_locale(&request.target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;
        let display_name = normalize_display_name(&request.display_name)?;

        let txn = self.db.begin().await?;
        let seller = seller::Entity::find_by_id(seller_id)
            .filter(seller::Column::TenantId.eq(tenant_id))
            .lock_exclusive()
            .one(&txn)
            .await?
            .ok_or(MarketplaceSellerTranslationExactLocaleError::SellerNotFound(seller_id))?;
        if seller.status == "closed" {
            return Err(MarketplaceSellerTranslationExactLocaleError::Validation(
                "closed marketplace seller presentation cannot be translated".to_string(),
            ));
        }
        let translations = load_translations(&txn, tenant_id, seller_id).await?;
        let source = exact_locale_row(&translations, &source_locale).ok_or_else(|| {
            MarketplaceSellerTranslationExactLocaleError::SourceLocaleNotFound {
                seller_id,
                locale: source_locale.clone(),
            }
        })?;
        let target = exact_locale_row(&translations, &target_locale);

        ensure_revision(
            "resource",
            &request.expected_resource_revision,
            &resource_revision(&seller, &translations),
        )?;
        ensure_revision(
            "source",
            &request.expected_source_revision,
            &locale_revision(source),
        )?;
        let current_target_revision = target.map(locale_revision);
        if request.expected_target_revision != current_target_revision {
            return Err(
                MarketplaceSellerTranslationExactLocaleError::RevisionConflict {
                    revision: "target",
                },
            );
        }

        let unchanged = target.is_some_and(|current| current.display_name == display_name);
        if !unchanged {
            let now = Utc::now().fixed_offset();
            if let Some(current) = target.cloned() {
                let mut active: seller_translation::ActiveModel = current.into();
                active.display_name = Set(display_name);
                active.updated_at = Set(now);
                active.update(&txn).await?;
            } else {
                seller_translation::ActiveModel {
                    id: Set(rustok_core::generate_id()),
                    tenant_id: Set(tenant_id),
                    seller_id: Set(seller_id),
                    locale: Set(target_locale.clone()),
                    display_name: Set(display_name),
                    created_at: Set(now),
                    updated_at: Set(now),
                }
                .insert(&txn)
                .await?;
            }
        }

        let translations_after = load_translations(&txn, tenant_id, seller_id).await?;
        let target_after = exact_locale_row(&translations_after, &target_locale)
            .cloned()
            .ok_or_else(|| {
                MarketplaceSellerTranslationExactLocaleError::TargetLocaleMissingAfterApply {
                    seller_id,
                    locale: target_locale,
                }
            })?;
        let resource_revision = resource_revision(&seller, &translations_after);
        let operation_id = operation_lease
            .map(|lease| lease.operation_id)
            .or_else(|| (!unchanged).then(rustok_core::generate_id));
        if !unchanged {
            record_seller_translation_change_in_tx(
                &txn,
                tenant_id,
                seller_id,
                operation_id.expect("changed seller Translation apply must have an operation id"),
                &resource_revision,
                translation_lifecycle_for_status(&seller.status)
                    .map_err(|error| {
                        MarketplaceSellerTranslationExactLocaleError::Validation(
                            error.to_string(),
                        )
                    })?,
            )
            .await
            .map_err(|error| {
                MarketplaceSellerTranslationExactLocaleError::Database(sea_orm::DbErr::Custom(
                    error.to_string(),
                ))
            })?;

            if let Err(error) = self
                .event_bus
                .publish_contract_in_tx(
                    &txn,
                    tenant_id,
                    actor_user_id,
                    MarketplaceSellerEvent::MarketplaceSellerProfileUpdated { seller_id },
                )
                .await
            {
                tracing::error!(
                    tenant_id = %tenant_id,
                    seller_id = %seller_id,
                    error = %error,
                    "Marketplace Seller Translation owner event publication failed"
                );
                return Err(MarketplaceSellerTranslationExactLocaleError::Database(
                    sea_orm::DbErr::Custom(
                        "marketplace seller Translation owner event publication unavailable"
                            .to_string(),
                    ),
                ));
            }
        }

        let receipt = MarketplaceSellerTranslationExactLocaleApplyReceipt {
            operation_id,
            seller_id,
            resource_revision,
            target_revision: locale_revision(&target_after),
            target: target_after.into(),
        };
        if let Some(lease) = operation_lease {
            idempotency::complete(&txn, lease, &receipt)
                .await
                .map_err(MarketplaceSellerTranslationExactLocaleError::OperationReceipt)?;
        }
        txn.commit().await?;
        Ok(receipt)
    }
}

async fn load_seller<C>(
    db: &C,
    tenant_id: Uuid,
    seller_id: Uuid,
) -> MarketplaceSellerTranslationExactLocaleResult<seller::Model>
where
    C: ConnectionTrait,
{
    seller::Entity::find_by_id(seller_id)
        .filter(seller::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or(MarketplaceSellerTranslationExactLocaleError::SellerNotFound(seller_id))
}

async fn load_translations<C>(
    db: &C,
    tenant_id: Uuid,
    seller_id: Uuid,
) -> MarketplaceSellerTranslationExactLocaleResult<Vec<seller_translation::Model>>
where
    C: ConnectionTrait,
{
    Ok(seller_translation::Entity::find()
        .filter(seller_translation::Column::TenantId.eq(tenant_id))
        .filter(seller_translation::Column::SellerId.eq(seller_id))
        .order_by_asc(seller_translation::Column::Locale)
        .all(db)
        .await?)
}

async fn load_translations_for_sellers<C>(
    db: &C,
    tenant_id: Uuid,
    seller_ids: &[Uuid],
) -> MarketplaceSellerTranslationExactLocaleResult<Vec<seller_translation::Model>>
where
    C: ConnectionTrait,
{
    if seller_ids.is_empty() {
        return Ok(Vec::new());
    }
    Ok(seller_translation::Entity::find()
        .filter(seller_translation::Column::TenantId.eq(tenant_id))
        .filter(seller_translation::Column::SellerId.is_in(seller_ids.to_vec()))
        .order_by_asc(seller_translation::Column::SellerId)
        .order_by_asc(seller_translation::Column::Locale)
        .all(db)
        .await?)
}

fn build_snapshot(
    seller: seller::Model,
    translations: Vec<seller_translation::Model>,
    source_locale: String,
    target_locale: String,
) -> MarketplaceSellerTranslationExactLocaleResult<MarketplaceSellerTranslationExactLocaleSnapshot>
{
    let source = exact_locale_row(&translations, &source_locale)
        .cloned()
        .ok_or_else(
            || MarketplaceSellerTranslationExactLocaleError::SourceLocaleNotFound {
                seller_id: seller.id,
                locale: source_locale.clone(),
            },
        )?;
    let target = exact_locale_row(&translations, &target_locale).cloned();
    let resource_revision = resource_revision(&seller, &translations);
    let source_revision = locale_revision(&source);
    let target_revision = target.as_ref().map(locale_revision);
    let exact_locales = translations
        .iter()
        .map(|translation| translation.locale.clone())
        .collect();

    Ok(MarketplaceSellerTranslationExactLocaleSnapshot {
        seller_id: seller.id,
        status: seller.status,
        source_locale,
        target_locale,
        resource_revision,
        source_revision,
        target_revision,
        exact_locales,
        source: source.into(),
        target: target.map(Into::into),
    })
}

fn exact_locale_row<'a>(
    translations: &'a [seller_translation::Model],
    locale: &str,
) -> Option<&'a seller_translation::Model> {
    translations
        .iter()
        .find(|translation| translation.locale == locale)
}

fn validate_identity(
    tenant_id: Uuid,
    seller_id: Uuid,
) -> MarketplaceSellerTranslationExactLocaleResult<()> {
    if tenant_id.is_nil() || seller_id.is_nil() {
        return Err(MarketplaceSellerTranslationExactLocaleError::Validation(
            "tenant_id and seller_id must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn canonical_locale(locale: &str) -> MarketplaceSellerTranslationExactLocaleResult<String> {
    normalize_seller_locale(locale).map_err(|error| {
        MarketplaceSellerTranslationExactLocaleError::Validation(error.to_string())
    })
}

fn validate_locale_pair(
    source_locale: &str,
    target_locale: &str,
) -> MarketplaceSellerTranslationExactLocaleResult<()> {
    if source_locale == target_locale {
        return Err(MarketplaceSellerTranslationExactLocaleError::Validation(
            "marketplace seller translation source and target locale must differ".to_string(),
        ));
    }
    Ok(())
}

fn normalize_display_name(
    display_name: &str,
) -> MarketplaceSellerTranslationExactLocaleResult<String> {
    let display_name = display_name.trim();
    if display_name.is_empty() {
        return Err(MarketplaceSellerTranslationExactLocaleError::Validation(
            "marketplace seller display_name must not be empty".to_string(),
        ));
    }
    if display_name.chars().count() > 160 {
        return Err(MarketplaceSellerTranslationExactLocaleError::Validation(
            "marketplace seller display_name must be at most 160 characters".to_string(),
        ));
    }
    Ok(display_name.to_string())
}

fn ensure_revision(
    revision: &'static str,
    expected: &str,
    current: &str,
) -> MarketplaceSellerTranslationExactLocaleResult<()> {
    if expected != current {
        return Err(MarketplaceSellerTranslationExactLocaleError::RevisionConflict { revision });
    }
    Ok(())
}

pub(crate) fn resource_revision(
    seller: &seller::Model,
    translations: &[seller_translation::Model],
) -> String {
    let mut digest = Sha256::new();
    append_text(
        &mut digest,
        "rustok-marketplace-seller/translation-resource/v1",
    );
    append_text(&mut digest, &seller.id.to_string());
    append_text(&mut digest, &seller.tenant_id.to_string());
    for translation in translations {
        append_translation(&mut digest, translation);
    }
    format!("sha256:{}", hex::encode(digest.finalize()))
}

fn locale_revision(translation: &seller_translation::Model) -> String {
    let mut digest = Sha256::new();
    append_text(
        &mut digest,
        "rustok-marketplace-seller/translation-locale/v1",
    );
    append_translation(&mut digest, translation);
    format!("sha256:{}", hex::encode(digest.finalize()))
}

fn append_translation(digest: &mut Sha256, translation: &seller_translation::Model) {
    append_text(digest, &translation.tenant_id.to_string());
    append_text(digest, &translation.seller_id.to_string());
    append_text(digest, &translation.locale);
    append_text(digest, &translation.display_name);
}

fn append_text(digest: &mut Sha256, value: &str) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value.as_bytes());
}
