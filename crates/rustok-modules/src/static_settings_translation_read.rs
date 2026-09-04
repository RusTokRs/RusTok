//! Owner-side read model for static Settings Translation onboarding.
//!
//! This module intentionally exposes owner contracts rather than Translation
//! provider types. The eventual adapter can map these bounded, exact-locale
//! facts into `rustok-translation-targets` without reading owner tables directly.

use std::collections::BTreeMap;

use rustok_api::TenantLocale;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::data::configure_tenant_scope;
use crate::operation_store::{StaticTenantLifecycleStore, StaticTenantLifecycleStoreError};
use crate::static_settings_localization::StaticSettingsLocalizationRegistry;
use crate::static_settings_source_locale::{
    StaticSettingsSourceLocaleError, StaticSettingsSourceLocaleService,
};

pub const MAX_STATIC_SETTINGS_CHANGE_PAGE_SIZE: u16 = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StaticSettingsChangeKind {
    BaseProjection,
    LocalizedTarget,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticSettingsChangeRecord {
    pub change_seq: u64,
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub change_kind: StaticSettingsChangeKind,
    pub field_id: Option<String>,
    pub locale: Option<String>,
    pub owner_revision: u64,
    pub target_revision: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticSettingsChangeReadRequest {
    pub tenant_id: Uuid,
    /// Exclusive cursor. `None` starts before the first change.
    pub after_seq: Option<u64>,
    /// Inclusive snapshot bound. Leave `None` on the first page; reuse the
    /// returned `through_seq` on every continuation page.
    pub through_seq: Option<u64>,
    pub limit: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticSettingsChangePage {
    pub changes: Vec<StaticSettingsChangeRecord>,
    /// Stable inclusive high-water mark for this scan. A repair reader must
    /// keep this value unchanged while draining continuation pages.
    pub through_seq: Option<u64>,
    /// Exclusive cursor for the next page, or `None` when the bounded scan is
    /// drained through `through_seq`.
    pub next_after_seq: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticSettingsExactLocaleField {
    pub field_id: String,
    pub source_value: String,
    pub exact_target_value: Option<String>,
    pub target_revision: Option<u64>,
    pub target_owner_revision: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticSettingsExactLocaleSnapshot {
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub source_locale: String,
    pub target_locale: String,
    pub owner_revision: u64,
    pub owner_change_seq: Option<u64>,
    /// Only owner-declared localized fields that currently have source copy are
    /// included. Missing optional source leaves therefore do not become phantom
    /// Translation units.
    pub fields: Vec<StaticSettingsExactLocaleField>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticSettingsExactLocaleProgress {
    pub source_units: u64,
    pub exact_units: u64,
    pub missing_units: u64,
    pub complete: bool,
    pub owner_change_seq: Option<u64>,
}

impl StaticSettingsExactLocaleSnapshot {
    pub fn progress(&self) -> StaticSettingsExactLocaleProgress {
        let source_units = self.fields.len() as u64;
        let exact_units = self
            .fields
            .iter()
            .filter(|field| field.exact_target_value.is_some())
            .count() as u64;
        StaticSettingsExactLocaleProgress {
            source_units,
            exact_units,
            missing_units: source_units.saturating_sub(exact_units),
            complete: exact_units == source_units,
            owner_change_seq: self.owner_change_seq,
        }
    }
}

#[derive(Debug, Error)]
pub enum StaticSettingsTranslationReadError {
    #[error("static Settings translation reads require a non-nil tenant")]
    InvalidIdentity,
    #[error("static Settings translation reads require an exact canonical tenant locale")]
    InvalidLocale,
    #[error("static Settings source and target locale must differ")]
    EqualSourceAndTargetLocale,
    #[error(
        "static Settings change page limit must be between 1 and {MAX_STATIC_SETTINGS_CHANGE_PAGE_SIZE}"
    )]
    InvalidPageLimit,
    #[error("static Settings change cursor bounds are invalid")]
    InvalidCursorBounds,
    #[error("static Settings owner operation is already active for `{0}`")]
    OwnerOperationInProgress(String),
    #[error("static Settings exact-locale snapshot changed while it was being read")]
    SnapshotUnstable,
    #[error("static Settings translation read state is inconsistent: {0}")]
    InconsistentState(String),
    #[error("static Settings translation read failed: {0}")]
    Database(String),
    #[error(transparent)]
    SourceLocale(#[from] StaticSettingsSourceLocaleError),
}

#[derive(Clone)]
pub struct StaticSettingsTranslationReadService {
    db: DatabaseConnection,
}

impl StaticSettingsTranslationReadService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Reads one keyset page from the content-free owner change journal.
    ///
    /// The first page captures the current high-water mark. Continuations must
    /// replay that exact bound, preventing newly committed owner changes from
    /// extending an in-progress repair scan indefinitely.
    pub async fn read_changes(
        &self,
        registry: &StaticSettingsLocalizationRegistry,
        request: StaticSettingsChangeReadRequest,
    ) -> Result<StaticSettingsChangePage, StaticSettingsTranslationReadError> {
        validate_tenant(request.tenant_id)?;
        if request.limit == 0 || request.limit > MAX_STATIC_SETTINGS_CHANGE_PAGE_SIZE {
            return Err(StaticSettingsTranslationReadError::InvalidPageLimit);
        }
        if let Some(through_seq) = request.through_seq
            && through_seq == 0
        {
            return Err(StaticSettingsTranslationReadError::InvalidCursorBounds);
        }
        if let (Some(after_seq), Some(through_seq)) = (request.after_seq, request.through_seq)
            && after_seq > through_seq
        {
            return Err(StaticSettingsTranslationReadError::InvalidCursorBounds);
        }

        let transaction = self.db.begin().await.map_err(database_error)?;
        configure_tenant_scope(&transaction, request.tenant_id)
            .await
            .map_err(|error| database_error(error.to_string()))?;

        let current_high_water = load_high_watermark(
            &transaction,
            request.tenant_id,
            registry.module_slug(),
        )
        .await?;
        if let Some(requested) = request.through_seq {
            match current_high_water {
                Some(current) if requested <= current => {}
                _ => return Err(StaticSettingsTranslationReadError::InvalidCursorBounds),
            }
        }
        let through_seq = request.through_seq.or(current_high_water);
        let Some(through_seq_value) = through_seq else {
            transaction.commit().await.map_err(database_error)?;
            return Ok(StaticSettingsChangePage {
                changes: Vec::new(),
                through_seq: None,
                next_after_seq: None,
            });
        };

        let after_seq = request.after_seq.unwrap_or(0);
        if after_seq >= through_seq_value {
            transaction.commit().await.map_err(database_error)?;
            return Ok(StaticSettingsChangePage {
                changes: Vec::new(),
                through_seq: Some(through_seq_value),
                next_after_seq: None,
            });
        }

        let fetch_limit = u64::from(request.limit) + 1;
        let rows = load_change_rows(
            &transaction,
            request.tenant_id,
            registry.module_slug(),
            after_seq,
            through_seq_value,
            fetch_limit,
        )
        .await?;
        transaction.commit().await.map_err(database_error)?;

        let has_more = rows.len() > usize::from(request.limit);
        let mut changes = rows;
        if has_more {
            changes.truncate(usize::from(request.limit));
        }
        let next_after_seq = has_more
            .then(|| changes.last().map(|change| change.change_seq))
            .flatten();

        Ok(StaticSettingsChangePage {
            changes,
            through_seq: Some(through_seq_value),
            next_after_seq,
        })
    }

    /// Returns one stable owner snapshot containing authoritative source copy
    /// plus exact target rows for a single canonical target locale.
    ///
    /// Runtime fallback is never consulted. The shared static owner revision is
    /// checked before and after the target read, so concurrent source or target
    /// mutation fails closed instead of producing mixed progress facts.
    pub async fn exact_locale_snapshot(
        &self,
        tenant_id: Uuid,
        registry: &StaticSettingsLocalizationRegistry,
        target_locale: &str,
    ) -> Result<StaticSettingsExactLocaleSnapshot, StaticSettingsTranslationReadError> {
        validate_tenant(tenant_id)?;
        let target_locale = canonical_locale(target_locale)?;
        let authoritative = StaticSettingsSourceLocaleService::new(self.db.clone())
            .authoritative_source_snapshot(tenant_id, registry)
            .await?;
        if authoritative.source_locale == target_locale {
            return Err(StaticSettingsTranslationReadError::EqualSourceAndTargetLocale);
        }

        let transaction = self.db.begin().await.map_err(database_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(|error| database_error(error.to_string()))?;
        let owner_before = StaticTenantLifecycleStore::snapshot(
            &transaction,
            tenant_id,
            registry.module_slug(),
        )
        .await
        .map_err(map_owner_error)?;
        if owner_before.active_idempotency_key.is_some() {
            return Err(StaticSettingsTranslationReadError::OwnerOperationInProgress(
                registry.module_slug().to_string(),
            ));
        }
        if owner_before.revision != authoritative.source.owner_revision {
            return Err(StaticSettingsTranslationReadError::SnapshotUnstable);
        }

        let exact_rows = load_exact_locale_rows(
            &transaction,
            tenant_id,
            registry.module_slug(),
            &target_locale,
        )
        .await?;
        let owner_change_seq =
            load_high_watermark(&transaction, tenant_id, registry.module_slug()).await?;

        let owner_after = StaticTenantLifecycleStore::snapshot(
            &transaction,
            tenant_id,
            registry.module_slug(),
        )
        .await
        .map_err(map_owner_error)?;
        if owner_after.active_idempotency_key.is_some()
            || owner_after.revision != owner_before.revision
        {
            return Err(StaticSettingsTranslationReadError::SnapshotUnstable);
        }
        transaction.commit().await.map_err(database_error)?;

        let mut fields = Vec::with_capacity(authoritative.source.values.len());
        for (field_id, source_value) in authoritative.source.values {
            let exact = exact_rows.get(&field_id);
            fields.push(StaticSettingsExactLocaleField {
                field_id,
                source_value,
                exact_target_value: exact.map(|row| row.value.clone()),
                target_revision: exact.map(|row| row.target_revision),
                target_owner_revision: exact.map(|row| row.owner_revision),
            });
        }

        Ok(StaticSettingsExactLocaleSnapshot {
            tenant_id,
            module_slug: registry.module_slug().to_string(),
            source_locale: authoritative.source_locale,
            target_locale,
            owner_revision: owner_after.revision,
            owner_change_seq,
            fields,
        })
    }
}

#[derive(Clone, Debug)]
struct ExactTargetRow {
    value: String,
    target_revision: u64,
    owner_revision: u64,
}

async fn load_high_watermark<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    module_slug: &str,
) -> Result<Option<u64>, StaticSettingsTranslationReadError> {
    let backend = connection.get_database_backend();
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            match backend {
                DbBackend::Postgres => {
                    "SELECT MAX(change_seq) AS max_change_seq FROM module_static_settings_changes WHERE tenant_id = $1 AND module_slug = $2"
                }
                _ => {
                    "SELECT MAX(change_seq) AS max_change_seq FROM module_static_settings_changes WHERE tenant_id = ?1 AND module_slug = ?2"
                }
            },
            vec![tenant_id.into(), module_slug.into()],
        ))
        .await
        .map_err(database_error)?
        .ok_or_else(|| {
            StaticSettingsTranslationReadError::InconsistentState(
                "aggregate change-cursor query returned no row".to_string(),
            )
        })?;
    let value: Option<i64> = row.try_get("", "max_change_seq").map_err(database_error)?;
    value.map(|value| positive_u64(value, "change_seq")).transpose()
}

async fn load_change_rows<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    module_slug: &str,
    after_seq: u64,
    through_seq: u64,
    limit: u64,
) -> Result<Vec<StaticSettingsChangeRecord>, StaticSettingsTranslationReadError> {
    let backend = connection.get_database_backend();
    let rows = connection
        .query_all_raw(Statement::from_sql_and_values(
            backend,
            match backend {
                DbBackend::Postgres => {
                    "SELECT change_seq, change_kind, field_id, locale, owner_revision, target_revision \
                     FROM module_static_settings_changes \
                     WHERE tenant_id = $1 AND module_slug = $2 AND change_seq > $3 AND change_seq <= $4 \
                     ORDER BY change_seq ASC LIMIT $5"
                }
                _ => {
                    "SELECT change_seq, change_kind, field_id, locale, owner_revision, target_revision \
                     FROM module_static_settings_changes \
                     WHERE tenant_id = ?1 AND module_slug = ?2 AND change_seq > ?3 AND change_seq <= ?4 \
                     ORDER BY change_seq ASC LIMIT ?5"
                }
            },
            vec![
                tenant_id.into(),
                module_slug.into(),
                integer_value(after_seq, "after_seq")?,
                integer_value(through_seq, "through_seq")?,
                integer_value(limit, "limit")?,
            ],
        ))
        .await
        .map_err(database_error)?;

    rows.into_iter()
        .map(|row| parse_change_row(&row, tenant_id, module_slug))
        .collect()
}

fn parse_change_row(
    row: &sea_orm::QueryResult,
    tenant_id: Uuid,
    module_slug: &str,
) -> Result<StaticSettingsChangeRecord, StaticSettingsTranslationReadError> {
    let change_seq: i64 = row.try_get("", "change_seq").map_err(database_error)?;
    let owner_revision: i64 = row.try_get("", "owner_revision").map_err(database_error)?;
    let target_revision: Option<i64> =
        row.try_get("", "target_revision").map_err(database_error)?;
    let change_kind: String = row.try_get("", "change_kind").map_err(database_error)?;
    let field_id: Option<String> = row.try_get("", "field_id").map_err(database_error)?;
    let locale: Option<String> = row.try_get("", "locale").map_err(database_error)?;

    let change_kind = match change_kind.as_str() {
        "base_projection" if field_id.is_none() && locale.is_none() && target_revision.is_none() => {
            StaticSettingsChangeKind::BaseProjection
        }
        "localized_target" if field_id.is_some() && locale.is_some() && target_revision.is_some() => {
            if field_id.as_deref().is_some_and(str::is_empty) {
                return Err(StaticSettingsTranslationReadError::InconsistentState(
                    "localized-target change contains an empty field identity".to_string(),
                ));
            }
            if let Some(locale) = &locale {
                ensure_stored_locale_is_canonical(locale)?;
            }
            StaticSettingsChangeKind::LocalizedTarget
        }
        _ => {
            return Err(StaticSettingsTranslationReadError::InconsistentState(
                "change journal row violates its kind-specific shape".to_string(),
            ));
        }
    };

    Ok(StaticSettingsChangeRecord {
        change_seq: positive_u64(change_seq, "change_seq")?,
        tenant_id,
        module_slug: module_slug.to_string(),
        change_kind,
        field_id,
        locale,
        owner_revision: positive_u64(owner_revision, "owner_revision")?,
        target_revision: target_revision
            .map(|value| positive_u64(value, "target_revision"))
            .transpose()?,
    })
}

async fn load_exact_locale_rows<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    module_slug: &str,
    locale: &str,
) -> Result<BTreeMap<String, ExactTargetRow>, StaticSettingsTranslationReadError> {
    let backend = connection.get_database_backend();
    let rows = connection
        .query_all_raw(Statement::from_sql_and_values(
            backend,
            match backend {
                DbBackend::Postgres => {
                    "SELECT field_id, value, revision, owner_revision \
                     FROM module_static_localized_settings \
                     WHERE tenant_id = $1 AND module_slug = $2 AND locale = $3 ORDER BY field_id ASC"
                }
                _ => {
                    "SELECT field_id, value, revision, owner_revision \
                     FROM module_static_localized_settings \
                     WHERE tenant_id = ?1 AND module_slug = ?2 AND locale = ?3 ORDER BY field_id ASC"
                }
            },
            vec![tenant_id.into(), module_slug.into(), locale.into()],
        ))
        .await
        .map_err(database_error)?;

    let mut exact = BTreeMap::new();
    for row in rows {
        let field_id: String = row.try_get("", "field_id").map_err(database_error)?;
        if field_id.is_empty() {
            return Err(StaticSettingsTranslationReadError::InconsistentState(
                "localized Settings row contains an empty field identity".to_string(),
            ));
        }
        let revision: i64 = row.try_get("", "revision").map_err(database_error)?;
        let owner_revision: i64 = row.try_get("", "owner_revision").map_err(database_error)?;
        let previous = exact.insert(
            field_id,
            ExactTargetRow {
                value: row.try_get("", "value").map_err(database_error)?,
                target_revision: positive_u64(revision, "target_revision")?,
                owner_revision: positive_u64(owner_revision, "owner_revision")?,
            },
        );
        if previous.is_some() {
            return Err(StaticSettingsTranslationReadError::InconsistentState(
                "localized Settings query returned duplicate field identities".to_string(),
            ));
        }
    }
    Ok(exact)
}

