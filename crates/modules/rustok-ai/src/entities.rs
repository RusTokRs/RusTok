#[cfg(feature = "server")]
pub mod ai_approval_requests {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "ai_approval_requests")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub session_id: Uuid,
        pub run_id: Uuid,
        pub approval_batch_id: String,
        pub tool_name: String,
        pub tool_call_id: String,
        pub tool_input: Json,
        pub reason: Option<String>,
        pub status: String,
        pub resolved_by: Option<Uuid>,
        pub resolved_at: Option<DateTimeWithTimeZone>,
        pub metadata: Json,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "server")]
pub mod ai_agent_principals {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "ai_agent_principals")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub slug: String,
        pub descriptor_owner: String,
        pub descriptor_slug: String,
        pub role_slugs: Json,
        pub permission_slugs: Json,
        pub is_active: bool,
        pub metadata: Json,
        pub created_by: Option<Uuid>,
        pub updated_by: Option<Uuid>,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "server")]
pub mod ai_agent_model_assignments {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "ai_agent_model_assignments")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub agent_principal_id: Uuid,
        pub provider_profile_id: Uuid,
        pub model_override: Option<String>,
        pub execution_mode: String,
        pub is_active: bool,
        pub metadata: Json,
        pub created_by: Option<Uuid>,
        pub updated_by: Option<Uuid>,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "server")]
pub mod ai_agent_workflow_runs {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "ai_agent_workflow_runs")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub workflow_owner: String,
        pub workflow_slug: String,
        pub initiator_id: Uuid,
        pub status: String,
        pub input_payload: Json,
        pub output_payload: Option<Json>,
        pub metadata: Json,
        pub created_at: DateTimeWithTimeZone,
        pub started_at: Option<DateTimeWithTimeZone>,
        pub completed_at: Option<DateTimeWithTimeZone>,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "server")]
pub mod ai_agent_workflow_stages {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "ai_agent_workflow_stages")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub workflow_run_id: Uuid,
        pub stage_id: String,
        pub agent_principal_id: Uuid,
        pub model_assignment_id: Option<Uuid>,
        pub run_id: Option<Uuid>,
        pub status: String,
        pub requires_approval: bool,
        pub input_payload: Json,
        pub output_payload: Option<Json>,
        pub error_message: Option<String>,
        pub metadata: Json,
        pub lease_token: Option<Uuid>,
        pub lease_expires_at: Option<DateTimeWithTimeZone>,
        pub attempt_count: i32,
        pub created_at: DateTimeWithTimeZone,
        pub started_at: Option<DateTimeWithTimeZone>,
        pub completed_at: Option<DateTimeWithTimeZone>,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "server")]
pub mod ai_chat_messages {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "ai_chat_messages")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub session_id: Uuid,
        pub run_id: Option<Uuid>,
        pub role: String,
        pub content: Option<String>,
        pub name: Option<String>,
        pub tool_call_id: Option<String>,
        pub tool_calls: Json,
        pub metadata: Json,
        pub created_by: Option<Uuid>,
        pub created_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "server")]
pub mod ai_chat_runs {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "ai_chat_runs")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub session_id: Uuid,
        pub provider_profile_id: Uuid,
        pub task_profile_id: Option<Uuid>,
        pub tool_profile_id: Option<Uuid>,
        pub status: String,
        pub model: String,
        pub execution_mode: String,
        pub execution_path: String,
        pub requested_locale: Option<String>,
        pub resolved_locale: String,
        pub temperature: Option<f32>,
        pub max_tokens: Option<i32>,
        pub error_message: Option<String>,
        pub pending_approval_id: Option<Uuid>,
        pub decision_trace: Json,
        pub metadata: Json,
        pub created_at: DateTimeWithTimeZone,
        pub started_at: DateTimeWithTimeZone,
        pub completed_at: Option<DateTimeWithTimeZone>,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "server")]
pub mod ai_chat_sessions {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "ai_chat_sessions")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub title: String,
        pub provider_profile_id: Uuid,
        pub task_profile_id: Option<Uuid>,
        pub tool_profile_id: Option<Uuid>,
        pub execution_mode: String,
        pub requested_locale: Option<String>,
        pub resolved_locale: String,
        pub status: String,
        pub created_by: Option<Uuid>,
        pub metadata: Json,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "server")]
pub mod ai_provider_profiles {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "ai_provider_profiles")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub slug: String,
        pub display_name: String,
        pub provider_slug: String,
        pub provider_target_id: String,
        pub model: String,
        pub credential_refs: Json,
        pub temperature: Option<f32>,
        pub max_tokens: Option<i32>,
        pub is_active: bool,
        pub capabilities: Json,
        pub allowed_task_profiles: Json,
        pub denied_task_profiles: Json,
        pub restricted_role_slugs: Json,
        pub metadata: Json,
        pub created_by: Option<Uuid>,
        pub updated_by: Option<Uuid>,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "server")]
