//! Exact-locale persistence for static module settings localization.
//!
//! Language-neutral settings remain in `tenant_modules.settings`. This owner
//! store persists only fields that the static module explicitly classified as
//! localizable through `ModuleSettingSpec` metadata. Reads are exact-locale
//! only and writes share the static lifecycle aggregate so base settings,
//! enablement, and localized copy cannot race past one another.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use rustok_api::{PortError, TenantLocale};
use rustok_outbox::idempotency::{self, Admission};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::data::configure_tenant_scope;
use crate::{
    ModuleCommandContext, ModuleSettingSpec, ModuleSettingsValidationError,
    StaticTenantLifecycleStore, StaticTenantLifecycleStoreError, is_valid_static_module_slug,
};

const OWNER_SLUG: &str = "modules.static_settings_localization";
const APPLY_OPERATION: &str = "apply_exact";
const MAX_LOCALIZED_VALUE_BYTES: usize = 1_048_576;

#[derive(Clone, Debug)]
pub struct StaticSettingsLocalizationRegistry {
    module_slug: String,
    schema: HashMap<String, ModuleSettingSpec>,
    localized_fields: BTreeMap<String, String>,
    sensitive_paths: BTreeSet<String>,
}

impl StaticSettingsLocalizationRegistry {
    pub fn new(
        module_slug: impl Into<String>,
        schema: HashMap<String, ModuleSettingSpec>,
        localized_fields: BTreeMap<String, String>,
        sensitive_paths: BTreeSet<String>,
    ) -> Result<Self, StaticSettingsLocalizationError> {
        let module_slug = module_slug.into();
        validate_module_slug(&module_slug)?;
        ModuleSettingSpec::validate_localization_registry(
            &module_slug,
            &schema,
            &localized_fields,
            &sensitive_paths,
        )
        .map_err(StaticSettingsLocalizationError::Metadata)?;
        Ok(Self {
            module_slug,
            schema,
            localized_fields,
            sensitive_paths,
        })
    }

    pub fn module_slug(&self) -> &str {
        &self.module_slug
    }

    pub fn localized_fields(&self) -> &BTreeMap<String, String> {
        &self.localized_fields
    }

    fn validate_field(&self, field_id: &str) -> Result<&str, StaticSettingsLocalizationError> {
        self.localized_fields
            .get(field_id)
            .map(String::as_str)
            .ok_or_else(|| StaticSettingsLocalizationError::UnknownField(field_id.to_string()))
    }

    fn validate_value(
        &self,
        field_id: &str,
        value: &str,
    ) -> Result<(), StaticSettingsLocalizationError> {
        if value.len() > MAX_LOCALIZED_VALUE_BYTES {
            return Err(StaticSettingsLocalizationError::InvalidValue {
                field_id: field_id.to_string(),
                reason: format!(
                    "localized value exceeds {MAX_LOCALIZED_VALUE_BYTES} UTF-8 bytes"
                ),
            });
        }
        let path = self.validate_field(field_id)?;
        let spec = resolve_validated_spec(&self.schema, path).ok_or_else(|| {
            StaticSettingsLocalizationError::Metadata(
                ModuleSettingsValidationError::InvalidSchema {
                    module_slug: self.module_slug.clone(),
                    key: path.to_string(),
                    reason: "validated localized field path could not be resolved".to_string(),
                },
            )
        })?;
        let length = value.chars().count() as f64;
        if let Some(min) = spec.min
            && length < min
        {
            return Err(StaticSettingsLocalizationError::InvalidValue {
                field_id: field_id.to_string(),
                reason: format!("length must be >= {min}"),
            });
        }
        if let Some(max) = spec.max
            && length > max
        {
            return Err(StaticSettingsLocalizationError::InvalidValue {
                field_id: field_id.to_string(),
                reason: format!("length must be <= {max}"),
            });
        }
        Ok(())
    }

