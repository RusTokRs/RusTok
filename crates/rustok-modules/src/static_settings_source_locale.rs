//! Explicit authoring-locale provenance for static module Settings.
//!
//! A source locale is never inferred from runtime fallback or a tenant default
//! at read time. Instead, an owner command binds one canonical `TenantLocale`
//! to the exact static Settings owner revision. Any later Settings or localized
//! owner mutation advances that shared revision and therefore makes the prior
//! source-locale assignment stale until it is explicitly reaffirmed.

use rustok_api::{PortError, TenantLocale};
use rustok_outbox::idempotency::{self, Admission};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::data::configure_tenant_scope;
use crate::operation_store::{StaticTenantLifecycleStore, StaticTenantLifecycleStoreError};
use crate::static_settings_localization::{
    StaticSettingsLocalizedSourceSnapshot, StaticSettingsLocalizationError,
    StaticSettingsLocalizationRegistry, StaticSettingsLocalizationService,
};
use crate::ModuleCommandContext;

const OWNER_SLUG: &str = "modules.static_settings_source_locale";
const ASSIGN_OPERATION: &str = "assign_source_locale";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticSettingsSourceLocaleRecord {
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub locale: String,
    pub owner_revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticSettingsSourceLocaleAssignCommand {
    pub tenant_id: Uuid,
    pub locale: String,
    pub expected_owner_revision: u64,
    pub context: ModuleCommandContext,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticSettingsAuthoritativeSourceSnapshot {
    pub source_locale: String,
    pub source: StaticSettingsLocalizedSourceSnapshot,
}

#[derive(Serialize)]
struct AssignReceiptRequest<'a> {
    module_slug: &'a str,
    locale: &'a str,
    expected_owner_revision: u64,
    context: &'a ModuleCommandContext,
}

#[derive(Debug, Error)]
pub enum StaticSettingsSourceLocaleError {
    #[error("static Settings source-locale ownership requires a valid tenant command identity")]
    InvalidIdentity,
    #[error("static Settings source locale must be an exact canonical tenant locale")]
    InvalidLocale,
    #[error("static Settings source locale is not assigned for `{0}`")]
    SourceLocaleUnassigned(String),
    #[error(
        "static Settings source locale for `{module_slug}` belongs to owner revision {recorded}, current revision is {current}"
    )]
    SourceLocaleStale {
        module_slug: String,
        recorded: u64,
        current: u64,
    },
    #[error("static Settings source snapshot changed while locale provenance was being verified")]
    SourceSnapshotUnstable,
    #[error(
        "static Settings owner revision conflict for `{module_slug}`: expected {expected}, current {current}"
    )]
    OwnerRevisionConflict {
        module_slug: String,
        expected: u64,
        current: u64,
    },
    #[error("static Settings owner operation is already active for `{0}`")]
    OwnerOperationInProgress(String),
    #[error("static Settings source-locale owner state is inconsistent: {0}")]
    InconsistentState(String),
    #[error("static Settings source-locale persistence failed: {0}")]
    Database(String),
    #[error(transparent)]
    Localization(#[from] StaticSettingsLocalizationError),
    #[error(transparent)]
    OperationReceipt(PortError),
}

#[derive(Clone)]
pub struct StaticSettingsSourceLocaleService {
    db: DatabaseConnection,
}

