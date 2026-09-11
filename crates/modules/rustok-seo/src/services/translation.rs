use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, Statement, TransactionTrait,
};
use uuid::Uuid;

use rustok_core::ModuleRuntimeExtensions;
use rustok_outbox::TransactionalEventBus;
use rustok_seo_targets::SeoTargetRegistry;

use crate::entities::{self as seo_meta, meta_translation};
use crate::{SeoError, SeoResult, SeoTargetSlug};

use super::events::SeoMetaUpsertedEventInput;
use super::SeoService;

pub const SEO_TRANSLATION_FIELD_KEYS: [&str; 5] = [
    "title",
    "description",
    "keywords",
    "og_title",
    "og_description",
];

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SeoTranslationValues {
    pub title: Option<String>,
    pub description: Option<String>,
    pub keywords: Option<String>,
    pub og_title: Option<String>,
    pub og_description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeoTranslationResourceSummary {
    pub target_kind: String,
    pub target_id: Uuid,
    pub resource_revision: String,
    pub exact_locales: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeoTranslationResourcePage {
    pub resources: Vec<SeoTranslationResourceSummary>,
    pub next_after: Option<(String, Uuid)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeoTranslationExactLocaleSnapshot {
    pub target_kind: String,
    pub target_id: Uuid,
    pub source_locale: String,
    pub target_locale: String,
    pub resource_revision: String,
    pub source_revision: String,
    pub target_revision: Option<String>,
    pub exact_locales: Vec<String>,
    pub source: SeoTranslationValues,
    pub target: SeoTranslationValues,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeoTranslationChangeLifecycle {
    Active,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeoTranslationChange {
    pub cursor: i64,
    pub target_kind: String,
    pub target_id: Uuid,
    pub resource_revision: String,
    pub lifecycle: SeoTranslationChangeLifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeoTranslationChangePage {
    pub changes: Vec<SeoTranslationChange>,
    pub next_cursor: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeoTranslationApplyOperation {
    pub actor_user_id: Uuid,
    pub idempotency_key: String,
    pub request_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeoTranslationExactLocaleApply {
    pub operation: SeoTranslationApplyOperation,
    pub source_locale: String,
    pub target_locale: String,
    pub values: SeoTranslationValues,
    pub expected_resource_revision: String,
    pub expected_source_revision: String,
    pub expected_target_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeoTranslationApplyReceipt {
    pub operation_id: Uuid,
    pub target_kind: String,
    pub target_id: Uuid,
    pub resource_revision: String,
    pub source_revision: String,
    pub target_revision: String,
}

#[derive(Debug, thiserror::Error)]
pub enum SeoTranslationOwnerError {
    #[error("invalid SEO Translation request: {0}")]
    Invalid(String),
    #[error("SEO explicit Translation resource not found")]
    NotFound,
    #[error("SEO exact source locale not found")]
    SourceLocaleNotFound,
    #[error("SEO Translation revision conflict")]
    RevisionConflict,
    #[error("SEO Translation idempotency key conflicts with an earlier request")]
    IdempotencyConflict,
    #[error("SEO Translation owner invariant failed: {0}")]
    OwnerInvariant(String),
    #[error(transparent)]
    Storage(#[from] SeoError),
}

#[derive(Clone)]
pub struct SeoTranslationService {
    runtime: SeoService,
}

impl SeoTranslationService {
    pub fn new(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        registry: std::sync::Arc<SeoTargetRegistry>,
    ) -> Self {
        Self {
            runtime: SeoService::new(db, event_bus, registry),
        }
    }

    pub fn from_runtime_extensions(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        extensions: &ModuleRuntimeExtensions,
    ) -> SeoResult<Self> {
        SeoService::from_runtime_extensions(db, event_bus, extensions).map(|runtime| Self { runtime })
    }

    pub async fn list_exact_resources(
        &self,
        tenant_id: Uuid,
        after: Option<(String, Uuid)>,
        limit: usize,
    ) -> Result<SeoTranslationResourcePage, SeoTranslationOwnerError> {
        if tenant_id.is_nil() || limit == 0 || limit > 500 {
            return Err(SeoTranslationOwnerError::Invalid(
                "tenant_id must be non-nil and limit must be in 1..=500".to_string(),
            ));
        }
        let (after_kind, after_id) = after
            .map(|(kind, id)| (kind, id))
            .unwrap_or_else(|| (String::new(), Uuid::nil()));
        let sql = r#"
SELECT state.target_kind,
       state.target_id,
       format('seo:%s', state.revision) AS resource_revision,
       ARRAY(
           SELECT translation.locale
           FROM meta_translations translation
           JOIN meta owner ON owner.id = translation.meta_id
           WHERE owner.tenant_id = state.tenant_id
             AND owner.target_type = state.target_kind
             AND owner.target_id = state.target_id
           ORDER BY translation.locale
       ) AS exact_locales
FROM seo_translation_resource_state state
WHERE state.tenant_id = $1
  AND ($2 = '' OR (state.target_kind, state.target_id) > ($2, $3))
ORDER BY state.target_kind, state.target_id
LIMIT $4
"#;
        let rows = self
            .runtime
            .db
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                sql,
                vec![
                    tenant_id.into(),
                    after_kind.into(),
                    after_id.into(),
                    ((limit + 1) as i64).into(),
                ],
            ))
            .await
            .map_err(SeoError::from)?;

        let mut resources = Vec::with_capacity(rows.len().min(limit));
        for row in rows.iter().take(limit) {
            resources.push(SeoTranslationResourceSummary {
                target_kind: row.try_get("", "target_kind").map_err(SeoError::from)?,
                target_id: row.try_get("", "target_id").map_err(SeoError::from)?,
                resource_revision: row
                    .try_get("", "resource_revision")
                    .map_err(SeoError::from)?,
                exact_locales: row.try_get("", "exact_locales").map_err(SeoError::from)?,
            });
        }
        let next_after = if rows.len() > limit {
            resources
                .last()
                .map(|item| (item.target_kind.clone(), item.target_id))
        } else {
            None
        };
        Ok(SeoTranslationResourcePage {
            resources,
            next_after,
        })
    }

    pub async fn read_exact_locale(
        &self,
        tenant_id: Uuid,
        target_kind: &str,
        target_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> Result<SeoTranslationExactLocaleSnapshot, SeoTranslationOwnerError> {
        validate_identity_and_locales(
            tenant_id,
            target_kind,
            target_id,
            source_locale,
            target_locale,
        )?;
        read_snapshot(
            &self.runtime.db,
            tenant_id,
            target_kind,
            target_id,
            source_locale,
            target_locale,
        )
        .await
    }

    pub async fn read_changes(
        &self,
        tenant_id: Uuid,
        after: Option<i64>,
        limit: usize,
    ) -> Result<SeoTranslationChangePage, SeoTranslationOwnerError> {
        if tenant_id.is_nil() || limit == 0 || limit > 500 || after.is_some_and(|value| value < 0) {
            return Err(SeoTranslationOwnerError::Invalid(
                "invalid tenant, cursor, or change-page limit".to_string(),
            ));
        }
        let rows = self
            .runtime
            .db
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"
SELECT change_seq, target_kind, target_id, resource_revision, lifecycle
FROM seo_translation_change_journal
WHERE tenant_id = $1 AND change_seq > $2
ORDER BY change_seq
LIMIT $3
"#,
                vec![
                    tenant_id.into(),
                    after.unwrap_or(0).into(),
                    ((limit + 1) as i64).into(),
                ],
            ))
            .await
            .map_err(SeoError::from)?;
        let mut changes = Vec::with_capacity(rows.len().min(limit));
        for row in rows.iter().take(limit) {
            let lifecycle: String = row.try_get("", "lifecycle").map_err(SeoError::from)?;
            changes.push(SeoTranslationChange {
                cursor: row.try_get("", "change_seq").map_err(SeoError::from)?,
                target_kind: row.try_get("", "target_kind").map_err(SeoError::from)?,
                target_id: row.try_get("", "target_id").map_err(SeoError::from)?,
                resource_revision: row
                    .try_get("", "resource_revision")
                    .map_err(SeoError::from)?,
                lifecycle: match lifecycle.as_str() {
                    "active" => SeoTranslationChangeLifecycle::Active,
                    "deleted" => SeoTranslationChangeLifecycle::Deleted,
                    other => {
                        return Err(SeoTranslationOwnerError::OwnerInvariant(format!(
                            "unknown change lifecycle {other}"
                        )));
                    }
                },
            });
        }
        let next_cursor = if rows.len() > limit {
            changes.last().map(|change| change.cursor)
        } else {
            None
        };
        Ok(SeoTranslationChangePage {
            changes,
            next_cursor,
        })
    }

    pub async fn apply_exact_locale(
        &self,
        tenant_id: Uuid,
        target_kind: &str,
        target_id: Uuid,
        command: SeoTranslationExactLocaleApply,
    ) -> Result<SeoTranslationApplyReceipt, SeoTranslationOwnerError> {
        validate_identity_and_locales(
            tenant_id,
            target_kind,
            target_id,
            command.source_locale.as_str(),
            command.target_locale.as_str(),
        )?;
        if command.operation.actor_user_id.is_nil()
            || command.operation.idempotency_key.trim().is_empty()
            || command.operation.request_fingerprint.trim().is_empty()
        {
            return Err(SeoTranslationOwnerError::Invalid(
                "apply operation identity is incomplete".to_string(),
            ));
        }

        let txn = self.runtime.db.begin().await.map_err(SeoError::from)?;
        if let Some(receipt) = load_receipt(
            &txn,
            tenant_id,
            command.operation.idempotency_key.as_str(),
        )
        .await?
        {
            if receipt.actor_user_id != command.operation.actor_user_id
                || receipt.target_kind != target_kind
                || receipt.target_id != target_id
                || receipt.request_fingerprint != command.operation.request_fingerprint
            {
                return Err(SeoTranslationOwnerError::IdempotencyConflict);
            }
            txn.rollback().await.map_err(SeoError::from)?;
            return Ok(receipt.into_public());
        }

        let owner = self
            .lock_meta(&txn, tenant_id, target_kind, target_id)
            .await?;
        let current = read_snapshot(
            &txn,
            tenant_id,
            target_kind,
            target_id,
            command.source_locale.as_str(),
            command.target_locale.as_str(),
        )
        .await?;
        if current.resource_revision != command.expected_resource_revision
            || current.source_revision != command.expected_source_revision
            || current.target_revision != command.expected_target_revision
        {
            return Err(SeoTranslationOwnerError::RevisionConflict);
        }

        let existing_target = meta_translation::Entity::find()
            .filter(meta_translation::Column::MetaId.eq(owner.id))
            .filter(meta_translation::Column::Locale.eq(command.target_locale.clone()))
            .one(&txn)
            .await
            .map_err(SeoError::from)?;
        if let Some(existing) = existing_target {
            let mut active: meta_translation::ActiveModel = existing.into();
            active.title = Set(command.values.title.clone());
            active.description = Set(command.values.description.clone());
            active.keywords = Set(command.values.keywords.clone());
            active.og_title = Set(command.values.og_title.clone());
            active.og_description = Set(command.values.og_description.clone());
            active.update(&txn).await.map_err(SeoError::from)?;
        } else {
            meta_translation::ActiveModel {
                id: Set(Uuid::new_v4()),
                meta_id: Set(owner.id),
                locale: Set(command.target_locale.clone()),
                title: Set(command.values.title.clone()),
                description: Set(command.values.description.clone()),
                keywords: Set(command.values.keywords.clone()),
                og_title: Set(command.values.og_title.clone()),
                og_description: Set(command.values.og_description.clone()),
                og_image: Set(None),
            }
            .insert(&txn)
            .await
            .map_err(SeoError::from)?;
        }

        self.runtime
            .publish_seo_meta_upserted_event_in_tx(
                &txn,
                SeoMetaUpsertedEventInput {
                    tenant_id,
                    target_kind,
                    target_id,
                    locale: command.target_locale.as_str(),
                    source: "translation",
                    transition_ref: Some(command.operation.idempotency_key.as_str()),
                },
            )
            .await?;

        let applied = read_snapshot(
            &txn,
            tenant_id,
            target_kind,
            target_id,
            command.source_locale.as_str(),
            command.target_locale.as_str(),
        )
        .await?;
        let target_revision = applied.target_revision.clone().ok_or_else(|| {
            SeoTranslationOwnerError::OwnerInvariant(
                "target revision missing after exact-locale apply".to_string(),
            )
        })?;
        let operation_id = Uuid::new_v4();
        txn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
INSERT INTO seo_translation_apply_receipts (
    tenant_id, idempotency_key, actor_user_id, target_kind, target_id,
    request_fingerprint, operation_id, resource_revision, source_revision, target_revision
) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
"#,
            vec![
                tenant_id.into(),
                command.operation.idempotency_key.into(),
                command.operation.actor_user_id.into(),
                target_kind.to_string().into(),
                target_id.into(),
                command.operation.request_fingerprint.into(),
                operation_id.into(),
                applied.resource_revision.clone().into(),
                applied.source_revision.clone().into(),
                target_revision.clone().into(),
            ],
        ))
        .await
        .map_err(SeoError::from)?;
        txn.commit().await.map_err(SeoError::from)?;

        Ok(SeoTranslationApplyReceipt {
            operation_id,
            target_kind: target_kind.to_string(),
            target_id,
            resource_revision: applied.resource_revision,
            source_revision: applied.source_revision,
            target_revision,
        })
    }

    async fn lock_meta(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        tenant_id: Uuid,
        target_kind: &str,
        target_id: Uuid,
    ) -> Result<seo_meta::Model, SeoTranslationOwnerError> {
        let row = txn
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"
SELECT id, tenant_id, target_type, target_id, no_index, no_follow, canonical_url, structured_data
FROM meta
WHERE tenant_id = $1 AND target_type = $2 AND target_id = $3
FOR UPDATE
"#,
                vec![tenant_id.into(), target_kind.to_string().into(), target_id.into()],
            ))
            .await
            .map_err(SeoError::from)?
            .ok_or(SeoTranslationOwnerError::NotFound)?;
        Ok(seo_meta::Model {
            id: row.try_get("", "id").map_err(SeoError::from)?,
            tenant_id: row.try_get("", "tenant_id").map_err(SeoError::from)?,
            target_type: row.try_get("", "target_type").map_err(SeoError::from)?,
            target_id: row.try_get("", "target_id").map_err(SeoError::from)?,
            no_index: row.try_get("", "no_index").map_err(SeoError::from)?,
            no_follow: row.try_get("", "no_follow").map_err(SeoError::from)?,
            canonical_url: row.try_get("", "canonical_url").map_err(SeoError::from)?,
            structured_data: row.try_get("", "structured_data").map_err(SeoError::from)?,
        })
    }
}

#[derive(Debug)]
struct StoredReceipt {
    actor_user_id: Uuid,
    target_kind: String,
    target_id: Uuid,
    request_fingerprint: String,
    operation_id: Uuid,
    resource_revision: String,
    source_revision: String,
    target_revision: String,
}

impl StoredReceipt {
    fn into_public(self) -> SeoTranslationApplyReceipt {
        SeoTranslationApplyReceipt {
            operation_id: self.operation_id,
            target_kind: self.target_kind,
            target_id: self.target_id,
            resource_revision: self.resource_revision,
            source_revision: self.source_revision,
            target_revision: self.target_revision,
        }
    }
}

async fn load_receipt<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> Result<Option<StoredReceipt>, SeoTranslationOwnerError> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
SELECT actor_user_id, target_kind, target_id, request_fingerprint, operation_id,
       resource_revision, source_revision, target_revision
FROM seo_translation_apply_receipts
WHERE tenant_id = $1 AND idempotency_key = $2
"#,
            vec![tenant_id.into(), idempotency_key.to_string().into()],
        ))
        .await
        .map_err(SeoError::from)?;
    row.map(|row| {
        Ok(StoredReceipt {
            actor_user_id: row.try_get("", "actor_user_id").map_err(SeoError::from)?,
            target_kind: row.try_get("", "target_kind").map_err(SeoError::from)?,
            target_id: row.try_get("", "target_id").map_err(SeoError::from)?,
            request_fingerprint: row
                .try_get("", "request_fingerprint")
                .map_err(SeoError::from)?,
            operation_id: row.try_get("", "operation_id").map_err(SeoError::from)?,
            resource_revision: row
                .try_get("", "resource_revision")
                .map_err(SeoError::from)?,
            source_revision: row
                .try_get("", "source_revision")
                .map_err(SeoError::from)?,
            target_revision: row
                .try_get("", "target_revision")
                .map_err(SeoError::from)?,
        })
    })
    .transpose()
}