    fn source_values(
        &self,
        settings: serde_json::Value,
    ) -> Result<BTreeMap<String, String>, StaticSettingsLocalizationError> {
        ModuleSettingSpec::localized_value_snapshot(
            &self.module_slug,
            &self.schema,
            &self.localized_fields,
            &self.sensitive_paths,
            settings,
        )
        .map_err(StaticSettingsLocalizationError::Metadata)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticSettingsLocalizedSourceSnapshot {
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub owner_revision: u64,
    pub values: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticLocalizedSettingRecord {
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub field_id: String,
    pub locale: String,
    pub value: String,
    pub target_revision: u64,
    pub owner_revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticLocalizedSettingApplyCommand {
    pub tenant_id: Uuid,
    pub field_id: String,
    pub locale: String,
    pub value: String,
    pub expected_owner_revision: u64,
    pub expected_target_revision: u64,
    pub context: ModuleCommandContext,
}

#[derive(Serialize)]
struct ApplyReceiptRequest<'a> {
    module_slug: &'a str,
    field_id: &'a str,
    locale: &'a str,
    value: &'a str,
    expected_owner_revision: u64,
    expected_target_revision: u64,
    context: &'a ModuleCommandContext,
}

#[derive(Debug, Error)]
pub enum StaticSettingsLocalizationError {
    #[error("static settings localization requires a non-nil tenant and canonical module identity")]
    InvalidIdentity,
    #[error("static settings localization requires a canonical tenant locale")]
    InvalidLocale,
    #[error("localized settings field `{0}` is not declared by the owner registry")]
    UnknownField(String),
    #[error("localized settings field `{field_id}` is invalid: {reason}")]
    InvalidValue { field_id: String, reason: String },
    #[error(transparent)]
    Metadata(#[from] ModuleSettingsValidationError),
    #[error(
        "localized settings target revision conflict for `{field_id}`/{locale}: expected {expected}, current {current}"
    )]
    TargetRevisionConflict {
        field_id: String,
        locale: String,
        expected: u64,
        current: u64,
    },
    #[error(
        "static settings owner revision conflict for `{module_slug}`: expected {expected}, current {current}"
    )]
    OwnerRevisionConflict {
        module_slug: String,
        expected: u64,
        current: u64,
    },
    #[error("static settings owner operation is already active for `{0}`")]
    OwnerOperationInProgress(String),
    #[error("static settings localized source snapshot changed while it was being read")]
    SourceSnapshotUnstable,
    #[error("static settings localization owner state is inconsistent: {0}")]
    InconsistentState(String),
    #[error("static settings localization persistence failed: {0}")]
    Database(String),
    #[error(transparent)]
    OperationReceipt(PortError),
}

pub struct StaticSettingsLocalizationService {
    db: DatabaseConnection,
}

