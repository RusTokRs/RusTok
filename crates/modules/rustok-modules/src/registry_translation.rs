//! Translation-safe owner operations for mutable registry publish-request copy.
//!
//! The registry has two localization planes with different ownership rules:
//! mutable publish-request copy and immutable release snapshots. This service
//! owns only the former. Published release rows and release changelog/provenance
//! are deliberately unreachable from this API.

use rustok_api::PortError;
use rustok_outbox::idempotency;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{ModuleCommandContext, ModuleMarketplaceContentProjection};

pub const MAX_REGISTRY_TRANSLATION_RESOURCE_PAGE: u16 = 200;
pub const MAX_REGISTRY_TRANSLATION_REQUEST_ID_BYTES: usize = 128;
pub const REGISTRY_TRANSLATION_OWNER_SLUG: &str = "modules";
pub const REGISTRY_TRANSLATION_APPLY_OPERATION: &str =
    "registry_publish_request_translation_apply";

const RESOURCE_REVISION_DOMAIN: &[u8] =
    b"rustok.modules.registry.publish-request.translation.resource.v1\0";
const LOCALE_REVISION_DOMAIN: &[u8] =
    b"rustok.modules.registry.publish-request.translation.locale.v1\0";

#[derive(Debug, Error)]
pub enum RegistryPublishRequestTranslationError {
    #[error("registry publish request `{0}` was not found")]
    RequestNotFound(String),
    #[error("registry publish request source locale `{locale}` was not found for `{request_id}`")]
    SourceLocaleNotFound { request_id: String, locale: String },
    #[error("registry publish request target locale `{locale}` was missing after apply for `{request_id}`")]
    TargetLocaleMissingAfterApply { request_id: String, locale: String },
    #[error("registry publish request translation is not editable in status `{0}`")]
    LifecycleNotEditable(String),
    #[error("registry publish request translation {revision} revision conflict")]
    RevisionConflict { revision: &'static str },
    #[error("registry publish request translation validation failed: {0}")]
    Validation(String),
    #[error("registry publish request translation operation receipt failed: {0}")]
    OperationReceipt(PortError),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

pub type RegistryPublishRequestTranslationResult<T> =
    Result<T, RegistryPublishRequestTranslationError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryPublishRequestTranslationRecord {
    pub locale: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryPublishRequestTranslationSnapshot {
    pub request_id: String,
    pub status: String,
    pub governance_revision: i64,
    pub source_locale: String,
    pub target_locale: String,
    pub resource_revision: String,
    pub source_revision: String,
    pub target_revision: Option<String>,
    pub exact_locales: Vec<String>,
    pub source: RegistryPublishRequestTranslationRecord,
    pub target: Option<RegistryPublishRequestTranslationRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryPublishRequestTranslationApply {
    pub source_locale: String,
    pub target_locale: String,
    pub name: String,
    pub description: String,
    pub expected_resource_revision: String,
    pub expected_source_revision: String,
    pub expected_target_revision: Option<String>,
    pub context: ModuleCommandContext,
    /// Authenticated host fact. Registry publish requests are platform-global;
    /// tenant module managers are never accepted by this owner path.
    pub actor_can_manage_modules: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryPublishRequestTranslationApplyReceipt {
    pub operation_id: uuid::Uuid,
    pub request_id: String,
    pub governance_revision: i64,
    pub resource_revision: String,
    pub target_revision: String,
    pub target: RegistryPublishRequestTranslationRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegistryPublishRequestTranslationProgressFacts {
    pub resources: u64,
    pub required_units: u64,
    pub exact_required_units: u64,
    pub complete_resources: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryPublishRequestTranslationChangeRecord {
    pub change_seq: u64,
    pub request_id: String,
    pub change_kind: String,
    pub locale: Option<String>,
    pub lifecycle: Option<String>,
}

#[derive(Clone)]
pub struct RegistryPublishRequestTranslationService {
    db: DatabaseConnection,
}

impl RegistryPublishRequestTranslationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub fn database(&self) -> &DatabaseConnection {
        &self.db
    }

    pub async fn list_exact_resources(
        &self,
        source_locale: &str,
        target_locale: &str,
        after: Option<&str>,
        limit: u16,
    ) -> RegistryPublishRequestTranslationResult<(Vec<RegistryPublishRequestTranslationSnapshot>, Option<String>)> {
        let source_locale = canonical_locale(source_locale)?;
        let target_locale = canonical_locale(target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;
        validate_page_limit(limit)?;
        let after = after.map(validate_request_id).transpose()?;
        let backend = self.db.get_database_backend();
        let mut values = vec![source_locale.clone().into()];
        let after_clause = if let Some(after) = after.as_ref() {
            values.push(after.clone().into());
            format!(" AND request.id > {}", placeholder(backend, 2))
        } else {
            String::new()
        };
        let limit_position = values.len() + 1;
        values.push((i64::from(limit) + 1).into());
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT request.id \
                     FROM registry_publish_requests AS request \
                     WHERE request.status IN ('draft', 'changes_requested') \
                       AND EXISTS ( \
                           SELECT 1 FROM registry_publish_request_translations AS source_translation \
                           WHERE source_translation.request_id = request.id \
                             AND source_translation.locale = {} \
                       ){after_clause} \
                     ORDER BY request.id ASC LIMIT {}",
                    placeholder(backend, 1),
                    placeholder(backend, limit_position),
                ),
                values,
            ))
            .await?;
        let mut ids = rows
            .into_iter()
            .map(|row| row.try_get::<String>("", "id"))
            .collect::<Result<Vec<_>, _>>()?;
        let has_more = ids.len() > usize::from(limit);
        if has_more {
            ids.truncate(usize::from(limit));
        }
        let next_after = has_more.then(|| ids.last().cloned()).flatten();

        let mut snapshots = Vec::with_capacity(ids.len());
        for request_id in ids {
            match self
                .read_exact_locale(&request_id, &source_locale, &target_locale)
                .await
            {
                Ok(snapshot) if is_editable_status(&snapshot.status) => snapshots.push(snapshot),
                Ok(_) | Err(RegistryPublishRequestTranslationError::SourceLocaleNotFound { .. }) => {}
                Err(error) => return Err(error),
            }
        }
        Ok((snapshots, next_after))
    }

    pub async fn read_exact_locale(
        &self,
        request_id: &str,
        source_locale: &str,
        target_locale: &str,
    ) -> RegistryPublishRequestTranslationResult<RegistryPublishRequestTranslationSnapshot> {
        let request_id = validate_request_id(request_id)?;
        let source_locale = canonical_locale(source_locale)?;
        let target_locale = canonical_locale(target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;
        let owner = load_request(&self.db, &request_id, false).await?;
        let translations = load_translations(&self.db, &request_id).await?;
        build_snapshot(
            request_id,
            owner,
            translations,
            source_locale,
            target_locale,
        )
    }

    pub async fn apply_exact_locale_with_operation(
        &self,
        request_id: &str,
        request: RegistryPublishRequestTranslationApply,
        operation_lease: idempotency::Lease,
    ) -> RegistryPublishRequestTranslationResult<RegistryPublishRequestTranslationApplyReceipt> {
        let request_id = validate_request_id(request_id)?;
        validate_platform_context(&request.context, request.actor_can_manage_modules)?;
        let source_locale = canonical_locale(&request.source_locale)?;
        let target_locale = canonical_locale(&request.target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;
        let content = ModuleMarketplaceContentProjection::try_new(
            &request.name,
            &request.description,
        )
        .map_err(|error| RegistryPublishRequestTranslationError::Validation(error.to_string()))?;
        if content.description.chars().count() < 20 {
            return Err(RegistryPublishRequestTranslationError::Validation(
                "registry marketplace translated description must contain at least 20 characters"
                    .to_string(),
            ));
        }

        let transaction = self.db.begin().await?;
        let owner = load_request(&transaction, &request_id, true).await?;
        if !is_editable_status(&owner.status) {
            return Err(RegistryPublishRequestTranslationError::LifecycleNotEditable(
                owner.status,
            ));
        }
        let translations = load_translations(&transaction, &request_id).await?;
        let source = exact_locale_row(&translations, &source_locale).ok_or_else(|| {
            RegistryPublishRequestTranslationError::SourceLocaleNotFound {
                request_id: request_id.clone(),
                locale: source_locale.clone(),
            }
        })?;
        let target = exact_locale_row(&translations, &target_locale);
        ensure_revision(
            "resource",
            &request.expected_resource_revision,
            &resource_revision(&request_id, &owner.status, &translations),
        )?;
        ensure_revision(
            "source",
            &request.expected_source_revision,
            &locale_revision(&request_id, source),
        )?;
        let current_target_revision = target.map(|row| locale_revision(&request_id, row));
        if request.expected_target_revision != current_target_revision {
            return Err(RegistryPublishRequestTranslationError::RevisionConflict {
                revision: "target",
            });
        }

        let unchanged = target.is_some_and(|current| {
            current.name == content.name && current.description == content.description
        });
        let mut governance_revision = owner.governance_revision;
        if !unchanged {
            upsert_translation(
                &transaction,
                &request_id,
                &target_locale,
                &content.name,
                &content.description,
            )
            .await?;
            governance_revision = advance_parent_revision(
                &transaction,
                &request_id,
                &owner.status,
                owner.governance_revision,
            )
            .await?;
        }

        let translations_after = load_translations(&transaction, &request_id).await?;
        let target_after = exact_locale_row(&translations_after, &target_locale)
            .cloned()
            .ok_or_else(|| {
                RegistryPublishRequestTranslationError::TargetLocaleMissingAfterApply {
                    request_id: request_id.clone(),
                    locale: target_locale,
                }
            })?;
        let receipt = RegistryPublishRequestTranslationApplyReceipt {
            operation_id: operation_lease.operation_id,
            request_id: request_id.clone(),
            governance_revision,
            resource_revision: resource_revision(&request_id, &owner.status, &translations_after),
            target_revision: locale_revision(&request_id, &target_after),
            target: target_after,
        };
        idempotency::complete(&transaction, operation_lease, &receipt)
            .await
            .map_err(RegistryPublishRequestTranslationError::OperationReceipt)?;
        transaction.commit().await?;
        Ok(receipt)
    }

    pub async fn read_exact_progress(
        &self,
        source_locale: &str,
        target_locale: &str,
    ) -> RegistryPublishRequestTranslationResult<RegistryPublishRequestTranslationProgressFacts> {
        let source_locale = canonical_locale(source_locale)?;
        let target_locale = canonical_locale(target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;
        let backend = self.db.get_database_backend();
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT \
                         COUNT(*) AS resources, \
                         COUNT(CASE WHEN TRIM(COALESCE(target_translation.name, '')) <> '' THEN 1 END) \
                           + COUNT(CASE WHEN TRIM(COALESCE(target_translation.description, '')) <> '' THEN 1 END) \
                           AS exact_required_units, \
                         COUNT(CASE \
                           WHEN TRIM(COALESCE(target_translation.name, '')) <> '' \
                            AND TRIM(COALESCE(target_translation.description, '')) <> '' \
                           THEN 1 END) AS complete_resources \
                     FROM registry_publish_requests AS request \
                     INNER JOIN registry_publish_request_translations AS source_translation \
                       ON source_translation.request_id = request.id \
                      AND source_translation.locale = {} \
                     LEFT JOIN registry_publish_request_translations AS target_translation \
                       ON target_translation.request_id = request.id \
                      AND target_translation.locale = {} \
                     WHERE request.status IN ('draft', 'changes_requested')",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                ),
                vec![source_locale.into(), target_locale.into()],
            ))
            .await?
            .ok_or_else(|| {
                RegistryPublishRequestTranslationError::Validation(
                    "registry Translation progress aggregate returned no row".to_string(),
                )
            })?;
        let resources = non_negative_count(row.try_get("", "resources")?, "resources")?;
        let required_units = resources.checked_mul(2).ok_or_else(|| {
            RegistryPublishRequestTranslationError::Validation(
                "registry Translation required-unit count overflowed".to_string(),
            )
        })?;
        let exact_required_units = non_negative_count(
            row.try_get("", "exact_required_units")?,
            "exact required units",
        )?;
        let complete_resources = non_negative_count(
            row.try_get("", "complete_resources")?,
            "complete resources",
        )?;
        if exact_required_units > required_units || complete_resources > resources {
            return Err(RegistryPublishRequestTranslationError::Validation(
                "registry Translation progress aggregate is inconsistent".to_string(),
            ));
        }
        Ok(RegistryPublishRequestTranslationProgressFacts {
            resources,
            required_units,
            exact_required_units,
            complete_resources,
        })
    }

    pub async fn translation_change_highwater(
        &self,
    ) -> RegistryPublishRequestTranslationResult<Option<u64>> {
        let backend = self.db.get_database_backend();
        let row = self
            .db
            .query_one_raw(Statement::from_string(
                backend,
                "SELECT MAX(change_seq) AS highwater FROM registry_publish_request_translation_changes"
                    .to_string(),
            ))
            .await?
            .ok_or_else(|| {
                RegistryPublishRequestTranslationError::Validation(
                    "registry Translation highwater aggregate returned no row".to_string(),
                )
            })?;
        row.try_get::<Option<i64>>("", "highwater")?
            .map(|value| non_negative_count(value, "change highwater"))
            .transpose()
    }

    pub async fn read_translation_changes(
        &self,
        after: u64,
        through: u64,
        limit: u16,
    ) -> RegistryPublishRequestTranslationResult<Vec<RegistryPublishRequestTranslationChangeRecord>> {
        validate_page_limit(limit)?;
        if through < after {
            return Err(RegistryPublishRequestTranslationError::Validation(
                "registry Translation frozen highwater must not precede the cursor".to_string(),
            ));
        }
        let after = i64::try_from(after).map_err(|_| {
            RegistryPublishRequestTranslationError::Validation(
                "registry Translation change cursor is too large".to_string(),
            )
        })?;
        let through = i64::try_from(through).map_err(|_| {
            RegistryPublishRequestTranslationError::Validation(
                "registry Translation frozen highwater is too large".to_string(),
            )
        })?;
        let backend = self.db.get_database_backend();
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT change_seq, request_id, change_kind, locale, lifecycle \
                     FROM registry_publish_request_translation_changes \
                     WHERE change_seq > {} AND change_seq <= {} \
                     ORDER BY change_seq ASC LIMIT {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                vec![after.into(), through.into(), i64::from(limit).into()],
            ))
            .await?;
        rows.into_iter()
            .map(|row| {
                Ok(RegistryPublishRequestTranslationChangeRecord {
                    change_seq: non_negative_count(
                        row.try_get("", "change_seq")?,
                        "change sequence",
                    )?,
                    request_id: row.try_get("", "request_id")?,
                    change_kind: row.try_get("", "change_kind")?,
                    locale: row.try_get("", "locale")?,
                    lifecycle: row.try_get("", "lifecycle")?,
                })
            })
            .collect()
    }
}

#[derive(Debug)]
struct RegistryPublishRequestOwnerRow {
    status: String,
    governance_revision: i64,
}

async fn load_request<C>(
    database: &C,
    request_id: &str,
    lock: bool,
) -> RegistryPublishRequestTranslationResult<RegistryPublishRequestOwnerRow>
where
    C: ConnectionTrait,
{
    let backend = database.get_database_backend();
    let lock_clause = if lock && backend == DbBackend::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    let row = database
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT status, revision FROM registry_publish_requests WHERE id = {}{lock_clause}",
                placeholder(backend, 1)
            ),
            vec![request_id.to_string().into()],
        ))
        .await?
        .ok_or_else(|| RegistryPublishRequestTranslationError::RequestNotFound(request_id.into()))?;
    Ok(RegistryPublishRequestOwnerRow {
        status: row.try_get("", "status")?,
        governance_revision: row.try_get("", "revision")?,
    })
}

async fn load_translations<C>(
    database: &C,
    request_id: &str,
) -> RegistryPublishRequestTranslationResult<Vec<RegistryPublishRequestTranslationRecord>>
where
    C: ConnectionTrait,
{
    let backend = database.get_database_backend();
    database
        .query_all_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT locale, name, description FROM registry_publish_request_translations \
                 WHERE request_id = {} ORDER BY locale ASC",
                placeholder(backend, 1)
            ),
            vec![request_id.to_string().into()],
        ))
        .await?
        .into_iter()
        .map(translation_from_row)
        .collect()
}

