impl GroupApplicationService {
    pub(crate) async fn upsert_policy_effective_owned(
        &self,
        context: &PortContext,
        mut request: UpsertGroupApplicationPolicyRequest,
    ) -> GroupsResult<UpsertGroupApplicationPolicyResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        request.locale = normalize_locale_tag(&request.locale).ok_or_else(|| {
            GroupsError::Validation("invalid application policy locale".to_string())
        })?;
        normalize_policy(&mut request.questions, &mut request.rules)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<UpsertGroupApplicationPolicyResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            UPSERT_POLICY_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let group_model = find_group_for_update(&transaction, tenant_id, request.group_id).await?;
        require_active_group(&group_model)?;
        crate::effective_membership_guard::require_effective_manager_owned(
            &transaction,
            context,
            tenant_id,
            request.group_id,
            actor_user_id,
            crate::effective_membership_guard::GroupManagerCapability::ManageSettings,
        )
        .await?;

        let now = Utc::now();
        let existing_policy = membership_policy::Entity::find()
            .filter(membership_policy::Column::TenantId.eq(tenant_id))
            .filter(membership_policy::Column::GroupId.eq(request.group_id))
            .one(&transaction)
            .await?;
        let created = existing_policy.is_none();
        let policy_model = if let Some(existing) = existing_policy {
            let next_revision = existing.revision.saturating_add(1).max(1);
            let mut active: membership_policy::ActiveModel = existing.into();
            active.revision = Set(next_revision);
            active.enabled = Set(request.enabled);
            active.updated_by_user_id = Set(actor_user_id);
            active.updated_at = Set(now.fixed_offset());
            active.update(&transaction).await?
        } else {
            membership_policy::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                group_id: Set(request.group_id),
                revision: Set(1),
                enabled: Set(request.enabled),
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
            .filter(membership_policy_translation::Column::Locale.eq(request.locale.clone()))
            .one(&transaction)
            .await?;
        let questions_value = serde_json::to_value(&request.questions).map_err(|error| {
            GroupsError::Invariant(format!(
                "application questions are not serializable: {error}"
            ))
        })?;
        let rules_value = serde_json::to_value(&request.rules).map_err(|error| {
            GroupsError::Invariant(format!("application rules are not serializable: {error}"))
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
                locale: Set(request.locale.clone()),
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
                group_id: request.group_id,
                revision: policy_model.revision.max(1) as u64,
                enabled: policy_model.enabled,
                locale: request.locale,
                questions: request.questions,
                rules: request.rules,
            },
            group_version,
            created,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            request.group_id,
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
                "group_version": group_version
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            request.group_id,
            actor_user_id,
            idempotency_key,
            UPSERT_POLICY_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }

    pub(crate) async fn submit_application_effective_owned(
        &self,
        context: &PortContext,
        mut request: SubmitGroupMembershipApplicationRequest,
    ) -> GroupsResult<SubmitGroupMembershipApplicationResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        normalize_application_submission(&mut request)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<SubmitGroupMembershipApplicationResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            SUBMIT_APPLICATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let group_model = find_group_for_update(&transaction, tenant_id, request.group_id).await?;
        require_application_group(&group_model)?;
        let policy =
            load_policy_for_locale(&transaction, tenant_id, request.group_id, &context.locale)
                .await?;
        if !policy.enabled {
            return Err(GroupsError::Conflict(
                "group membership applications are disabled".to_string(),
            ));
        }
        validate_submission(&policy, &request)?;
        crate::effective_membership_guard::require_user_not_denied_owned(
            &transaction,
            tenant_id,
            request.group_id,
            actor_user_id,
            true,
        )
        .await?;

        let existing_membership = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(request.group_id))
            .filter(membership::Column::UserId.eq(actor_user_id))
            .one(&transaction)
            .await?;
        let existing_application = membership_application::Entity::find()
            .filter(membership_application::Column::TenantId.eq(tenant_id))
            .filter(membership_application::Column::GroupId.eq(request.group_id))
            .filter(membership_application::Column::UserId.eq(actor_user_id))
            .one(&transaction)
            .await?;
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
                group_id: Set(request.group_id),
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
        let answers_value = serde_json::to_value(&request.answers).map_err(|error| {
            GroupsError::Invariant(format!("application answers are not serializable: {error}"))
        })?;
        let acknowledged_value =
            serde_json::to_value(&request.acknowledged_rule_keys).map_err(|error| {
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
                group_id: Set(request.group_id),
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
            request.group_id,
            actor_user_id,
            "group.membership_application_submitted",
            Some(actor_user_id),
            json!({
                "application_id": result.application.id,
                "policy_id": result.application.policy_id,
                "policy_revision": result.application.policy_revision,
                "policy_locale": result.application.policy_locale,
                "group_version": group_version
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            request.group_id,
            actor_user_id,
            idempotency_key,
            SUBMIT_APPLICATION_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }

    pub(crate) async fn review_application_effective_owned(
        &self,
        context: &PortContext,
        mut request: ReviewGroupMembershipApplicationRequest,
    ) -> GroupsResult<ReviewGroupMembershipApplicationResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        request.note = normalize_optional_note(request.note)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<ReviewGroupMembershipApplicationResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            REVIEW_APPLICATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let application_model =
            find_application_for_update(&transaction, tenant_id, request.application_id).await?;
        let group_model =
            find_group_for_update(&transaction, tenant_id, application_model.group_id).await?;
        require_active_group(&group_model)?;
        crate::effective_membership_guard::require_effective_manager_owned(
            &transaction,
            context,
            tenant_id,
            application_model.group_id,
            actor_user_id,
            crate::effective_membership_guard::GroupManagerCapability::Moderate,
        )
        .await?;
        if application_model.status != GroupApplicationStatus::Pending.as_str() {
            return Err(GroupsError::Conflict(
                "membership application is no longer pending".to_string(),
            ));
        }
        crate::effective_membership_guard::require_user_not_denied_owned(
            &transaction,
            tenant_id,
            application_model.group_id,
            application_model.user_id,
            true,
        )
        .await?;
        let membership_model = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(application_model.group_id))
            .filter(membership::Column::UserId.eq(application_model.user_id))
            .one(&transaction)
            .await?
            .ok_or_else(|| {
                GroupsError::Invariant("pending application membership is missing".to_string())
            })?;

        let now = Utc::now();
        let approved = request.decision == GroupApplicationReviewDecision::Approve;
        let mut membership_active: membership::ActiveModel = membership_model.into();
        membership_active.role = Set(GroupRole::Member.as_str().to_string());
        membership_active.status = Set(if approved {
            GroupMembershipStatus::Active.as_str().to_string()
        } else {
            GroupMembershipStatus::Left.as_str().to_string()
        });
        membership_active.joined_at = Set(approved.then_some(now.fixed_offset()));
        membership_active.left_at = Set((!approved).then_some(now.fixed_offset()));
        membership_active.updated_at = Set(now.fixed_offset());
        let membership_model = membership_active.update(&transaction).await?;

        let mut application_active: membership_application::ActiveModel = application_model.into();
        application_active.status = Set(if approved {
            GroupApplicationStatus::Approved.as_str().to_string()
        } else {
            GroupApplicationStatus::Rejected.as_str().to_string()
        });
        application_active.reviewed_at = Set(Some(now.fixed_offset()));
        application_active.reviewed_by_user_id = Set(Some(actor_user_id));
        application_active.review_note = Set(request.note.clone());
        application_active.updated_at = Set(now.fixed_offset());
        let application_model = application_active.update(&transaction).await?;

        let group_version = if approved {
            increment_group_membership_version(&transaction, group_model, now).await?
        } else {
            increment_group_version(&transaction, group_model, now).await?
        };
        let result = ReviewGroupMembershipApplicationResult {
            application: map_application(application_model)?,
            membership: map_membership(membership_model)?,
            group_version,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            result.application.group_id,
            actor_user_id,
            if approved {
                "group.membership_application_approved"
            } else {
                "group.membership_application_rejected"
            },
            Some(result.application.user_id),
            json!({
                "application_id": result.application.id,
                "decision": if approved { "approve" } else { "reject" },
                "review_note_present": result.application.review_note.is_some(),
                "group_version": group_version
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            result.application.group_id,
            actor_user_id,
            idempotency_key,
            REVIEW_APPLICATION_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }
}
