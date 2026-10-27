use std::collections::{BTreeMap, BTreeSet};

use rustok_api::normalize_locale_tag;
use uuid::Uuid;

use crate::application_model::{
    CancelGroupMembershipApplicationCommand, GroupsStorefrontApplicationAnswer,
    GroupsStorefrontApplicationPolicy, GroupsStorefrontApplicationPolicyPrecondition,
    GroupsStorefrontApplicationPolicyQuery, GroupsStorefrontMyApplicationQuery,
    SubmitGroupMembershipApplicationCommand,
};

pub const GROUP_APPLICATION_QUERY_KEY: &str = "apply";
pub const GROUP_APPLICATION_POLICY_CHANGED_CODE: &str = "groups.application_policy_changed";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupsStorefrontApplicationInputError {
    InvalidGroupId,
    InvalidApplicationId,
    InvalidLocale,
    InvalidPolicy,
    UnknownQuestion,
    MissingRequiredAnswer,
    AnswerTooLong,
    UnknownRule,
    MissingRequiredRule,
}

pub fn prepare_group_application_policy_query(
    group_id: &str,
    locale: &str,
) -> Result<GroupsStorefrontApplicationPolicyQuery, GroupsStorefrontApplicationInputError> {
    Ok(GroupsStorefrontApplicationPolicyQuery {
        group_id: normalize_uuid(group_id)
            .map_err(|_| GroupsStorefrontApplicationInputError::InvalidGroupId)?,
        locale: normalize_locale_tag(locale)
            .ok_or(GroupsStorefrontApplicationInputError::InvalidLocale)?,
    })
}

pub fn prepare_my_group_membership_application_query(
    group_id: &str,
) -> Result<GroupsStorefrontMyApplicationQuery, GroupsStorefrontApplicationInputError> {
    Ok(GroupsStorefrontMyApplicationQuery {
        group_id: normalize_uuid(group_id)
            .map_err(|_| GroupsStorefrontApplicationInputError::InvalidGroupId)?,
    })
}

pub fn prepare_cancel_group_membership_application(
    application_id: &str,
) -> Result<CancelGroupMembershipApplicationCommand, GroupsStorefrontApplicationInputError> {
    Ok(CancelGroupMembershipApplicationCommand {
        idempotency_key: format!("groups-storefront-cancel-application-{}", Uuid::new_v4()),
        application_id: normalize_uuid(application_id)
            .map_err(|_| GroupsStorefrontApplicationInputError::InvalidApplicationId)?,
    })
}