fn translation_from_row(
    row: QueryResult,
) -> RegistryPublishRequestTranslationResult<RegistryPublishRequestTranslationRecord> {
    Ok(RegistryPublishRequestTranslationRecord {
        locale: row.try_get("", "locale")?,
        name: row.try_get("", "name")?,
        description: row.try_get("", "description")?,
    })
}

async fn upsert_translation<C>(
    database: &C,
    request_id: &str,
    locale: &str,
    name: &str,
    description: &str,
) -> RegistryPublishRequestTranslationResult<()>
where
    C: ConnectionTrait,
{
    let backend = database.get_database_backend();
    let now = database_now(backend);
    database
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO registry_publish_request_translations \
                 (request_id, locale, name, description, created_at, updated_at) \
                 VALUES ({}, {}, {}, {}, {now}, {now}) \
                 ON CONFLICT (request_id, locale) DO UPDATE SET \
                 name = excluded.name, description = excluded.description, updated_at = {now}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
            ),
            vec![
                request_id.to_string().into(),
                locale.to_string().into(),
                name.to_string().into(),
                description.to_string().into(),
            ],
        ))
        .await?;
    Ok(())
}

async fn advance_parent_revision<C>(
    database: &C,
    request_id: &str,
    status: &str,
    expected_revision: i64,
) -> RegistryPublishRequestTranslationResult<i64>
where
    C: ConnectionTrait,
{
    let backend = database.get_database_backend();
    let next_revision = expected_revision.checked_add(1).ok_or_else(|| {
        RegistryPublishRequestTranslationError::Validation(
            "registry publish request governance revision overflowed".to_string(),
        )
    })?;
    let result = database
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE registry_publish_requests SET revision = {}, updated_at = {} \
                 WHERE id = {} AND revision = {} AND status = {}",
                placeholder(backend, 1),
                database_now(backend),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
            ),
            vec![
                next_revision.into(),
                request_id.to_string().into(),
                expected_revision.into(),
                status.to_string().into(),
            ],
        ))
        .await?;
    if result.rows_affected() != 1 {
        return Err(RegistryPublishRequestTranslationError::RevisionConflict {
            revision: "governance",
        });
    }
    Ok(next_revision)
}

