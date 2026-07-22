use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::application_model::{
    GroupsStorefrontApplicationAnswer, GroupsStorefrontApplicationPolicy,
    GroupsStorefrontApplicationPolicyQuery, SubmitGroupMembershipApplicationCommand,
};

pub const GROUP_APPLICATION_QUERY_KEY: &str = "apply";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupsStorefrontApplicationInputError {
    InvalidGroupId,
    UnknownQuestion,
    MissingRequiredAnswer,
    AnswerTooLong,
    UnknownRule,
    MissingRequiredRule,
}

pub fn prepare_group_application_policy_query(
    group_id: &str,
) -> Result<GroupsStorefrontApplicationPolicyQuery, GroupsStorefrontApplicationInputError> {
    Ok(GroupsStorefrontApplicationPolicyQuery {
        group_id: normalize_uuid(group_id)
            .map_err(|_| GroupsStorefrontApplicationInputError::InvalidGroupId)?,
    })
}

pub fn prepare_submit_group_membership_application(
    policy: &GroupsStorefrontApplicationPolicy,
    answers: BTreeMap<String, String>,
    acknowledged_rule_keys: BTreeSet<String>,
) -> Result<SubmitGroupMembershipApplicationCommand, GroupsStorefrontApplicationInputError> {
    let question_map = policy
        .questions
        .iter()
        .map(|question| (question.key.as_str(), question))
        .collect::<BTreeMap<_, _>>();
    for key in answers.keys() {
        if !question_map.contains_key(key.as_str()) {
            return Err(GroupsStorefrontApplicationInputError::UnknownQuestion);
        }
    }
    for question in &policy.questions {
        let answer = answers.get(&question.key).map(String::as_str).unwrap_or("");
        if question.required && answer.trim().is_empty() {
            return Err(GroupsStorefrontApplicationInputError::MissingRequiredAnswer);
        }
        if answer.chars().count() > question.max_answer_chars as usize {
            return Err(GroupsStorefrontApplicationInputError::AnswerTooLong);
        }
    }
    let rule_keys = policy
        .rules
        .iter()
        .map(|rule| rule.key.as_str())
        .collect::<BTreeSet<_>>();
    if acknowledged_rule_keys
        .iter()
        .any(|key| !rule_keys.contains(key.as_str()))
    {
        return Err(GroupsStorefrontApplicationInputError::UnknownRule);
    }
    for rule in &policy.rules {
        if rule.required && !acknowledged_rule_keys.contains(&rule.key) {
            return Err(GroupsStorefrontApplicationInputError::MissingRequiredRule);
        }
    }
    Ok(SubmitGroupMembershipApplicationCommand {
        idempotency_key: format!("groups-storefront-submit-application-{}", Uuid::new_v4()),
        group_id: policy.group_id.clone(),
        answers: answers
            .into_iter()
            .map(|(key, value)| GroupsStorefrontApplicationAnswer {
                key,
                value: value.trim().to_string(),
            })
            .collect(),
        acknowledged_rule_keys: acknowledged_rule_keys.into_iter().collect(),
    })
}

fn normalize_uuid(value: &str) -> Result<String, uuid::Error> {
    Uuid::parse_str(value.trim()).map(|value| value.to_string())
}