impl StaticSettingsSourceLocaleService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Returns the persisted provenance row without pretending that a stale
    /// assignment is authoritative for the current owner revision.
    pub async fn read_source_locale(
        &self,
        tenant_id: Uuid,
        registry: &StaticSettingsLocalizationRegistry,
    ) -> Result<Option<StaticSettingsSourceLocaleRecord>, StaticSettingsSourceLocaleError> {
        validate_tenant(tenant_id)?;
        let transaction = self.db.begin().await.map_err(database_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(|error| database_error(error.to_string()))?;
        let record = load_source_locale(
            &transaction,
            tenant_id,
            registry.module_slug(),
        )
        .await?;
        if let Some(record) = &record {
            ensure_stored_locale_is_canonical(&record.locale)?;
        }
        transaction.commit().await.map_err(database_error)?;
        Ok(record)
    }

    /// Returns localizable Settings copy only when one explicit source locale
    /// is bound to the same stable owner revision as the source snapshot.
    /// Legacy rows and Settings changed after the assignment fail closed.
    pub async fn authoritative_source_snapshot(
        &self,
        tenant_id: Uuid,
        registry: &StaticSettingsLocalizationRegistry,
    ) -> Result<StaticSettingsAuthoritativeSourceSnapshot, StaticSettingsSourceLocaleError> {
        validate_tenant(tenant_id)?;
        let source = StaticSettingsLocalizationService::new(self.db.clone())
            .source_snapshot(tenant_id, registry)
            .await
            .map_err(StaticSettingsSourceLocaleError::Localization)?;

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
            return Err(StaticSettingsSourceLocaleError::OwnerOperationInProgress(
                registry.module_slug().to_string(),
            ));
        }
        if owner_before.revision != source.owner_revision {
            return Err(StaticSettingsSourceLocaleError::SourceSnapshotUnstable);
        }

        let record = load_source_locale(
            &transaction,
            tenant_id,
            registry.module_slug(),
        )
        .await?
        .ok_or_else(|| {
            StaticSettingsSourceLocaleError::SourceLocaleUnassigned(
                registry.module_slug().to_string(),
            )
        })?;
        ensure_stored_locale_is_canonical(&record.locale)?;
        if record.owner_revision != source.owner_revision {
            return Err(StaticSettingsSourceLocaleError::SourceLocaleStale {
                module_slug: registry.module_slug().to_string(),
                recorded: record.owner_revision,
                current: source.owner_revision,
            });
        }

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
            return Err(StaticSettingsSourceLocaleError::SourceSnapshotUnstable);
        }
        transaction.commit().await.map_err(database_error)?;

        Ok(StaticSettingsAuthoritativeSourceSnapshot {
            source_locale: record.locale,
            source,
        })
    }

    /// Explicitly binds one canonical source locale to the next shared static
    /// Settings owner revision. The command is CAS-guarded and replay-safe.
    pub async fn assign_source_locale(
        &self,
        registry: &StaticSettingsLocalizationRegistry,
        command: StaticSettingsSourceLocaleAssignCommand,
    ) -> Result<StaticSettingsSourceLocaleRecord, StaticSettingsSourceLocaleError> {
        validate_assign_command(&command)?;
        let locale = canonical_locale(&command.locale)?;
        let receipt_request = AssignReceiptRequest {
            module_slug: registry.module_slug(),
            locale: &locale,
            expected_owner_revision: command.expected_owner_revision,
            context: &command.context,
        };
        let lease = match idempotency::admit(
            &self.db,
            idempotency::OwnerOperationScope::Tenant(command.tenant_id),
            OWNER_SLUG,
            &command.context.idempotency_key.to_string(),
            ASSIGN_OPERATION,
            &receipt_request,
        )
        .await
        .map_err(StaticSettingsSourceLocaleError::OperationReceipt)?
        {
            Admission::Replay(value) => {
                return serde_json::from_value(value).map_err(|error| {
                    StaticSettingsSourceLocaleError::OperationReceipt(
                        PortError::invariant_violation(
                            "modules.static_settings_source_locale_receipt_corrupt",
                            error.to_string(),
                        ),
                    )
                });
            }
            Admission::ReplayError(error) => {
                return Err(StaticSettingsSourceLocaleError::OperationReceipt(error));
            }
            Admission::Run(lease) => lease,
        };

        if let Err(error) = StaticTenantLifecycleStore::claim(
            &self.db,
            command.tenant_id,
            registry.module_slug(),
            command.expected_owner_revision,
            command.context.idempotency_key,
        )
        .await
        .map_err(map_owner_error)
        {
            fail_receipt(&self.db, lease, &error).await?;
            return Err(error);
        }

        let transaction = match self.db.begin().await {
            Ok(transaction) => transaction,
            Err(error) => {
                let error = database_error(error);
                abandon_claim_and_fail(
                    &self.db,
                    registry.module_slug(),
                    &command,
                    lease,
                    &error,
                )
                .await?;
                return Err(error);
            }
        };

        let result = async {
            configure_tenant_scope(&transaction, command.tenant_id)
                .await
                .map_err(|error| database_error(error.to_string()))?;
            let next_owner_revision = command
                .expected_owner_revision
                .checked_add(1)
                .ok_or_else(|| {
                    StaticSettingsSourceLocaleError::InconsistentState(
                        "static Settings owner revision overflow".to_string(),
                    )
                })?;

            persist_source_locale(
                &transaction,
                command.tenant_id,
                registry.module_slug(),
                &locale,
                next_owner_revision,
            )
            .await?;
            persist_source_projection_change(
                &transaction,
                command.tenant_id,
                registry.module_slug(),
                next_owner_revision,
            )
            .await?;

            let advanced_owner_revision = StaticTenantLifecycleStore::advance(
                &transaction,
                command.tenant_id,
                registry.module_slug(),
                command.expected_owner_revision,
                command.context.idempotency_key,
            )
            .await
            .map_err(map_owner_error)?;
            if advanced_owner_revision != next_owner_revision {
                return Err(StaticSettingsSourceLocaleError::InconsistentState(
                    "static Settings owner revision advanced unexpectedly".to_string(),
                ));
            }
            StaticTenantLifecycleStore::release(
                &transaction,
                command.tenant_id,
                registry.module_slug(),
                command.context.idempotency_key,
            )
            .await
            .map_err(map_owner_error)?;

            let record = StaticSettingsSourceLocaleRecord {
                tenant_id: command.tenant_id,
                module_slug: registry.module_slug().to_string(),
                locale: locale.clone(),
                owner_revision: next_owner_revision,
            };
            idempotency::complete(&transaction, lease, &record)
                .await
                .map_err(StaticSettingsSourceLocaleError::OperationReceipt)?;
            Ok::<_, StaticSettingsSourceLocaleError>(record)
        }
        .await;

        match result {
            Ok(record) => {
                transaction.commit().await.map_err(database_error)?;
                Ok(record)
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                abandon_claim_and_fail(
                    &self.db,
                    registry.module_slug(),
                    &command,
                    lease,
                    &error,
                )
                .await?;
                Err(error)
            }
        }
    }
}

