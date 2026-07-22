const UPSERT_POLICY_IF_CURRENT_COMMAND: &str =
    "groups.upsert_membership_application_policy_if_current.v1";
const SUBMIT_APPLICATION_IF_CURRENT_COMMAND: &str =
    "groups.submit_membership_application_if_current.v1";

pub const GROUP_APPLICATION_POLICY_CHANGED_CODE: &str =
    "groups.application_policy_changed";

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

impl GroupApplicationService {
    async fn upsert_policy_if_current_owned(
        &self,
        context: &PortContext,
        mut request: UpsertGroupApplicationPolicyIfCurrentRequest,
    ) -> GroupsResult<UpsertGroupApplicationPolicyResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        request.policy.locale = normalize_locale_tag(&request.policy.locale).ok_or_else(|| {
            GroupsError::Validation("invalid application policy locale".to_string())
        })?;
        normalize_policy(
            &mut request.policy.questions,
            &mut request.policy.rules,
        )?;
        if let Some(expected) = request.expected_policy.as_mut() {
            normalize_policy_precondition(expected)?;
            if expected.locale != request.policy.locale {
                return Err(GroupsError::Validation(
                    "expected policy locale must match the policy locale being updated"
                        .to_string(),
                ));
            }
        }
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<UpsertGroupApplicationPolicyResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            UPSERT_POLICY_IF_CURRENT_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let group_model =
            find_group_for_update(&transaction, tenant_id, request.policy.group_id).await?;
        require_active_group(&group_model)?;
        authorize_policy_management(
            &transaction,
            context,
            tenant_id,
            request.policy.group_id,
            actor_user_id,
        )
        .await?;

        let existing_policy = membership_policy::Entity::find()
            .filter(membership_policy::Column::TenantId.eq(tenant_id))
            .filter(membership_policy::Column::GroupId.eq(request.policy.group_id))
            .one(&transaction)
            .await?;
        ensure_policy_update_precondition(
            request.expected_policy.as_ref(),
            existing_policy.as_ref(),
            &request.policy.locale,
        )?;

