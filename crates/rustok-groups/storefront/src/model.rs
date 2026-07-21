use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsStorefrontFilters {
    pub page: u64,
    pub per_page: u64,
    pub search: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsStorefrontListItem {
    pub id: String,
    pub handle: String,
    pub title: String,
    pub summary: Option<String>,
    pub visibility: String,
    pub join_policy: String,
    pub member_count: u64,
    pub effective_locale: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsStorefrontDirectory {
    pub items: Vec<GroupsStorefrontListItem>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptGroupInvitationCommand {
    pub idempotency_key: String,
    pub token: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptTargetedGroupInvitationCommand {
    pub idempotency_key: String,
    pub invitation_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsStorefrontMembership {
    pub id: String,
    pub group_id: String,
    pub user_id: String,
    pub role: String,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsStorefrontAcceptInvitationResult {
    pub invitation_id: String,
    pub group_id: String,
    pub membership: GroupsStorefrontMembership,
    pub group_version: u64,
    pub replayed: bool,
}