impl StaticSettingsLocalizationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn source_snapshot(
        &self,
        tenant_id: Uuid,
        registry: &StaticSettingsLocalizationRegistry,
    ) -> Result<StaticSettingsLocalizedSourceSnapshot, StaticSettingsLocalizationError> {
        validate_tenant(tenant_id)?;
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
            return Err(StaticSettingsLocalizationError::OwnerOperationInProgress(
                registry.module_slug().to_string(),
            ));
        }
        let settings = load_base_settings(&transaction, tenant_id, registry.module_slug()).await?;
        let owner_after = StaticTenantLifecycleStore::snapshot(
            &transaction,
            tenant_id,
            registry.module_slug(),
        )
        .await
        .map_err(map_owner_error)?;
        if owner_after.active_idempotency_key.is_some()
            || owner_before.revision != owner_after.revision
        {
            return Err(StaticSettingsLocalizationError::SourceSnapshotUnstable);
        }
        let values = registry.source_values(settings)?;
        transaction.commit().await.map_err(database_error)?;
        Ok(StaticSettingsLocalizedSourceSnapshot {
            tenant_id,
            module_slug: registry.module_slug().to_string(),
            owner_revision: owner_after.revision,
            values,
        })
    }

    pub async fn read_exact(
        &self,
        tenant_id: Uuid,
        registry: &StaticSettingsLocalizationRegistry,
        field_id: &str,
        locale: &str,
    ) -> Result<Option<StaticLocalizedSettingRecord>, StaticSettingsLocalizationError> {
        validate_tenant(tenant_id)?;
        registry.validate_field(field_id)?;
        let locale = canonical_locale(locale)?;
        let transaction = self.db.begin().await.map_err(database_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(|error| database_error(error.to_string()))?;
        let row = load_exact_row(
            &transaction,
            tenant_id,
            registry.module_slug(),
            field_id,
            &locale,
        )
        .await?;
        transaction.commit().await.map_err(database_error)?;
        Ok(row)
    }

    pub async fn apply_exact(
        &self,
        registry: &StaticSettingsLocalizationRegistry,
        command: StaticLocalizedSettingApplyCommand,
    ) -> Result<StaticLocalizedSettingRecord, StaticSettingsLocalizationError> {
        validate_apply_command(registry, &command)?;
        registry.validate_value(&command.field_id, &command.value)?;
        let locale = canonical_locale(&command.locale)?;
        let receipt_request = ApplyReceiptRequest {
            module_slug: registry.module_slug(),
            field_id: &command.field_id,
            locale: &locale,
            value: &command.value,
            expected_owner_revision: command.expected_owner_revision,
            expected_target_revision: command.expected_target_revision,
            context: &command.context,
        };
        let lease = match idempotency::admit(
            &self.db,
            idempotency::OwnerOperationScope::Tenant(command.tenant_id),
            OWNER_SLUG,
            &command.context.idempotency_key.to_string(),
            APPLY_OPERATION,
            &receipt_request,
        )
        .await
        .map_err(StaticSettingsLocalizationError::OperationReceipt)?
        {
            Admission::Replay(value) => {
                return serde_json::from_value(value).map_err(|error| {
                    StaticSettingsLocalizationError::OperationReceipt(
                        PortError::invariant_violation(
                            "modules.static_settings_localization_receipt_corrupt",
                            error.to_string(),
                        ),
                    )
                });
            }
            Admission::ReplayError(error) => {
                return Err(StaticSettingsLocalizationError::OperationReceipt(error));
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
                abandon_claim_and_fail(&self.db, registry.module_slug(), &command, lease, &error)
                    .await?;
                return Err(error);
            }
        };

        let result = async {
            configure_tenant_scope(&transaction, command.tenant_id)
                .await
                .map_err(|error| database_error(error.to_string()))?;
            let current = load_exact_row(
                &transaction,
                command.tenant_id,
                registry.module_slug(),
                &command.field_id,
                &locale,
            )
            .await?;
            let current_target_revision = current.as_ref().map_or(0, |row| row.target_revision);
            if current_target_revision != command.expected_target_revision {
                return Err(StaticSettingsLocalizationError::TargetRevisionConflict {
                    field_id: command.field_id.clone(),
                    locale: locale.clone(),
                    expected: command.expected_target_revision,
                    current: current_target_revision,
                });
            }
            let next_target_revision = command
                .expected_target_revision
                .checked_add(1)
                .ok_or_else(|| {
                    StaticSettingsLocalizationError::InconsistentState(
                        "localized target revision overflow".to_string(),
                    )
                })?;
            let next_owner_revision = command
                .expected_owner_revision
                .checked_add(1)
                .ok_or_else(|| {
                    StaticSettingsLocalizationError::InconsistentState(
                        "static owner revision overflow".to_string(),
                    )
                })?;
            persist_exact_row(
                &transaction,
                command.tenant_id,
                registry.module_slug(),
                &command.field_id,
                &locale,
                &command.value,
                command.expected_target_revision,
                next_target_revision,
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
                return Err(StaticSettingsLocalizationError::InconsistentState(
                    "static owner revision advanced unexpectedly".to_string(),
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
            let record = StaticLocalizedSettingRecord {
                tenant_id: command.tenant_id,
                module_slug: registry.module_slug().to_string(),
                field_id: command.field_id.clone(),
                locale: locale.clone(),
                value: command.value.clone(),
                target_revision: next_target_revision,
                owner_revision: next_owner_revision,
            };
            idempotency::complete(&transaction, lease, &record)
                .await
                .map_err(StaticSettingsLocalizationError::OperationReceipt)?;
            Ok::<_, StaticSettingsLocalizationError>(record)
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

async fn load_base_settings<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    module_slug: &str,
) -> Result<serde_json::Value, StaticSettingsLocalizationError> {
    let backend = connection.get_database_backend();
    connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            match backend {
                DbBackend::Postgres => {
                    "SELECT settings FROM tenant_modules WHERE tenant_id = $1 AND module_slug = $2 LIMIT 1"
                }
                _ => {
                    "SELECT settings FROM tenant_modules WHERE tenant_id = ?1 AND module_slug = ?2 LIMIT 1"
                }
            },
            vec![tenant_id.into(), module_slug.into()],
        ))
        .await
        .map_err(database_error)?
        .map(|row| row.try_get("", "settings").map_err(database_error))
        .transpose()
        .map(|settings| settings.unwrap_or_else(|| serde_json::json!({})))
}

async fn load_exact_row<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    module_slug: &str,
    field_id: &str,
    locale: &str,
) -> Result<Option<StaticLocalizedSettingRecord>, StaticSettingsLocalizationError> {
    let backend = connection.get_database_backend();
    connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            match backend {
                DbBackend::Postgres => {
                    "SELECT value, revision, owner_revision FROM module_static_localized_settings \
                     WHERE tenant_id = $1 AND module_slug = $2 AND field_id = $3 AND locale = $4"
                }
                _ => {
                    "SELECT value, revision, owner_revision FROM module_static_localized_settings \
                     WHERE tenant_id = ?1 AND module_slug = ?2 AND field_id = ?3 AND locale = ?4"
                }
            },
            vec![
                tenant_id.into(),
                module_slug.into(),
                field_id.into(),
                locale.into(),
            ],
        ))
        .await
        .map_err(database_error)?
        .map(|row| {
            let revision = positive_revision(&row, "revision")?;
            let owner_revision = positive_revision(&row, "owner_revision")?;
            Ok(StaticLocalizedSettingRecord {
                tenant_id,
                module_slug: module_slug.to_string(),
                field_id: field_id.to_string(),
                locale: locale.to_string(),
                value: row.try_get("", "value").map_err(database_error)?,
                target_revision: revision,
                owner_revision,
            })
        })
        .transpose()
}

