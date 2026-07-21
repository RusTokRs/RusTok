use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminApplicationQuestion {
    pub key: String,
    pub prompt: String,
    pub help_text: Option<String>,
    pub required: bool,
    pub max_answer_chars: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminApplicationRule {
    pub key: String,
    pub title: String,
    pub body: String,
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminApplicationPolicy {
    pub id: String,
    pub group_id: String,
    pub revision: u64,
    pub enabled: bool,
    pub locale: String,
    pub questions: Vec<GroupsAdminApplicationQuestion>,
    pub rules: Vec<GroupsAdminApplicationRule>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminApplicationPolicyQuery {
    pub group_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpsertGroupApplicationPolicyCommand {
    pub idempotency_key: String,
    pub group_id: String,
    pub locale: String,
    pub enabled: bool,
    pub questions: Vec<GroupsAdminApplicationQuestion>,
    pub rules: Vec<GroupsAdminApplicationRule>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminUpsertApplicationPolicyResult {
    pub policy: GroupsAdminApplicationPolicy,
    pub group_version: u64,
    pub created: bool,
    pub replayed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminApplicationAnswer {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminMembership {
    pub id: String,
    pub group_id: String,
    pub user_id: String,
    pub role: String,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminMembershipApplication {
    pub id: String,
    pub group_id: String,
    pub user_id: String,
    pub policy_id: String,
    pub policy_revision: u64,
    pub policy_locale: String,
    pub questions: Vec<GroupsAdminApplicationQuestion>,
    pub rules: Vec<GroupsAdminApplicationRule>,
    pub answers: Vec<GroupsAdminApplicationAnswer>,
    pub acknowledged_rule_keys: Vec<String>,
    pub status: String,
    pub submitted_at: String,
    pub reviewed_at: Option<String>,
    pub reviewed_by_user_id: Option<String>,
    pub review_note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminMembershipApplicationConnection {
    pub items: Vec<GroupsAdminMembershipApplication>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminMembershipApplicationQuery {
    pub group_id: String,
    pub status: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupsAdminApplicationReviewDecision {
    Approve,
    Reject,
}

impl GroupsAdminApplicationReviewDecision {
    pub const fn as_graphql_enum(self) -> &'static str {
        match self {
            Self::Approve => "APPROVE",
            Self::Reject => "REJECT",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewGroupMembershipApplicationCommand {
    pub idempotency_key: String,
    pub application_id: String,
    pub decision: GroupsAdminApplicationReviewDecision,
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsAdminReviewApplicationResult {
    pub application: GroupsAdminMembershipApplication,
    pub membership: GroupsAdminMembership,
    pub group_version: u64,
    pub replayed: bool,
}
