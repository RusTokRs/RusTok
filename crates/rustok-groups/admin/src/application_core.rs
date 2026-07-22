use rustok_api::normalize_locale_tag;
use uuid::Uuid;

use crate::application_model::{
    GroupsAdminApplicationPolicyQuery, GroupsAdminApplicationQuestion,
    GroupsAdminApplicationReviewDecision, GroupsAdminApplicationRule,
    GroupsAdminMembershipApplicationQuery, ReviewGroupMembershipApplicationCommand,
    UpsertGroupApplicationPolicyCommand,
};

const MAX_POLICY_QUESTIONS: usize = 20;
const MAX_POLICY_RULES: usize = 20;
const MAX_REVIEW_NOTE_CHARS: usize = 2_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupsAdminApplicationInputError {
    InvalidGroupId,
    InvalidApplicationId,
    InvalidLocale,
    TooManyQuestions,
    TooManyRules,
    InvalidQuestion,
    InvalidRule,
    InvalidStatus,
    ReviewNoteTooLong,
}

pub fn prepare_group_application_policy_query(
    group_id: &str,
    locale: &str,
) -> Result<GroupsAdminApplicationPolicyQuery, GroupsAdminApplicationInputError> {
    Ok(GroupsAdminApplicationPolicyQuery {
        group_id: normalize_uuid(group_id)
            .map_err(|_| GroupsAdminApplicationInputError::InvalidGroupId)?,
        locale: normalize_locale_tag(locale)
            .ok_or(GroupsAdminApplicationInputError::InvalidLocale)?,
    })
}

pub fn prepare_upsert_group_application_policy(
    group_id: &str,
    locale: &str,
    enabled: bool,
    mut questions: Vec<GroupsAdminApplicationQuestion>,
    mut rules: Vec<GroupsAdminApplicationRule>,
) -> Result<UpsertGroupApplicationPolicyCommand, GroupsAdminApplicationInputError> {
    let group_id =
        normalize_uuid(group_id).map_err(|_| GroupsAdminApplicationInputError::InvalidGroupId)?;
    let locale =
        normalize_locale_tag(locale).ok_or(GroupsAdminApplicationInputError::InvalidLocale)?;
    if questions.len() > MAX_POLICY_QUESTIONS {
        return Err(GroupsAdminApplicationInputError::TooManyQuestions);
    }
    if rules.len() > MAX_POLICY_RULES {
        return Err(GroupsAdminApplicationInputError::TooManyRules);
    }
    for question in &mut questions {
        question.key = normalize_key(&question.key)
            .ok_or(GroupsAdminApplicationInputError::InvalidQuestion)?;
        question.prompt = question.prompt.trim().to_string();
        question.help_text = normalize_optional_text(question.help_text.take());
        if question.prompt.is_empty()
            || question.prompt.chars().count() > 500
            || !(1..=4_000).contains(&question.max_answer_chars)
        {
            return Err(GroupsAdminApplicationInputError::InvalidQuestion);
        }
    }
    for rule in &mut rules {
        rule.key = normalize_key(&rule.key).ok_or(GroupsAdminApplicationInputError::InvalidRule)?;
        rule.title = rule.title.trim().to_string();
        rule.body = rule.body.trim().to_string();
        if rule.title.is_empty()
            || rule.title.chars().count() > 240
            || rule.body.is_empty()
            || rule.body.chars().count() > 10_000
        {
            return Err(GroupsAdminApplicationInputError::InvalidRule);
        }
    }
    Ok(UpsertGroupApplicationPolicyCommand {
        idempotency_key: format!("groups-admin-upsert-application-policy-{}", Uuid::new_v4()),
        group_id,
        locale,
        enabled,
        questions,
        rules,
    })
}

pub fn prepare_group_membership_application_query(
    group_id: &str,
    status: Option<&str>,
) -> Result<GroupsAdminMembershipApplicationQuery, GroupsAdminApplicationInputError> {
    let group_id =
        normalize_uuid(group_id).map_err(|_| GroupsAdminApplicationInputError::InvalidGroupId)?;
    let status = status
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
        .map(|value| match value.as_str() {
            "pending" | "approved" | "rejected" | "cancelled" => Ok(value),
            _ => Err(GroupsAdminApplicationInputError::InvalidStatus),
        })
        .transpose()?;
    Ok(GroupsAdminMembershipApplicationQuery {
        group_id,
        status,
        page: 1,
        per_page: 24,
    })
}

pub fn prepare_review_group_membership_application(
    application_id: &str,
    decision: GroupsAdminApplicationReviewDecision,
    note: Option<String>,
) -> Result<ReviewGroupMembershipApplicationCommand, GroupsAdminApplicationInputError> {
    let application_id = normalize_uuid(application_id)
        .map_err(|_| GroupsAdminApplicationInputError::InvalidApplicationId)?;
    let note = normalize_optional_text(note);
    if note
        .as_deref()
        .is_some_and(|value| value.chars().count() > MAX_REVIEW_NOTE_CHARS)
    {
        return Err(GroupsAdminApplicationInputError::ReviewNoteTooLong);
    }
    Ok(ReviewGroupMembershipApplicationCommand {
        idempotency_key: format!("groups-admin-review-application-{}", Uuid::new_v4()),
        application_id,
        decision,
        note,
    })
}

fn normalize_uuid(value: &str) -> Result<String, uuid::Error> {
    Uuid::parse_str(value.trim()).map(|value| value.to_string())
}

fn normalize_key(value: &str) -> Option<String> {
    let value = value.trim().to_ascii_lowercase();
    (!value.is_empty()
        && value.len() <= 64
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '-' | '_')
        }))
    .then_some(value)
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}
