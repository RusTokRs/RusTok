use rustok_api::normalize_locale_tag;
use uuid::Uuid;

use crate::application_model::{
    GroupsAdminApplicationPolicyLocaleCatalogQuery, GroupsAdminApplicationPolicyPrecondition,
    GroupsAdminApplicationPolicyQuery, GroupsAdminApplicationQuestion,
    GroupsAdminApplicationReviewDecision, GroupsAdminApplicationRule,
    GroupsAdminMembershipApplicationQuery, ReopenGroupMembershipApplicationCommand,
    ReviewGroupMembershipApplicationCommand, UpsertGroupApplicationPolicyCommand,
};

const MAX_POLICY_QUESTIONS: usize = 20;
const MAX_POLICY_RULES: usize = 20;
const MAX_REVIEW_NOTE_CHARS: usize = 2_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupsAdminApplicationInputError {
    InvalidGroupId,
    InvalidApplicationId,
    InvalidLocale,
    InvalidExpectedPolicy,
    TooManyQuestions,
    TooManyRules,
    InvalidQuestion,
    InvalidRule,
    InvalidStatus,
    ReviewNoteTooLong,
}

pub fn prepare_group_application_policy_locale_catalog_query(
    group_id: &str,
) -> Result<GroupsAdminApplicationPolicyLocaleCatalogQuery, GroupsAdminApplicationInputError> {
    Ok(GroupsAdminApplicationPolicyLocaleCatalogQuery {
        group_id: normalize_uuid(group_id)
            .map_err(|_| GroupsAdminApplicationInputError::InvalidGroupId)?,
    })
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
    expected_policy: Option<GroupsAdminApplicationPolicyPrecondition>,
    enabled: bool,
    mut questions: Vec<GroupsAdminApplicationQuestion>,
    mut rules: Vec<GroupsAdminApplicationRule>,
) -> Result<UpsertGroupApplicationPolicyCommand, GroupsAdminApplicationInputError> {
    let group_id =
        normalize_uuid(group_id).map_err(|_| GroupsAdminApplicationInputError::InvalidGroupId)?;
    let locale =
        normalize_locale_tag(locale).ok_or(GroupsAdminApplicationInputError::InvalidLocale)?;
    let expected_policy = expected_policy
        .map(|mut expected| {
            expected.policy_id = normalize_uuid(&expected.policy_id)
                .map_err(|_| GroupsAdminApplicationInputError::InvalidExpectedPolicy)?;
            expected.locale = normalize_locale_tag(&expected.locale)
                .ok_or(GroupsAdminApplicationInputError::InvalidExpectedPolicy)?;
            if expected.revision == 0 || expected.locale != locale {
                return Err(GroupsAdminApplicationInputError::InvalidExpectedPolicy);
            }
            Ok(expected)
        })
        .transpose()?;
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
        expected_policy,
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

pub fn prepare_reopen_group_membership_application(
    application_id: &str,
) -> Result<ReopenGroupMembershipApplicationCommand, GroupsAdminApplicationInputError> {
    Ok(ReopenGroupMembershipApplicationCommand {
        idempotency_key: format!("groups-admin-reopen-application-{}", Uuid::new_v4()),
        application_id: normalize_uuid(application_id)
            .map_err(|_| GroupsAdminApplicationInputError::InvalidApplicationId)?,
    })
}

fn normalize_uuid(value: &str) -> Result<String, ()> {
    let parsed = Uuid::parse_str(value.trim()).map_err(|_| ())?;
    if parsed.is_nil() {
        return Err(());
    }
    Ok(parsed.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_queries_reject_nil_and_invalid_uuids() {
        let nil_uuid = Uuid::nil().to_string();
        let valid_uuid = "550e8400-e29b-41d4-a716-446655440000";

        assert_eq!(
            prepare_group_application_policy_locale_catalog_query(&nil_uuid),
            Err(GroupsAdminApplicationInputError::InvalidGroupId)
        );
        assert_eq!(
            prepare_group_application_policy_locale_catalog_query("not-a-uuid"),
            Err(GroupsAdminApplicationInputError::InvalidGroupId)
        );

        let query = prepare_group_application_policy_locale_catalog_query(valid_uuid).unwrap();
        assert_eq!(query.group_id, valid_uuid);

        assert_eq!(
            prepare_group_application_policy_query(&nil_uuid, "en"),
            Err(GroupsAdminApplicationInputError::InvalidGroupId)
        );
        assert_eq!(
            prepare_group_application_policy_query(valid_uuid, "invalid locale!"),
            Err(GroupsAdminApplicationInputError::InvalidLocale)
        );

        let query = prepare_group_application_policy_query(valid_uuid, "EN-us").unwrap();
        assert_eq!(query.group_id, valid_uuid);
        assert_eq!(query.locale, "en-US");
    }

    #[test]
    fn upsert_application_policy_validations() {
        let nil_uuid = Uuid::nil().to_string();
        let valid_group = "550e8400-e29b-41d4-a716-446655440000";
        let valid_policy = "550e8400-e29b-41d4-a716-446655440001";

        assert_eq!(
            prepare_upsert_group_application_policy(&nil_uuid, "en", None, true, vec![], vec![]),
            Err(GroupsAdminApplicationInputError::InvalidGroupId)
        );
        assert_eq!(
            prepare_upsert_group_application_policy(valid_group, "???", None, true, vec![], vec![]),
            Err(GroupsAdminApplicationInputError::InvalidLocale)
        );

        // Precondition with nil policy_id
        let nil_precondition = GroupsAdminApplicationPolicyPrecondition {
            policy_id: nil_uuid.clone(),
            revision: 1,
            locale: "en".to_string(),
        };
        assert_eq!(
            prepare_upsert_group_application_policy(
                valid_group,
                "en",
                Some(nil_precondition),
                true,
                vec![],
                vec![]
            ),
            Err(GroupsAdminApplicationInputError::InvalidExpectedPolicy)
        );

        // Precondition with revision 0
        let zero_rev_precondition = GroupsAdminApplicationPolicyPrecondition {
            policy_id: valid_policy.to_string(),
            revision: 0,
            locale: "en".to_string(),
        };
        assert_eq!(
            prepare_upsert_group_application_policy(
                valid_group,
                "en",
                Some(zero_rev_precondition),
                true,
                vec![],
                vec![]
            ),
            Err(GroupsAdminApplicationInputError::InvalidExpectedPolicy)
        );

        // Precondition with locale mismatch
        let mismatch_precondition = GroupsAdminApplicationPolicyPrecondition {
            policy_id: valid_policy.to_string(),
            revision: 1,
            locale: "de".to_string(),
        };
        assert_eq!(
            prepare_upsert_group_application_policy(
                valid_group,
                "en",
                Some(mismatch_precondition),
                true,
                vec![],
                vec![]
            ),
            Err(GroupsAdminApplicationInputError::InvalidExpectedPolicy)
        );

        // Question count bounds
        let too_many_questions = (0..21)
            .map(|i| GroupsAdminApplicationQuestion {
                key: format!("q{}", i),
                prompt: "Question prompt".to_string(),
                help_text: None,
                required: true,
                max_answer_chars: 100,
            })
            .collect();
        assert_eq!(
            prepare_upsert_group_application_policy(
                valid_group,
                "en",
                None,
                true,
                too_many_questions,
                vec![]
            ),
            Err(GroupsAdminApplicationInputError::TooManyQuestions)
        );

        // Rule count bounds
        let too_many_rules = (0..21)
            .map(|i| GroupsAdminApplicationRule {
                key: format!("r{}", i),
                title: "Rule title".to_string(),
                body: "Rule body".to_string(),
                required: true,
            })
            .collect();
        assert_eq!(
            prepare_upsert_group_application_policy(
                valid_group,
                "en",
                None,
                true,
                vec![],
                too_many_rules
            ),
            Err(GroupsAdminApplicationInputError::TooManyRules)
        );

        // Question invalid key
        let invalid_key_question = vec![GroupsAdminApplicationQuestion {
            key: "Invalid Key!".to_string(),
            prompt: "Prompt".to_string(),
            help_text: None,
            required: true,
            max_answer_chars: 100,
        }];
        assert_eq!(
            prepare_upsert_group_application_policy(
                valid_group,
                "en",
                None,
                true,
                invalid_key_question,
                vec![]
            ),
            Err(GroupsAdminApplicationInputError::InvalidQuestion)
        );

        // Question invalid prompt or max_answer_chars
        let empty_prompt_question = vec![GroupsAdminApplicationQuestion {
            key: "q1".to_string(),
            prompt: "   ".to_string(),
            help_text: None,
            required: true,
            max_answer_chars: 100,
        }];
        assert_eq!(
            prepare_upsert_group_application_policy(
                valid_group,
                "en",
                None,
                true,
                empty_prompt_question,
                vec![]
            ),
            Err(GroupsAdminApplicationInputError::InvalidQuestion)
        );

        let zero_chars_question = vec![GroupsAdminApplicationQuestion {
            key: "q1".to_string(),
            prompt: "Prompt".to_string(),
            help_text: None,
            required: true,
            max_answer_chars: 0,
        }];
        assert_eq!(
            prepare_upsert_group_application_policy(
                valid_group,
                "en",
                None,
                true,
                zero_chars_question,
                vec![]
            ),
            Err(GroupsAdminApplicationInputError::InvalidQuestion)
        );

        // Rule invalid key or empty title/body
        let invalid_rule = vec![GroupsAdminApplicationRule {
            key: "r1".to_string(),
            title: "".to_string(),
            body: "Body".to_string(),
            required: true,
        }];
        assert_eq!(
            prepare_upsert_group_application_policy(
                valid_group,
                "en",
                None,
                true,
                vec![],
                invalid_rule
            ),
            Err(GroupsAdminApplicationInputError::InvalidRule)
        );

        // Valid command preparation
        let valid_cmd = prepare_upsert_group_application_policy(
            valid_group,
            "en",
            Some(GroupsAdminApplicationPolicyPrecondition {
                policy_id: valid_policy.to_string(),
                revision: 2,
                locale: "en".to_string(),
            }),
            true,
            vec![GroupsAdminApplicationQuestion {
                key: "Q1".to_string(),
                prompt: "  Why do you want to join?  ".to_string(),
                help_text: Some("  Details  ".to_string()),
                required: true,
                max_answer_chars: 500,
            }],
            vec![GroupsAdminApplicationRule {
                key: "R1".to_string(),
                title: " Be kind ".to_string(),
                body: " Respect others ".to_string(),
                required: true,
            }],
        )
        .unwrap();

        assert_eq!(valid_cmd.group_id, valid_group);
        assert_eq!(valid_cmd.locale, "en");
        assert_eq!(valid_cmd.questions[0].key, "q1");
        assert_eq!(valid_cmd.questions[0].prompt, "Why do you want to join?");
        assert_eq!(valid_cmd.questions[0].help_text.as_deref(), Some("Details"));
        assert_eq!(valid_cmd.rules[0].key, "r1");
        assert_eq!(valid_cmd.rules[0].title, "Be kind");
        assert_eq!(valid_cmd.rules[0].body, "Respect others");
    }

    #[test]
    fn membership_application_lifecycle_validations() {
        let nil_uuid = Uuid::nil().to_string();
        let valid_group = "550e8400-e29b-41d4-a716-446655440000";
        let valid_app = "550e8400-e29b-41d4-a716-446655440001";

        // Query status validation
        assert_eq!(
            prepare_group_membership_application_query(&nil_uuid, None),
            Err(GroupsAdminApplicationInputError::InvalidGroupId)
        );
        assert_eq!(
            prepare_group_membership_application_query(valid_group, Some("unknown_status")),
            Err(GroupsAdminApplicationInputError::InvalidStatus)
        );
        let q = prepare_group_membership_application_query(valid_group, Some(" PENDING ")).unwrap();
        assert_eq!(q.status.as_deref(), Some("pending"));

        // Review command
        assert_eq!(
            prepare_review_group_membership_application(
                &nil_uuid,
                GroupsAdminApplicationReviewDecision::Approve,
                None
            ),
            Err(GroupsAdminApplicationInputError::InvalidApplicationId)
        );
        let long_note = "a".repeat(2001);
        assert_eq!(
            prepare_review_group_membership_application(
                valid_app,
                GroupsAdminApplicationReviewDecision::Reject,
                Some(long_note)
            ),
            Err(GroupsAdminApplicationInputError::ReviewNoteTooLong)
        );

        let review_cmd = prepare_review_group_membership_application(
            valid_app,
            GroupsAdminApplicationReviewDecision::Approve,
            Some("  Welcome to the group!  ".to_string()),
        )
        .unwrap();
        assert_eq!(review_cmd.application_id, valid_app);
        assert_eq!(review_cmd.note.as_deref(), Some("Welcome to the group!"));

        // Reopen command
        assert_eq!(
            prepare_reopen_group_membership_application(&nil_uuid),
            Err(GroupsAdminApplicationInputError::InvalidApplicationId)
        );
        let reopen_cmd = prepare_reopen_group_membership_application(valid_app).unwrap();
        assert_eq!(reopen_cmd.application_id, valid_app);
    }
}