pub fn prepare_submit_group_membership_application(
    policy: &GroupsStorefrontApplicationPolicy,
    answers: BTreeMap<String, String>,
    acknowledged_rule_keys: BTreeSet<String>,
) -> Result<SubmitGroupMembershipApplicationCommand, GroupsStorefrontApplicationInputError> {
    let parsed_policy_id = Uuid::parse_str(policy.id.trim())
        .map_err(|_| GroupsStorefrontApplicationInputError::InvalidPolicy)?;
    let parsed_group_id = Uuid::parse_str(policy.group_id.trim())
        .map_err(|_| GroupsStorefrontApplicationInputError::InvalidPolicy)?;
    if policy.revision == 0
        || parsed_policy_id.is_nil()
        || parsed_group_id.is_nil()
        || normalize_locale_tag(&policy.locale).as_deref() != Some(policy.locale.as_str())
    {
        return Err(GroupsStorefrontApplicationInputError::InvalidPolicy);
    }
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
        expected_policy: GroupsStorefrontApplicationPolicyPrecondition::from(policy),
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

pub fn is_application_policy_changed(error: &str) -> bool {
    error.contains(GROUP_APPLICATION_POLICY_CHANGED_CODE)
}

fn normalize_uuid(value: &str) -> Result<String, ()> {
    let parsed = Uuid::parse_str(value.trim()).map_err(|_| ())?;
    if parsed.is_nil() {
        return Err(());
    }
    Ok(parsed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application_model::{
        GroupsStorefrontApplicationQuestion, GroupsStorefrontApplicationRule,
    };

    #[test]
    fn queries_and_cancel_reject_nil_and_invalid_uuids() {
        let nil_uuid = Uuid::nil().to_string();
        let valid_uuid = "550e8400-e29b-41d4-a716-446655440000";

        assert_eq!(
            prepare_group_application_policy_query(&nil_uuid, "en"),
            Err(GroupsStorefrontApplicationInputError::InvalidGroupId)
        );
        assert_eq!(
            prepare_group_application_policy_query("not-a-uuid", "en"),
            Err(GroupsStorefrontApplicationInputError::InvalidGroupId)
        );
        assert_eq!(
            prepare_group_application_policy_query(valid_uuid, "invalid locale!"),
            Err(GroupsStorefrontApplicationInputError::InvalidLocale)
        );
        let q = prepare_group_application_policy_query(valid_uuid, "en-US").unwrap();
        assert_eq!(q.group_id, valid_uuid);
        assert_eq!(q.locale, "en-US");

        assert_eq!(
            prepare_my_group_membership_application_query(&nil_uuid),
            Err(GroupsStorefrontApplicationInputError::InvalidGroupId)
        );
        let my_q = prepare_my_group_membership_application_query(valid_uuid).unwrap();
        assert_eq!(my_q.group_id, valid_uuid);

        assert_eq!(
            prepare_cancel_group_membership_application(&nil_uuid),
            Err(GroupsStorefrontApplicationInputError::InvalidApplicationId)
        );
        let cancel_cmd = prepare_cancel_group_membership_application(valid_uuid).unwrap();
        assert_eq!(cancel_cmd.application_id, valid_uuid);
        assert!(cancel_cmd.idempotency_key.starts_with("groups-storefront-cancel-application-"));
    }

    #[test]
    fn submit_application_validations() {
        let nil_uuid = Uuid::nil().to_string();
        let valid_group = "550e8400-e29b-41d4-a716-446655440000";
        let valid_policy = "550e8400-e29b-41d4-a716-446655440001";

        let base_policy = GroupsStorefrontApplicationPolicy {
            id: valid_policy.to_string(),
            group_id: valid_group.to_string(),
            revision: 1,
            enabled: true,
            locale: "en".to_string(),
            questions: vec![
                GroupsStorefrontApplicationQuestion {
                    key: "motivation".to_string(),
                    prompt: "Why join?".to_string(),
                    help_text: None,
                    required: true,
                    max_answer_chars: 100,
                },
                GroupsStorefrontApplicationQuestion {
                    key: "optional_info".to_string(),
                    prompt: "Additional info".to_string(),
                    help_text: None,
                    required: false,
                    max_answer_chars: 50,
                },
            ],
            rules: vec![
                GroupsStorefrontApplicationRule {
                    key: "rule_respect".to_string(),
                    title: "Be respectful".to_string(),
                    body: "Treat all members kindly".to_string(),
                    required: true,
                },
                GroupsStorefrontApplicationRule {
                    key: "rule_optional".to_string(),
                    title: "Optional guideline".to_string(),
                    body: "Guideline".to_string(),
                    required: false,
                },
            ],
        };

        // Invalid policy cases
        let mut p = base_policy.clone();
        p.revision = 0;
        assert_eq!(
            prepare_submit_group_membership_application(&p, BTreeMap::new(), BTreeSet::new()),
            Err(GroupsStorefrontApplicationInputError::InvalidPolicy)
        );

        let mut p = base_policy.clone();
        p.id = nil_uuid.clone();
        assert_eq!(
            prepare_submit_group_membership_application(&p, BTreeMap::new(), BTreeSet::new()),
            Err(GroupsStorefrontApplicationInputError::InvalidPolicy)
        );

        let mut p = base_policy.clone();
        p.group_id = nil_uuid.clone();
        assert_eq!(
            prepare_submit_group_membership_application(&p, BTreeMap::new(), BTreeSet::new()),
            Err(GroupsStorefrontApplicationInputError::InvalidPolicy)
        );

        let mut p = base_policy.clone();
        p.locale = "invalid locale!".to_string();
        assert_eq!(
            prepare_submit_group_membership_application(&p, BTreeMap::new(), BTreeSet::new()),
            Err(GroupsStorefrontApplicationInputError::InvalidPolicy)
        );

        // Unknown question
        let mut answers = BTreeMap::new();
        answers.insert("unknown_q".to_string(), "answer".to_string());
        assert_eq!(
            prepare_submit_group_membership_application(&base_policy, answers, BTreeSet::new()),
            Err(GroupsStorefrontApplicationInputError::UnknownQuestion)
        );

        // Missing required answer
        let answers = BTreeMap::new();
        assert_eq!(
            prepare_submit_group_membership_application(&base_policy, answers, BTreeSet::new()),
            Err(GroupsStorefrontApplicationInputError::MissingRequiredAnswer)
        );

        let mut answers = BTreeMap::new();
        answers.insert("motivation".to_string(), "   ".to_string());
        assert_eq!(
            prepare_submit_group_membership_application(&base_policy, answers, BTreeSet::new()),
            Err(GroupsStorefrontApplicationInputError::MissingRequiredAnswer)
        );

        // Answer too long
        let mut answers = BTreeMap::new();
        answers.insert("motivation".to_string(), "a".repeat(101));
        assert_eq!(
            prepare_submit_group_membership_application(&base_policy, answers, BTreeSet::new()),
            Err(GroupsStorefrontApplicationInputError::AnswerTooLong)
        );

        // Unknown rule
        let mut answers = BTreeMap::new();
        answers.insert("motivation".to_string(), "Valid answer".to_string());
        let mut acknowledged = BTreeSet::new();
        acknowledged.insert("unknown_rule".to_string());
        assert_eq!(
            prepare_submit_group_membership_application(&base_policy, answers.clone(), acknowledged),
            Err(GroupsStorefrontApplicationInputError::UnknownRule)
        );

        // Missing required rule
        assert_eq!(
            prepare_submit_group_membership_application(&base_policy, answers.clone(), BTreeSet::new()),
            Err(GroupsStorefrontApplicationInputError::MissingRequiredRule)
        );

        // Valid submission
        let mut acknowledged = BTreeSet::new();
        acknowledged.insert("rule_respect".to_string());
        let cmd = prepare_submit_group_membership_application(&base_policy, answers, acknowledged).unwrap();
        assert_eq!(cmd.group_id, valid_group);
        assert_eq!(cmd.expected_policy.policy_id, valid_policy);
        assert_eq!(cmd.expected_policy.revision, 1);
        assert_eq!(cmd.answers.len(), 1);
        assert_eq!(cmd.answers[0].key, "motivation");
        assert_eq!(cmd.answers[0].value, "Valid answer");
        assert_eq!(cmd.acknowledged_rule_keys, vec!["rule_respect".to_string()]);
    }
}