fn validate_tenant(tenant_id: Uuid) -> Result<(), StaticSettingsTranslationReadError> {
    if tenant_id.is_nil() {
        return Err(StaticSettingsTranslationReadError::InvalidIdentity);
    }
    Ok(())
}

fn canonical_locale(locale: &str) -> Result<String, StaticSettingsTranslationReadError> {
    let canonical =
        TenantLocale::new(locale).map_err(|_| StaticSettingsTranslationReadError::InvalidLocale)?;
    if canonical.as_str() != locale {
        return Err(StaticSettingsTranslationReadError::InvalidLocale);
    }
    Ok(canonical.into_inner())
}

fn ensure_stored_locale_is_canonical(
    locale: &str,
) -> Result<(), StaticSettingsTranslationReadError> {
    canonical_locale(locale).map(|_| ()).map_err(|_| {
        StaticSettingsTranslationReadError::InconsistentState(
            "stored Settings translation locale is not canonical TenantLocale data".to_string(),
        )
    })
}

fn positive_u64(
    value: i64,
    field: &str,
) -> Result<u64, StaticSettingsTranslationReadError> {
    if value <= 0 {
        return Err(StaticSettingsTranslationReadError::InconsistentState(format!(
            "{field} must be positive"
        )));
    }
    u64::try_from(value).map_err(|_| {
        StaticSettingsTranslationReadError::InconsistentState(format!(
            "{field} exceeds supported range"
        ))
    })
}

