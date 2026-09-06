use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

pub mod audit_entry {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "group_audit_entries")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub group_id: Uuid,
        pub actor_user_id: Option<Uuid>,
        pub action: String,
        pub target_user_id: Option<Uuid>,
        pub details: Json,
        pub correlation_id: String,
        pub created_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod command_receipt {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "group_command_receipts")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub group_id: Uuid,
        pub actor_user_id: Uuid,
        pub idempotency_key: String,
        pub command_type: String,
        pub request_hash: String,
        pub response: Json,
        pub created_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}
