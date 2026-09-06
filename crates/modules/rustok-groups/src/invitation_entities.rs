use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

pub mod invitation {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "group_invitations")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub group_id: Uuid,
        pub invited_by_user_id: Uuid,
        pub target_user_id: Option<Uuid>,
        pub token_hash: String,
        pub max_uses: i32,
        pub use_count: i32,
        pub expires_at: DateTimeWithTimeZone,
        pub revoked_at: Option<DateTimeWithTimeZone>,
        pub revoked_by_user_id: Option<Uuid>,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod redemption {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "group_invitation_redemptions")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub invitation_id: Uuid,
        pub group_id: Uuid,
        pub user_id: Uuid,
        pub redeemed_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}