fn integer_value(
    value: u64,
    field: &str,
) -> Result<sea_orm::Value, StaticSettingsTranslationReadError> {
    i64::try_from(value).map(Into::into).map_err(|_| {
        StaticSettingsTranslationReadError::InconsistentState(format!(
            "{field} exceeds storage range"
        ))
    })
}

fn map_owner_error(error: StaticTenantLifecycleStoreError) -> StaticSettingsTranslationReadError {
    match error {
        StaticTenantLifecycleStoreError::OperationInProgress { module_slug } => {
            StaticSettingsTranslationReadError::OwnerOperationInProgress(module_slug)
        }
        StaticTenantLifecycleStoreError::RevisionConflict { .. } => {
            StaticSettingsTranslationReadError::SnapshotUnstable
        }
        StaticTenantLifecycleStoreError::RevisionOverflow(module_slug) => {
            StaticSettingsTranslationReadError::InconsistentState(format!(
                "owner revision overflow for `{module_slug}`"
            ))
        }
        StaticTenantLifecycleStoreError::InconsistentState(module_slug) => {
            StaticSettingsTranslationReadError::InconsistentState(format!(
                "owner aggregate is inconsistent for `{module_slug}`"
            ))
        }
        StaticTenantLifecycleStoreError::Database(error) => {
            StaticSettingsTranslationReadError::Database(error)
        }
    }
}

