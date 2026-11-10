use super::*;

pub(super) const UPSERT_POLICY_IF_CURRENT_COMMAND: &str =
    "groups.upsert_membership_application_policy_if_current.v1";
pub(super) const SUBMIT_APPLICATION_IF_CURRENT_COMMAND: &str =
    "groups.submit_membership_application_if_current.v1";

pub const GROUP_APPLICATION_POLICY_CHANGED_CODE: &str = "groups.application_policy_changed";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupApplicationPolicyPrecondition {
    pub policy_id: Uuid,
    pub revision: u64,
    pub locale: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpsertGroupApplicationPolicyIfCurrentRequest {
    pub expected_policy: Option<GroupApplicationPolicyPrecondition>,
    pub policy: UpsertGroupApplicationPolicyRequest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitGroupMembershipApplicationIfCurrentRequest {
    pub expected_policy: GroupApplicationPolicyPrecondition,
    pub submission: SubmitGroupMembershipApplicationRequest,
}

#[async_trait]
pub trait GroupApplicationCasCommandPort: Send + Sync {
    async fn upsert_group_application_policy_if_current(
        &self,
        context: PortContext,
        request: UpsertGroupApplicationPolicyIfCurrentRequest,
    ) -> Result<UpsertGroupApplicationPolicyResult, PortError>;

    async fn submit_group_membership_application_if_current(
        &self,
        context: PortContext,
        request: SubmitGroupMembershipApplicationIfCurrentRequest,
    ) -> Result<SubmitGroupMembershipApplicationResult, PortError>;
}

#[async_trait]
impl GroupApplicationCasCommandPort for GroupApplicationService {
    async fn upsert_group_application_policy_if_current(
        &self,
        context: PortContext,
        request: UpsertGroupApplicationPolicyIfCurrentRequest,
    ) -> Result<UpsertGroupApplicationPolicyResult, PortError> {
        self.upsert_policy_if_current_effective_owned(&context, request)
            .await
            .map_err(map_application_cas_error)
    }

    async fn submit_group_membership_application_if_current(
        &self,
        context: PortContext,
        request: SubmitGroupMembershipApplicationIfCurrentRequest,
    ) -> Result<SubmitGroupMembershipApplicationResult, PortError> {
        self.submit_application_if_current_effective_owned(&context, request)
            .await
            .map_err(map_application_cas_error)
    }
}

pub(super) async fn find_candidate_application_for_update(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
) -> GroupsResult<Option<membership_application::Model>> {
    crate::membership_enforcement_transaction::reserve_group_write_for_update(
        transaction,
        tenant_id,
        group_id,
    )
    .await?;
    let query = || {
        membership_application::Entity::find()
            .filter(membership_application::Column::TenantId.eq(tenant_id))
            .filter(membership_application::Column::GroupId.eq(group_id))
            .filter(membership_application::Column::UserId.eq(user_id))
    };
    let model = match transaction.get_database_backend() {
        DbBackend::Sqlite => query().one(transaction).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(transaction).await?,
        _ => unreachable!("unsupported SeaORM database backend"),
    };
    Ok(model)
}

pub(super) fn normalize_policy_precondition(
    expected: &mut GroupApplicationPolicyPrecondition,
) -> GroupsResult<()> {
    if expected.revision == 0 {
        return Err(GroupsError::Validation(
            "expected policy revision must be at least 1".to_string(),
        ));
    }
    expected.locale = normalize_locale_tag(&expected.locale)
        .ok_or_else(|| GroupsError::Validation("invalid expected policy locale".to_string()))?;
    Ok(())
}

pub(super) fn ensure_policy_update_precondition(
    expected: Option<&GroupApplicationPolicyPrecondition>,
    current: Option<&membership_policy::Model>,
    locale: &str,
) -> GroupsResult<()> {
    // Validate current storage even when the caller supplied a mismatching ID or no precondition.
    // A corrupt revision must not be reported as an ordinary stale-policy conflict.
    let current_revision = current
        .map(|value| crate::domain::stored_revision(value.revision))
        .transpose()?;
    let matches = match (expected, current, current_revision) {
        (None, None, None) => true,
        (Some(expected), Some(current), Some(revision)) => {
            expected.policy_id == current.id
                && expected.revision == revision
                && expected.locale == locale
        }
        _ => false,
    };
    if matches {
        Ok(())
    } else {
        Err(policy_changed_error(
            expected,
            current
                .zip(current_revision)
                .map(|(value, revision)| (value.id, revision, locale)),
        ))
    }
}

pub(super) fn ensure_loaded_policy_precondition(
    expected: &GroupApplicationPolicyPrecondition,
    current: &GroupApplicationPolicy,
) -> GroupsResult<()> {
    if expected.policy_id == current.id
        && expected.revision == current.revision
        && expected.locale == current.locale
    {
        Ok(())
    } else {
        Err(policy_changed_error(
            Some(expected),
            Some((current.id, current.revision, current.locale.as_str())),
        ))
    }
}

pub(super) fn policy_changed_error(
    expected: Option<&GroupApplicationPolicyPrecondition>,
    current: Option<(Uuid, u64, &str)>,
) -> GroupsError {
    let expected_description = expected
        .map(|value| format!("{}@{}:{}", value.policy_id, value.revision, value.locale))
        .unwrap_or_else(|| "none".to_string());
    let current_description = current
        .map(|(policy_id, revision, locale)| format!("{policy_id}@{revision}:{locale}"))
        .unwrap_or_else(|| "none".to_string());
    GroupsError::Conflict(format!(
        "{GROUP_APPLICATION_POLICY_CHANGED_CODE}: expected={expected_description}; current={current_description}"
    ))
}

pub(super) fn map_application_cas_error(error: GroupsError) -> PortError {
    match error {
        GroupsError::Conflict(message)
            if message.starts_with(GROUP_APPLICATION_POLICY_CHANGED_CODE) =>
        {
            let message = message
                .strip_prefix(GROUP_APPLICATION_POLICY_CHANGED_CODE)
                .unwrap_or(&message)
                .trim_start_matches(':')
                .trim()
                .to_string();
            PortError::conflict(GROUP_APPLICATION_POLICY_CHANGED_CODE, message)
        }
        other => other.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrupt_current_revision_is_not_a_stale_precondition() {
        let now = Utc::now().fixed_offset();
        let mut policy = membership_policy::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            group_id: Uuid::new_v4(),
            revision: 1,
            enabled: true,
            created_by_user_id: Uuid::new_v4(),
            updated_by_user_id: Uuid::new_v4(),
            created_at: now,
            updated_at: now,
        };
        let expected = GroupApplicationPolicyPrecondition {
            policy_id: policy.id,
            revision: 1,
            locale: "en".into(),
        };
        assert!(ensure_policy_update_precondition(Some(&expected), Some(&policy), "en").is_ok());
        for revision in [0, -1, i64::MIN] {
            policy.revision = revision;
            for precondition in [None, Some(&expected)] {
                assert!(matches!(
                    ensure_policy_update_precondition(precondition, Some(&policy), "en"),
                    Err(GroupsError::Invariant(_))
                ));
            }
        }
        policy.revision = 2;
        assert!(matches!(
            ensure_policy_update_precondition(Some(&expected), Some(&policy), "en"),
            Err(GroupsError::Conflict(_))
        ));
    }
}