async fn read_snapshot<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    target_kind: &str,
    target_id: Uuid,
    source_locale: &str,
    target_locale: &str,
) -> Result<SeoTranslationExactLocaleSnapshot, SeoTranslationOwnerError> {
    let owner = seo_meta::Entity::find()
        .filter(seo_meta::Column::TenantId.eq(tenant_id))
        .filter(seo_meta::Column::TargetType.eq(target_kind))
        .filter(seo_meta::Column::TargetId.eq(target_id))
        .one(db)
        .await
        .map_err(SeoError::from)?
        .ok_or(SeoTranslationOwnerError::NotFound)?;
    let translations = meta_translation::Entity::find()
        .filter(meta_translation::Column::MetaId.eq(owner.id))
        .order_by_asc(meta_translation::Column::Locale)
        .all(db)
        .await
        .map_err(SeoError::from)?;
    let source = translations
        .iter()
        .find(|row| row.locale == source_locale)
        .ok_or(SeoTranslationOwnerError::SourceLocaleNotFound)?;
    let target = translations.iter().find(|row| row.locale == target_locale);

    let resource_row = db
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
SELECT revision FROM seo_translation_resource_state
WHERE tenant_id = $1 AND target_kind = $2 AND target_id = $3
"#,
            vec![tenant_id.into(), target_kind.to_string().into(), target_id.into()],
        ))
        .await
        .map_err(SeoError::from)?
        .ok_or_else(|| {
            SeoTranslationOwnerError::OwnerInvariant(
                "resource revision state missing for explicit SEO metadata".to_string(),
            )
        })?;
    let source_revision = read_locale_revision(
        db,
        tenant_id,
        target_kind,
        target_id,
        source_locale,
    )
    .await?
    .ok_or_else(|| {
        SeoTranslationOwnerError::OwnerInvariant(
            "source locale revision state missing for explicit SEO metadata".to_string(),
        )
    })?;
    let target_revision = read_locale_revision(
        db,
        tenant_id,
        target_kind,
        target_id,
        target_locale,
    )
    .await?;
    let resource_revision: i64 = resource_row
        .try_get("", "revision")
        .map_err(SeoError::from)?;

    Ok(SeoTranslationExactLocaleSnapshot {
        target_kind: target_kind.to_string(),
        target_id,
        source_locale: source_locale.to_string(),
        target_locale: target_locale.to_string(),
        resource_revision: format!("seo:{resource_revision}"),
        source_revision: format!("seo-locale:{source_revision}"),
        target_revision: target_revision.map(|revision| format!("seo-locale:{revision}")),
        exact_locales: translations.iter().map(|row| row.locale.clone()).collect(),
        source: values_from_model(source),
        target: target.map(values_from_model).unwrap_or_default(),
    })
}

