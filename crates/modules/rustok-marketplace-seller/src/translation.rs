use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::entities::{seller, seller_translation};
use crate::localized_sellers::normalize_seller_locale;

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
/// presentation write.
pub struct MarketplaceSellerTranslationService {
    db: DatabaseConnection,
}

impl MarketplaceSellerTranslationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub fn database(&self) -> &DatabaseConnection {
        &self.db
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
            .ok_or(MarketplaceSellerTranslationExactLocaleError::SellerNotFound(
                seller_id,
            ))?;
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

        if !target.is_some_and(|current| current.display_name == display_name) {
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
        let receipt = MarketplaceSellerTranslationExactLocaleApplyReceipt {
            seller_id,
            resource_revision: resource_revision(&seller, &translations_after),
            target_revision: locale_revision(&target_after),
            target: target_after.into(),
        };
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
        .ok_or(MarketplaceSellerTranslationExactLocaleError::SellerNotFound(
            seller_id,
        ))
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

fn build_snapshot(
    seller: seller::Model,
    translations: Vec<seller_translation::Model>,
    source_locale: String,
    target_locale: String,
) -> MarketplaceSellerTranslationExactLocaleResult<
    MarketplaceSellerTranslationExactLocaleSnapshot,
> {
    let source = exact_locale_row(&translations, &source_locale)
        .cloned()
        .ok_or_else(|| MarketplaceSellerTranslationExactLocaleError::SourceLocaleNotFound {
            seller_id: seller.id,
            locale: source_locale.clone(),
        })?;
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
