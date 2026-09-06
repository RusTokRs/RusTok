impl GroupTargetedInvitationService {
    pub(crate) async fn accept_targeted_group_invitation_effective_owned(
        &self,
        context: &PortContext,
        request: AcceptTargetedGroupInvitationRequest,
    ) -> GroupsResult<AcceptGroupInvitationResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<AcceptGroupInvitationResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            ACCEPT_TARGETED_INVITATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let invitation_model =
            find_invitation_for_update(&transaction, tenant_id, request.invitation_id).await?;
        let group_model =
            find_group_for_update(&transaction, tenant_id, invitation_model.group_id).await?;
        require_active_group(&group_model)?;
        crate::effective_membership_guard::require_user_not_denied_owned(
            &transaction,
            tenant_id,
            invitation_model.group_id,
            actor_user_id,
            true,
        )
        .await?;
        ensure_targeted_invitation_active(&invitation_model, actor_user_id)?;

        if redemption::Entity::find()
            .filter(redemption::Column::TenantId.eq(tenant_id))
            .filter(redemption::Column::InvitationId.eq(invitation_model.id))
            .filter(redemption::Column::UserId.eq(actor_user_id))
            .one(&transaction)
            .await?
            .is_some()
        {
            return Err(targeted_invitation_unavailable());
        }

        let existing_membership = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(invitation_model.group_id))
            .filter(membership::Column::UserId.eq(actor_user_id))
            .one(&transaction)
            .await?;

        let now = Utc::now();
        let membership_model = if let Some(existing) = existing_membership {
            let mut active: membership::ActiveModel = existing.into();
            active.role = Set(GroupRole::Member.as_str().to_string());
            active.status = Set(GroupMembershipStatus::Active.as_str().to_string());
            active.invited_by_user_id = Set(Some(invitation_model.invited_by_user_id));
            active.joined_at = Set(Some(now.fixed_offset()));
            active.left_at = Set(None);
            active.updated_at = Set(now.fixed_offset());
            active.update(&transaction).await?
        } else {
            membership::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                group_id: Set(invitation_model.group_id),
                user_id: Set(actor_user_id),
                role: Set(GroupRole::Member.as_str().to_string()),
                status: Set(GroupMembershipStatus::Active.as_str().to_string()),
                invited_by_user_id: Set(Some(invitation_model.invited_by_user_id)),
                joined_at: Set(Some(now.fixed_offset())),
                left_at: Set(None),
                metadata: Set(json!({})),
                created_at: Set(now.fixed_offset()),
                updated_at: Set(now.fixed_offset()),
            }
            .insert(&transaction)
            .await?
        };

        redemption::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            invitation_id: Set(invitation_model.id),
            group_id: Set(invitation_model.group_id),
            user_id: Set(actor_user_id),
            redeemed_at: Set(now.fixed_offset()),
        }
        .insert(&transaction)
        .await?;

        let invitation_id = invitation_model.id;
        let group_id = invitation_model.group_id;
        let mut invitation_active: invitation::ActiveModel = invitation_model.into();
        invitation_active.use_count = Set(1);
        invitation_active.updated_at = Set(now.fixed_offset());
        invitation_active.update(&transaction).await?;

        let group_version =
            increment_group_membership_version(&transaction, group_model, now).await?;
        let result = AcceptGroupInvitationResult {
            invitation_id,
            group_id,
            membership: map_membership(membership_model)?,
            group_version,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            group_id,
            actor_user_id,
            "group.targeted_invitation_accepted",
            Some(actor_user_id),
            json!({
                "invitation_id": invitation_id,
                "group_version": group_version
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            group_id,
            actor_user_id,
            idempotency_key,
            ACCEPT_TARGETED_INVITATION_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }
}
