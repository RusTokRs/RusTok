use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminFilters {
    pub page: u64,
    pub per_page: u64,
    pub search: Option<String>,
    pub include_non_public: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminListItem {
    pub id: String,
    pub handle: String,
    pub title: String,
    pub visibility: String,
    pub join_policy: String,
    pub status: String,
    pub member_count: u64,
    pub effective_locale: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminDirectory {
    pub items: Vec<GroupsAdminListItem>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupsAdminAssignableRole {
    Admin,
    Moderator,
    Member,
}

impl GroupsAdminAssignableRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Moderator => "moderator",
            Self::Member => "member",
        }
    }

    pub const fn as_graphql_enum(self) -> &'static str {
        match self {
            Self::Admin => "ADMIN",
            Self::Moderator => "MODERATOR",
            Self::Member => "MEMBER",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeGroupRoleCommand {
    pub idempotency_key: String,
    pub group_id: String,
    pub target_user_id: String,
    pub role: GroupsAdminAssignableRole,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferGroupOwnershipCommand {
    pub idempotency_key: String,
    pub group_id: String,
    pub new_owner_user_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminGovernanceResult {
    pub group_id: String,
    pub actor_user_id: String,
    pub target_user_id: String,
    pub previous_role: String,
    pub current_role: String,
    pub group_version: u64,
    pub replayed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminTranslation {
    pub id: String,
    pub group_id: String,
    pub locale: String,
    pub title: String,
    pub summary: Option<String>,
    pub body: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminTranslationQuery {
    pub group_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpsertGroupTranslationCommand {
    pub idempotency_key: String,
    pub group_id: String,
    pub locale: String,
    pub title: String,
    pub summary: Option<String>,
    pub body: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteGroupTranslationCommand {
    pub idempotency_key: String,
    pub group_id: String,
    pub locale: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminTranslationMutationResult {
    pub translation: GroupsAdminTranslation,
    pub group_version: u64,
    pub created: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminDeleteTranslationResult {
    pub group_id: String,
    pub locale: String,
    pub group_version: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminInvitation {
    pub id: String,
    pub group_id: String,
    pub invited_by_user_id: String,
    pub target_user_id: Option<String>,
    pub max_uses: u32,
    pub use_count: u32,
    pub expires_at: String,
    pub revoked_at: Option<String>,
    pub revoked_by_user_id: Option<String>,
    pub created_at: String,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminInvitationConnection {
    pub items: Vec<GroupsAdminInvitation>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminInvitationQuery {
    pub group_id: String,
    pub page: u64,
    pub per_page: u64,
    pub include_inactive: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateGroupInvitationCommand {
    pub idempotency_key: String,
    pub group_id: String,
    pub target_user_id: Option<String>,
    pub expires_in_seconds: u64,
    pub max_uses: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminCreateInvitationResult {
    pub invitation: GroupsAdminInvitation,
    pub token: Option<String>,
    pub group_version: u64,
    pub replayed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevokeGroupInvitationCommand {
    pub idempotency_key: String,
    pub invitation_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminRevokeInvitationResult {
    pub invitation: GroupsAdminInvitation,
    pub group_version: u64,
    pub replayed: bool,
}