#[allow(clippy::too_many_arguments)]
async fn persist_exact_row<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    module_slug: &str,
    field_id: &str,
    locale: &str,
    value: &str,
    expected_target_revision: u64,
    next_target_revision: u64,
    next_owner_revision: u64,
) -> Result<(), StaticSettingsLocalizationError> {
    let backend = connection.get_database_backend();
    if expected_target_revision == 0 {
        connection
            .execute_raw(Statement::from_sql_and_values(
                backend,
                match backend {
                    DbBackend::Postgres => {
                        "INSERT INTO module_static_localized_settings \
                         (tenant_id, module_slug, field_id, locale, value, revision, owner_revision, created_at, updated_at) \
                         VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())"
                    }
                    _ => {
                        "INSERT INTO module_static_localized_settings \
                         (tenant_id, module_slug, field_id, locale, value, revision, owner_revision, created_at, updated_at) \
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
                    }
                },
                vec![
                    tenant_id.into(),
                    module_slug.into(),
                    field_id.into(),
                    locale.into(),
                    value.into(),
                    revision_value(next_target_revision)?,
                    revision_value(next_owner_revision)?,
                ],
            ))
            .await
            .map_err(database_error)?;
        return Ok(());
    }

    let result = connection
        .execute_raw(Statement::from_sql_and_values(
            backend,
            match backend {
                DbBackend::Postgres => {
                    "UPDATE module_static_localized_settings \
                     SET value = $1, revision = $2, owner_revision = $3, updated_at = NOW() \
                     WHERE tenant_id = $4 AND module_slug = $5 AND field_id = $6 AND locale = $7 AND revision = $8"
                }
                _ => {
                    "UPDATE module_static_localized_settings \
                     SET value = ?1, revision = ?2, owner_revision = ?3, updated_at = CURRENT_TIMESTAMP \
                     WHERE tenant_id = ?4 AND module_slug = ?5 AND field_id = ?6 AND locale = ?7 AND revision = ?8"
                }
            },
            vec![
                value.into(),
                revision_value(next_target_revision)?,
                revision_value(next_owner_revision)?,
                tenant_id.into(),
                module_slug.into(),
                field_id.into(),
                locale.into(),
                revision_value(expected_target_revision)?,
            ],
        ))
        .await
        .map_err(database_error)?;
    if result.rows_affected() != 1 {
        return Err(StaticSettingsLocalizationError::InconsistentState(
            "localized target changed after the owner aggregate was claimed".to_string(),
        ));
    }
    Ok(())
}

