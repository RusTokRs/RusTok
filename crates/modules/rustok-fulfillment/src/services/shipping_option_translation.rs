use std::fmt::Write as _;

use rustok_api::{TenantLocale, sha256_digest};
use rustok_core::generate_id;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::entities::{shipping_option, shipping_option_translation};

#[derive(Debug, Error)]
pub enum ShippingOptionTranslationExactLocaleError {
    #[error("Shipping option not found: {0}")]
    ShippingOptionNotFound(Uuid),

    #[error(
        "Shipping option translation source locale not found: {locale} for shipping option {shipping_option_id}"
    )]
    SourceLocaleNotFound {
        shipping_option_id: Uuid,
        locale: String,
    },

    #[error(
        "Shipping option translation target locale missing after apply: {locale} for shipping option {shipping_option_id}"
    )]
    TargetLocaleMissingAfterApply {
        shipping_option_id: Uuid,
        locale: String,
    },

    #[error("Shipping option translation {revision} revision conflict")]
    RevisionConflict { revision: &'static str },

    #[error("Shipping option translation validation failed: {0}")]
    Validation(String),

    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

pub type ShippingOptionTranslationExactLocaleResult<T> =
    Result<T, ShippingOptionTranslationExactLocaleError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShippingOptionTranslationExactLocaleRecord {
    pub locale: String,
    pub name: String,
}

impl From<shipping_option_translation::Model> for ShippingOptionTranslationExactLocaleRecord {
    fn from(value: shipping_option_translation::Model) -> Self {
        Self {
            locale: value.locale,
            name: value.name,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShippingOptionTranslationExactLocaleSnapshot {
    pub shipping_option_id: Uuid,
    pub active: bool,
    pub source_locale: String,
    pub target_locale: String,
    pub resource_revision: String,
    pub source_revision: String,
    pub target_revision: Option<String>,
    pub exact_locales: Vec<String>,
    pub source: ShippingOptionTranslationExactLocaleRecord,
    pub target: Option<ShippingOptionTranslationExactLocaleRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShippingOptionTranslationExactLocaleApply {
    pub source_locale: String,
    pub target_locale: String,
    pub name: String,
    pub expected_resource_revision: String,
    pub expected_source_revision: String,
    pub expected_target_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShippingOptionTranslationExactLocaleApplyReceipt {
    pub shipping_option_id: Uuid,
    pub resource_revision: String,
    pub target_revision: String,
    pub target: ShippingOptionTranslationExactLocaleRecord,
}

/// Fulfillment-owned exact-locale mutation boundary for Shipping Option presentation copy.
///
/// The parent Shipping Option row is locked for the complete compare-and-swap mutation so all
/// Translation callers serialize on owner state. Revisions intentionally cover only owner identity
/// plus localized `name`; amount, currency, provider, shipping-profile rules, metadata and active
/// state are operational Fulfillment data and are excluded from Translation revisions.
pub struct ShippingOptionTranslationService {
    db: DatabaseConnection,
}

impl ShippingOptionTranslationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub fn database(&self) -> &DatabaseConnection {
        &self.db
    }

    pub async fn read_exact_locale(
        &self,
        tenant_id: Uuid,
        shipping_option_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> ShippingOptionTranslationExactLocaleResult<ShippingOptionTranslationExactLocaleSnapshot>
    {
        validate_tenant(tenant_id)?;
        let source_locale = canonical_locale(source_locale)?;
        let target_locale = canonical_locale(target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;

        let option = load_shipping_option(&self.db, tenant_id, shipping_option_id).await?;
        let translations = load_translations(&self.db, shipping_option_id).await?;
        build_snapshot(option, translations, source_locale, target_locale)
    }

    pub async fn apply_exact_locale(
        &self,
        tenant_id: Uuid,
        shipping_option_id: Uuid,
        request: ShippingOptionTranslationExactLocaleApply,
    ) -> ShippingOptionTranslationExactLocaleResult<ShippingOptionTranslationExactLocaleApplyReceipt>
    {
        validate_tenant(tenant_id)?;
        let source_locale = canonical_locale(&request.source_locale)?;
        let target_locale = canonical_locale(&request.target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;
        let target_name = normalize_name(&request.name)?;

        let txn = self.db.begin().await?;
        let option = shipping_option::Entity::find_by_id(shipping_option_id)
            .filter(shipping_option::Column::TenantId.eq(tenant_id))
            .lock_exclusive()
            .one(&txn)
            .await?
            .ok_or(ShippingOptionTranslationExactLocaleError::ShippingOptionNotFound(
                shipping_option_id,
            ))?;
        let translations = load_translations(&txn, shipping_option_id).await?;
        let source = exact_locale_row(&translations, &source_locale).ok_or_else(|| {
            ShippingOptionTranslationExactLocaleError::SourceLocaleNotFound {
                shipping_option_id,
                locale: source_locale.clone(),
            }
        })?;
        let target = exact_locale_row(&translations, &target_locale);

        ensure_revision(
            "resource",
            &request.expected_resource_revision,
            &resource_revision(&option, &translations),
        )?;
        ensure_revision(
            "source",
            &request.expected_source_revision,
            &locale_revision(source),
        )?;
        let current_target_revision = target.map(locale_revision);
        if request.expected_target_revision != current_target_revision {
            return Err(ShippingOptionTranslationExactLocaleError::RevisionConflict {
                revision: "target",
            });
        }

        let unchanged = target.is_some_and(|existing| existing.name == target_name);
        if !unchanged {
            if let Some(existing) = target.cloned() {
                let mut active: shipping_option_translation::ActiveModel = existing.into();
                active.name = Set(target_name);
                active.update(&txn).await?;
            } else {
                shipping_option_translation::ActiveModel {
                    id: Set(generate_id()),
                    shipping_option_id: Set(shipping_option_id),
                    locale: Set(target_locale.clone()),
                    name: Set(target_name),
                }
                .insert(&txn)
                .await?;
            }
        }

        let translations_after = load_translations(&txn, shipping_option_id).await?;
        let target_after = exact_locale_row(&translations_after, &target_locale)
            .cloned()
            .ok_or_else(|| {
                ShippingOptionTranslationExactLocaleError::TargetLocaleMissingAfterApply {
                    shipping_option_id,
                    locale: target_locale.clone(),
                }
            })?;
        let resource_revision = resource_revision(&option, &translations_after);
        let receipt = ShippingOptionTranslationExactLocaleApplyReceipt {
            shipping_option_id,
            resource_revision,
            target_revision: locale_revision(&target_after),
            target: ShippingOptionTranslationExactLocaleRecord::from(target_after),
        };

        txn.commit().await?;
        Ok(receipt)
    }
}

async fn load_shipping_option<C>(
    db: &C,
    tenant_id: Uuid,
    shipping_option_id: Uuid,
) -> ShippingOptionTranslationExactLocaleResult<shipping_option::Model>
where
    C: ConnectionTrait,
{
    shipping_option::Entity::find_by_id(shipping_option_id)
        .filter(shipping_option::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or(ShippingOptionTranslationExactLocaleError::ShippingOptionNotFound(
            shipping_option_id,
        ))
}

async fn load_translations<C>(
    db: &C,
    shipping_option_id: Uuid,
) -> ShippingOptionTranslationExactLocaleResult<Vec<shipping_option_translation::Model>>
where
    C: ConnectionTrait,
{
    Ok(shipping_option_translation::Entity::find()
        .filter(shipping_option_translation::Column::ShippingOptionId.eq(shipping_option_id))
        .order_by_asc(shipping_option_translation::Column::Locale)
        .all(db)
        .await?)
}

fn build_snapshot(
    option: shipping_option::Model,
    translations: Vec<shipping_option_translation::Model>,
    source_locale: String,
    target_locale: String,
) -> ShippingOptionTranslationExactLocaleResult<ShippingOptionTranslationExactLocaleSnapshot> {
    let source = exact_locale_row(&translations, &source_locale)
        .cloned()
        .ok_or_else(|| ShippingOptionTranslationExactLocaleError::SourceLocaleNotFound {
            shipping_option_id: option.id,
            locale: source_locale.clone(),
        })?;
    let target = exact_locale_row(&translations, &target_locale).cloned();
    let resource_revision = resource_revision(&option, &translations);
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

    Ok(ShippingOptionTranslationExactLocaleSnapshot {
        shipping_option_id: option.id,
        active: option.active,
        source_locale,
        target_locale,
        resource_revision,
        source_revision,
        target_revision,
        exact_locales,
        source: ShippingOptionTranslationExactLocaleRecord::from(source),
        target: target.map(ShippingOptionTranslationExactLocaleRecord::from),
    })
}

fn exact_locale_row<'a>(
    translations: &'a [shipping_option_translation::Model],
    locale: &str,
) -> Option<&'a shipping_option_translation::Model> {
    translations
        .iter()
        .find(|translation| translation.locale == locale)
}

fn validate_tenant(tenant_id: Uuid) -> ShippingOptionTranslationExactLocaleResult<()> {
    if tenant_id.is_nil() {
        return Err(ShippingOptionTranslationExactLocaleError::Validation(
            "Fulfillment translation tenant_id must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn canonical_locale(locale: &str) -> ShippingOptionTranslationExactLocaleResult<String> {
    TenantLocale::new(locale)
        .map(TenantLocale::into_inner)
        .map_err(|error| ShippingOptionTranslationExactLocaleError::Validation(error.to_string()))
}

fn validate_locale_pair(
    source_locale: &str,
    target_locale: &str,
) -> ShippingOptionTranslationExactLocaleResult<()> {
    if source_locale == target_locale {
        return Err(ShippingOptionTranslationExactLocaleError::Validation(
            "Shipping option translation source and target locale must differ".to_string(),
        ));
    }
    Ok(())
}

fn normalize_name(name: &str) -> ShippingOptionTranslationExactLocaleResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ShippingOptionTranslationExactLocaleError::Validation(
            "Shipping option name cannot be empty".to_string(),
        ));
    }
    if name.chars().count() > 120 {
        return Err(ShippingOptionTranslationExactLocaleError::Validation(
            "Shipping option name must be at most 120 characters".to_string(),
        ));
    }
    Ok(name.to_string())
}

fn ensure_revision(
    revision: &'static str,
    expected: &str,
    current: &str,
) -> ShippingOptionTranslationExactLocaleResult<()> {
    if expected != current {
        return Err(ShippingOptionTranslationExactLocaleError::RevisionConflict { revision });
    }
    Ok(())
}

pub(crate) fn resource_revision(
    option: &shipping_option::Model,
    translations: &[shipping_option_translation::Model],
) -> String {
    let mut payload = Vec::new();
    append_text(
        &mut payload,
        "rustok-fulfillment/shipping-option-translation-resource/v1",
    );
    append_text(&mut payload, &option.id.to_string());
    append_text(&mut payload, &option.tenant_id.to_string());
    for translation in translations {
        append_translation(&mut payload, translation);
    }
    digest_revision(&payload)
}

fn locale_revision(translation: &shipping_option_translation::Model) -> String {
    let mut payload = Vec::new();
    append_text(
        &mut payload,
        "rustok-fulfillment/shipping-option-translation-locale/v1",
    );
    append_translation(&mut payload, translation);
    digest_revision(&payload)
}

fn append_translation(payload: &mut Vec<u8>, translation: &shipping_option_translation::Model) {
    append_text(payload, &translation.shipping_option_id.to_string());
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