async fn read_locale_revision<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    target_kind: &str,
    target_id: Uuid,
    locale: &str,
) -> Result<Option<i64>, SeoTranslationOwnerError> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
SELECT revision FROM seo_translation_locale_state
WHERE tenant_id = $1 AND target_kind = $2 AND target_id = $3 AND locale = $4
"#,
            vec![
                tenant_id.into(),
                target_kind.to_string().into(),
                target_id.into(),
                locale.to_string().into(),
            ],
        ))
        .await
        .map_err(SeoError::from)?;
    row.map(|row| row.try_get("", "revision").map_err(SeoError::from))
        .transpose()
        .map_err(SeoTranslationOwnerError::from)
}

fn values_from_model(model: &meta_translation::Model) -> SeoTranslationValues {
    SeoTranslationValues {
        title: model.title.clone(),
        description: model.description.clone(),
        keywords: model.keywords.clone(),
        og_title: model.og_title.clone(),
        og_description: model.og_description.clone(),
    }
}

fn validate_identity_and_locales(
    tenant_id: Uuid,
    target_kind: &str,
    target_id: Uuid,
    source_locale: &str,
    target_locale: &str,
) -> Result<(), SeoTranslationOwnerError> {
    if tenant_id.is_nil()
        || target_id.is_nil()
        || target_kind.trim().is_empty()
        || source_locale.trim().is_empty()
        || target_locale.trim().is_empty()
        || source_locale == target_locale
    {
        return Err(SeoTranslationOwnerError::Invalid(
            "identity and exact source/target locales are invalid".to_string(),
        ));
    }
    SeoTargetSlug::new(target_kind.to_string()).map_err(|error| {
        SeoTranslationOwnerError::Invalid(format!("invalid SEO target kind: {error}"))
    })?;
    Ok(())
}