fn build_snapshot(
    request_id: String,
    owner: RegistryPublishRequestOwnerRow,
    translations: Vec<RegistryPublishRequestTranslationRecord>,
    source_locale: String,
    target_locale: String,
) -> RegistryPublishRequestTranslationResult<RegistryPublishRequestTranslationSnapshot> {
    let source = exact_locale_row(&translations, &source_locale)
        .cloned()
        .ok_or_else(|| RegistryPublishRequestTranslationError::SourceLocaleNotFound {
            request_id: request_id.clone(),
            locale: source_locale.clone(),
        })?;
    let target = exact_locale_row(&translations, &target_locale).cloned();
    Ok(RegistryPublishRequestTranslationSnapshot {
        request_id: request_id.clone(),
        status: owner.status.clone(),
        governance_revision: owner.governance_revision,
        source_locale,
        target_locale,
        resource_revision: resource_revision(&request_id, &owner.status, &translations),
        source_revision: locale_revision(&request_id, &source),
        target_revision: target
            .as_ref()
            .map(|row| locale_revision(&request_id, row)),
        exact_locales: translations
            .iter()
            .filter(|row| row.locale != "und")
            .map(|row| row.locale.clone())
            .collect(),
        source,
        target,
    })
}

fn exact_locale_row<'a>(
    translations: &'a [RegistryPublishRequestTranslationRecord],
    locale: &str,
) -> Option<&'a RegistryPublishRequestTranslationRecord> {
    translations.iter().find(|row| row.locale == locale)
}

