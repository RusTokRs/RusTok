use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;

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
    pub source_provenance: Json,
    pub parent_release_slug: Option<String>,
    pub parent_release_version: Option<String>,
    pub parent_release_digest: Option<String>,
    pub error_count: i32,
    pub last_error_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

pub mod draft_revision {
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
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::Entity",
            from = "Column::ScriptId",
            to = "super::Column::Id"
        )]
        Script,
    }

    impl Related<super::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Script.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod draft_tombstone {
    use chrono::{DateTime, Utc};
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "alloy_script_tombstones")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub deleted_at: DateTime<Utc>,
        pub deleted_by: String,
        pub delete_reason: String,
        pub idempotency_key: Uuid,
        pub request_digest: String,
        pub retention_policy: String,
        pub retain_until: Option<DateTime<Utc>>,
        pub retention_revision: i32,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod draft_retention_receipt {
    use chrono::{DateTime, Utc};
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "alloy_script_retention_receipts")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub script_id: Uuid,
        pub tenant_id: Uuid,
        pub action: String,
        pub actor_id: String,
        pub idempotency_key: Uuid,
        pub request_digest: String,
        pub deletion_request_digest: String,
        pub retention_policy: String,
        pub retain_until: Option<DateTime<Utc>>,
        pub retention_revision: i32,
        pub recorded_at: DateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod draft_purge_receipt {
    use chrono::{DateTime, Utc};
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "alloy_script_purge_receipts")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub script_id: Uuid,
        pub tenant_id: Uuid,
        pub retention_policy: String,
        pub retain_until: DateTime<Utc>,
        pub purged_at: DateTime<Utc>,
        pub source_revision_count: i32,
        pub review_count: i32,
        pub test_run_count: i32,
        pub deletion_request_digest: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod draft_review {
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
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::Entity",
            from = "Column::ScriptId",
            to = "super::Column::Id"
        )]
        Script,
    }

    impl Related<super::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Script.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod draft_test_run {
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

pub mod release_import {
    use chrono::{DateTime, Utc};
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "alloy_release_imports")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub idempotency_key: Uuid,
        pub request_digest: String,
        pub script_id: Uuid,
        pub parent_release_slug: String,
        pub parent_release_version: String,
        pub parent_release_digest: String,
        pub created_at: DateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod component_candidate {
    use chrono::{DateTime, Utc};
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "alloy_component_candidates")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub idempotency_key: Uuid,
        pub request_digest: String,
        pub script_id: Uuid,
        pub parent_revision: i32,
        pub parent_source_digest: String,
        pub parent_release_slug: String,
        pub parent_release_version: String,
        pub parent_release_digest: String,
        pub workspace: Json,
        pub source_digest: String,
        pub scenario_digest: String,
        pub actor_id: String,
        pub created_at: DateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod component_candidate_review {
    use chrono::{DateTime, Utc};
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "alloy_component_candidate_reviews")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub candidate_id: Uuid,
        pub tenant_id: Uuid,
        pub source_digest: String,
        pub scenario_digest: String,
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

pub mod component_candidate_build {
    use chrono::{DateTime, Utc};
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "alloy_component_candidate_builds")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub candidate_id: Uuid,
        pub tenant_id: Uuid,
        pub candidate_source_digest: String,
        pub scenario_digest: String,
        pub archive_source_digest: String,
        pub build_request_id: Uuid,
        pub source_reference: String,
        pub actor_id: Uuid,
        pub idempotency_key: Uuid,
        pub request_digest: String,
        pub created_at: DateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod component_candidate_build_execution {
    use chrono::{DateTime, Utc};
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "alloy_component_candidate_build_executions")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub candidate_build_id: Uuid,
        pub candidate_id: Uuid,
        pub tenant_id: Uuid,
        pub candidate_source_digest: String,
        pub scenario_digest: String,
        pub archive_source_digest: String,
        pub build_request_id: Uuid,
        pub source_reference: String,
        pub build_result_revision: i64,
        pub component_digest: String,
        pub sbom_digest: String,
        pub provenance_digest: String,
        pub publication: Json,
        pub scenario_comparison: Json,
        pub created_at: DateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}
