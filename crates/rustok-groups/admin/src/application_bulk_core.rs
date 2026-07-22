const MAX_BULK_REVIEW_ITEMS: usize = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupsAdminBulkReviewInputError {
    EmptySelection,
    TooManyApplications,
    DuplicateApplication,
    InvalidApplicationId,
    ConfirmationRequired,
    ReviewNoteTooLong,
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