pub fn registry_publish_request_translation_resource_revision(
    request_id: &str,
    status: &str,
    translations: &[RegistryPublishRequestTranslationRecord],
) -> String {
    resource_revision(request_id, status, translations)
}

pub fn registry_publish_request_translation_locale_revision(
    request_id: &str,
    translation: &RegistryPublishRequestTranslationRecord,
) -> String {
    locale_revision(request_id, translation)
}

fn resource_revision(
    request_id: &str,
    status: &str,
    translations: &[RegistryPublishRequestTranslationRecord],
) -> String {
    let mut rows = translations.to_vec();
    rows.sort_by(|left, right| left.locale.cmp(&right.locale));
    let mut hasher = Sha256::new();
    hasher.update(RESOURCE_REVISION_DOMAIN);
    hash_part(&mut hasher, request_id);
    hash_part(&mut hasher, status);
    for row in rows {
        hash_part(&mut hasher, &row.locale);
        hash_part(&mut hasher, &row.name);
        hash_part(&mut hasher, &row.description);
    }
    hex::encode(hasher.finalize())
}

fn locale_revision(
    request_id: &str,
    translation: &RegistryPublishRequestTranslationRecord,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(LOCALE_REVISION_DOMAIN);
    hash_part(&mut hasher, request_id);
    hash_part(&mut hasher, &translation.locale);
    hash_part(&mut hasher, &translation.name);
    hash_part(&mut hasher, &translation.description);
    hex::encode(hasher.finalize())
}

