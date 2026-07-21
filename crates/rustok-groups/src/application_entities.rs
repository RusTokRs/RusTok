use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

pub mod membership_policy {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "group_membership_policies")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub group_id: Uuid,
        pub revision: i64,
        pub enabled: bool,
        pub created_by_user_id: Uuid,
        pub updated_by_user_id: Uuid,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod membership_policy_translation {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "group_membership_policy_translations")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub policy_id: Uuid,
        pub locale: String,
        pub questions: Json,
        pub rules: Json,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod membership_application {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "group_membership_applications")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub group_id: Uuid,
        pub user_id: Uuid,
        pub policy_id: Uuid,
        pub policy_revision: i64,
        pub policy_locale: String,
        pub policy_snapshot: Json,
        pub answers: Json,
        pub acknowledged_rule_keys: Json,
        pub status: String,
        pub submitted_at: DateTimeWithTimeZone,
        pub reviewed_at: Option<DateTimeWithTimeZone>,
        pub reviewed_by_user_id: Option<Uuid>,
        pub review_note: Option<String>,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}
