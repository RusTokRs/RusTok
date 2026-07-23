impl GroupInvitationService {
    pub(crate) async fn create_group_invitation_effective_owned(
        &self,
        context: &PortContext,
        request: CreateGroupInvitationRequest,
    ) -> GroupsResult<CreateGroupInvitationResult> {
        require_write(context)?;
        validate_create_request(&request)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;
        let group_model = find_group_for_update(&transaction, tenant_id, request.group_id).await?;

        if let Some(mut replayed) = replay_receipt::<CreateGroupInvitationResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            CREATE_INVITATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.token = None;
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        require_active_group(&group_model)?;
        crate::effective_membership_guard::require_effective_manager_owned(
            &transaction,
            context,
            tenant_id,
            request.group_id,
            actor_user_id,
            crate::effective_membership_guard::GroupManagerCapability::Moderate,
        )
        .await?;

        let now = Utc::now();
        let expires_at = now + Duration::seconds(request.expires_in_seconds as i64);
        let token = generate_invitation_token();
        let token_hash = invitation_token_hash(&token);
        let invitation_model = invitation::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            group_id: Set(request.group_id),
            invited_by_user_id: Set(actor_user_id),
            target_user_id: Set(request.target_user_id),
            token_hash: Set(token_hash),
            max_uses: Set(request.max_uses as i32),
            use_count: Set(0),
            expires_at: Set(expires_at.fixed_offset()),
            revoked_at: Set(None),
            revoked_by_user_id: Set(None),
            created_at: Set(now.fixed_offset()),
            updated_at: Set(now.fixed_offset()),
        }
        .insert(&transaction)
        .await?;

        let group_version = increment_group_version(&transaction, group_model, now).await?;
        let result = CreateGroupInvitationResult {
            invitation: map_invitation(invitation_model)?,
            token: Some(token),
            group_version,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            request.group_id,
            actor_user_id,
            "group.invitation_created",
            request.target_user_id,
            json!({
                "invitation_id": result.invitation.id,
                "target_user_id": request.target_user_id,
                "max_uses": request.max_uses,
                "expires_at": expires_at,
                "group_version": group_version
            }),
        )
        .await?;
        let mut stored_result = result.clone();
        stored_result.token = None;
        store_receipt(
            &transaction,
            tenant_id,
            request.group_id,
            actor_user_id,
            idempotency_key,
            CREATE_INVITATION_COMMAND,
            request_hash,
            &stored_result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }

    pub(crate) async fn revoke_group_invitation_effective_owned(
        &self,
        context: &PortContext,
        request: RevokeGroupInvitationRequest,
    ) -> GroupsResult<RevokeGroupInvitationResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;
        let invitation_model =
            find_invitation_for_update(&transaction, tenant_id, request.invitation_id).await?;

        if let Some(mut replayed) = replay_receipt::<RevokeGroupInvitationResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            REVOKE_INVITATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let group_model =
            find_group_for_update(&transaction, tenant_id, invitation_model.group_id).await?;
        crate::effective_membership_guard::require_effective_manager_owned(
            &transaction,
            context,
            tenant_id,
            invitation_model.group_id,
            actor_user_id,
            crate::effective_membership_guard::GroupManagerCapability::Moderate,
        )
        .await?;
        if invitation_model.revoked_at.is_some() {
            return Err(GroupsError::Conflict(
                "group invitation is already revoked".to_string(),
            ));
        }

        let now = Utc::now();
        let mut active: invitation::ActiveModel = invitation_model.into();
        active.revoked_at = Set(Some(now.fixed_offset()));
        active.revoked_by_user_id = Set(Some(actor_user_id));
        active.updated_at = Set(now.fixed_offset());
        let revoked = active.update(&transaction).await?;
        let group_version = increment_group_version(&transaction, group_model, now).await?;
        let result = RevokeGroupInvitationResult {
            invitation: map_invitation(revoked)?,
            group_version,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            result.invitation.group_id,
            actor_user_id,
            "group.invitation_revoked",
            result.invitation.target_user_id,
            json!({
                "invitation_id": result.invitation.id,
                "group_version": group_version
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            result.invitation.group_id,
            actor_user_id,
            idempotency_key,
            REVOKE_INVITATION_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }

    pub(crate) async fn accept_group_invitation_effective_owned(
        &self,
        context: &PortContext,
        request: AcceptGroupInvitationRequest,
    ) -> GroupsResult<AcceptGroupInvitationResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let token = request.token.trim();
        if token.len() < 32 || token.len() > 160 {
            return Err(invalid_invitation_token());
        }
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<AcceptGroupInvitationResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            ACCEPT_INVITATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let token_hash = invitation_token_hash(token);
        let invitation_model =
            find_invitation_by_token_for_update(&transaction, tenant_id, &token_hash).await?;
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
        ensure_invitation_active(&invitation_model, actor_user_id)?;

        if redemption::Entity::find()
            .filter(redemption::Column::TenantId.eq(tenant_id))
            .filter(redemption::Column::InvitationId.eq(invitation_model.id))
            .filter(redemption::Column::UserId.eq(actor_user_id))
            .one(&transaction)
            .await?
            .is_some()
        {
            return Err(GroupsError::Conflict(
                "group invitation was already accepted by this user".to_string(),
            ));
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
        let next_use_count = invitation_model.use_count.saturating_add(1);
        let mut invitation_active: invitation::ActiveModel = invitation_model.into();
        invitation_active.use_count = Set(next_use_count);
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
            "group.invitation_accepted",
            Some(actor_user_id),
            json!({
                "invitation_id": invitation_id,
                "use_count": next_use_count,
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
            ACCEPT_INVITATION_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }
}
