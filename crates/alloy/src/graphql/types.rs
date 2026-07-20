use async_graphql::{Enum, InputObject, OneofObject, SimpleObject, Union};
use chrono::{DateTime, Utc};
use rustok_api::graphql::PageInfo;
use uuid::Uuid;

use crate::{
    context::ExecutionPhase,
    execution_log::ExecutionLogEntry,
    model::{
        AlloyWorkspace, EventType, HttpMethod, ReviewDecision, ReviewStatus, Script, ScriptStatus,
        ScriptTrigger, TestRun, TestRunStatus,
    },
};

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlScriptStatus {
    Draft,
    Active,
    Paused,
    Disabled,
    Archived,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlReviewStatus {
    ChangesRequested,
    Approved,
    Rejected,
    Archived,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlTestRunStatus {
    Pending,
    Passed,
    Failed,
}

impl From<TestRunStatus> for GqlTestRunStatus {
    fn from(status: TestRunStatus) -> Self {
        match status {
            TestRunStatus::Pending => Self::Pending,
            TestRunStatus::Passed => Self::Passed,
            TestRunStatus::Failed => Self::Failed,
        }
    }
}

impl From<GqlReviewStatus> for ReviewStatus {
    fn from(status: GqlReviewStatus) -> Self {
        match status {
            GqlReviewStatus::ChangesRequested => Self::ChangesRequested,
            GqlReviewStatus::Approved => Self::Approved,
            GqlReviewStatus::Rejected => Self::Rejected,
            GqlReviewStatus::Archived => Self::Archived,
        }
    }
}

impl From<ReviewStatus> for GqlReviewStatus {
    fn from(status: ReviewStatus) -> Self {
        match status {
            ReviewStatus::ChangesRequested => Self::ChangesRequested,
            ReviewStatus::Approved => Self::Approved,
            ReviewStatus::Rejected => Self::Rejected,
            ReviewStatus::Archived => Self::Archived,
        }
    }
}

impl From<ScriptStatus> for GqlScriptStatus {
    fn from(status: ScriptStatus) -> Self {
        match status {
            ScriptStatus::Draft => Self::Draft,
            ScriptStatus::Active => Self::Active,
            ScriptStatus::Paused => Self::Paused,
            ScriptStatus::Disabled => Self::Disabled,
            ScriptStatus::Archived => Self::Archived,
        }
    }
}

impl From<GqlScriptStatus> for ScriptStatus {
    fn from(status: GqlScriptStatus) -> Self {
        match status {
            GqlScriptStatus::Draft => Self::Draft,
            GqlScriptStatus::Active => Self::Active,
            GqlScriptStatus::Paused => Self::Paused,
            GqlScriptStatus::Disabled => Self::Disabled,
            GqlScriptStatus::Archived => Self::Archived,
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlEventType {
    BeforeCreate,
    AfterCreate,
    BeforeUpdate,
    AfterUpdate,
    BeforeDelete,
    AfterDelete,
    OnCommit,
}

impl From<EventType> for GqlEventType {
    fn from(event: EventType) -> Self {
        match event {
            EventType::BeforeCreate => Self::BeforeCreate,
            EventType::AfterCreate => Self::AfterCreate,
            EventType::BeforeUpdate => Self::BeforeUpdate,
            EventType::AfterUpdate => Self::AfterUpdate,
            EventType::BeforeDelete => Self::BeforeDelete,
            EventType::AfterDelete => Self::AfterDelete,
            EventType::OnCommit => Self::OnCommit,
        }
    }
}

impl From<GqlEventType> for EventType {
    fn from(event: GqlEventType) -> Self {
        match event {
            GqlEventType::BeforeCreate => Self::BeforeCreate,
            GqlEventType::AfterCreate => Self::AfterCreate,
            GqlEventType::BeforeUpdate => Self::BeforeUpdate,
            GqlEventType::AfterUpdate => Self::AfterUpdate,
            GqlEventType::BeforeDelete => Self::BeforeDelete,
            GqlEventType::AfterDelete => Self::AfterDelete,
            GqlEventType::OnCommit => Self::OnCommit,
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlHttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

impl From<HttpMethod> for GqlHttpMethod {
    fn from(method: HttpMethod) -> Self {
        match method {
            HttpMethod::GET => Self::Get,
            HttpMethod::POST => Self::Post,
            HttpMethod::PUT => Self::Put,
            HttpMethod::DELETE => Self::Delete,
        }
    }
}

impl From<GqlHttpMethod> for HttpMethod {
    fn from(method: GqlHttpMethod) -> Self {
        match method {
            GqlHttpMethod::Get => HttpMethod::GET,
            GqlHttpMethod::Post => HttpMethod::POST,
            GqlHttpMethod::Put => HttpMethod::PUT,
            GqlHttpMethod::Delete => HttpMethod::DELETE,
        }
    }
}

#[derive(SimpleObject)]
pub struct EventTrigger {
    pub entity_type: String,
    pub event: GqlEventType,
}

#[derive(SimpleObject)]
pub struct CronTrigger {
    pub expression: String,
}

#[derive(SimpleObject)]
pub struct ApiTrigger {
    pub path: String,
    pub method: GqlHttpMethod,
}

#[derive(SimpleObject)]
pub struct ManualTrigger {
    pub placeholder: bool,
}

#[derive(Union)]
pub enum GqlScriptTrigger {
    Event(EventTrigger),
    Cron(CronTrigger),
    Api(ApiTrigger),
    Manual(ManualTrigger),
}

impl From<ScriptTrigger> for GqlScriptTrigger {
    fn from(trigger: ScriptTrigger) -> Self {
        match trigger {
            ScriptTrigger::Event { entity_type, event } => Self::Event(EventTrigger {
                entity_type,
                event: event.into(),
            }),
            ScriptTrigger::Cron { expression } => Self::Cron(CronTrigger { expression }),
            ScriptTrigger::Manual => Self::Manual(ManualTrigger { placeholder: true }),
            ScriptTrigger::Api { path, method } => Self::Api(ApiTrigger {
                path,
                method: method.into(),
            }),
        }
    }
}

#[derive(SimpleObject)]
pub struct GqlScript {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub workspace: async_graphql::Json<AlloyWorkspace>,
    pub trigger: GqlScriptTrigger,
    pub status: GqlScriptStatus,
    pub version: u32,
    pub run_as_system: bool,
    pub permissions: Vec<String>,
    pub author_id: Option<String>,
    pub error_count: u32,
    pub last_error_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Script> for GqlScript {
    fn from(script: Script) -> Self {
        Self {
            id: script.id,
            tenant_id: script.tenant_id,
            name: script.name,
            description: script.description,
            workspace: async_graphql::Json(script.workspace),
            trigger: script.trigger.into(),
            status: script.status.into(),
            version: script.version,
            run_as_system: script.run_as_system,
            permissions: script.permissions,
            author_id: script.author_id,
            error_count: script.error_count,
            last_error_at: script.last_error_at,
            created_at: script.created_at,
            updated_at: script.updated_at,
        }
    }
}

#[derive(SimpleObject)]
pub struct GqlExecutionResult {
    pub execution_id: Uuid,
    pub success: bool,
    pub duration_ms: i64,
    pub error: Option<String>,
    pub return_value: Option<async_graphql::Json<serde_json::Value>>,
    pub changes: Option<async_graphql::Json<serde_json::Value>>,
}

#[derive(SimpleObject)]
pub struct GqlReviewDecision {
    pub id: Uuid,
    pub script_id: Uuid,
    pub revision: u32,
    pub source_digest: String,
    pub status: GqlReviewStatus,
    pub policy_revision: String,
    pub actor_id: String,
    pub reason: Option<String>,
    pub idempotency_key: Uuid,
    pub created_at: DateTime<Utc>,
}

impl From<ReviewDecision> for GqlReviewDecision {
    fn from(decision: ReviewDecision) -> Self {
        Self {
            id: decision.id,
            script_id: decision.script_id,
            revision: decision.revision,
            source_digest: decision.source_digest,
            status: decision.status.into(),
            policy_revision: decision.policy_revision,
            actor_id: decision.actor_id,
            reason: decision.reason,
            idempotency_key: decision.idempotency_key,
            created_at: decision.created_at,
        }
    }
}

#[derive(SimpleObject)]
pub struct GqlTestRun {
    pub id: Uuid,
    pub script_id: Uuid,
    pub revision: u32,
    pub source_digest: String,
    pub test_path: String,
    pub actor_id: String,
    pub idempotency_key: Uuid,
    pub status: GqlTestRunStatus,
    pub passed: Option<bool>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl From<TestRun> for GqlTestRun {
    fn from(run: TestRun) -> Self {
        Self {
            id: run.id,
            script_id: run.script_id,
            revision: run.revision,
            source_digest: run.source_digest,
            test_path: run.test_path,
            actor_id: run.actor_id,
            idempotency_key: run.idempotency_key,
            status: run.status.into(),
            passed: run.passed,
            error: run.error,
            created_at: run.created_at,
            completed_at: run.completed_at,
        }
    }
}

#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlExecutionPhase {
    Before,
    After,
    OnCommit,
    Manual,
    Scheduled,
}

impl From<ExecutionPhase> for GqlExecutionPhase {
    fn from(phase: ExecutionPhase) -> Self {
        match phase {
            ExecutionPhase::Before => Self::Before,
            ExecutionPhase::After => Self::After,
            ExecutionPhase::OnCommit => Self::OnCommit,
            ExecutionPhase::Manual => Self::Manual,
            ExecutionPhase::Scheduled => Self::Scheduled,
        }
    }
}

#[derive(SimpleObject)]
pub struct GqlExecutionLogEntry {
    pub id: Uuid,
    pub script_id: Uuid,
    pub script_name: String,
    pub phase: GqlExecutionPhase,
    pub outcome: String,
    pub duration_ms: i64,
    pub error: Option<String>,
    pub user_id: Option<String>,
    pub tenant_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

impl From<ExecutionLogEntry> for GqlExecutionLogEntry {
    fn from(entry: ExecutionLogEntry) -> Self {
        Self {
            id: entry.id,
            script_id: entry.script_id,
            script_name: entry.script_name,
            phase: entry.phase.into(),
            outcome: entry.outcome,
            duration_ms: entry.duration_ms,
            error: entry.error,
            user_id: entry.user_id,
            tenant_id: entry.tenant_id,
            created_at: entry.created_at,
        }
    }
}

#[derive(SimpleObject)]
pub struct GqlExecutionLogConnection {
    pub items: Vec<GqlExecutionLogEntry>,
    pub page_info: PageInfo,
}

#[derive(SimpleObject)]
pub struct GqlScriptConnection {
    pub items: Vec<GqlScript>,
    pub page_info: PageInfo,
}

#[derive(InputObject)]
pub struct EventTriggerInput {
    pub entity_type: String,
    pub event: GqlEventType,
}

#[derive(InputObject)]
pub struct CronTriggerInput {
    pub expression: String,
}

#[derive(InputObject)]
pub struct ApiTriggerInput {
    pub path: String,
    pub method: GqlHttpMethod,
}

#[derive(OneofObject)]
pub enum ScriptTriggerInput {
    Event(EventTriggerInput),
    Cron(CronTriggerInput),
    Api(ApiTriggerInput),
    Manual(bool),
}

impl From<ScriptTriggerInput> for ScriptTrigger {
    fn from(input: ScriptTriggerInput) -> Self {
        match input {
            ScriptTriggerInput::Event(event) => ScriptTrigger::Event {
                entity_type: event.entity_type,
                event: event.event.into(),
            },
            ScriptTriggerInput::Cron(cron) => ScriptTrigger::Cron {
                expression: cron.expression,
            },
            ScriptTriggerInput::Api(api) => ScriptTrigger::Api {
                path: api.path,
                method: api.method.into(),
            },
            ScriptTriggerInput::Manual(_) => ScriptTrigger::Manual,
        }
    }
}

#[derive(InputObject)]
pub struct CreateScriptInput {
    pub name: String,
    pub description: Option<String>,
    pub workspace: async_graphql::Json<AlloyWorkspace>,
    pub trigger: ScriptTriggerInput,
    pub status: Option<GqlScriptStatus>,
    #[graphql(default)]
    pub run_as_system: bool,
    #[graphql(default)]
    pub permissions: Vec<String>,
    pub author_id: Option<String>,
}

#[derive(InputObject)]
pub struct UpdateScriptInput {
    pub expected_version: u32,
    pub name: Option<String>,
    pub description: Option<String>,
    pub workspace: Option<async_graphql::Json<AlloyWorkspace>>,
    pub trigger: Option<ScriptTriggerInput>,
    pub status: Option<GqlScriptStatus>,
    pub run_as_system: Option<bool>,
    pub permissions: Option<Vec<String>>,
    pub author_id: Option<String>,
    #[graphql(default)]
    pub clear_author_id: bool,
}

#[derive(InputObject)]
pub struct RunScriptInput {
    pub script_name: String,
    pub expected_version: u32,
    pub params: Option<async_graphql::Json<serde_json::Value>>,
}

#[derive(InputObject)]
pub struct ReviewScriptInput {
    pub expected_version: u32,
    pub status: GqlReviewStatus,
    pub policy_revision: String,
    pub reason: Option<String>,
    pub idempotency_key: Uuid,
}

#[derive(InputObject)]
pub struct RunWorkspaceTestInput {
    pub expected_version: u32,
    pub test_path: String,
    pub idempotency_key: Uuid,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn execution_entry(phase: ExecutionPhase) -> ExecutionLogEntry {
        ExecutionLogEntry {
            id: Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap(),
            script_id: Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap(),
            script_name: "canonical_graphql_mapping".to_string(),
            phase,
            outcome: "failed".to_string(),
            duration_ms: 321,
            error: Some("sandbox timeout".to_string()),
            user_id: Some("operator-9".to_string()),
            tenant_id: Some(Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap()),
            created_at: Utc.with_ymd_and_hms(2026, 6, 19, 13, 30, 0).unwrap(),
        }
    }

    #[test]
    fn gql_execution_log_entry_preserves_canonical_transport_fields() {
        let gql = GqlExecutionLogEntry::from(execution_entry(ExecutionPhase::Scheduled));

        assert_eq!(
            gql.id,
            Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap()
        );
        assert_eq!(
            gql.script_id,
            Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap()
        );
        assert_eq!(gql.script_name, "canonical_graphql_mapping");
        assert_eq!(gql.phase, GqlExecutionPhase::Scheduled);
        assert_eq!(gql.outcome, "failed");
        assert_eq!(gql.duration_ms, 321);
        assert_eq!(gql.error.as_deref(), Some("sandbox timeout"));
        assert_eq!(gql.user_id.as_deref(), Some("operator-9"));
        assert_eq!(
            gql.tenant_id,
            Some(Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap())
        );
        assert_eq!(
            gql.created_at,
            Utc.with_ymd_and_hms(2026, 6, 19, 13, 30, 0).unwrap()
        );
    }

    #[test]
    fn gql_execution_phase_mapping_covers_runtime_phases() {
        let phases = [
            (ExecutionPhase::Before, GqlExecutionPhase::Before),
            (ExecutionPhase::After, GqlExecutionPhase::After),
            (ExecutionPhase::OnCommit, GqlExecutionPhase::OnCommit),
            (ExecutionPhase::Manual, GqlExecutionPhase::Manual),
            (ExecutionPhase::Scheduled, GqlExecutionPhase::Scheduled),
        ];

        for (phase, expected) in phases {
            assert_eq!(GqlExecutionPhase::from(phase), expected);
        }
    }
}
