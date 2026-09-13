use rustok_api::PortError;
use rustok_auth::{
    OAuthAppTranslationLifecycle, oauth_app_translation_locale_revision,
};
use rustok_outbox::idempotency;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, FromQueryResult,
    QueryFilter, QueryOrder, QuerySelect, Statement, TransactionTrait,
    sea_query::Expr,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::models::{oauth_app_translations, oauth_apps};

pub const MAX_OAUTH_APP_TRANSLATION_RESOURCE_PAGE: u16 = 200;

#[derive(Debug, Error)]
pub enum OAuthAppTranslationError {
    #[error("OAuth application {0} not found")]
    AppNotFound(Uuid),
    #[error("OAuth application source locale `{locale}` not found for {app_id}")]
    SourceLocaleNotFound { app_id: Uuid, locale: String },
    #[error("OAuth application target locale `{locale}` missing after apply for {app_id}")]
    TargetLocaleMissingAfterApply { app_id: Uuid, locale: String },
    #[error("OAuth application translation {revision} revision conflict")]
    RevisionConflict { revision: &'static str },
    #[error("OAuth application translation validation failed: {0}")]
    Validation(String),
    #[error("OAuth application translation operation receipt failed: {0}")]
    OperationReceipt(PortError),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

pub type OAuthAppTranslationResult<T> = Result<T, OAuthAppTranslationError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthAppTranslationRecord {
    pub locale: String,
    pub name: String,
    pub description: Option<String>,
}

impl From<oauth_app_translations::Model> for OAuthAppTranslationRecord {
    fn from(value: oauth_app_translations::Model) -> Self {
        Self {
            locale: value.locale,
            name: value.name,
            description: value.description,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthAppTranslationSnapshot {
    pub app_id: Uuid,
    pub lifecycle: OAuthAppTranslationLifecycle,
    pub source_locale: String,
    pub target_locale: String,
    pub resource_revision: String,
    pub source_revision: String,
    pub target_revision: Option<String>,
    pub exact_locales: Vec<String>,
    pub source: OAuthAppTranslationRecord,
    pub target: Option<OAuthAppTranslationRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthAppTranslationApply {
    pub source_locale: String,
    pub target_locale: String,
    pub name: String,
    pub description: Option<String>,
    pub expected_resource_revision: String,
    pub expected_source_revision: String,
    pub expected_target_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthAppTranslationApplyReceipt {
    pub operation_id: Option<Uuid>,
    pub app_id: Uuid,
    pub resource_revision: String,
    pub target_revision: String,
    pub target: OAuthAppTranslationRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OAuthAppTranslationProgressFacts {
    pub resources: u64,
    pub exact_required_units: u64,
    pub optional_units: u64,
    pub exact_optional_units: u64,
    pub complete_resources: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthAppTranslationChangeRecord {
    pub change_seq: u64,
    pub app_id: Uuid,
    pub resource_revision: String,
    pub lifecycle: OAuthAppTranslationLifecycle,
}

#[derive(Clone)]
pub struct OAuthAppTranslationService {
    db: DatabaseConnection,
}

impl OAuthAppTranslationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
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
    ) -> OAuthAppTranslationResult<(Vec<OAuthAppTranslationSnapshot>, Option<Uuid>)> {
        validate_identity(tenant_id, Uuid::nil(), false)?;
        let source_locale = canonical_locale(source_locale)?;
        let target_locale = canonical_locale(target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;
        if limit == 0 || limit > MAX_OAUTH_APP_TRANSLATION_RESOURCE_PAGE {
            return Err(OAuthAppTranslationError::Validation(format!(
                "OAuth application Translation resource page size must be between 1 and {MAX_OAUTH_APP_TRANSLATION_RESOURCE_PAGE}"
            )));
        }

        let source_app_ids = sea_orm::sea_query::Query::select()
            .column(oauth_app_translations::Column::AppId)
            .from(oauth_app_translations::Entity)
            .and_where(Expr::col(oauth_app_translations::Column::TenantId).eq(tenant_id))
            .and_where(Expr::col(oauth_app_translations::Column::Locale).eq(source_locale.clone()))
            .to_owned();
        let mut query = oauth_apps::Entity::find()
            .filter(oauth_apps::Column::TenantId.eq(tenant_id))
            .filter(oauth_apps::Column::IsActive.eq(true))
            .filter(oauth_apps::Column::RevokedAt.is_null())
            .filter(oauth_apps::Column::Id.in_subquery(source_app_ids))
            .order_by_asc(oauth_apps::Column::Id);
        if let Some(after) = after {
            query = query.filter(oauth_apps::Column::Id.gt(after));
        }
        let mut apps = query.limit(u64::from(limit) + 1).all(&self.db).await?;
        let has_more = apps.len() > usize::from(limit);
        if has_more {
            apps.truncate(usize::from(limit));
        }
        let next_after = has_more.then(|| apps.last().map(|app| app.id)).flatten();

        let mut snapshots = Vec::with_capacity(apps.len());
        for app in apps {
            let translations = oauth_apps::load_translations(&self.db, tenant_id, app.id).await?;
            snapshots.push(build_snapshot(
                app,
                translations,
                source_locale.clone(),
                target_locale.clone(),
            )?);
        }
        Ok((snapshots, next_after))
    }

    pub async fn read_exact_locale(
        &self,
        tenant_id: Uuid,
        app_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> OAuthAppTranslationResult<OAuthAppTranslationSnapshot> {
        validate_identity(tenant_id, app_id, true)?;
        let source_locale = canonical_locale(source_locale)?;
        let target_locale = canonical_locale(target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;
        let app = load_app(&self.db, tenant_id, app_id).await?;
        let translations = oauth_apps::load_translations(&self.db, tenant_id, app_id).await?;
        build_snapshot(app, translations, source_locale, target_locale)
    }

    pub async fn apply_exact_locale_with_operation(
        &self,
        tenant_id: Uuid,
        app_id: Uuid,
        request: OAuthAppTranslationApply,
        operation_lease: idempotency::Lease,
    ) -> OAuthAppTranslationResult<OAuthAppTranslationApplyReceipt> {
        self.apply_exact_locale_inner(tenant_id, app_id, request, Some(operation_lease))
            .await
    }

    async fn apply_exact_locale_inner(
        &self,
        tenant_id: Uuid,
        app_id: Uuid,
        request: OAuthAppTranslationApply,
        operation_lease: Option<idempotency::Lease>,
    ) -> OAuthAppTranslationResult<OAuthAppTranslationApplyReceipt> {
        validate_identity(tenant_id, app_id, true)?;
        let source_locale = canonical_locale(&request.source_locale)?;
        let target_locale = canonical_locale(&request.target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;
        let name = normalize_name(&request.name)?;
        let description = normalize_description(request.description);

        let txn = self.db.begin().await?;
        let app = oauth_apps::Entity::find_by_id(app_id)
            .filter(oauth_apps::Column::TenantId.eq(tenant_id))
            .lock_exclusive()
            .one(&txn)
            .await?
            .ok_or(OAuthAppTranslationError::AppNotFound(app_id))?;
        if oauth_apps::translation_lifecycle(&app) != OAuthAppTranslationLifecycle::Active {
            return Err(OAuthAppTranslationError::Validation(
                "revoked OAuth application presentation cannot be translated".to_string(),
            ));
        }
        let translations = oauth_apps::load_translations(&txn, tenant_id, app_id).await?;
        let source = exact_locale_row(&translations, &source_locale).ok_or_else(|| {
            OAuthAppTranslationError::SourceLocaleNotFound {
                app_id,
                locale: source_locale.clone(),
            }
        })?;
        let target = exact_locale_row(&translations, &target_locale);
        ensure_revision(
            "resource",
            &request.expected_resource_revision,
            &oauth_apps::translation_resource_revision(&app, &translations),
        )?;
        ensure_revision(
            "source",
            &request.expected_source_revision,
            &locale_revision(source),
        )?;
        let current_target_revision = target.map(locale_revision);
        if request.expected_target_revision != current_target_revision {
            return Err(OAuthAppTranslationError::RevisionConflict { revision: "target" });
        }

        let unchanged = target.is_some_and(|current| {
            current.name == name && current.description == description
        });
        if !unchanged {
            oauth_apps::upsert_translation(
                &txn,
                tenant_id,
                app_id,
                target_locale.as_str(),
                name,
                description,
            )
            .await?;
            oauth_apps::Entity::update_many()
                .col_expr(oauth_apps::Column::UpdatedAt, Expr::current_timestamp())
                .filter(oauth_apps::Column::TenantId.eq(tenant_id))
                .filter(oauth_apps::Column::Id.eq(app_id))
                .exec(&txn)
                .await?;
        }

        let translations_after = oauth_apps::load_translations(&txn, tenant_id, app_id).await?;
        let target_after = exact_locale_row(&translations_after, &target_locale)
            .cloned()
            .ok_or_else(|| OAuthAppTranslationError::TargetLocaleMissingAfterApply {
                app_id,
                locale: target_locale,
            })?;
        let resource_revision = oauth_apps::translation_resource_revision(&app, &translations_after);
        let operation_id = operation_lease
            .map(|lease| lease.operation_id)
            .or_else(|| (!unchanged).then(rustok_core::generate_id));
        if !unchanged {
            oauth_apps::record_translation_change(&txn, &app, operation_id).await?;
        }
        let receipt = OAuthAppTranslationApplyReceipt {
            operation_id,
            app_id,
            resource_revision,
            target_revision: locale_revision(&target_after),
            target: target_after.into(),
        };
        if let Some(lease) = operation_lease {
            idempotency::complete(&txn, lease, &receipt)
                .await
                .map_err(OAuthAppTranslationError::OperationReceipt)?;
        }
        txn.commit().await?;
        Ok(receipt)
    }

    pub async fn read_exact_progress(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> OAuthAppTranslationResult<OAuthAppTranslationProgressFacts> {
        validate_identity(tenant_id, Uuid::nil(), false)?;
        let source_locale = canonical_locale(source_locale)?;
        let target_locale = canonical_locale(target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;
        let backend = self.db.get_database_backend();
        let (sql, values) = match backend {
            DbBackend::Postgres => (
                OAUTH_APP_TRANSLATION_PROGRESS_POSTGRES_SQL,
                vec![tenant_id.into(), source_locale.into(), target_locale.into()],
            ),
            _ => (
                OAUTH_APP_TRANSLATION_PROGRESS_QUESTION_MARK_SQL,
                vec![target_locale.into(), tenant_id.into(), source_locale.into()],
            ),
        };
        let row = ProgressRow::find_by_statement(Statement::from_sql_and_values(
            backend, sql, values,
        ))
        .one(&self.db)
        .await?
        .ok_or_else(|| {
            OAuthAppTranslationError::Validation(
                "OAuth application Translation progress aggregate returned no row".to_string(),
            )
        })?;
        let facts = OAuthAppTranslationProgressFacts {
            resources: count(row.resources, "resources")?,
            exact_required_units: count(row.exact_required_units, "exact required units")?,
            optional_units: count(row.optional_units, "optional units")?,
            exact_optional_units: count(row.exact_optional_units, "exact optional units")?,
            complete_resources: count(row.complete_resources, "complete resources")?,
        };
        if facts.exact_required_units > facts.resources
            || facts.optional_units > facts.resources
            || facts.exact_optional_units > facts.optional_units
            || facts.complete_resources > facts.resources
        {
            return Err(OAuthAppTranslationError::Validation(
                "OAuth application Translation progress aggregate is inconsistent".to_string(),
            ));
        }
        Ok(facts)
    }

    pub async fn translation_change_highwater(
        &self,
        tenant_id: Uuid,
    ) -> OAuthAppTranslationResult<Option<u64>> {
        validate_identity(tenant_id, Uuid::nil(), false)?;
        let backend = self.db.get_database_backend();
        let sql = match backend {
            DbBackend::Postgres => {
                "SELECT MAX(change_seq) AS highwater FROM oauth_app_translation_change_journal WHERE tenant_id = $1"
            }
            _ => {
                "SELECT MAX(change_seq) AS highwater FROM oauth_app_translation_change_journal WHERE tenant_id = ?"
            }
        };
        let row = HighwaterRow::find_by_statement(Statement::from_sql_and_values(
            backend,
            sql,
            vec![tenant_id.into()],
        ))
        .one(&self.db)
        .await?
        .ok_or_else(|| OAuthAppTranslationError::Validation("OAuth application Translation highwater aggregate returned no row".to_string()))?;
        row.highwater
            .map(|value| count(value, "change highwater"))
            .transpose()
    }

    pub async fn read_translation_changes(
        &self,
        tenant_id: Uuid,
        after: u64,
        through: u64,
        limit: u16,
    ) -> OAuthAppTranslationResult<Vec<OAuthAppTranslationChangeRecord>> {
        validate_identity(tenant_id, Uuid::nil(), false)?;
        if limit == 0 || limit > MAX_OAUTH_APP_TRANSLATION_RESOURCE_PAGE {
            return Err(OAuthAppTranslationError::Validation(format!(
                "OAuth application Translation change page size must be between 1 and {MAX_OAUTH_APP_TRANSLATION_RESOURCE_PAGE}"
            )));
        }
        let after = i64::try_from(after).map_err(|_| OAuthAppTranslationError::Validation(
            "OAuth application Translation change cursor is too large".to_string(),
        ))?;
        let through = i64::try_from(through).map_err(|_| OAuthAppTranslationError::Validation(
            "OAuth application Translation change cursor is too large".to_string(),
        ))?;
        let backend = self.db.get_database_backend();
        let (sql, values) = match backend {
            DbBackend::Postgres => (
                "SELECT change_seq, app_id, resource_revision, lifecycle FROM oauth_app_translation_change_journal WHERE tenant_id = $1 AND change_seq > $2 AND change_seq <= $3 ORDER BY change_seq ASC LIMIT $4",
                vec![tenant_id.into(), after.into(), through.into(), i64::from(limit).into()],
            ),
            _ => (
                "SELECT change_seq, app_id, resource_revision, lifecycle FROM oauth_app_translation_change_journal WHERE tenant_id = ? AND change_seq > ? AND change_seq <= ? ORDER BY change_seq ASC LIMIT ?",
                vec![tenant_id.into(), after.into(), through.into(), i64::from(limit).into()],
            ),
        };
        let rows = ChangeRow::find_by_statement(Statement::from_sql_and_values(
            backend, sql, values,
        ))
        .all(&self.db)
        .await?;
        rows.into_iter()
            .map(|row| {
                let lifecycle = OAuthAppTranslationLifecycle::parse(row.lifecycle.as_str())
                    .ok_or_else(|| OAuthAppTranslationError::Validation(
                        "OAuth application Translation journal lifecycle is invalid".to_string(),
                    ))?;
                Ok(OAuthAppTranslationChangeRecord {
                    change_seq: count(row.change_seq, "change sequence")?,
                    app_id: row.app_id,
                    resource_revision: row.resource_revision,
                    lifecycle,
                })
            })
            .collect()
    }
}

async fn load_app<C>(db: &C, tenant_id: Uuid, app_id: Uuid) -> OAuthAppTranslationResult<oauth_apps::Model>
where
    C: ConnectionTrait,
{
    oauth_apps::Entity::find_by_id(app_id)
        .filter(oauth_apps::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or(OAuthAppTranslationError::AppNotFound(app_id))
}

fn build_snapshot(
    app: oauth_apps::Model,
    translations: Vec<oauth_app_translations::Model>,
    source_locale: String,
    target_locale: String,
) -> OAuthAppTranslationResult<OAuthAppTranslationSnapshot> {
    let source = exact_locale_row(&translations, &source_locale)
        .cloned()
        .ok_or_else(|| OAuthAppTranslationError::SourceLocaleNotFound {
            app_id: app.id,
            locale: source_locale.clone(),
        })?;
    let target = exact_locale_row(&translations, &target_locale).cloned();
    Ok(OAuthAppTranslationSnapshot {
        app_id: app.id,
        lifecycle: oauth_apps::translation_lifecycle(&app),
        source_locale,
        target_locale,
        resource_revision: oauth_apps::translation_resource_revision(&app, &translations),
        source_revision: locale_revision(&source),
        target_revision: target.as_ref().map(locale_revision),
        exact_locales: translations
            .iter()
            .filter(|row| row.locale != "und")
            .map(|row| row.locale.clone())
            .collect(),
        source: source.into(),
        target: target.map(Into::into),
    })
}

fn exact_locale_row<'a>(
    translations: &'a [oauth_app_translations::Model],
    locale: &str,
) -> Option<&'a oauth_app_translations::Model> {
    translations.iter().find(|row| row.locale == locale)
}

fn locale_revision(translation: &oauth_app_translations::Model) -> String {
    oauth_app_translation_locale_revision(
        translation.tenant_id,
        translation.app_id,
        translation.locale.as_str(),
        translation.name.as_str(),
        translation.description.as_deref(),
    )
}

fn validate_identity(tenant_id: Uuid, app_id: Uuid, require_app: bool) -> OAuthAppTranslationResult<()> {
    if tenant_id.is_nil() || (require_app && app_id.is_nil()) {
        return Err(OAuthAppTranslationError::Validation(
            "OAuth application Translation identity must use non-nil UUIDs".to_string(),
        ));
    }
    Ok(())
}

fn canonical_locale(locale: &str) -> OAuthAppTranslationResult<String> {
    oauth_apps::normalize_runtime_copy_locale(locale)
        .map_err(|error| OAuthAppTranslationError::Validation(error.to_string()))
}

fn validate_locale_pair(source: &str, target: &str) -> OAuthAppTranslationResult<()> {
    if source == target {
        return Err(OAuthAppTranslationError::Validation(
            "OAuth application Translation source and target locale must differ".to_string(),
        ));
    }
    Ok(())
}

fn normalize_name(value: &str) -> OAuthAppTranslationResult<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 255 {
        return Err(OAuthAppTranslationError::Validation(
            "OAuth application translated name must contain 1 to 255 characters".to_string(),
        ));
    }
    Ok(value.to_string())
}

fn normalize_description(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn ensure_revision(
    revision: &'static str,
    expected: &str,
    current: &str,
) -> OAuthAppTranslationResult<()> {
    if expected != current {
        return Err(OAuthAppTranslationError::RevisionConflict { revision });
    }
    Ok(())
}

fn count(value: i64, field: &'static str) -> OAuthAppTranslationResult<u64> {
    u64::try_from(value).map_err(|_| OAuthAppTranslationError::Validation(format!(
        "OAuth application Translation {field} must not be negative"
    )))
}

#[derive(Debug, FromQueryResult)]
struct ProgressRow {
    resources: i64,
    exact_required_units: i64,
    optional_units: i64,
    exact_optional_units: i64,
    complete_resources: i64,
}

#[derive(Debug, FromQueryResult)]
struct HighwaterRow {
    highwater: Option<i64>,
}

#[derive(Debug, FromQueryResult)]
struct ChangeRow {
    change_seq: i64,
    app_id: Uuid,
    resource_revision: String,
    lifecycle: String,
}

const OAUTH_APP_TRANSLATION_PROGRESS_POSTGRES_SQL: &str = r#"
SELECT
    COUNT(*) AS resources,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.name, '')) <> '' THEN 1 END)
        AS exact_required_units,
    COUNT(CASE WHEN TRIM(COALESCE(source_translation.description, '')) <> '' THEN 1 END)
        AS optional_units,
    COUNT(CASE
        WHEN TRIM(COALESCE(source_translation.description, '')) <> ''
         AND TRIM(COALESCE(target_translation.description, '')) <> ''
        THEN 1 END) AS exact_optional_units,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.name, '')) <> '' THEN 1 END)
        AS complete_resources
FROM oauth_app_translations AS source_translation
INNER JOIN oauth_apps AS app
    ON app.tenant_id = source_translation.tenant_id
   AND app.id = source_translation.app_id
LEFT JOIN oauth_app_translations AS target_translation
    ON target_translation.tenant_id = source_translation.tenant_id
   AND target_translation.app_id = source_translation.app_id
   AND target_translation.locale = $3
WHERE source_translation.tenant_id = $1
  AND source_translation.locale = $2
  AND app.is_active = TRUE
  AND app.revoked_at IS NULL
"#;

const OAUTH_APP_TRANSLATION_PROGRESS_QUESTION_MARK_SQL: &str = r#"
SELECT
    COUNT(*) AS resources,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.name, '')) <> '' THEN 1 END)
        AS exact_required_units,
    COUNT(CASE WHEN TRIM(COALESCE(source_translation.description, '')) <> '' THEN 1 END)
        AS optional_units,
    COUNT(CASE
        WHEN TRIM(COALESCE(source_translation.description, '')) <> ''
         AND TRIM(COALESCE(target_translation.description, '')) <> ''
        THEN 1 END) AS exact_optional_units,
    COUNT(CASE WHEN TRIM(COALESCE(target_translation.name, '')) <> '' THEN 1 END)
        AS complete_resources
FROM oauth_app_translations AS source_translation
INNER JOIN oauth_apps AS app
    ON app.tenant_id = source_translation.tenant_id
   AND app.id = source_translation.app_id
LEFT JOIN oauth_app_translations AS target_translation
    ON target_translation.tenant_id = source_translation.tenant_id
   AND target_translation.app_id = source_translation.app_id
   AND target_translation.locale = ?
WHERE source_translation.tenant_id = ?
  AND source_translation.locale = ?
  AND app.is_active = TRUE
  AND app.revoked_at IS NULL
"#;
