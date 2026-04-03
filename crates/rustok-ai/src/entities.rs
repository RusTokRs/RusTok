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
        pub provider_kind: String,
        pub base_url: String,
        pub model: String,
        pub api_key_secret: Option<String>,
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
