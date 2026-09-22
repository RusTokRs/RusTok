use chrono::Utc;
use rustok_api::RuntimeLocale;
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, DatabaseConnection, EntityTrait, QueryFilter, Set, TransactionTrait,
};
use thiserror::Error;
use uuid::Uuid;

use crate::model::{RhaiWorkspace, Script, ScriptTrigger, SourceProvenance};

use super::{
    SeaOrmScriptPresentationStore, ScriptPresentationStoreError, ScriptsActiveModel, ScriptsColumn,
    ScriptsEntity,
};

mod source_revision {
    use chrono::{DateTime, Utc};
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "alloy_script_revisions")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub script_id: Uuid,
        pub tenant_id: Uuid,
        pub revision: i32,
        pub parent_revision: Option<i32>,
        pub source_digest: String,
        pub workspace: Json,
        pub author_id: Option<String>,
        pub source_provenance: Json,
        pub parent_release_slug: Option<String>,
        pub parent_release_version: Option<String>,
        pub parent_release_digest: Option<String>,
        pub created_at: DateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[derive(Debug, Clone)]
pub struct ScriptPresentationAuthoringMutation {
    pub source_locale: RuntimeLocale,
    /// `None` creates the concrete-locale source row; `Some` performs CAS.
    pub expected_copy_revision: Option<i64>,
    pub description: Option<String>,
}

#[derive(Debug, Error)]
pub enum ScriptAuthoringStoreError {
    #[error("Alloy script was not found")]
    NotFound,
    #[error("Alloy script name already exists in this tenant")]
    DuplicateName,
    #[error("Alloy script revision conflict; expected {expected}")]
    RevisionConflict { expected: u32 },
    #[error("Alloy script presentation already exists")]
    PresentationAlreadyExists,
    #[error("Alloy script presentation revision conflict; expected {expected}")]
    PresentationRevisionConflict { expected: i64 },
    #[error("Alloy script authoring presentation mutation is inconsistent")]
    InvalidPresentationMutation,
    #[error("Alloy script authoring storage failed: {0}")]
    Storage(String),
}

impl From<DbErr> for ScriptAuthoringStoreError {
    fn from(error: DbErr) -> Self {
        Self::Storage(error.to_string())
    }
}

impl From<ScriptPresentationStoreError> for ScriptAuthoringStoreError {
    fn from(error: ScriptPresentationStoreError) -> Self {
        match error {
            ScriptPresentationStoreError::NotFound => Self::NotFound,
            ScriptPresentationStoreError::AlreadyExists => Self::PresentationAlreadyExists,
            ScriptPresentationStoreError::RevisionConflict { expected } => {
                Self::PresentationRevisionConflict { expected }
            }
            ScriptPresentationStoreError::InvalidStoredLocale => {
                Self::Storage("presentation store returned an invalid locale".to_string())
            }
            ScriptPresentationStoreError::Storage(message) => Self::Storage(message),
        }
    }
}

/// Durable writer for canonical Alloy authoring commands.
///
/// Runtime/lifecycle callers continue to use [`super::ScriptRegistry`]. This
/// store exists specifically because canonical authoring may mutate executable
/// Script state and localized presentation copy in one database transaction.
#[derive(Clone)]
pub struct SeaOrmScriptAuthoringStore {
    db: DatabaseConnection,
    tenant_id: Uuid,
    presentations: SeaOrmScriptPresentationStore,
}