async fn load_source_locale<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    module_slug: &str,
) -> Result<Option<StaticSettingsSourceLocaleRecord>, StaticSettingsSourceLocaleError> {
    let backend = connection.get_database_backend();
    connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            match backend {
                DbBackend::Postgres => {
                    "SELECT locale, owner_revision FROM module_static_settings_source_locales \
                     WHERE tenant_id = $1 AND module_slug = $2 LIMIT 1"
                }
                _ => {
                    "SELECT locale, owner_revision FROM module_static_settings_source_locales \
                     WHERE tenant_id = ?1 AND module_slug = ?2 LIMIT 1"
                }
            },
            vec![tenant_id.into(), module_slug.into()],
        ))
        .await
        .map_err(database_error)?
        .map(|row| {
            Ok(StaticSettingsSourceLocaleRecord {
                tenant_id,
                module_slug: module_slug.to_string(),
                locale: row.try_get("", "locale").map_err(database_error)?,
                owner_revision: positive_revision(&row, "owner_revision")?,
            })
        })
        .transpose()
}

async fn persist_source_locale<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    module_slug: &str,
    locale: &str,
    owner_revision: u64,
) -> Result<(), StaticSettingsSourceLocaleError> {
    let backend = connection.get_database_backend();
    connection
        .execute_raw(Statement::from_sql_and_values(
            backend,
            match backend {
                DbBackend::Postgres => {
                    "INSERT INTO module_static_settings_source_locales \
                     (tenant_id, module_slug, locale, owner_revision, created_at, updated_at) \
                     VALUES ($1, $2, $3, $4, NOW(), NOW()) \
                     ON CONFLICT (tenant_id, module_slug) DO UPDATE SET \
                     locale = EXCLUDED.locale, owner_revision = EXCLUDED.owner_revision, updated_at = NOW()"
                }
                _ => {
                    "INSERT INTO module_static_settings_source_locales \
                     (tenant_id, module_slug, locale, owner_revision, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP) \
                     ON CONFLICT (tenant_id, module_slug) DO UPDATE SET \
                     locale = excluded.locale, owner_revision = excluded.owner_revision, updated_at = CURRENT_TIMESTAMP"
                }
            },
            vec![
                tenant_id.into(),
                module_slug.into(),
                locale.into(),
                revision_value(owner_revision)?,
            ],
        ))
        .await
        .map_err(database_error)?;
    Ok(())
}