fn hash_part(hasher: &mut Sha256, value: &str) {
    let bytes = value.as_bytes();
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn validate_platform_context(
    context: &ModuleCommandContext,
    actor_can_manage_modules: bool,
) -> RegistryPublishRequestTranslationResult<()> {
    context.validate().map_err(|error| {
        RegistryPublishRequestTranslationError::Validation(error.to_string())
    })?;
    if context.tenant_id.is_some() {
        return Err(RegistryPublishRequestTranslationError::Validation(
            "registry publish-request Translation is platform-scoped and rejects tenant context"
                .to_string(),
        ));
    }
    if !actor_can_manage_modules {
        return Err(RegistryPublishRequestTranslationError::Validation(
            "registry publish-request Translation requires modules:manage authority".to_string(),
        ));
    }
    Ok(())
}

fn validate_request_id(request_id: &str) -> RegistryPublishRequestTranslationResult<String> {
    let request_id = request_id.trim();
    if request_id.is_empty()
        || request_id.len() > MAX_REGISTRY_TRANSLATION_REQUEST_ID_BYTES
        || request_id.chars().any(char::is_control)
    {
        return Err(RegistryPublishRequestTranslationError::Validation(format!(
            "registry Translation request id must contain 1 to {MAX_REGISTRY_TRANSLATION_REQUEST_ID_BYTES} safe bytes"
        )));
    }
    Ok(request_id.to_string())
}

fn canonical_locale(locale: &str) -> RegistryPublishRequestTranslationResult<String> {
    let locale = rustok_api::normalize_locale_tag(locale).ok_or_else(|| {
        RegistryPublishRequestTranslationError::Validation(
            "registry Translation locale must be a valid normalized locale tag".to_string(),
        )
    })?;
    if locale == "und" {
        return Err(RegistryPublishRequestTranslationError::Validation(
            "registry Translation exact locale must not be `und`".to_string(),
        ));
    }
    Ok(locale)
}

fn validate_locale_pair(source: &str, target: &str) -> RegistryPublishRequestTranslationResult<()> {
    if source == target {
        return Err(RegistryPublishRequestTranslationError::Validation(
            "registry Translation source and target locale must differ".to_string(),
        ));
    }
    Ok(())
}

fn validate_page_limit(limit: u16) -> RegistryPublishRequestTranslationResult<()> {
    if limit == 0 || limit > MAX_REGISTRY_TRANSLATION_RESOURCE_PAGE {
        return Err(RegistryPublishRequestTranslationError::Validation(format!(
            "registry Translation page size must be between 1 and {MAX_REGISTRY_TRANSLATION_RESOURCE_PAGE}"
        )));
    }
    Ok(())
}

fn is_editable_status(status: &str) -> bool {
    matches!(status, "draft" | "changes_requested")
}

fn ensure_revision(
    revision: &'static str,
    expected: &str,
    current: &str,
) -> RegistryPublishRequestTranslationResult<()> {
    if expected != current {
        return Err(RegistryPublishRequestTranslationError::RevisionConflict { revision });
    }
    Ok(())
}

fn non_negative_count(
    value: i64,
    field: &'static str,
) -> RegistryPublishRequestTranslationResult<u64> {
    u64::try_from(value).map_err(|_| {
        RegistryPublishRequestTranslationError::Validation(format!(
            "registry Translation {field} must not be negative"
        ))
    })
}

fn placeholder(backend: DbBackend, position: usize) -> String {
    if backend == DbBackend::Postgres {
        format!("${position}")
    } else {
        "?".to_string()
    }
}

fn database_now(backend: DbBackend) -> &'static str {
    if backend == DbBackend::Postgres {
        "NOW()"
    } else {
        "datetime('now')"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(locale: &str, name: &str, description: &str) -> RegistryPublishRequestTranslationRecord {
        RegistryPublishRequestTranslationRecord {
            locale: locale.to_string(),
            name: name.to_string(),
            description: description.to_string(),
        }
    }

    #[test]
    fn resource_revision_is_order_independent_but_lifecycle_sensitive() {
        let first = vec![
            row("en", "Module", "English module description"),
            row("nl", "Module NL", "Nederlandse modulebeschrijving"),
        ];
        let second = vec![first[1].clone(), first[0].clone()];
        assert_eq!(
            resource_revision("rpr_1", "draft", &first),
            resource_revision("rpr_1", "draft", &second)
        );
        assert_ne!(
            resource_revision("rpr_1", "draft", &first),
            resource_revision("rpr_1", "submitted", &first)
        );
    }

    #[test]
    fn locale_revision_is_copy_only() {
        let translation = row("en", "Module", "English module description");
        let revision = locale_revision("rpr_1", &translation);
        assert_eq!(revision, locale_revision("rpr_1", &translation));
    }

    #[test]
    fn only_pre_review_copy_states_are_editable() {
        assert!(is_editable_status("draft"));
        assert!(is_editable_status("changes_requested"));
        for status in ["submitted", "validating", "approved", "rejected", "published"] {
            assert!(!is_editable_status(status));
        }
    }
}