impl SeaOrmScriptAuthoringStore {
    pub fn new(db: DatabaseConnection, tenant_id: Uuid) -> Self {
        Self {
            presentations: SeaOrmScriptPresentationStore::new(db.clone()),
            db,
            tenant_id,
        }
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub async fn create(
        &self,
        mut script: Script,
        presentation: Option<ScriptPresentationAuthoringMutation>,
    ) -> Result<Script, ScriptAuthoringStoreError> {
        self.ensure_owned(&script)?;
        validate_script(&script)?;
        validate_create_presentation(&script, presentation.as_ref())?;

        let now = Utc::now();
        script.version = 1;
        script.created_at = now;
        script.updated_at = now;
        let transaction = self.db.begin().await?;

        if ScriptsEntity::find()
            .filter(ScriptsColumn::TenantId.eq(self.tenant_id))
            .filter(ScriptsColumn::Name.eq(script.name.clone()))
            .one(&transaction)
            .await?
            .is_some()
        {
            return Err(ScriptAuthoringStoreError::DuplicateName);
        }

        script_active_model(&script)?.insert(&transaction).await?;
        insert_source_revision(&transaction, &script, None).await?;

        if let Some(presentation) = presentation {
            self.presentations
                .create_source_in_transaction(
                    &transaction,
                    self.tenant_id,
                    script.id,
                    presentation.source_locale,
                    presentation.description,
                )
                .await?;
        }

        transaction.commit().await?;
        Ok(script)
    }

    pub async fn update(
        &self,
        previous: &Script,
        mut next: Script,
        presentation: Option<ScriptPresentationAuthoringMutation>,
    ) -> Result<Script, ScriptAuthoringStoreError> {
        self.ensure_owned(previous)?;
        self.ensure_owned(&next)?;
        validate_script(&next)?;
        validate_update_presentation(previous, &next, presentation.as_ref())?;
        if previous.id != next.id || previous.tenant_id != next.tenant_id {
            return Err(ScriptAuthoringStoreError::NotFound);
        }
        if previous.parent_release != next.parent_release {
            return Err(ScriptAuthoringStoreError::Storage(
                "a draft cannot replace or remove its imported parent release".to_string(),
            ));
        }
        if previous.version != next.version {
            return Err(ScriptAuthoringStoreError::RevisionConflict {
                expected: previous.version,
            });
        }

        let expected_revision = i32::try_from(previous.version).map_err(|_| {
            ScriptAuthoringStoreError::RevisionConflict {
                expected: previous.version,
            }
        })?;
        if expected_revision <= 0 {
            return Err(ScriptAuthoringStoreError::RevisionConflict {
                expected: previous.version,
            });
        }
        let next_revision = expected_revision.checked_add(1).ok_or_else(|| {
            ScriptAuthoringStoreError::Storage("script revision overflow".to_string())
        })?;
        next.version = u32::try_from(next_revision)
            .map_err(|_| ScriptAuthoringStoreError::Storage("script revision overflow".into()))?;
        next.updated_at = Utc::now();

        let transaction = self.db.begin().await?;
        let current = ScriptsEntity::find_by_id(previous.id)
            .filter(ScriptsColumn::TenantId.eq(self.tenant_id))
            .one(&transaction)
            .await?
            .ok_or(ScriptAuthoringStoreError::NotFound)?;
        if current.version != expected_revision {
            return Err(ScriptAuthoringStoreError::RevisionConflict {
                expected: previous.version,
            });
        }
        if current.name != next.name
            && ScriptsEntity::find()
                .filter(ScriptsColumn::TenantId.eq(self.tenant_id))
                .filter(ScriptsColumn::Name.eq(next.name.clone()))
                .filter(ScriptsColumn::Id.ne(next.id))
                .one(&transaction)
                .await?
                .is_some()
        {
            return Err(ScriptAuthoringStoreError::DuplicateName);
        }

        let (trigger_type, trigger_config) = trigger_to_parts(&next.trigger);
        let updated = ScriptsEntity::update_many()
            .col_expr(ScriptsColumn::Name, Expr::value(next.name.clone()))
            .col_expr(
                ScriptsColumn::Description,
                Expr::value(next.description.clone()),
            )
            .col_expr(
                ScriptsColumn::Workspace,
                Expr::value(workspace_to_json(&next.workspace)?),
            )
            .col_expr(ScriptsColumn::TriggerType, Expr::value(trigger_type))
            .col_expr(ScriptsColumn::TriggerConfig, Expr::value(trigger_config))
            .col_expr(ScriptsColumn::Status, Expr::value(next.status.as_str()))
            .col_expr(ScriptsColumn::Version, Expr::value(next_revision))
            .col_expr(ScriptsColumn::RunAsSystem, Expr::value(next.run_as_system))
            .col_expr(
                ScriptsColumn::Permissions,
                Expr::value(permissions_to_json(&next.permissions)),
            )
            .col_expr(
                ScriptsColumn::AuthorId,
                Expr::value(next.author_id.clone()),
            )
            .col_expr(
                ScriptsColumn::SourceProvenance,
                Expr::value(source_provenance_to_json(&next.source_provenance)?),
            )
            .col_expr(
                ScriptsColumn::ErrorCount,
                Expr::value(i32::try_from(next.error_count).map_err(|_| {
                    ScriptAuthoringStoreError::Storage("script error count exceeds i32".into())
                })?),
            )
            .col_expr(
                ScriptsColumn::LastErrorAt,
                Expr::value(next.last_error_at),
            )
            .col_expr(ScriptsColumn::UpdatedAt, Expr::value(next.updated_at))
            .filter(ScriptsColumn::Id.eq(next.id))
            .filter(ScriptsColumn::TenantId.eq(self.tenant_id))
            .filter(ScriptsColumn::Version.eq(expected_revision))
            .exec(&transaction)
            .await?;
        if updated.rows_affected != 1 {
            return Err(ScriptAuthoringStoreError::RevisionConflict {
                expected: previous.version,
            });
        }

        ensure_source_revision(&transaction, previous).await?;
        insert_source_revision(&transaction, &next, Some(expected_revision)).await?;

        if let Some(presentation) = presentation {
            match presentation.expected_copy_revision {
                Some(expected_copy_revision) => {
                    self.presentations
                        .compare_and_set_source_in_transaction(
                            &transaction,
                            self.tenant_id,
                            next.id,
                            &presentation.source_locale,
                            expected_copy_revision,
                            presentation.description,
                        )
                        .await?;
                }
                None => {
                    self.presentations
                        .create_source_in_transaction(
                            &transaction,
                            self.tenant_id,
                            next.id,
                            presentation.source_locale,
                            presentation.description,
                        )
                        .await?;
                }
            }
        }

        transaction.commit().await?;
        Ok(next)
    }

    fn ensure_owned(&self, script: &Script) -> Result<(), ScriptAuthoringStoreError> {
        (script.tenant_id == self.tenant_id)
            .then_some(())
            .ok_or(ScriptAuthoringStoreError::NotFound)
    }
}

fn validate_create_presentation(
    script: &Script,
    presentation: Option<&ScriptPresentationAuthoringMutation>,
) -> Result<(), ScriptAuthoringStoreError> {
    match presentation {
        Some(mutation)
            if mutation.expected_copy_revision.is_none()
                && mutation.description == script.description =>
        {
            Ok(())
        }
        None if script.description.is_none() => Ok(()),
        _ => Err(ScriptAuthoringStoreError::InvalidPresentationMutation),
    }
}

fn validate_update_presentation(
    previous: &Script,
    next: &Script,
    presentation: Option<&ScriptPresentationAuthoringMutation>,
) -> Result<(), ScriptAuthoringStoreError> {
    match presentation {
        Some(mutation) if mutation.description == next.description => Ok(()),
        None if previous.description == next.description => Ok(()),
        _ => Err(ScriptAuthoringStoreError::InvalidPresentationMutation),
    }
}

fn validate_script(script: &Script) -> Result<(), ScriptAuthoringStoreError> {
    script
        .workspace
        .validate()
        .map_err(|error| ScriptAuthoringStoreError::Storage(error.to_string()))?;
    script
        .source_provenance
        .validate()
        .map_err(|error| ScriptAuthoringStoreError::Storage(error.to_string()))?;
    if let Some(release) = &script.parent_release {
        release
            .validate()
            .map_err(|error| ScriptAuthoringStoreError::Storage(error.to_string()))?;
    }
    Ok(())
}

fn trigger_to_parts(trigger: &ScriptTrigger) -> (String, serde_json::Value) {
    match trigger {
        ScriptTrigger::Event { entity_type, event } => (
            "event".to_string(),
            serde_json::json!({
                "entity_type": entity_type,
                "event": event.as_str(),
            }),
        ),
        ScriptTrigger::Cron { expression } => (
            "cron".to_string(),
            serde_json::json!({ "expression": expression }),
        ),
        ScriptTrigger::Manual => ("manual".to_string(), serde_json::json!({})),
        ScriptTrigger::Api { path, method } => (
            "api".to_string(),
            serde_json::json!({
                "path": path,
                "method": method.as_str(),
            }),
        ),
    }
}

fn permissions_to_json(permissions: &[String]) -> serde_json::Value {
    serde_json::Value::Array(
        permissions
            .iter()
            .map(|value| serde_json::Value::String(value.clone()))
            .collect(),
    )
}

fn workspace_to_json(
    workspace: &RhaiWorkspace,
) -> Result<serde_json::Value, ScriptAuthoringStoreError> {
    workspace
        .validate()
        .map_err(|error| ScriptAuthoringStoreError::Storage(error.to_string()))?;
    serde_json::to_value(workspace)
        .map_err(|error| ScriptAuthoringStoreError::Storage(error.to_string()))
}

fn source_provenance_to_json(
    provenance: &SourceProvenance,
) -> Result<serde_json::Value, ScriptAuthoringStoreError> {
    provenance
        .validate()
        .map_err(|error| ScriptAuthoringStoreError::Storage(error.to_string()))?;
    serde_json::to_value(provenance)
        .map_err(|error| ScriptAuthoringStoreError::Storage(error.to_string()))
}

fn script_active_model(
    script: &Script,
) -> Result<ScriptsActiveModel, ScriptAuthoringStoreError> {
    let (trigger_type, trigger_config) = trigger_to_parts(&script.trigger);
    Ok(ScriptsActiveModel {
        id: Set(script.id),
        tenant_id: Set(script.tenant_id),
        name: Set(script.name.clone()),
        description: Set(script.description.clone()),
        workspace: Set(workspace_to_json(&script.workspace)?),
        trigger_type: Set(trigger_type),
        trigger_config: Set(trigger_config),
        status: Set(script.status.as_str().to_string()),
        version: Set(i32::try_from(script.version).map_err(|_| {
            ScriptAuthoringStoreError::Storage("script revision exceeds i32".into())
        })?),
        run_as_system: Set(script.run_as_system),
        permissions: Set(permissions_to_json(&script.permissions)),
        author_id: Set(script.author_id.clone()),
        source_provenance: Set(source_provenance_to_json(&script.source_provenance)?),
        parent_release_slug: Set(script
            .parent_release
            .as_ref()
            .map(|release| release.slug.clone())),
        parent_release_version: Set(script
            .parent_release
            .as_ref()
            .map(|release| release.version.clone())),
        parent_release_digest: Set(script
            .parent_release
            .as_ref()
            .map(|release| release.digest.clone())),
        error_count: Set(i32::try_from(script.error_count).map_err(|_| {
            ScriptAuthoringStoreError::Storage("script error count exceeds i32".into())
        })?),
        last_error_at: Set(script.last_error_at),
        created_at: Set(script.created_at),
        updated_at: Set(script.updated_at),
    })
}

async fn ensure_source_revision(
    transaction: &sea_orm::DatabaseTransaction,
    script: &Script,
) -> Result<(), ScriptAuthoringStoreError> {
    let revision = i32::try_from(script.version).map_err(|_| {
        ScriptAuthoringStoreError::Storage("script revision exceeds i32".into())
    })?;
    if source_revision::Entity::find()
        .filter(source_revision::Column::ScriptId.eq(script.id))
        .filter(source_revision::Column::Revision.eq(revision))
        .one(transaction)
        .await?
        .is_none()
    {
        insert_source_revision(
            transaction,
            script,
            revision.checked_sub(1).filter(|parent| *parent > 0),
        )
        .await?;
    }
    Ok(())
}

async fn insert_source_revision(
    transaction: &sea_orm::DatabaseTransaction,
    script: &Script,
    parent_revision: Option<i32>,
) -> Result<(), ScriptAuthoringStoreError> {
    let revision = i32::try_from(script.version).map_err(|_| {
        ScriptAuthoringStoreError::Storage("script revision exceeds i32".into())
    })?;
    let source_digest = script
        .workspace
        .digest()
        .map_err(|error| ScriptAuthoringStoreError::Storage(error.to_string()))?;
    source_revision::ActiveModel {
        id: Set(Uuid::new_v4()),
        script_id: Set(script.id),
        tenant_id: Set(script.tenant_id),
        revision: Set(revision),
        parent_revision: Set(parent_revision),
        source_digest: Set(source_digest),
        workspace: Set(workspace_to_json(&script.workspace)?),
        author_id: Set(script.author_id.clone()),
        source_provenance: Set(source_provenance_to_json(&script.source_provenance)?),
        parent_release_slug: Set(script
            .parent_release
            .as_ref()
            .map(|release| release.slug.clone())),
        parent_release_version: Set(script
            .parent_release
            .as_ref()
            .map(|release| release.version.clone())),
        parent_release_digest: Set(script
            .parent_release
            .as_ref()
            .map(|release| release.digest.clone())),
        created_at: Set(script.updated_at),
    }
    .insert(transaction)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RhaiWorkspace, ScriptStatus, ScriptTrigger, SourceProvenance};
    use rustok_api::StoredLocale;
    use sea_orm::Database;
    use sea_orm_migration::prelude::SchemaManager;