pub mod ai_task_profiles {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "ai_task_profiles")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub slug: String,
        pub display_name: String,
        pub description: Option<String>,
        pub target_capability: String,
        pub system_prompt: Option<String>,
        pub allowed_provider_profile_ids: Json,
        pub preferred_provider_profile_ids: Json,
        pub fallback_strategy: String,
        pub tool_profile_id: Option<Uuid>,
        pub approval_policy: Json,
        pub default_execution_mode: String,
        pub is_active: bool,
        pub metadata: Json,
        pub created_by: Option<Uuid>,
        pub updated_by: Option<Uuid>,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "server")]
pub mod ai_tool_profiles {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "ai_tool_profiles")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub slug: String,
        pub display_name: String,
        pub description: Option<String>,
        pub allowed_tools: Json,
        pub denied_tools: Json,
        pub sensitive_tools: Json,
        pub is_active: bool,
        pub metadata: Json,
        pub created_by: Option<Uuid>,
        pub updated_by: Option<Uuid>,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "server")]
pub mod ai_tool_traces {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "ai_tool_traces")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub session_id: Uuid,
        pub run_id: Uuid,
        pub tool_name: String,
        pub status: String,
        pub input_payload: Json,
        pub output_payload: Option<Json>,
        pub error_message: Option<String>,
        pub duration_ms: Option<i64>,
        pub sensitive: bool,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "server")]
pub mod ai_structured_executions {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "ai_structured_executions")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub owner: String,
        pub task_slug: String,
        pub idempotency_key: String,
        pub request_digest: String,
        pub prompt_policy_digest: String,
        pub input_schema_digest: String,
        pub input_digest: String,
        pub output_schema_digest: String,
        pub classification: String,
        pub evidence_digest: String,
        pub input_bytes: i64,
        pub max_output_bytes: i64,
        pub max_attempts: i32,
        pub status: String,
        pub actor_kind: String,
        pub actor_id: String,
        pub correlation_id: String,
        pub causation_id: Option<String>,
        pub traceparent: Option<String>,
        pub error_code: Option<String>,
        pub retryable: bool,
        pub retry_after_ms: Option<i64>,
        pub lease_token: Option<Uuid>,
        pub lease_expires_at: Option<DateTimeWithTimeZone>,
        pub cancel_requested_at: Option<DateTimeWithTimeZone>,
        pub cancel_idempotency_key: Option<String>,
        pub cancel_request_digest: Option<String>,
        pub cancel_actor_kind: Option<String>,
        pub cancel_actor_id: Option<String>,
        pub created_at: DateTimeWithTimeZone,
        pub started_at: Option<DateTimeWithTimeZone>,
        pub completed_at: Option<DateTimeWithTimeZone>,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "server")]
pub mod ai_structured_cancellation_intents {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "ai_structured_cancellation_intents")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub owner: String,
        pub execution_idempotency_key: String,
        pub cancellation_idempotency_key: String,
        pub request_digest: String,
        pub actor_kind: String,
        pub actor_id: String,
        pub created_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "server")]
pub mod ai_structured_attempts {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "ai_structured_attempts")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub execution_id: Uuid,
        pub attempt: i32,
        pub provider_profile_id: Uuid,
        pub provider_slug: String,
        pub model: String,
        pub fallback: bool,
        pub status: String,
        pub price_snapshot_digest: String,
        pub currency_code: String,
        pub input_cost_per_million_minor: i64,
        pub output_cost_per_million_minor: i64,
        pub input_tokens: Option<i64>,
        pub output_tokens: Option<i64>,
        pub total_tokens: Option<i64>,
        pub cost_minor_units: Option<i64>,
        pub error_code: Option<String>,
        pub retryable: bool,
        pub retry_after_ms: Option<i64>,
        pub created_at: DateTimeWithTimeZone,
        pub started_at: DateTimeWithTimeZone,
        pub completed_at: Option<DateTimeWithTimeZone>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "server")]
pub mod ai_structured_budgets {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "ai_structured_budgets")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub currency_code: String,
        pub limit_minor_units: i64,
        pub reserved_minor_units: i64,
        pub committed_minor_units: i64,
        pub max_concurrent: i32,
        pub in_flight: i32,
        pub revision: i64,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "server")]
pub mod ai_structured_provider_policies {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "ai_structured_provider_policies")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub provider_profile_id: Uuid,
        pub allowed_classifications: Json,
        pub currency_code: String,
        pub input_cost_per_million_minor: i64,
        pub output_cost_per_million_minor: i64,
        pub max_concurrent: i32,
        pub in_flight: i32,
        pub is_active: bool,
        pub revision: i64,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "server")]
pub mod ai_structured_reservations {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "ai_structured_reservations")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub execution_id: Uuid,
        pub budget_id: Uuid,
        pub currency_code: String,
        pub reserved_minor_units: i64,
        pub committed_minor_units: i64,
        pub state: String,
        pub revision: i64,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(feature = "server")]
pub mod ai_structured_results {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "ai_structured_results")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub execution_id: Uuid,
        pub request_digest: String,
        pub output_digest: String,
        pub key_id: String,
        pub nonce: Vec<u8>,
        pub ciphertext: Vec<u8>,
        pub plaintext_bytes: i64,
        pub replay_count: i64,
        pub created_at: DateTimeWithTimeZone,
        pub expires_at: DateTimeWithTimeZone,
        pub last_replayed_at: Option<DateTimeWithTimeZone>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}
