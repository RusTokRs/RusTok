use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Read-focused mapping of `group_memberships` including the monotonic subject revision.
///
/// The legacy `entities::membership` mapping remains available while command owners migrate
/// incrementally. Both entities map the same owner table; new code that participates in
/// moderation/enforcement identity must use this revision-aware model.
pub mod membership_state {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "group_memberships")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub group_id: Uuid,
        pub user_id: Uuid,
        pub role: String,
        pub status: String,
        pub invited_by_user_id: Option<Uuid>,
        pub joined_at: Option<DateTimeWithTimeZone>,
        pub left_at: Option<DateTimeWithTimeZone>,
        pub revision: i64,
        pub metadata: Json,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// Groups-owned current enforcement projection.
///
/// This table contains only bounded state required by Groups access/lifecycle invariants.
/// Moderation reports, cases, policy snapshots, appeal state, and queue data remain owned by
/// `rustok-moderation` and must never be copied here.
pub mod membership_enforcement {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "group_membership_enforcements")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub membership_id: Uuid,
        pub tenant_id: Uuid,
        pub group_id: Uuid,
        pub user_id: Uuid,
        pub state: String,
        pub reason_code: String,
        pub source_kind: String,
        pub effective_from: DateTimeWithTimeZone,
        pub effective_until: Option<DateTimeWithTimeZone>,
        pub restore_status: String,
        pub moderation_decision_id: Option<Uuid>,
        pub moderation_decision_hash: Option<String>,
        pub actor_kind: String,
        pub actor_id: String,
        pub revision: i64,
        pub revoked_at: Option<DateTimeWithTimeZone>,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}