    async fn fixture() -> (
        SeaOrmScriptAuthoringStore,
        SeaOrmScriptPresentationStore,
        Uuid,
    ) {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("sqlite memory database should connect");
        let manager = SchemaManager::new(&database);
        for migration in crate::migrations::migrations() {
            migration
                .up(&manager)
                .await
                .expect("Alloy migrations should apply");
        }
        let tenant_id = Uuid::new_v4();
        (
            SeaOrmScriptAuthoringStore::new(database.clone(), tenant_id),
            SeaOrmScriptPresentationStore::new(database),
            tenant_id,
        )
    }

    fn script(tenant_id: Uuid, name: &str, description: Option<&str>) -> Script {
        let mut script = Script::new(
            name,
            RhaiWorkspace::single_source("40 + 2"),
            ScriptTrigger::Manual,
        );
        script.tenant_id = tenant_id;
        script.description = description.map(str::to_string);
        script.author_id = Some("owner:author".to_string());
        script.source_provenance = SourceProvenance::http("alloy_create_script");
        script
    }

    #[tokio::test]
    async fn create_commits_script_source_revision_and_presentation_atomically() {
        let (store, presentations, tenant_id) = fixture().await;
        let script = script(tenant_id, "localized", Some("Owner description"));
        let script_id = script.id;
        let locale = RuntimeLocale::new("en").expect("concrete locale");
        let saved = store
            .create(
                script,
                Some(ScriptPresentationAuthoringMutation {
                    source_locale: locale.clone(),
                    expected_copy_revision: None,
                    description: Some("Owner description".to_string()),
                }),
            )
            .await
            .expect("authoring create should commit");

        assert_eq!(saved.version, 1);
        assert_eq!(saved.description.as_deref(), Some("Owner description"));
        let presentation = presentations
            .find_exact(tenant_id, script_id, &StoredLocale::from(locale))
            .await
            .expect("presentation lookup should succeed")
            .expect("presentation should exist");
        assert_eq!(presentation.copy_revision, 1);
        assert_eq!(presentation.description, saved.description);
        assert!(
            source_revision::Entity::find()
                .filter(source_revision::Column::ScriptId.eq(script_id))
                .filter(source_revision::Column::Revision.eq(1))
                .one(presentations.connection())
                .await
                .expect("source revision lookup should succeed")
                .is_some()
        );
    }

