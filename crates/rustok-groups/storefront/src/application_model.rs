use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsStorefrontApplicationQuestion {
    pub key: String,
    pub prompt: String,
    pub help_text: Option<String>,
    pub required: bool,
    pub max_answer_chars: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsStorefrontApplicationRule {
    pub key: String,
    pub title: String,
    pub body: String,
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsStorefrontApplicationPolicy {
    pub id: String,
    pub group_id: String,
    pub revision: u64,
    pub enabled: bool,
    pub locale: String,
    pub questions: Vec<GroupsStorefrontApplicationQuestion>,
    pub rules: Vec<GroupsStorefrontApplicationRule>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsStorefrontApplicationPolicyQuery {
    pub group_id: String,
    pub locale: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsStorefrontApplicationPolicyPrecondition {
    pub policy_id: String,
    pub revision: u64,
    pub locale: String,
}

impl From<&GroupsStorefrontApplicationPolicy> for GroupsStorefrontApplicationPolicyPrecondition {
    fn from(value: &GroupsStorefrontApplicationPolicy) -> Self {
        Self {
            policy_id: value.id.clone(),
            revision: value.revision,
            locale: value.locale.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsStorefrontApplicationAnswer {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitGroupMembershipApplicationCommand {
    pub idempotency_key: String,
    pub group_id: String,
    pub expected_policy: GroupsStorefrontApplicationPolicyPrecondition,
    pub answers: Vec<GroupsStorefrontApplicationAnswer>,
    pub acknowledged_rule_keys: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsStorefrontMembershipApplication {
    pub id: String,
    pub group_id: String,
    pub user_id: String,
    pub policy_id: String,
    pub policy_revision: u64,
    pub policy_locale: String,
    pub status: String,
    pub submitted_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsStorefrontApplicationMembership {
    pub id: String,
    pub group_id: String,
    pub user_id: String,
    pub role: String,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupsStorefrontSubmitApplicationResult {
    pub application: GroupsStorefrontMembershipApplication,
    pub membership: GroupsStorefrontApplicationMembership,
    pub group_version: u64,
    pub replayed: bool,
}
