use async_graphql::{Enum, InputObject, OneofObject, SimpleObject, Union};
use chrono::{DateTime, Utc};
use rustok_api::graphql::PageInfo;
use uuid::Uuid;

use crate::{
    context::ExecutionPhase,
    execution_log::ExecutionLogEntry,
    model::{EventType, HttpMethod, Script, ScriptStatus, ScriptTrigger},
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
    pub code: String,
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
            code: script.code,
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

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
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
    pub code: String,
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
    pub name: Option<String>,
    pub description: Option<String>,
    pub code: Option<String>,
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
    pub params: Option<async_graphql::Json<serde_json::Value>>,
}