async fn abandon_claim_and_fail(
    database: &DatabaseConnection,
    module_slug: &str,
    command: &StaticLocalizedSettingApplyCommand,
    lease: idempotency::Lease,
    error: &StaticSettingsLocalizationError,
) -> Result<(), StaticSettingsLocalizationError> {
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
    error: &StaticSettingsLocalizationError,
) -> Result<(), StaticSettingsLocalizationError> {
    let port_error = match error {
        StaticSettingsLocalizationError::InvalidIdentity
        | StaticSettingsLocalizationError::InvalidLocale
        | StaticSettingsLocalizationError::UnknownField(_)
        | StaticSettingsLocalizationError::InvalidValue { .. }
        | StaticSettingsLocalizationError::Metadata(_) => PortError::validation(
            "modules.static_settings_localization_invalid",
            error.to_string(),
        ),
        StaticSettingsLocalizationError::TargetRevisionConflict { .. }
        | StaticSettingsLocalizationError::OwnerRevisionConflict { .. }
        | StaticSettingsLocalizationError::OwnerOperationInProgress(_)
        | StaticSettingsLocalizationError::SourceSnapshotUnstable => PortError::conflict(
            "modules.static_settings_localization_conflict",
            error.to_string(),
        ),
        _ => PortError::invariant_violation(
            "modules.static_settings_localization_failed",
            error.to_string(),
        ),
    };
    idempotency::fail(database, lease, &port_error)
        .await
        .map_err(StaticSettingsLocalizationError::OperationReceipt)
}

fn validate_apply_command(
    registry: &StaticSettingsLocalizationRegistry,
    command: &StaticLocalizedSettingApplyCommand,
) -> Result<(), StaticSettingsLocalizationError> {
    validate_tenant(command.tenant_id)?;
    if command.context.tenant_id != Some(command.tenant_id) || command.context.validate().is_err() {
        return Err(StaticSettingsLocalizationError::InvalidIdentity);
    }
    registry.validate_field(&command.field_id)?;
    Ok(())
}

fn validate_tenant(tenant_id: Uuid) -> Result<(), StaticSettingsLocalizationError> {
    if tenant_id.is_nil() {
        return Err(StaticSettingsLocalizationError::InvalidIdentity);
    }
    Ok(())
}

fn validate_module_slug(module_slug: &str) -> Result<(), StaticSettingsLocalizationError> {
    if !is_valid_static_module_slug(module_slug) {
        return Err(StaticSettingsLocalizationError::InvalidIdentity);
    }
    Ok(())
}

fn canonical_locale(locale: &str) -> Result<String, StaticSettingsLocalizationError> {
    let canonical =
        TenantLocale::new(locale).map_err(|_| StaticSettingsLocalizationError::InvalidLocale)?;
    if canonical.as_str() != locale {
        return Err(StaticSettingsLocalizationError::InvalidLocale);
    }
    Ok(canonical.into_inner())
}

