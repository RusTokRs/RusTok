use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ActiveValue, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, TransactionTrait,
};

use crate::error::{ScriptError, ScriptResult};
use crate::model::{
    validate_transition, AlloyWorkspace, EventType, HttpMethod, ReviewCommand, ReviewDecision,
    ReviewStatus, Script, ScriptId, ScriptSourceRevision, ScriptStatus, ScriptTrigger, TestCommand,
    TestRun, TestRunClaim, TestRunCompletion, TestRunLease, TestRunStatus,
};
use crate::storage::{ScriptPage, ScriptQuery, ScriptRegistry};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "scripts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub workspace: Json,
    pub trigger_type: String,
    pub trigger_config: Json,
    pub status: String,
    pub version: i32,
    pub run_as_system: bool,
    pub permissions: Json,
    pub author_id: Option<String>,
    pub error_count: i32,
    pub last_error_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

mod draft_revision {
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
        pub created_at: DateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

mod draft_review {
    use chrono::{DateTime, Utc};
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "alloy_script_reviews")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub script_id: Uuid,
        pub tenant_id: Uuid,
        pub revision: i32,
        pub source_digest: String,
        pub status: String,
        pub policy_revision: String,
        pub actor_id: String,
        pub reason: Option<String>,
        pub idempotency_key: Uuid,
        pub request_digest: String,
        pub created_at: DateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

mod draft_test_run {
    use chrono::{DateTime, Utc};
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "alloy_script_test_runs")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub script_id: Uuid,
        pub tenant_id: Uuid,
        pub revision: i32,
        pub source_digest: String,
        pub test_path: String,
        pub actor_id: String,
        pub idempotency_key: Uuid,
        pub request_digest: String,
        pub status: String,
        pub passed: Option<bool>,
        pub error: Option<String>,
        pub lease_token: Option<Uuid>,
        pub lease_expires_at: Option<DateTime<Utc>>,
        pub created_at: DateTime<Utc>,
        pub completed_at: Option<DateTime<Utc>>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[derive(Clone)]
pub struct SeaOrmStorage {
    db: DatabaseConnection,
    tenant_id: Option<Uuid>,
}

impl SeaOrmStorage {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            tenant_id: None,
        }
    }

    pub fn with_tenant(db: DatabaseConnection, tenant_id: Uuid) -> Self {
        Self {
            db,
            tenant_id: Some(tenant_id),
        }
    }

    pub fn for_tenant(&self, tenant_id: Uuid) -> Self {
        Self {
            db: self.db.clone(),
            tenant_id: Some(tenant_id),
        }
    }

    fn scoped_by_id(&self, id: ScriptId) -> sea_orm::Select<Entity> {
        let select = Entity::find_by_id(id);
        if let Some(tenant_id) = self.tenant_id {
            select.filter(Column::TenantId.eq(tenant_id))
        } else {
            select
        }
    }

    fn ensure_script_scope(&self, script: &Script) -> ScriptResult<()> {
        if let Some(tenant_id) = self.tenant_id {
            if script.tenant_id != tenant_id {
                return Err(ScriptError::NotFound {
                    name: script.id.to_string(),
                });
            }
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

    fn trigger_from_parts(
        trigger_type: &str,
        trigger_config: &serde_json::Value,
    ) -> ScriptResult<ScriptTrigger> {
        match trigger_type {
            "event" => {
                let entity_type = trigger_config
                    .get("entity_type")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string();
                let event_str = trigger_config
                    .get("event")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default();
                let event = EventType::parse(event_str)
                    .ok_or_else(|| ScriptError::InvalidTrigger(format!("event: {event_str}")))?;
                Ok(ScriptTrigger::Event { entity_type, event })
            }
            "cron" => {
                let expression = trigger_config
                    .get("expression")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string();
                Ok(ScriptTrigger::Cron { expression })
            }
            "manual" => Ok(ScriptTrigger::Manual),
            "api" => {
                let path = trigger_config
                    .get("path")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string();
                let method_str = trigger_config
                    .get("method")
                    .and_then(|value| value.as_str())
                    .unwrap_or("GET");
                let method = HttpMethod::parse(method_str)
                    .ok_or_else(|| ScriptError::InvalidTrigger(format!("method: {method_str}")))?;
                Ok(ScriptTrigger::Api { path, method })
            }
            _ => Err(ScriptError::InvalidTrigger(trigger_type.to_string())),
        }
    }

    fn status_from_str(value: &str) -> ScriptResult<ScriptStatus> {
        match value {
            "draft" => Ok(ScriptStatus::Draft),
            "active" => Ok(ScriptStatus::Active),
            "paused" => Ok(ScriptStatus::Paused),
            "disabled" => Ok(ScriptStatus::Disabled),
            "archived" => Ok(ScriptStatus::Archived),
            _ => Err(ScriptError::InvalidStatus(value.to_string())),
        }
    }

    fn model_to_script(model: Model) -> ScriptResult<Script> {
        let trigger = Self::trigger_from_parts(&model.trigger_type, &model.trigger_config)?;
        let status = Self::status_from_str(&model.status)?;
        let permissions = model
            .permissions
            .as_array()
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let workspace: AlloyWorkspace =
            serde_json::from_value(model.workspace).map_err(|error| {
                ScriptError::InvalidWorkspace(format!("stored workspace is invalid: {error}"))
            })?;
        workspace.validate().map_err(ScriptError::from)?;

        Ok(Script {
            id: model.id,
            tenant_id: model.tenant_id,
            name: model.name,
            description: model.description,
            workspace,
            trigger,
            status,
            version: model.version.max(1) as u32,
            run_as_system: model.run_as_system,
            permissions,
            author_id: model.author_id,
            created_at: model.created_at,
            updated_at: model.updated_at,
            error_count: model.error_count.max(0) as u32,
            last_error_at: model.last_error_at,
        })
    }

    fn permissions_to_json(permissions: &[String]) -> serde_json::Value {
        serde_json::Value::Array(
            permissions
                .iter()
                .map(|value| serde_json::Value::String(value.clone()))
                .collect(),
        )
    }

    fn workspace_to_json(workspace: &AlloyWorkspace) -> ScriptResult<serde_json::Value> {
        workspace.validate().map_err(ScriptError::from)?;
        serde_json::to_value(workspace)
            .map_err(|error| ScriptError::InvalidWorkspace(error.to_string()))
    }

    fn source_digest(workspace: &AlloyWorkspace) -> ScriptResult<String> {
        workspace.digest().map_err(ScriptError::from)
    }

    fn model_to_source_revision(
        model: draft_revision::Model,
    ) -> ScriptResult<ScriptSourceRevision> {
        let revision = u32::try_from(model.revision).map_err(|_| {
            ScriptError::Storage("durable source revision is outside the supported range".into())
        })?;
        if revision == 0 {
            return Err(ScriptError::Storage(
                "durable source revision must be positive".into(),
            ));
        }
        let parent_revision = model
            .parent_revision
            .map(u32::try_from)
            .transpose()
            .map_err(|_| {
                ScriptError::Storage(
                    "durable source parent revision is outside the supported range".into(),
                )
            })?;
        if parent_revision.is_some_and(|parent| parent == 0 || parent >= revision) {
            return Err(ScriptError::Storage(
                "durable source parent revision must precede its child revision".into(),
            ));
        }

        let workspace: AlloyWorkspace =
            serde_json::from_value(model.workspace).map_err(|error| {
                ScriptError::InvalidWorkspace(format!(
                    "stored revision workspace is invalid: {error}"
                ))
            })?;
        workspace.validate().map_err(ScriptError::from)?;

        Ok(ScriptSourceRevision {
            script_id: model.script_id,
            tenant_id: model.tenant_id,
            revision,
            parent_revision,
            source_digest: model.source_digest,
            workspace,
            author_id: model.author_id,
            created_at: model.created_at,
        })
    }

    fn model_to_review_decision(model: draft_review::Model) -> ScriptResult<ReviewDecision> {
        let revision = u32::try_from(model.revision).map_err(|_| {
            ScriptError::Storage("durable review revision is outside the supported range".into())
        })?;
        let status = ReviewStatus::parse(&model.status).ok_or_else(|| {
            ScriptError::Storage(format!("stored review status is invalid: {}", model.status))
        })?;
        Ok(ReviewDecision {
            id: model.id,
            script_id: model.script_id,
            tenant_id: model.tenant_id,
            revision,
            source_digest: model.source_digest,
            status,
            policy_revision: model.policy_revision,
            actor_id: model.actor_id,
            reason: model.reason,
            idempotency_key: model.idempotency_key,
            request_digest: model.request_digest,
            created_at: model.created_at,
        })
    }

    fn model_to_test_run(model: draft_test_run::Model) -> ScriptResult<TestRun> {
        let revision = u32::try_from(model.revision).map_err(|_| {
            ScriptError::Storage("durable test revision is outside the supported range".into())
        })?;
        if revision == 0 {
            return Err(ScriptError::Storage(
                "durable test revision must be positive".into(),
            ));
        }
        let status = TestRunStatus::parse(&model.status).ok_or_else(|| {
            ScriptError::Storage(format!("stored test status is invalid: {}", model.status))
        })?;
        match status {
            TestRunStatus::Pending
                if model.passed.is_some()
                    || model.error.is_some()
                    || model.completed_at.is_some()
                    || model.lease_token.is_none()
                    || model.lease_expires_at.is_none() =>
            {
                return Err(ScriptError::Storage(
                    "pending test run has invalid terminal or lease fields".into(),
                ));
            }
            TestRunStatus::Passed
                if model.passed != Some(true)
                    || model.error.is_some()
                    || model.completed_at.is_none()
                    || model.lease_token.is_some()
                    || model.lease_expires_at.is_some() =>
            {
                return Err(ScriptError::Storage(
                    "passed test run has invalid result or lease fields".into(),
                ));
            }
            TestRunStatus::Failed
                if model.passed != Some(false)
                    || model.completed_at.is_none()
                    || model.lease_token.is_some()
                    || model.lease_expires_at.is_some() =>
            {
                return Err(ScriptError::Storage(
                    "failed test run has invalid result or lease fields".into(),
                ));
            }
            _ => {}
        }
        Ok(TestRun {
            id: model.id,
            script_id: model.script_id,
            tenant_id: model.tenant_id,
            revision,
            source_digest: model.source_digest,
            test_path: model.test_path,
            actor_id: model.actor_id,
            idempotency_key: model.idempotency_key,
            request_digest: model.request_digest,
            status,
            passed: model.passed,
            error: model.error,
            created_at: model.created_at,
            completed_at: model.completed_at,
        })
    }

    async fn source_for_test_run(
        transaction: &sea_orm::DatabaseTransaction,
        script_id: ScriptId,
        tenant_id: Uuid,
        revision: i32,
        test_path: &str,
    ) -> ScriptResult<ScriptSourceRevision> {
        let model = draft_revision::Entity::find()
            .filter(draft_revision::Column::ScriptId.eq(script_id))
            .filter(draft_revision::Column::TenantId.eq(tenant_id))
            .filter(draft_revision::Column::Revision.eq(revision))
            .one(transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .ok_or_else(|| ScriptError::NotFound {
                name: format!("{script_id}@{revision}"),
            })?;
        let source = Self::model_to_source_revision(model)?;
        source.workspace.validate_rhai_test(test_path)?;
        Ok(source)
    }

    async fn insert_revision_snapshot(
        transaction: &sea_orm::DatabaseTransaction,
        script: &Script,
        parent_revision: Option<i32>,
    ) -> ScriptResult<()> {
        let revision = i32::try_from(script.version).map_err(|_| {
            ScriptError::Storage("script revision exceeds the durable revision range".into())
        })?;
        draft_revision::ActiveModel {
            id: ActiveValue::Set(Uuid::new_v4()),
            script_id: ActiveValue::Set(script.id),
            tenant_id: ActiveValue::Set(script.tenant_id),
            revision: ActiveValue::Set(revision),
            parent_revision: ActiveValue::Set(parent_revision),
            source_digest: ActiveValue::Set(Self::source_digest(&script.workspace)?),
            workspace: ActiveValue::Set(Self::workspace_to_json(&script.workspace)?),
            author_id: ActiveValue::Set(script.author_id.clone()),
            created_at: ActiveValue::Set(script.updated_at),
        }
        .insert(transaction)
        .await
        .map_err(|error| ScriptError::Storage(error.to_string()))?;
        Ok(())
    }

    async fn ensure_revision_snapshot(
        transaction: &sea_orm::DatabaseTransaction,
        script: &Script,
    ) -> ScriptResult<()> {
        let revision = i32::try_from(script.version).map_err(|_| {
            ScriptError::Storage("script revision exceeds the durable revision range".into())
        })?;
        let existing = draft_revision::Entity::find()
            .filter(draft_revision::Column::ScriptId.eq(script.id))
            .filter(draft_revision::Column::Revision.eq(revision))
            .one(transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        if existing.is_none() {
            Self::insert_revision_snapshot(
                transaction,
                script,
                revision.checked_sub(1).filter(|parent| *parent > 0),
            )
            .await?;
        }
        Ok(())
    }

    fn apply_query(
        select: sea_orm::Select<Entity>,
        query: ScriptQuery,
        tenant_id: Option<Uuid>,
    ) -> sea_orm::Select<Entity> {
        let select = match query {
            ScriptQuery::ById(id) => select.filter(Column::Id.eq(id)),
            ScriptQuery::ByName(name) => select.filter(Column::Name.eq(name)),
            ScriptQuery::ByEvent { entity_type, event } => select
                .filter(Column::TriggerType.eq("event"))
                .filter(Column::Status.eq(ScriptStatus::Active.as_str()))
                .filter(Expr::cust_with_values(
                    "trigger_config->>'entity_type' = $1",
                    [entity_type],
                ))
                .filter(Expr::cust_with_values(
                    "trigger_config->>'event' = $1",
                    [event.as_str()],
                )),
            ScriptQuery::ByApiPath(path) => select
                .filter(Column::TriggerType.eq("api"))
                .filter(Column::Status.eq(ScriptStatus::Active.as_str()))
                .filter(Expr::cust_with_values(
                    "trigger_config->>'path' = $1",
                    [path],
                )),
            ScriptQuery::Scheduled => select
                .filter(Column::TriggerType.eq("cron"))
                .filter(Column::Status.eq(ScriptStatus::Active.as_str())),
            ScriptQuery::ByStatus(status) => select.filter(Column::Status.eq(status.as_str())),
            ScriptQuery::All => select,
        };

        if let Some(tid) = tenant_id {
            select.filter(Column::TenantId.eq(tid))
        } else {
            select
        }
    }
}

#[async_trait::async_trait]
impl ScriptRegistry for SeaOrmStorage {
    async fn find(&self, query: ScriptQuery) -> ScriptResult<Vec<Script>> {
        let select = Self::apply_query(Entity::find(), query, self.tenant_id);
        let models = select
            .order_by_asc(Column::Name)
            .all(&self.db)
            .await
            .map_err(|err| ScriptError::Storage(err.to_string()))?;

        models.into_iter().map(Self::model_to_script).collect()
    }

    async fn find_paginated(
        &self,
        query: ScriptQuery,
        offset: u64,
        limit: u64,
    ) -> ScriptResult<ScriptPage> {
        let total = Self::apply_query(Entity::find(), query.clone(), self.tenant_id)
            .order_by_asc(Column::Name)
            .count(&self.db)
            .await
            .map_err(|err| ScriptError::Storage(err.to_string()))?;

        let models = Self::apply_query(Entity::find(), query, self.tenant_id)
            .order_by_asc(Column::Name)
            .offset(offset)
            .limit(limit)
            .all(&self.db)
            .await
            .map_err(|err| ScriptError::Storage(err.to_string()))?;

        let items: ScriptResult<Vec<Script>> =
            models.into_iter().map(Self::model_to_script).collect();

        Ok(ScriptPage {
            items: items?,
            total,
        })
    }

    async fn get(&self, id: ScriptId) -> ScriptResult<Script> {
        let model = self
            .scoped_by_id(id)
            .one(&self.db)
            .await
            .map_err(|err| ScriptError::Storage(err.to_string()))?
            .ok_or_else(|| ScriptError::NotFound {
                name: id.to_string(),
            })?;

        Self::model_to_script(model)
    }

    async fn get_source_revision(
        &self,
        id: ScriptId,
        revision: u32,
    ) -> ScriptResult<ScriptSourceRevision> {
        let revision = i32::try_from(revision).map_err(|_| {
            ScriptError::Storage("requested source revision is outside the durable range".into())
        })?;
        let mut query = draft_revision::Entity::find()
            .filter(draft_revision::Column::ScriptId.eq(id))
            .filter(draft_revision::Column::Revision.eq(revision));
        if let Some(tenant_id) = self.tenant_id {
            query = query.filter(draft_revision::Column::TenantId.eq(tenant_id));
        }
        let model = query
            .one(&self.db)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .ok_or_else(|| ScriptError::NotFound {
                name: format!("{id}@{revision}"),
            })?;

        Self::model_to_source_revision(model)
    }

    async fn list_source_revisions(&self, id: ScriptId) -> ScriptResult<Vec<ScriptSourceRevision>> {
        let mut query =
            draft_revision::Entity::find().filter(draft_revision::Column::ScriptId.eq(id));
        if let Some(tenant_id) = self.tenant_id {
            query = query.filter(draft_revision::Column::TenantId.eq(tenant_id));
        }
        let models = query
            .order_by_asc(draft_revision::Column::Revision)
            .all(&self.db)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;

        models
            .into_iter()
            .map(Self::model_to_source_revision)
            .collect()
    }

    async fn review(&self, command: ReviewCommand) -> ScriptResult<ReviewDecision> {
        command.validate()?;
        let request_digest = command.request_digest()?;
        let revision = i32::try_from(command.expected_revision).map_err(|_| {
            ScriptError::RevisionConflict {
                expected: command.expected_revision,
            }
        })?;
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;

        let mut existing_query = draft_review::Entity::find()
            .filter(draft_review::Column::ScriptId.eq(command.script_id))
            .filter(draft_review::Column::Revision.eq(revision))
            .filter(draft_review::Column::IdempotencyKey.eq(command.idempotency_key));
        if let Some(tenant_id) = self.tenant_id {
            existing_query = existing_query.filter(draft_review::Column::TenantId.eq(tenant_id));
        }
        if let Some(existing) = existing_query
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
        {
            let existing = Self::model_to_review_decision(existing)?;
            if existing.request_digest == request_digest {
                transaction
                    .commit()
                    .await
                    .map_err(|error| ScriptError::Storage(error.to_string()))?;
                return Ok(existing);
            }
            return Err(crate::model::ReviewError::IdempotencyConflict.into());
        }

        let mut script_query = Entity::find_by_id(command.script_id);
        if let Some(tenant_id) = self.tenant_id {
            script_query = script_query.filter(Column::TenantId.eq(tenant_id));
        }
        let script_model = script_query
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .ok_or_else(|| ScriptError::NotFound {
                name: command.script_id.to_string(),
            })?;
        if script_model.version != revision {
            return Err(ScriptError::RevisionConflict {
                expected: command.expected_revision,
            });
        }

        let source_revision = draft_revision::Entity::find()
            .filter(draft_revision::Column::ScriptId.eq(command.script_id))
            .filter(draft_revision::Column::Revision.eq(revision))
            .filter(draft_revision::Column::TenantId.eq(script_model.tenant_id))
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .ok_or_else(|| ScriptError::NotFound {
                name: format!("{}@{}", command.script_id, command.expected_revision),
            })?;

        // Acquire the current script row through the same revision-CAS predicate
        // before reading review state. The no-op update serializes concurrent
        // review decisions and workspace saves without changing the revision.
        let mut assert_current = Entity::update_many()
            .col_expr(Column::UpdatedAt, Expr::col(Column::UpdatedAt))
            .filter(Column::Id.eq(command.script_id))
            .filter(Column::Version.eq(revision));
        if let Some(tenant_id) = self.tenant_id {
            assert_current = assert_current.filter(Column::TenantId.eq(tenant_id));
        }
        let asserted = assert_current
            .exec(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        if asserted.rows_affected != 1 {
            return Err(ScriptError::RevisionConflict {
                expected: command.expected_revision,
            });
        }

        let existing = draft_review::Entity::find()
            .filter(draft_review::Column::ScriptId.eq(command.script_id))
            .filter(draft_review::Column::Revision.eq(revision))
            .filter(draft_review::Column::TenantId.eq(script_model.tenant_id))
            .filter(draft_review::Column::IdempotencyKey.eq(command.idempotency_key))
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        if let Some(existing) = existing {
            let existing = Self::model_to_review_decision(existing)?;
            if existing.request_digest == request_digest {
                transaction
                    .commit()
                    .await
                    .map_err(|error| ScriptError::Storage(error.to_string()))?;
                return Ok(existing);
            }
            return Err(crate::model::ReviewError::IdempotencyConflict.into());
        }

        let current = draft_review::Entity::find()
            .filter(draft_review::Column::ScriptId.eq(command.script_id))
            .filter(draft_review::Column::Revision.eq(revision))
            .filter(draft_review::Column::TenantId.eq(script_model.tenant_id))
            .order_by_desc(draft_review::Column::CreatedAt)
            .order_by_desc(draft_review::Column::Id)
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .map(Self::model_to_review_decision)
            .transpose()?;
        validate_transition(current.map(|decision| decision.status), command.status)?;

        let decision = ReviewDecision {
            id: Uuid::new_v4(),
            script_id: command.script_id,
            tenant_id: script_model.tenant_id,
            revision: command.expected_revision,
            source_digest: source_revision.source_digest,
            status: command.status,
            policy_revision: command.policy_revision,
            actor_id: command.actor_id,
            reason: command.reason,
            idempotency_key: command.idempotency_key,
            request_digest,
            created_at: Utc::now(),
        };
        draft_review::ActiveModel {
            id: ActiveValue::Set(decision.id),
            script_id: ActiveValue::Set(decision.script_id),
            tenant_id: ActiveValue::Set(decision.tenant_id),
            revision: ActiveValue::Set(revision),
            source_digest: ActiveValue::Set(decision.source_digest.clone()),
            status: ActiveValue::Set(decision.status.as_str().to_string()),
            policy_revision: ActiveValue::Set(decision.policy_revision.clone()),
            actor_id: ActiveValue::Set(decision.actor_id.clone()),
            reason: ActiveValue::Set(decision.reason.clone()),
            idempotency_key: ActiveValue::Set(decision.idempotency_key),
            request_digest: ActiveValue::Set(decision.request_digest.clone()),
            created_at: ActiveValue::Set(decision.created_at),
        }
        .insert(&transaction)
        .await
        .map_err(|error| ScriptError::Storage(error.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        Ok(decision)
    }

    async fn list_reviews(&self, id: ScriptId, revision: u32) -> ScriptResult<Vec<ReviewDecision>> {
        let revision = i32::try_from(revision).map_err(|_| {
            ScriptError::Storage("requested review revision is outside the durable range".into())
        })?;
        let mut query = draft_review::Entity::find()
            .filter(draft_review::Column::ScriptId.eq(id))
            .filter(draft_review::Column::Revision.eq(revision));
        if let Some(tenant_id) = self.tenant_id {
            query = query.filter(draft_review::Column::TenantId.eq(tenant_id));
        }
        query
            .order_by_asc(draft_review::Column::CreatedAt)
            .order_by_asc(draft_review::Column::Id)
            .all(&self.db)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .into_iter()
            .map(Self::model_to_review_decision)
            .collect()
    }

    async fn claim_test_run(&self, command: TestCommand) -> ScriptResult<TestRunClaim> {
        command.validate()?;
        let request_digest = command.request_digest()?;
        let revision = i32::try_from(command.expected_revision).map_err(|_| {
            ScriptError::RevisionConflict {
                expected: command.expected_revision,
            }
        })?;
        let now = Utc::now();
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        let mut existing_query = draft_test_run::Entity::find()
            .filter(draft_test_run::Column::ScriptId.eq(command.script_id))
            .filter(draft_test_run::Column::Revision.eq(revision))
            .filter(draft_test_run::Column::IdempotencyKey.eq(command.idempotency_key));
        if let Some(tenant_id) = self.tenant_id {
            existing_query = existing_query.filter(draft_test_run::Column::TenantId.eq(tenant_id));
        }
        if let Some(existing) = existing_query
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
        {
            let existing_run = Self::model_to_test_run(existing.clone())?;
            if existing_run.request_digest != request_digest {
                return Err(crate::model::TestRunError::IdempotencyConflict.into());
            }
            if existing_run.status.is_terminal() {
                transaction
                    .commit()
                    .await
                    .map_err(|error| ScriptError::Storage(error.to_string()))?;
                return Ok(TestRunClaim::Replay(existing_run));
            }
            if existing
                .lease_expires_at
                .is_some_and(|expires_at| expires_at > now)
            {
                transaction
                    .commit()
                    .await
                    .map_err(|error| ScriptError::Storage(error.to_string()))?;
                return Ok(TestRunClaim::InProgress(existing_run));
            }
            let lease_token = Uuid::new_v4();
            let recovered = draft_test_run::Entity::update_many()
                .col_expr(
                    draft_test_run::Column::LeaseToken,
                    Expr::value(Some(lease_token)),
                )
                .col_expr(
                    draft_test_run::Column::LeaseExpiresAt,
                    Expr::value(Some(crate::test_run_lease_expires_at(now))),
                )
                .filter(draft_test_run::Column::Id.eq(existing.id))
                .filter(draft_test_run::Column::Status.eq(TestRunStatus::Pending.as_str()))
                .filter(draft_test_run::Column::LeaseExpiresAt.lte(now))
                .exec(&transaction)
                .await
                .map_err(|error| ScriptError::Storage(error.to_string()))?;
            if recovered.rows_affected != 1 {
                return Err(crate::model::TestRunError::LeaseLost.into());
            }
            let source = Self::source_for_test_run(
                &transaction,
                command.script_id,
                existing.tenant_id,
                revision,
                &command.test_path,
            )
            .await?;
            if source.source_digest != existing_run.source_digest {
                return Err(ScriptError::Storage(
                    "test run source digest does not match its immutable revision".into(),
                ));
            }
            transaction
                .commit()
                .await
                .map_err(|error| ScriptError::Storage(error.to_string()))?;
            return Ok(TestRunClaim::Claimed(TestRunLease {
                run: existing_run,
                lease_token,
                source,
            }));
        }

        let mut script_query = Entity::find_by_id(command.script_id);
        if let Some(tenant_id) = self.tenant_id {
            script_query = script_query.filter(Column::TenantId.eq(tenant_id));
        }
        let script = script_query
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .ok_or_else(|| ScriptError::NotFound {
                name: command.script_id.to_string(),
            })?;
        if script.version != revision {
            return Err(ScriptError::RevisionConflict {
                expected: command.expected_revision,
            });
        }
        let source = Self::source_for_test_run(
            &transaction,
            command.script_id,
            script.tenant_id,
            revision,
            &command.test_path,
        )
        .await?;

        // Serialize a new test claim with source saves through the same current
        // revision predicate, then recheck idempotency before inserting.
        let mut assert_current = Entity::update_many()
            .col_expr(Column::UpdatedAt, Expr::col(Column::UpdatedAt))
            .filter(Column::Id.eq(command.script_id))
            .filter(Column::Version.eq(revision));
        if let Some(tenant_id) = self.tenant_id {
            assert_current = assert_current.filter(Column::TenantId.eq(tenant_id));
        }
        if assert_current
            .exec(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .rows_affected
            != 1
        {
            return Err(ScriptError::RevisionConflict {
                expected: command.expected_revision,
            });
        }
        let existing = draft_test_run::Entity::find()
            .filter(draft_test_run::Column::ScriptId.eq(command.script_id))
            .filter(draft_test_run::Column::TenantId.eq(script.tenant_id))
            .filter(draft_test_run::Column::Revision.eq(revision))
            .filter(draft_test_run::Column::IdempotencyKey.eq(command.idempotency_key))
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        if existing.is_some() {
            return Err(crate::model::TestRunError::LeaseLost.into());
        }

        let lease_token = Uuid::new_v4();
        let run = TestRun {
            id: Uuid::new_v4(),
            script_id: command.script_id,
            tenant_id: script.tenant_id,
            revision: command.expected_revision,
            source_digest: source.source_digest.clone(),
            test_path: command.test_path,
            actor_id: command.actor_id,
            idempotency_key: command.idempotency_key,
            request_digest,
            status: TestRunStatus::Pending,
            passed: None,
            error: None,
            created_at: now,
            completed_at: None,
        };
        draft_test_run::ActiveModel {
            id: ActiveValue::Set(run.id),
            script_id: ActiveValue::Set(run.script_id),
            tenant_id: ActiveValue::Set(run.tenant_id),
            revision: ActiveValue::Set(revision),
            source_digest: ActiveValue::Set(run.source_digest.clone()),
            test_path: ActiveValue::Set(run.test_path.clone()),
            actor_id: ActiveValue::Set(run.actor_id.clone()),
            idempotency_key: ActiveValue::Set(run.idempotency_key),
            request_digest: ActiveValue::Set(run.request_digest.clone()),
            status: ActiveValue::Set(TestRunStatus::Pending.as_str().to_string()),
            passed: ActiveValue::Set(None),
            error: ActiveValue::Set(None),
            lease_token: ActiveValue::Set(Some(lease_token)),
            lease_expires_at: ActiveValue::Set(Some(crate::test_run_lease_expires_at(now))),
            created_at: ActiveValue::Set(now),
            completed_at: ActiveValue::Set(None),
        }
        .insert(&transaction)
        .await
        .map_err(|error| ScriptError::Storage(error.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        Ok(TestRunClaim::Claimed(TestRunLease {
            run,
            lease_token,
            source,
        }))
    }

    async fn complete_test_run(
        &self,
        run_id: Uuid,
        lease_token: Uuid,
        completion: TestRunCompletion,
    ) -> ScriptResult<TestRun> {
        completion.validate()?;
        let now = Utc::now();
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        let mut query = draft_test_run::Entity::find_by_id(run_id);
        if let Some(tenant_id) = self.tenant_id {
            query = query.filter(draft_test_run::Column::TenantId.eq(tenant_id));
        }
        let model = query
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .ok_or_else(|| ScriptError::NotFound {
                name: run_id.to_string(),
            })?;
        let existing = Self::model_to_test_run(model.clone())?;
        if existing.status.is_terminal() {
            transaction
                .commit()
                .await
                .map_err(|error| ScriptError::Storage(error.to_string()))?;
            return Ok(existing);
        }
        if model.lease_token != Some(lease_token)
            || model
                .lease_expires_at
                .map_or(true, |expires_at| expires_at <= now)
        {
            return Err(crate::model::TestRunError::LeaseLost.into());
        }
        let status = if completion.passed {
            TestRunStatus::Passed
        } else {
            TestRunStatus::Failed
        };
        let updated = draft_test_run::Entity::update_many()
            .col_expr(draft_test_run::Column::Status, Expr::value(status.as_str()))
            .col_expr(
                draft_test_run::Column::Passed,
                Expr::value(Some(completion.passed)),
            )
            .col_expr(
                draft_test_run::Column::Error,
                Expr::value(completion.error.clone()),
            )
            .col_expr(
                draft_test_run::Column::LeaseToken,
                Expr::value(None::<Uuid>),
            )
            .col_expr(
                draft_test_run::Column::LeaseExpiresAt,
                Expr::value(None::<DateTime<Utc>>),
            )
            .col_expr(draft_test_run::Column::CompletedAt, Expr::value(Some(now)))
            .filter(draft_test_run::Column::Id.eq(run_id))
            .filter(draft_test_run::Column::Status.eq(TestRunStatus::Pending.as_str()))
            .filter(draft_test_run::Column::LeaseToken.eq(lease_token))
            .filter(draft_test_run::Column::LeaseExpiresAt.gt(now))
            .exec(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        if updated.rows_affected != 1 {
            return Err(crate::model::TestRunError::LeaseLost.into());
        }
        let run = TestRun {
            status,
            passed: Some(completion.passed),
            error: completion.error,
            completed_at: Some(now),
            ..existing
        };
        transaction
            .commit()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        Ok(run)
    }

    async fn get_by_name(&self, name: &str) -> ScriptResult<Script> {
        let mut query = Entity::find().filter(Column::Name.eq(name));
        if let Some(tid) = self.tenant_id {
            query = query.filter(Column::TenantId.eq(tid));
        }
        let model = query
            .one(&self.db)
            .await
            .map_err(|err| ScriptError::Storage(err.to_string()))?
            .ok_or_else(|| ScriptError::NotFound {
                name: name.to_string(),
            })?;

        Self::model_to_script(model)
    }

    async fn save(&self, mut script: Script) -> ScriptResult<Script> {
        self.ensure_script_scope(&script)?;
        script.workspace.validate().map_err(ScriptError::from)?;
        let now = Utc::now();
        let (trigger_type, trigger_config) = Self::trigger_to_parts(&script.trigger);
        let permissions_json = Self::permissions_to_json(&script.permissions);

        if let Some(existing) = self
            .scoped_by_id(script.id)
            .one(&self.db)
            .await
            .map_err(|err| ScriptError::Storage(err.to_string()))?
        {
            let expected_revision =
                i32::try_from(script.version).map_err(|_| ScriptError::RevisionConflict {
                    expected: script.version,
                })?;
            if expected_revision <= 0 || existing.version != expected_revision {
                return Err(ScriptError::RevisionConflict {
                    expected: script.version,
                });
            }
            let next_revision = expected_revision
                .checked_add(1)
                .ok_or_else(|| ScriptError::Storage("script version overflow".into()))?;
            script.version = next_revision as u32;
            script.updated_at = now;

            let transaction = self
                .db
                .begin()
                .await
                .map_err(|error| ScriptError::Storage(error.to_string()))?;
            let mut update = Entity::update_many()
                .col_expr(Column::Name, Expr::value(script.name.clone()))
                .col_expr(Column::Description, Expr::value(script.description.clone()))
                .col_expr(
                    Column::Workspace,
                    Expr::value(Self::workspace_to_json(&script.workspace)?),
                )
                .col_expr(Column::TriggerType, Expr::value(trigger_type))
                .col_expr(Column::TriggerConfig, Expr::value(trigger_config))
                .col_expr(Column::Status, Expr::value(script.status.as_str()))
                .col_expr(Column::Version, Expr::value(next_revision))
                .col_expr(Column::RunAsSystem, Expr::value(script.run_as_system))
                .col_expr(Column::Permissions, Expr::value(permissions_json))
                .col_expr(Column::AuthorId, Expr::value(script.author_id.clone()))
                .col_expr(Column::ErrorCount, Expr::value(script.error_count as i32))
                .col_expr(Column::LastErrorAt, Expr::value(script.last_error_at))
                .col_expr(Column::UpdatedAt, Expr::value(script.updated_at))
                .filter(Column::Id.eq(script.id))
                .filter(Column::Version.eq(expected_revision));
            if let Some(tenant_id) = self.tenant_id {
                update = update.filter(Column::TenantId.eq(tenant_id));
            }
            let result = update
                .exec(&transaction)
                .await
                .map_err(|err| ScriptError::Storage(err.to_string()))?;
            if result.rows_affected != 1 {
                return Err(ScriptError::RevisionConflict {
                    expected: script.version.saturating_sub(1),
                });
            }
            let previous = Self::model_to_script(existing)?;
            Self::ensure_revision_snapshot(&transaction, &previous).await?;
            Self::insert_revision_snapshot(&transaction, &script, Some(expected_revision)).await?;
            transaction
                .commit()
                .await
                .map_err(|error| ScriptError::Storage(error.to_string()))?;
            return self.get(script.id).await;
        }

        script.version = 1;
        script.created_at = now;
        script.updated_at = now;

        let model = ActiveModel {
            id: ActiveValue::Set(script.id),
            tenant_id: ActiveValue::Set(script.tenant_id),
            name: ActiveValue::Set(script.name.clone()),
            description: ActiveValue::Set(script.description.clone()),
            workspace: ActiveValue::Set(Self::workspace_to_json(&script.workspace)?),
            trigger_type: ActiveValue::Set(trigger_type),
            trigger_config: ActiveValue::Set(trigger_config),
            status: ActiveValue::Set(script.status.as_str().to_string()),
            version: ActiveValue::Set(script.version as i32),
            run_as_system: ActiveValue::Set(script.run_as_system),
            permissions: ActiveValue::Set(permissions_json),
            author_id: ActiveValue::Set(script.author_id.clone()),
            error_count: ActiveValue::Set(script.error_count as i32),
            last_error_at: ActiveValue::Set(script.last_error_at),
            created_at: ActiveValue::Set(script.created_at),
            updated_at: ActiveValue::Set(script.updated_at),
        };

        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        model
            .insert(&transaction)
            .await
            .map_err(|err| ScriptError::Storage(err.to_string()))?;
        Self::insert_revision_snapshot(&transaction, &script, None).await?;
        transaction
            .commit()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;

        self.get(script.id).await
    }

    async fn delete(&self, id: ScriptId) -> ScriptResult<()> {
        let mut delete = Entity::delete_many().filter(Column::Id.eq(id));
        if let Some(tenant_id) = self.tenant_id {
            delete = delete.filter(Column::TenantId.eq(tenant_id));
        }
        let result = delete
            .exec(&self.db)
            .await
            .map_err(|err| ScriptError::Storage(err.to_string()))?;

        if result.rows_affected == 0 {
            return Err(ScriptError::NotFound {
                name: id.to_string(),
            });
        }

        Ok(())
    }

    async fn set_status(&self, id: ScriptId, status: ScriptStatus) -> ScriptResult<()> {
        let mut script = self.get(id).await?;
        script.status = status;
        self.save(script).await?;
        Ok(())
    }

    async fn record_error(&self, id: ScriptId) -> ScriptResult<bool> {
        let mut script = self.get(id).await?;
        let should_disable = script.register_error();
        let status = if should_disable {
            ScriptStatus::Disabled
        } else {
            script.status
        };
        script.status = status;
        self.save(script).await?;

        Ok(should_disable)
    }

    async fn reset_errors(&self, id: ScriptId) -> ScriptResult<()> {
        let mut script = self.get(id).await?;
        script.reset_errors();
        self.save(script).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Database;
    use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};

    async fn storage_with_script() -> (SeaOrmStorage, Uuid, Uuid, Script) {
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
        let owner_tenant = Uuid::new_v4();
        let other_tenant = Uuid::new_v4();
        let storage = SeaOrmStorage::new(database);
        let mut script = Script::new(
            "tenant-only",
            AlloyWorkspace::single_source("40 + 2"),
            ScriptTrigger::Manual,
        );
        script.tenant_id = owner_tenant;
        let script = storage.save(script).await.expect("script should save");
        (storage, owner_tenant, other_tenant, script)
    }

    #[tokio::test]
    async fn tenant_scoped_single_record_paths_hide_and_preserve_other_tenant_scripts() {
        let (storage, owner_tenant, other_tenant, script) = storage_with_script().await;
        let other = storage.for_tenant(other_tenant);

        assert!(matches!(
            other.get(script.id).await,
            Err(ScriptError::NotFound { .. })
        ));
        assert!(matches!(
            other.save(script.clone()).await,
            Err(ScriptError::NotFound { .. })
        ));
        assert!(matches!(
            other.delete(script.id).await,
            Err(ScriptError::NotFound { .. })
        ));
        assert!(matches!(
            other.set_status(script.id, ScriptStatus::Disabled).await,
            Err(ScriptError::NotFound { .. })
        ));
        assert!(matches!(
            other.record_error(script.id).await,
            Err(ScriptError::NotFound { .. })
        ));
        assert!(matches!(
            other.reset_errors(script.id).await,
            Err(ScriptError::NotFound { .. })
        ));

        let owner_script = storage
            .for_tenant(owner_tenant)
            .get(script.id)
            .await
            .expect("other tenant operations must not mutate the owner script");
        assert_eq!(owner_script.tenant_id, owner_tenant);
        assert_eq!(owner_script.status, ScriptStatus::Draft);
        assert_eq!(owner_script.error_count, 0);
    }

    #[tokio::test]
    async fn save_rejects_a_stale_revision_without_overwriting_the_current_script() {
        let (storage, owner_tenant, _, script) = storage_with_script().await;
        let owner = storage.for_tenant(owner_tenant);
        let stale = owner
            .get(script.id)
            .await
            .expect("stale script should load");
        let mut current = owner
            .get(script.id)
            .await
            .expect("current script should load");
        current.workspace = AlloyWorkspace::single_source("41 + 2");

        let updated = owner
            .save(current)
            .await
            .expect("current revision should save");

        assert_eq!(updated.version, 2);
        assert!(matches!(
            owner.save(stale).await,
            Err(ScriptError::RevisionConflict { expected: 1 })
        ));
        assert_eq!(
            owner
                .get(script.id)
                .await
                .expect("current script should remain available")
                .workspace
                .entrypoint_source()
                .expect("workspace source"),
            "41 + 2"
        );
    }

    #[tokio::test]
    async fn save_persists_immutable_source_revision_lineage() {
        let (storage, owner_tenant, _, script) = storage_with_script().await;
        let owner = storage.for_tenant(owner_tenant);
        let mut next = owner
            .get(script.id)
            .await
            .expect("current script should load");
        next.workspace = AlloyWorkspace::single_source("41 + 2");
        next.author_id = Some("author:next".into());
        let saved = owner.save(next).await.expect("next revision should save");

        let revisions = owner
            .list_source_revisions(script.id)
            .await
            .expect("owner revision ledger should load");

        assert_eq!(saved.version, 2);
        assert_eq!(revisions.len(), 2);
        assert_eq!(revisions[0].revision, 1);
        assert_eq!(revisions[0].parent_revision, None);
        assert_eq!(
            revisions[0]
                .workspace
                .entrypoint_source()
                .expect("workspace source"),
            "40 + 2"
        );
        assert_eq!(revisions[1].revision, 2);
        assert_eq!(revisions[1].parent_revision, Some(1));
        assert_eq!(
            revisions[1]
                .workspace
                .entrypoint_source()
                .expect("workspace source"),
            "41 + 2"
        );
        assert_eq!(revisions[1].author_id.as_deref(), Some("author:next"));
        assert_eq!(
            revisions[1].source_digest,
            SeaOrmStorage::source_digest(&AlloyWorkspace::single_source("41 + 2"))
                .expect("workspace digest")
        );
    }

    #[tokio::test]
    async fn source_revision_queries_are_tenant_scoped() {
        let (storage, owner_tenant, other_tenant, script) = storage_with_script().await;
        let owner = storage.for_tenant(owner_tenant);
        let other = storage.for_tenant(other_tenant);

        assert_eq!(
            owner
                .get_source_revision(script.id, 1)
                .await
                .expect("owner source revision should load")
                .workspace
                .entrypoint_source()
                .expect("workspace source"),
            "40 + 2"
        );
        assert!(matches!(
            other.get_source_revision(script.id, 1).await,
            Err(ScriptError::NotFound { .. })
        ));
        assert!(other
            .list_source_revisions(script.id)
            .await
            .expect("other tenant revision list should be empty")
            .is_empty());
    }
}