async fn persist_source_projection_change<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    module_slug: &str,
    owner_revision: u64,
) -> Result<(), StaticSettingsSourceLocaleError> {
    let backend = connection.get_database_backend();
    connection
        .execute_raw(Statement::from_sql_and_values(
            backend,
            match backend {
                DbBackend::Postgres => {
                    "INSERT INTO module_static_settings_changes \
                     (tenant_id, module_slug, change_kind, field_id, locale, owner_revision, target_revision, created_at) \
                     VALUES ($1, $2, 'base_projection', NULL, NULL, $3, NULL, NOW())"
                }
                _ => {
                    "INSERT INTO module_static_settings_changes \
                     (tenant_id, module_slug, change_kind, field_id, locale, owner_revision, target_revision, created_at) \
                     VALUES (?1, ?2, 'base_projection', NULL, NULL, ?3, NULL, CURRENT_TIMESTAMP)"
                }
            },
            vec![
                tenant_id.into(),
                module_slug.into(),
                revision_value(owner_revision)?,
            ],
        ))
        .await
        .map_err(database_error)?;
    Ok(())
}

async fn abandon_claim_and_fail(
    database: &DatabaseConnection,
    module_slug: &str,
    command: &StaticSettingsSourceLocaleAssignCommand,
    lease: idempotency::Lease,
    error: &StaticSettingsSourceLocaleError,
) -> Result<(), StaticSettingsSourceLocaleError> {
    let release = StaticTenantLifecycleStore::release(
        database,
        command.tenant_id,
        module_slug,
        command.context.idempotency_key,
    )
    .await
    .map_err(map_owner_error);
    let receipt = fail_receipt(database, lease, error).await;
    release?;
    receipt
}

async fn fail_receipt(
    database: &DatabaseConnection,
    lease: idempotency::Lease,
    error: &StaticSettingsSourceLocaleError,
) -> Result<(), StaticSettingsSourceLocaleError> {
    let port_error = match error {
        StaticSettingsSourceLocaleError::InvalidIdentity
        | StaticSettingsSourceLocaleError::InvalidLocale => PortError::validation(
            "modules.static_settings_source_locale_invalid",
            error.to_string(),
        ),
        StaticSettingsSourceLocaleError::SourceLocaleUnassigned(_)
        | StaticSettingsSourceLocaleError::SourceLocaleStale { .. }
        | StaticSettingsSourceLocaleError::SourceSnapshotUnstable
        | StaticSettingsSourceLocaleError::OwnerRevisionConflict { .. }
        | StaticSettingsSourceLocaleError::OwnerOperationInProgress(_) => PortError::conflict(
            "modules.static_settings_source_locale_conflict",
            error.to_string(),
        ),
        _ => PortError::invariant_violation(
            "modules.static_settings_source_locale_failed",
            error.to_string(),
        ),
    };
    idempotency::fail(database, lease, &port_error)
        .await
        .map_err(StaticSettingsSourceLocaleError::OperationReceipt)
}