fn resolve_validated_spec<'a>(
    schema: &'a HashMap<String, ModuleSettingSpec>,
    path: &str,
) -> Option<&'a ModuleSettingSpec> {
    let mut segments = path.split('.');
    let root = segments.next()?;
    let mut spec = schema.get(root)?;
    for segment in segments {
        spec = spec.properties.get(segment)?;
    }
    Some(spec)
}

fn positive_revision(
    row: &sea_orm::QueryResult,
    column: &str,
) -> Result<u64, StaticSettingsLocalizationError> {
    let revision: i64 = row.try_get("", column).map_err(database_error)?;
    if revision <= 0 {
        return Err(StaticSettingsLocalizationError::InconsistentState(format!(
            "{column} must be positive"
        )));
    }
    u64::try_from(revision).map_err(|_| {
        StaticSettingsLocalizationError::InconsistentState(format!(
            "{column} exceeds supported revision range"
        ))
    })
}

fn revision_value(
    revision: u64,
) -> Result<sea_orm::Value, StaticSettingsLocalizationError> {
    i64::try_from(revision)
        .map(Into::into)
        .map_err(|_| {
            StaticSettingsLocalizationError::InconsistentState(
                "revision exceeds storage range".to_string(),
            )
        })
}

fn map_owner_error(error: StaticTenantLifecycleStoreError) -> StaticSettingsLocalizationError {
    match error {
        StaticTenantLifecycleStoreError::RevisionConflict {
            module_slug,
            expected,
            current,
        } => StaticSettingsLocalizationError::OwnerRevisionConflict {
            module_slug,
            expected,
            current,
        },
        StaticTenantLifecycleStoreError::OperationInProgress { module_slug } => {
            StaticSettingsLocalizationError::OwnerOperationInProgress(module_slug)
        }
        StaticTenantLifecycleStoreError::RevisionOverflow(module_slug) => {
            StaticSettingsLocalizationError::InconsistentState(format!(
                "owner revision overflow for `{module_slug}`"
            ))
        }
        StaticTenantLifecycleStoreError::InconsistentState(module_slug) => {
            StaticSettingsLocalizationError::InconsistentState(format!(
                "owner aggregate is inconsistent for `{module_slug}`"
            ))
        }
        StaticTenantLifecycleStoreError::Database(error) => {
            StaticSettingsLocalizationError::Database(error)
        }
    }
}

fn database_error(error: impl std::fmt::Display) -> StaticSettingsLocalizationError {
    StaticSettingsLocalizationError::Database(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> StaticSettingsLocalizationRegistry {
        StaticSettingsLocalizationRegistry::new(
            "storefront",
            HashMap::from([(
                "hero".to_string(),
                ModuleSettingSpec {
                    value_type: "object".to_string(),
                    properties: HashMap::from([(
                        "title".to_string(),
                        ModuleSettingSpec {
                            value_type: "string".to_string(),
                            min: Some(1.0),
                            max: Some(12.0),
                            ..Default::default()
                        },
                    )]),
                    ..Default::default()
                },
            )]),
            BTreeMap::from([(
                "storefront.hero.title".to_string(),
                "hero.title".to_string(),
            )]),
            BTreeSet::new(),
        )
        .expect("registry")
    }

    #[test]
    fn registry_rejects_unknown_fields_and_target_length_drift() {
        let registry = registry();
        assert!(matches!(
            registry.validate_field("storefront.hero.subtitle"),
            Err(StaticSettingsLocalizationError::UnknownField(_))
        ));
        assert!(registry
            .validate_value("storefront.hero.title", "Hello")
            .is_ok());
        assert!(matches!(
            registry.validate_value("storefront.hero.title", "this title is far too long"),
            Err(StaticSettingsLocalizationError::InvalidValue { .. })
        ));
    }

    #[test]
    fn exact_locale_requires_canonical_tenant_locale() {
        assert_eq!(canonical_locale("pt-BR").expect("canonical"), "pt-BR");
        assert!(canonical_locale("pt_br").is_err());
        assert!(canonical_locale("und").is_err());
    }
}