    #[tokio::test]
    async fn inline_description_without_presentation_is_rejected() {
        let (store, _, tenant_id) = fixture().await;
        assert!(matches!(
            store
                .create(script(tenant_id, "unlocalized", Some("No locale")), None)
                .await,
            Err(ScriptAuthoringStoreError::InvalidPresentationMutation)
        ));
    }

    #[tokio::test]
    async fn presentation_conflict_rolls_back_operational_script_update() {
        let (store, presentations, tenant_id) = fixture().await;
        let locale = RuntimeLocale::new("en").expect("concrete locale");
        let created = store
            .create(
                script(tenant_id, "atomic", Some("First")),
                Some(ScriptPresentationAuthoringMutation {
                    source_locale: locale.clone(),
                    expected_copy_revision: None,
                    description: Some("First".to_string()),
                }),
            )
            .await
            .expect("authoring create should commit");
        let mut next = created.clone();
        next.status = ScriptStatus::Active;
        next.description = Some("Second".to_string());
        next.source_provenance = SourceProvenance::http("alloy_update_script");

        assert!(matches!(
            store
                .update(
                    &created,
                    next,
                    Some(ScriptPresentationAuthoringMutation {
                        source_locale: locale,
                        expected_copy_revision: Some(99),
                        description: Some("Second".to_string()),
                    }),
                )
                .await,
            Err(ScriptAuthoringStoreError::PresentationRevisionConflict { expected: 99 })
        ));

        let stored = ScriptsEntity::find_by_id(created.id)
            .one(presentations.connection())
            .await
            .expect("script lookup should succeed")
            .expect("script should remain");
        assert_eq!(stored.version, 1);
        assert_eq!(stored.status, ScriptStatus::Draft.as_str());
        assert_eq!(stored.description.as_deref(), Some("First"));
    }

    #[tokio::test]
    async fn operational_only_update_does_not_require_presentation_mutation() {
        let (store, presentations, tenant_id) = fixture().await;
        let created = store
            .create(script(tenant_id, "operational", None), None)
            .await
            .expect("script without copy should create");
        let mut next = created.clone();
        next.status = ScriptStatus::Active;
        next.source_provenance = SourceProvenance::http("alloy_update_script");
        let updated = store
            .update(&created, next, None)
            .await
            .expect("operational-only update should commit");
        assert_eq!(updated.version, 2);
        assert_eq!(updated.status, ScriptStatus::Active);
        assert!(
            source_revision::Entity::find()
                .filter(source_revision::Column::ScriptId.eq(updated.id))
                .filter(source_revision::Column::Revision.eq(2))
                .one(presentations.connection())
                .await
                .expect("source revision lookup should succeed")
                .is_some()
        );
    }
}