        let now = Utc::now();
        let created = existing_policy.is_none();
        let policy_model = if let Some(existing) = existing_policy {
            let next_revision = existing.revision.saturating_add(1).max(1);
            let mut active: membership_policy::ActiveModel = existing.into();
            active.revision = Set(next_revision);
            active.enabled = Set(request.policy.enabled);
            active.updated_by_user_id = Set(actor_user_id);
            active.updated_at = Set(now.fixed_offset());
            active.update(&transaction).await?
        } else {
            membership_policy::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                group_id: Set(request.policy.group_id),
                revision: Set(1),
                enabled: Set(request.policy.enabled),
                created_by_user_id: Set(actor_user_id),
                updated_by_user_id: Set(actor_user_id),
                created_at: Set(now.fixed_offset()),
                updated_at: Set(now.fixed_offset()),
            }
            .insert(&transaction)
            .await?
        };

        let existing_translation = membership_policy_translation::Entity::find()
            .filter(membership_policy_translation::Column::TenantId.eq(tenant_id))
            .filter(membership_policy_translation::Column::PolicyId.eq(policy_model.id))
            .filter(
                membership_policy_translation::Column::Locale
                    .eq(request.policy.locale.clone()),
            )
            .one(&transaction)
            .await?;
        let questions_value = serde_json::to_value(&request.policy.questions).map_err(|error| {
            GroupsError::Invariant(format!(
                "application questions are not serializable: {error}"
            ))
        })?;
        let rules_value = serde_json::to_value(&request.policy.rules).map_err(|error| {
            GroupsError::Invariant(format!(
                "application rules are not serializable: {error}"
            ))
        })?;
        if let Some(existing) = existing_translation {
            let mut active: membership_policy_translation::ActiveModel = existing.into();
            active.questions = Set(questions_value);
            active.rules = Set(rules_value);
            active.updated_at = Set(now.fixed_offset());
            active.update(&transaction).await?;
        } else {
            membership_policy_translation::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                policy_id: Set(policy_model.id),
                locale: Set(request.policy.locale.clone()),
                questions: Set(questions_value),
                rules: Set(rules_value),
                created_at: Set(now.fixed_offset()),
                updated_at: Set(now.fixed_offset()),
            }
            .insert(&transaction)
            .await?;
        }

        let group_version = increment_group_version(&transaction, group_model, now).await?;
        let result = UpsertGroupApplicationPolicyResult {
            policy: GroupApplicationPolicy {
                id: policy_model.id,
                group_id: request.policy.group_id,
                revision: policy_model.revision.max(1) as u64,
                enabled: policy_model.enabled,
                locale: request.policy.locale,
                questions: request.policy.questions,
                rules: request.policy.rules,
            },
            group_version,
            created,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            result.policy.group_id,
            actor_user_id,
            "group.membership_application_policy_upserted",
            None,
            json!({
                "policy_id": result.policy.id,
                "policy_revision": result.policy.revision,
                "locale": result.policy.locale,
                "enabled": result.policy.enabled,
                "question_count": result.policy.questions.len(),
                "rule_count": result.policy.rules.len(),
                "group_version": group_version,
                "expected_revision_enforced": true
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            result.policy.group_id,
            actor_user_id,
            idempotency_key,
            UPSERT_POLICY_IF_CURRENT_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }

    async fn submit_application_if_current_owned(
        &self,
        context: &PortContext,
        mut request: SubmitGroupMembershipApplicationIfCurrentRequest,
    ) -> GroupsResult<SubmitGroupMembershipApplicationResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        normalize_policy_precondition(&mut request.expected_policy)?;
        normalize_application_submission(&mut request.submission)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<SubmitGroupMembershipApplicationResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            SUBMIT_APPLICATION_IF_CURRENT_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let prelocked_application = find_candidate_application_for_update(
            &transaction,
            tenant_id,
            request.submission.group_id,
            actor_user_id,
        )
        .await?;
        let group_model = find_group_for_update(
            &transaction,
            tenant_id,
            request.submission.group_id,
        )
        .await?;
        require_application_group(&group_model)?;
        let policy = load_policy_for_locale(
            &transaction,
            tenant_id,
            request.submission.group_id,
            &context.locale,
        )
        .await?;
        ensure_loaded_policy_precondition(&request.expected_policy, &policy)?;
        if !policy.enabled {
            return Err(GroupsError::Conflict(
                "group membership applications are disabled".to_string(),
            ));
        }
        validate_submission(&policy, &request.submission)?;

        let existing_membership = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(request.submission.group_id))
            .filter(membership::Column::UserId.eq(actor_user_id))
            .one(&transaction)
            .await?;
        if existing_membership
            .as_ref()
            .is_some_and(|row| row.status == GroupMembershipStatus::Banned.as_str())
        {
            return Err(GroupsError::Forbidden(
                "group membership is banned".to_string(),
            ));
        }
        if existing_membership
            .as_ref()
            .is_some_and(|row| row.status == GroupMembershipStatus::Active.as_str())
        {
            return Err(GroupsError::Conflict(
                "user is already an active group member".to_string(),
            ));
        }

        let existing_application = match prelocked_application {
            Some(existing) => Some(existing),
            None => {
                find_candidate_application_for_update(
                    &transaction,
                    tenant_id,
                    request.submission.group_id,
                    actor_user_id,
                )
                .await?
            }
        };
        if existing_application
            .as_ref()
            .is_some_and(|row| row.status == GroupApplicationStatus::Pending.as_str())
        {
            return Err(GroupsError::Conflict(
                "a membership application is already pending".to_string(),
            ));
        }
        if existing_application
            .as_ref()
            .is_some_and(|row| row.status == GroupApplicationStatus::Approved.as_str())
        {
            return Err(GroupsError::Conflict(
                "membership application is already approved".to_string(),
            ));
        }

        let now = Utc::now();
        let membership_model = if let Some(existing) = existing_membership {
            let mut active: membership::ActiveModel = existing.into();
            active.role = Set(GroupRole::Member.as_str().to_string());
            active.status = Set(GroupMembershipStatus::Pending.as_str().to_string());
            active.joined_at = Set(None);
            active.left_at = Set(None);
            active.updated_at = Set(now.fixed_offset());
            active.update(&transaction).await?
        } else {
            membership::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                group_id: Set(request.submission.group_id),
                user_id: Set(actor_user_id),
                role: Set(GroupRole::Member.as_str().to_string()),
                status: Set(GroupMembershipStatus::Pending.as_str().to_string()),
                invited_by_user_id: Set(None),
                joined_at: Set(None),
                left_at: Set(None),
                metadata: Set(json!({})),
                created_at: Set(now.fixed_offset()),
                updated_at: Set(now.fixed_offset()),
            }
            .insert(&transaction)
            .await?
        };

        let policy_snapshot = json!({
            "questions": policy.questions,
            "rules": policy.rules
        });
        let answers_value = serde_json::to_value(&request.submission.answers).map_err(|error| {
            GroupsError::Invariant(format!(
                "application answers are not serializable: {error}"
            ))
        })?;
        let acknowledged_value = serde_json::to_value(
            &request.submission.acknowledged_rule_keys,
        )
        .map_err(|error| {
            GroupsError::Invariant(format!(
                "application acknowledgements are not serializable: {error}"
            ))
        })?;
        let application_model = if let Some(existing) = existing_application {
            let mut active: membership_application::ActiveModel = existing.into();
            active.policy_id = Set(policy.id);
            active.policy_revision = Set(policy.revision as i64);
            active.policy_locale = Set(policy.locale.clone());
            active.policy_snapshot = Set(policy_snapshot);
            active.answers = Set(answers_value);
            active.acknowledged_rule_keys = Set(acknowledged_value);
            active.status = Set(GroupApplicationStatus::Pending.as_str().to_string());
            active.submitted_at = Set(now.fixed_offset());
            active.reviewed_at = Set(None);
            active.reviewed_by_user_id = Set(None);
            active.review_note = Set(None);
            active.updated_at = Set(now.fixed_offset());
            active.update(&transaction).await?
        } else {
            membership_application::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                group_id: Set(request.submission.group_id),
                user_id: Set(actor_user_id),
                policy_id: Set(policy.id),
                policy_revision: Set(policy.revision as i64),
                policy_locale: Set(policy.locale.clone()),
                policy_snapshot: Set(policy_snapshot),
                answers: Set(answers_value),
                acknowledged_rule_keys: Set(acknowledged_value),
                status: Set(GroupApplicationStatus::Pending.as_str().to_string()),
                submitted_at: Set(now.fixed_offset()),
                reviewed_at: Set(None),
                reviewed_by_user_id: Set(None),
                review_note: Set(None),
                created_at: Set(now.fixed_offset()),
                updated_at: Set(now.fixed_offset()),
            }
            .insert(&transaction)
            .await?
        };

        let group_version = increment_group_version(&transaction, group_model, now).await?;
        let result = SubmitGroupMembershipApplicationResult {
            application: map_application(application_model)?,
            membership: map_membership(membership_model)?,
            group_version,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            request.submission.group_id,
            actor_user_id,
            "group.membership_application_submitted",
            Some(actor_user_id),
            json!({
                "application_id": result.application.id,
                "policy_id": result.application.policy_id,
                "policy_revision": result.application.policy_revision,
                "policy_locale": result.application.policy_locale,
                "group_version": group_version,
                "expected_revision_enforced": true,
                "application_lock_order": "application_then_group_when_existing"
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            request.submission.group_id,
            actor_user_id,
            idempotency_key,
            SUBMIT_APPLICATION_IF_CURRENT_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }
}

#[async_trait]
impl GroupApplicationCasCommandPort for GroupApplicationService {
    async fn upsert_group_application_policy_if_current(
        &self,
        context: PortContext,
        request: UpsertGroupApplicationPolicyIfCurrentRequest,
    ) -> Result<UpsertGroupApplicationPolicyResult, PortError> {
        self.upsert_policy_if_current_owned(&context, request)
            .await
            .map_err(map_application_cas_error)
    }

    async fn submit_group_membership_application_if_current(
        &self,
        context: PortContext,
        request: SubmitGroupMembershipApplicationIfCurrentRequest,
    ) -> Result<SubmitGroupMembershipApplicationResult, PortError> {
        self.submit_application_if_current_owned(&context, request)
            .await
            .map_err(map_application_cas_error)
    }
}

async fn find_candidate_application_for_update(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
) -> GroupsResult<Option<membership_application::Model>> {
    let query = || {
        membership_application::Entity::find()
            .filter(membership_application::Column::TenantId.eq(tenant_id))
            .filter(membership_application::Column::GroupId.eq(group_id))
            .filter(membership_application::Column::UserId.eq(user_id))
    };
    let model = match transaction.get_database_backend() {
        DbBackend::Sqlite => query().one(transaction).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(transaction).await?,
    };
    Ok(model)
}

fn normalize_policy_precondition(
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

fn ensure_policy_update_precondition(
    expected: Option<&GroupApplicationPolicyPrecondition>,
    current: Option<&membership_policy::Model>,
    locale: &str,
) -> GroupsResult<()> {
    let matches = match (expected, current) {
        (None, None) => true,
        (Some(expected), Some(current)) => {
            expected.policy_id == current.id
                && expected.revision == current.revision.max(1) as u64
                && expected.locale == locale
        }
        _ => false,
    };
    if matches {
        Ok(())
    } else {
        Err(policy_changed_error(
            expected,
            current.map(|value| (value.id, value.revision.max(1) as u64, locale)),
        ))
    }
}

fn ensure_loaded_policy_precondition(
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

fn policy_changed_error(
    expected: Option<&GroupApplicationPolicyPrecondition>,
    current: Option<(Uuid, u64, &str)>,
) -> GroupsError {
    let expected_description = expected
        .map(|value| {
            format!(
                "{}@{}:{}",
                value.policy_id, value.revision, value.locale
            )
        })
        .unwrap_or_else(|| "none".to_string());
    let current_description = current
        .map(|(policy_id, revision, locale)| {
            format!("{policy_id}@{revision}:{locale}")
        })
        .unwrap_or_else(|| "none".to_string());
    GroupsError::Conflict(format!(
        "{GROUP_APPLICATION_POLICY_CHANGED_CODE}: expected={expected_description}; current={current_description}"
    ))
}

fn map_application_cas_error(error: GroupsError) -> PortError {
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
