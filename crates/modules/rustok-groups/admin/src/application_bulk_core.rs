use uuid::Uuid;

use crate::application_model::{
    BulkReviewGroupMembershipApplicationsCommand, GroupsAdminApplicationReviewDecision,
    GroupsAdminMembershipApplicationQuery,
};

const MAX_BULK_REVIEW_ITEMS: usize = 50;
const MAX_REVIEW_NOTE_CHARS: usize = 2_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupsAdminBulkReviewInputError {
    EmptySelection,
    TooManyApplications,
    DuplicateApplication,
    InvalidGroupId,
    InvalidApplicationId,
    ConfirmationRequired,
    ReviewNoteTooLong,
}

pub fn prepare_bulk_review_group_membership_application_query(
    group_id: &str,
) -> Result<GroupsAdminMembershipApplicationQuery, GroupsAdminBulkReviewInputError> {
    Ok(GroupsAdminMembershipApplicationQuery {
        group_id: normalize_uuid(group_id)
            .map_err(|_| GroupsAdminBulkReviewInputError::InvalidGroupId)?,
        status: Some("pending".to_string()),
        page: 1,
        per_page: MAX_BULK_REVIEW_ITEMS as u64,
    })
}

pub fn prepare_bulk_review_group_membership_applications(
    application_ids: Vec<String>,
    decision: GroupsAdminApplicationReviewDecision,
    note: Option<String>,
    confirmed: bool,
) -> Result<BulkReviewGroupMembershipApplicationsCommand, GroupsAdminBulkReviewInputError> {
    if !confirmed {
        return Err(GroupsAdminBulkReviewInputError::ConfirmationRequired);
    }
    if application_ids.is_empty() {
        return Err(GroupsAdminBulkReviewInputError::EmptySelection);
    }
    if application_ids.len() > MAX_BULK_REVIEW_ITEMS {
        return Err(GroupsAdminBulkReviewInputError::TooManyApplications);
    }

    let mut normalized_ids = Vec::with_capacity(application_ids.len());
    let mut unique_ids = std::collections::BTreeSet::new();
    for application_id in application_ids {
        let application_id = normalize_uuid(&application_id)
            .map_err(|_| GroupsAdminBulkReviewInputError::InvalidApplicationId)?;
        if !unique_ids.insert(application_id.clone()) {
            return Err(GroupsAdminBulkReviewInputError::DuplicateApplication);
        }
        normalized_ids.push(application_id);
    }

    let note = normalize_optional_text(note);
    if note
        .as_deref()
        .is_some_and(|value| value.chars().count() > MAX_REVIEW_NOTE_CHARS)
    {
        return Err(GroupsAdminBulkReviewInputError::ReviewNoteTooLong);
    }

    Ok(BulkReviewGroupMembershipApplicationsCommand {
        idempotency_key: format!("groups-admin-bulk-review-{}", Uuid::new_v4()),
        application_ids: normalized_ids,
        decision,
        note,
        confirmed,
    })
}

fn normalize_uuid(value: &str) -> Result<String, ()> {
    let parsed = Uuid::parse_str(value.trim()).map_err(|_| ())?;
    if parsed.is_nil() {
        return Err(());
    }
    Ok(parsed.to_string())
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
    fn bulk_review_query_validation() {
        let nil_uuid = Uuid::nil().to_string();
        let valid_group = "550e8400-e29b-41d4-a716-446655440000";

        assert_eq!(
            prepare_bulk_review_group_membership_application_query(&nil_uuid),
            Err(GroupsAdminBulkReviewInputError::InvalidGroupId)
        );
        assert_eq!(
            prepare_bulk_review_group_membership_application_query("bad-uuid"),
            Err(GroupsAdminBulkReviewInputError::InvalidGroupId)
        );

        let query = prepare_bulk_review_group_membership_application_query(valid_group).unwrap();
        assert_eq!(query.group_id, valid_group);
        assert_eq!(query.status.as_deref(), Some("pending"));
        assert_eq!(query.page, 1);
        assert_eq!(query.per_page, 50);
    }

    #[test]
    fn bulk_review_command_validation() {
        let nil_uuid = Uuid::nil().to_string();
        let valid_app1 = "550e8400-e29b-41d4-a716-446655440001";
        let valid_app2 = "550e8400-e29b-41d4-a716-446655440002";

        // Unconfirmed
        assert_eq!(
            prepare_bulk_review_group_membership_applications(
                vec![valid_app1.to_string()],
                GroupsAdminApplicationReviewDecision::Approve,
                None,
                false
            ),
            Err(GroupsAdminBulkReviewInputError::ConfirmationRequired)
        );

        // Empty selection
        assert_eq!(
            prepare_bulk_review_group_membership_applications(
                vec![],
                GroupsAdminApplicationReviewDecision::Approve,
                None,
                true
            ),
            Err(GroupsAdminBulkReviewInputError::EmptySelection)
        );

        // Too many applications (> 50)
        let too_many = (0..51)
            .map(|_| Uuid::new_v4().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            prepare_bulk_review_group_membership_applications(
                too_many,
                GroupsAdminApplicationReviewDecision::Approve,
                None,
                true
            ),
            Err(GroupsAdminBulkReviewInputError::TooManyApplications)
        );

        // Nil UUID in selection
        assert_eq!(
            prepare_bulk_review_group_membership_applications(
                vec![valid_app1.to_string(), nil_uuid.clone()],
                GroupsAdminApplicationReviewDecision::Approve,
                None,
                true
            ),
            Err(GroupsAdminBulkReviewInputError::InvalidApplicationId)
        );

        // Invalid UUID in selection
        assert_eq!(
            prepare_bulk_review_group_membership_applications(
                vec![valid_app1.to_string(), "not-a-uuid".to_string()],
                GroupsAdminApplicationReviewDecision::Approve,
                None,
                true
            ),
            Err(GroupsAdminBulkReviewInputError::InvalidApplicationId)
        );

        // Duplicate UUID in selection
        assert_eq!(
            prepare_bulk_review_group_membership_applications(
                vec![valid_app1.to_string(), valid_app1.to_string()],
                GroupsAdminApplicationReviewDecision::Approve,
                None,
                true
            ),
            Err(GroupsAdminBulkReviewInputError::DuplicateApplication)
        );

        // Review note too long
        let long_note = "x".repeat(2001);
        assert_eq!(
            prepare_bulk_review_group_membership_applications(
                vec![valid_app1.to_string(), valid_app2.to_string()],
                GroupsAdminApplicationReviewDecision::Reject,
                Some(long_note),
                true
            ),
            Err(GroupsAdminBulkReviewInputError::ReviewNoteTooLong)
        );

        // Valid bulk review
        let cmd = prepare_bulk_review_group_membership_applications(
            vec![format!("  {}  ", valid_app1), valid_app2.to_string()],
            GroupsAdminApplicationReviewDecision::Approve,
            Some("  Bulk approved by admin  ".to_string()),
            true,
        )
        .unwrap();

        assert_eq!(cmd.application_ids, vec![valid_app1, valid_app2]);
        assert_eq!(cmd.decision, GroupsAdminApplicationReviewDecision::Approve);
        assert_eq!(cmd.note.as_deref(), Some("Bulk approved by admin"));
        assert!(cmd.confirmed);
    }
}