fn validate_assign_command(
    command: &StaticSettingsSourceLocaleAssignCommand,
) -> Result<(), StaticSettingsSourceLocaleError> {
    validate_tenant(command.tenant_id)?;
    if command.context.tenant_id != Some(command.tenant_id) || command.context.validate().is_err() {
        return Err(StaticSettingsSourceLocaleError::InvalidIdentity);
    }
    Ok(())
}

fn validate_tenant(tenant_id: Uuid) -> Result<(), StaticSettingsSourceLocaleError> {
    if tenant_id.is_nil() {
        return Err(StaticSettingsSourceLocaleError::InvalidIdentity);
    }
    Ok(())
}

fn canonical_locale(locale: &str) -> Result<String, StaticSettingsSourceLocaleError> {
    let canonical =
        TenantLocale::new(locale).map_err(|_| StaticSettingsSourceLocaleError::InvalidLocale)?;
    if canonical.as_str() != locale {
        return Err(StaticSettingsSourceLocaleError::InvalidLocale);
    }
    Ok(canonical.into_inner())
}

fn ensure_stored_locale_is_canonical(locale: &str) -> Result<(), StaticSettingsSourceLocaleError> {
    canonical_locale(locale).map(|_| ()).map_err(|_| {
        StaticSettingsSourceLocaleError::InconsistentState(
            "stored source locale is not canonical TenantLocale data".to_string(),
        )
    })
}

fn positive_revision(
    row: &sea_orm::QueryResult,
    column: &str,
) -> Result<u64, StaticSettingsSourceLocaleError> {
    let revision: i64 = row.try_get("", column).map_err(database_error)?;
    if revision <= 0 {
        return Err(StaticSettingsSourceLocaleError::InconsistentState(format!(
            "{column} must be positive"
        )));
    }
    u64::try_from(revision).map_err(|_| {
        StaticSettingsSourceLocaleError::InconsistentState(format!(
            "{column} exceeds supported revision range"
        ))
    })
}

fn revision_value(revision: u64) -> Result<sea_orm::Value, StaticSettingsSourceLocaleError> {
    i64::try_from(revision)
        .map(Into::into)
        .map_err(|_| {
            StaticSettingsSourceLocaleError::InconsistentState(
                "revision exceeds storage range".to_string(),
            )
        })
}

fn map_owner_error(error: StaticTenantLifecycleStoreError) -> StaticSettingsSourceLocaleError {
    match error {
        StaticTenantLifecycleStoreError::RevisionConflict {
            module_slug,
            expected,
            current,
        } => StaticSettingsSourceLocaleError::OwnerRevisionConflict {
            module_slug,
            expected,
            current,
        },
        StaticTenantLifecycleStoreError::OperationInProgress { module_slug } => {
            StaticSettingsSourceLocaleError::OwnerOperationInProgress(module_slug)
        }
        StaticTenantLifecycleStoreError::RevisionOverflow(module_slug) => {
            StaticSettingsSourceLocaleError::InconsistentState(format!(
                "owner revision overflow for `{module_slug}`"
            ))
        }
        StaticTenantLifecycleStoreError::InconsistentState(module_slug) => {
            StaticSettingsSourceLocaleError::InconsistentState(format!(
                "owner aggregate is inconsistent for `{module_slug}`"
            ))
        }
        StaticTenantLifecycleStoreError::Database(error) => {
            StaticSettingsSourceLocaleError::Database(error)
        }
    }
}

fn database_error(error: impl std::fmt::Display) -> StaticSettingsSourceLocaleError {
    StaticSettingsSourceLocaleError::Database(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_locale_requires_exact_canonical_tenant_locale() {
        assert_eq!(canonical_locale("en").expect("canonical"), "en");
        assert_eq!(canonical_locale("pt-BR").expect("canonical"), "pt-BR");
        assert!(canonical_locale("pt_br").is_err());
        assert!(canonical_locale("und").is_err());
    }
}