fn database_error(error: impl std::fmt::Display) -> StaticSettingsTranslationReadError {
    StaticSettingsTranslationReadError::Database(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_counts_only_exact_rows() {
        let snapshot = StaticSettingsExactLocaleSnapshot {
            tenant_id: Uuid::from_u128(1),
            module_slug: "storefront".to_string(),
            source_locale: "en".to_string(),
            target_locale: "de".to_string(),
            owner_revision: 4,
            owner_change_seq: Some(11),
            fields: vec![
                StaticSettingsExactLocaleField {
                    field_id: "storefront.hero.title".to_string(),
                    source_value: "Hello".to_string(),
                    exact_target_value: Some("Hallo".to_string()),
                    target_revision: Some(2),
                    target_owner_revision: Some(3),
                },
                StaticSettingsExactLocaleField {
                    field_id: "storefront.hero.subtitle".to_string(),
                    source_value: "Welcome".to_string(),
                    exact_target_value: None,
                    target_revision: None,
                    target_owner_revision: None,
                },
            ],
        };
        assert_eq!(
            snapshot.progress(),
            StaticSettingsExactLocaleProgress {
                source_units: 2,
                exact_units: 1,
                missing_units: 1,
                complete: false,
                owner_change_seq: Some(11),
            }
        );
    }

    #[test]
    fn exact_locale_requires_canonical_target() {
        assert_eq!(canonical_locale("pt-BR").expect("canonical"), "pt-BR");
        assert!(canonical_locale("pt_br").is_err());
        assert!(canonical_locale("und").is_err());
    }
}
