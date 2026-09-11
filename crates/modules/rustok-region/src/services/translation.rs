use std::fmt::Write as _;

use rustok_api::{TenantLocale, sha256_digest};
use rustok_commerce_foundation::entities;
use rustok_core::generate_id;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::error::RegionError;

#[derive(Debug, Error)]
pub enum RegionTranslationExactLocaleError {
    #[error(transparent)]
    Region(#[from] RegionError),

    #[error("Region translation source locale not found: {locale} for region {region_id}")]
    SourceLocaleNotFound { region_id: Uuid, locale: String },

    #[error("Region translation target locale missing after apply: {locale} for region {region_id}")]
    TargetLocaleMissingAfterApply { region_id: Uuid, locale: String },

    #[error("Region translation {revision} revision conflict")]
    RevisionConflict { revision: &'static str },
}

impl From<sea_orm::DbErr> for RegionTranslationExactLocaleError {
    fn from(error: sea_orm::DbErr) -> Self {
        Self::Region(RegionError::Database(error))
    }
}

pub type RegionTranslationExactLocaleResult<T> = Result<T, RegionTranslationExactLocaleError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionTranslationExactLocaleRecord {
    pub locale: String,
    pub name: String,
}

impl From<entities::region_translation::Model> for RegionTranslationExactLocaleRecord {
    fn from(value: entities::region_translation::Model) -> Self {
        Self {
            locale: value.locale,
            name: value.name,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionTranslationExactLocaleSnapshot {
    pub region_id: Uuid,
    pub source_locale: String,
    pub target_locale: String,
    pub resource_revision: String,
    pub source_revision: String,
    pub target_revision: Option<String>,
    pub exact_locales: Vec<String>,
    pub source: RegionTranslationExactLocaleRecord,
    pub target: Option<RegionTranslationExactLocaleRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionTranslationExactLocaleApply {
    pub source_locale: String,
    pub target_locale: String,
    pub name: String,
    pub expected_resource_revision: String,
    pub expected_source_revision: String,
    pub expected_target_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionTranslationExactLocaleApplyReceipt {
    pub region_id: Uuid,
    pub resource_revision: String,
    pub target_revision: String,
    pub target: RegionTranslationExactLocaleRecord,
}

/// Region-owned exact-locale mutation boundary for Translation callers.
///
/// Resource/source/target revisions are semantic SHA-256 digests. The parent
/// Region row is the serialization lock shared with the canonical Region write
/// path, so one-locale CAS applies cannot race a sibling-locale replacement.
pub struct RegionTranslationService {
    db: DatabaseConnection,
}

impl RegionTranslationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn read_exact_locale(
        &self,
        tenant_id: Uuid,
        region_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> RegionTranslationExactLocaleResult<RegionTranslationExactLocaleSnapshot> {
        let source_locale = canonical_locale(source_locale)?;
        let target_locale = canonical_locale(target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;

        let region = load_region(&self.db, tenant_id, region_id).await?;
        let translations = load_translations(&self.db, region_id).await?;
        build_snapshot(region, translations, source_locale, target_locale)
    }

    pub async fn apply_exact_locale(
        &self,
        tenant_id: Uuid,
        region_id: Uuid,
        request: RegionTranslationExactLocaleApply,
    ) -> RegionTranslationExactLocaleResult<RegionTranslationExactLocaleApplyReceipt> {
        let source_locale = canonical_locale(&request.source_locale)?;
        let target_locale = canonical_locale(&request.target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;
        let target_name = normalize_name(&request.name)?;

        let txn = self.db.begin().await?;
        let region = entities::region::Entity::find_by_id(region_id)
            .filter(entities::region::Column::TenantId.eq(tenant_id))
            .lock_exclusive()
            .one(&txn)
            .await?
            .ok_or(RegionError::RegionNotFound(region_id))?;
        let translations = load_translations(&txn, region_id).await?;
        let source = exact_locale_row(&translations, &source_locale).ok_or_else(|| {
            RegionTranslationExactLocaleError::SourceLocaleNotFound {
                region_id,
                locale: source_locale.clone(),
            }
        })?;
        let target = exact_locale_row(&translations, &target_locale);

        let current_resource_revision = resource_revision(&region, &translations);
        let current_source_revision = locale_revision(source);
        let current_target_revision = target.map(locale_revision);
        ensure_revision(
            "resource",
            &request.expected_resource_revision,
            &current_resource_revision,
        )?;
        ensure_revision(
            "source",
            &request.expected_source_revision,
            &current_source_revision,
        )?;
        if request.expected_target_revision != current_target_revision {
            return Err(RegionTranslationExactLocaleError::RevisionConflict {
                revision: "target",
            });
        }

        if let Some(existing) = target.cloned() {
            if existing.name != target_name {
                let mut active: entities::region_translation::ActiveModel = existing.into();
                active.name = Set(target_name);
                active.update(&txn).await?;
            }
        } else {
            entities::region_translation::ActiveModel {
                id: Set(generate_id()),
                region_id: Set(region_id),
                locale: Set(target_locale.clone()),
                name: Set(target_name),
            }
            .insert(&txn)
            .await?;
        }

        let translations_after = load_translations(&txn, region_id).await?;
        let target_after = exact_locale_row(&translations_after, &target_locale)
            .cloned()
            .ok_or_else(|| {
                RegionTranslationExactLocaleError::TargetLocaleMissingAfterApply {
                    region_id,
                    locale: target_locale.clone(),
                }
            })?;
        let receipt = RegionTranslationExactLocaleApplyReceipt {
            region_id,
            resource_revision: resource_revision(&region, &translations_after),
            target_revision: locale_revision(&target_after),
            target: RegionTranslationExactLocaleRecord::from(target_after),
        };
        txn.commit().await?;
        Ok(receipt)
    }
}

async fn load_region<C>(
    db: &C,
    tenant_id: Uuid,
    region_id: Uuid,
) -> RegionTranslationExactLocaleResult<entities::region::Model>
where
    C: ConnectionTrait,
{
    entities::region::Entity::find_by_id(region_id)
        .filter(entities::region::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| RegionError::RegionNotFound(region_id).into())
}

async fn load_translations<C>(
    db: &C,
    region_id: Uuid,
) -> RegionTranslationExactLocaleResult<Vec<entities::region_translation::Model>>
where
    C: ConnectionTrait,
{
    Ok(entities::region_translation::Entity::find()
        .filter(entities::region_translation::Column::RegionId.eq(region_id))
        .order_by_asc(entities::region_translation::Column::Locale)
        .all(db)
        .await?)
}

fn build_snapshot(
    region: entities::region::Model,
    translations: Vec<entities::region_translation::Model>,
    source_locale: String,
    target_locale: String,
) -> RegionTranslationExactLocaleResult<RegionTranslationExactLocaleSnapshot> {
    let source = exact_locale_row(&translations, &source_locale)
        .cloned()
        .ok_or_else(
            || RegionTranslationExactLocaleError::SourceLocaleNotFound {
                region_id: region.id,
                locale: source_locale.clone(),
            },
        )?;
    let target = exact_locale_row(&translations, &target_locale).cloned();
    let resource_revision = resource_revision(&region, &translations);
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

    Ok(RegionTranslationExactLocaleSnapshot {
        region_id: region.id,
        source_locale,
        target_locale,
        resource_revision,
        source_revision,
        target_revision,
        exact_locales,
        source: RegionTranslationExactLocaleRecord::from(source),
        target: target.map(RegionTranslationExactLocaleRecord::from),
    })
}

fn exact_locale_row<'a>(
    translations: &'a [entities::region_translation::Model],
    locale: &str,
) -> Option<&'a entities::region_translation::Model> {
    translations
        .iter()
        .find(|translation| translation.locale == locale)
}

fn canonical_locale(locale: &str) -> RegionTranslationExactLocaleResult<String> {
    TenantLocale::new(locale)
        .map(TenantLocale::into_inner)
        .map_err(|error| RegionError::Validation(error.to_string()).into())
}

fn validate_locale_pair(
    source_locale: &str,
    target_locale: &str,
) -> RegionTranslationExactLocaleResult<()> {
    if source_locale == target_locale {
        return Err(RegionError::Validation(
            "Region translation source and target locale must differ".to_string(),
        )
        .into());
    }
    Ok(())
}

fn normalize_name(name: &str) -> RegionTranslationExactLocaleResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(RegionError::Validation("Region name cannot be empty".to_string()).into());
    }
    if name.chars().count() > 100 {
        return Err(RegionError::Validation(
            "Region name must be at most 100 characters".to_string(),
        )
        .into());
    }
    Ok(name.to_string())
}

fn ensure_revision(
    revision: &'static str,
    expected: &str,
    current: &str,
) -> RegionTranslationExactLocaleResult<()> {
    if expected != current {
        return Err(RegionTranslationExactLocaleError::RevisionConflict { revision });
    }
    Ok(())
}

fn resource_revision(
    region: &entities::region::Model,
    translations: &[entities::region_translation::Model],
) -> String {
    let mut payload = Vec::new();
    append_text(&mut payload, "rustok-region/translation-resource/v1");
    append_text(&mut payload, &region.id.to_string());
    append_text(&mut payload, &region.tenant_id.to_string());
    for translation in translations {
        append_translation(&mut payload, translation);
    }
    digest_revision(&payload)
}

fn locale_revision(translation: &entities::region_translation::Model) -> String {
    let mut payload = Vec::new();
    append_text(&mut payload, "rustok-region/translation-locale/v1");
    append_translation(&mut payload, translation);
    digest_revision(&payload)
}

fn append_translation(payload: &mut Vec<u8>, translation: &entities::region_translation::Model) {
    append_text(payload, &translation.region_id.to_string());
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
