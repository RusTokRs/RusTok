use std::collections::BTreeSet;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::application_entities::membership_application;
use crate::applications_legacy_module::{
    BulkReviewGroupMembershipApplicationItemResult,
    BulkReviewGroupMembershipApplicationsRequest, BulkReviewGroupMembershipApplicationsResult,
    CancelGroupMembershipApplicationRequest, GroupApplicationBulkReviewCommandPort,
    GroupApplicationCasCommandPort, GroupApplicationCommandPort,
    GroupApplicationLifecycleCommandPort, GroupApplicationLifecycleReadPort,
    GroupApplicationLifecycleResult, GroupApplicationPolicyLocaleCatalog,
    GroupApplicationPolicyManagementReadPort, GroupApplicationPolicyManagementView,
    GroupApplicationReadPort, GroupApplicationReviewCommandPort,
    GroupMembershipApplication, GroupMembershipApplicationConnection,
    ListGroupApplicationPolicyLocalesRequest, ListGroupMembershipApplicationsRequest,
    ReadGroupApplicationPolicyForManagementRequest, ReadGroupApplicationPolicyRequest,
    ReadMyGroupMembershipApplicationRequest, ReopenGroupMembershipApplicationRequest,
    ReviewGroupMembershipApplicationRequest, ReviewGroupMembershipApplicationResult,
    SubmitGroupMembershipApplicationIfCurrentRequest, SubmitGroupMembershipApplicationRequest,
    SubmitGroupMembershipApplicationResult, UpsertGroupApplicationPolicyIfCurrentRequest,
    UpsertGroupApplicationPolicyRequest, UpsertGroupApplicationPolicyResult,
};
use crate::effective_membership_guard::{
    GroupManagerCapability, actor_user_id, has_existing_receipt, require_candidate_not_denied,
    require_effective_manager, require_user_not_denied, tenant_id,
};

const MAX_BULK_REVIEW_ITEMS: usize = 50;
const MAX_REVIEW_NOTE_CHARS: usize = 2_000;

#[derive(Clone)]
pub struct GroupApplicationService {
    db: DatabaseConnection,
    legacy: crate::applications_legacy_module::GroupApplicationService,
}

impl GroupApplicationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            legacy: crate::applications_legacy_module::GroupApplicationService::new(db.clone()),
            db,
        }
    }

    async fn application_subject(
        &self,
        context: &PortContext,
        application_id: Uuid,
    ) -> Result<Option<(Uuid, Uuid)>, PortError> {
        let tenant_id = tenant_id(context)?;
        membership_application::Entity::find()
            .filter(membership_application::Column::TenantId.eq(tenant_id))
            .filter(membership_application::Column::Id.eq(application_id))
            .one(&self.db)
            .await
            .map(|row| row.map(|row| (row.group_id, row.user_id)))
            .map_err(|error| {
                PortError::unavailable("groups.application_lookup_unavailable", error.to_string())
            })
    }

    async fn precheck_review(
        &self,
        context: &PortContext,
        application_id: Uuid,
    ) -> Result<(), PortError> {
        if has_existing_receipt(&self.db, context).await? {
            return Ok(());
        }
        let Some((group_id, candidate_user_id)) =
            self.application_subject(context, application_id).await?
        else {
            return Ok(());
        };
        require_effective_manager(
            &self.db,
            context,
            group_id,
            GroupManagerCapability::Moderate,
        )
        .await?;
        require_user_not_denied(
            &self.db,
            tenant_id(context)?,
            group_id,
            candidate_user_id,
            true,
        )
        .await
    }

    async fn review_effective(
        &self,
        context: PortContext,
        request: ReviewGroupMembershipApplicationRequest,
    ) -> Result<ReviewGroupMembershipApplicationResult, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        self.precheck_review(&context, request.application_id).await?;
        GroupApplicationReviewCommandPort::review_group_membership_application(
            &self.legacy,
            context,
            request,
        )
        .await
    }
}

#[async_trait]
impl GroupApplicationReadPort for GroupApplicationService {
    async fn read_group_application_policy(
        &self,
        context: PortContext,
        request: ReadGroupApplicationPolicyRequest,
    ) -> Result<crate::applications_legacy_module::GroupApplicationPolicy, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        require_candidate_not_denied(&self.db, &context, request.group_id, false).await?;
        GroupApplicationReadPort::read_group_application_policy(&self.legacy, context, request).await
    }

    async fn list_group_membership_applications(
        &self,
        context: PortContext,
        request: ListGroupMembershipApplicationsRequest,
    ) -> Result<GroupMembershipApplicationConnection, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        require_effective_manager(
            &self.db,
            &context,
            request.group_id,
            GroupManagerCapability::Moderate,
        )
        .await?;
        GroupApplicationReadPort::list_group_membership_applications(
            &self.legacy,
            context,
            request,
        )
        .await
    }
}

#[async_trait]
impl GroupApplicationCommandPort for GroupApplicationService {
    async fn upsert_group_application_policy(
        &self,
        context: PortContext,
        request: UpsertGroupApplicationPolicyRequest,
    ) -> Result<UpsertGroupApplicationPolicyResult, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        if !has_existing_receipt(&self.db, &context).await? {
            require_effective_manager(
                &self.db,
                &context,
                request.group_id,
                GroupManagerCapability::ManageSettings,
            )
            .await?;
        }
        GroupApplicationCommandPort::upsert_group_application_policy(
            &self.legacy,
            context,
            request,
        )
        .await
    }

    async fn submit_group_membership_application(
        &self,
        context: PortContext,
        request: SubmitGroupMembershipApplicationRequest,
    ) -> Result<SubmitGroupMembershipApplicationResult, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        if !has_existing_receipt(&self.db, &context).await? {
            require_candidate_not_denied(&self.db, &context, request.group_id, true).await?;
        }
        GroupApplicationCommandPort::submit_group_membership_application(
            &self.legacy,
            context,
            request,
        )
        .await
    }

    async fn review_group_membership_application(
        &self,
        context: PortContext,
        request: ReviewGroupMembershipApplicationRequest,
    ) -> Result<ReviewGroupMembershipApplicationResult, PortError> {
        self.review_effective(context, request).await
    }
}

#[async_trait]
impl GroupApplicationReviewCommandPort for GroupApplicationService {
    async fn review_group_membership_application(
        &self,
        context: PortContext,
        request: ReviewGroupMembershipApplicationRequest,
    ) -> Result<ReviewGroupMembershipApplicationResult, PortError> {
        self.review_effective(context, request).await
    }
}

#[async_trait]
impl GroupApplicationCasCommandPort for GroupApplicationService {
    async fn upsert_group_application_policy_if_current(
        &self,
        context: PortContext,
        request: UpsertGroupApplicationPolicyIfCurrentRequest,
    ) -> Result<UpsertGroupApplicationPolicyResult, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        if !has_existing_receipt(&self.db, &context).await? {
            require_effective_manager(
                &self.db,
                &context,
                request.policy.group_id,
                GroupManagerCapability::ManageSettings,
            )
            .await?;
        }
        GroupApplicationCasCommandPort::upsert_group_application_policy_if_current(
            &self.legacy,
            context,
            request,
        )
        .await
    }

    async fn submit_group_membership_application_if_current(
        &self,
        context: PortContext,
        request: SubmitGroupMembershipApplicationIfCurrentRequest,
    ) -> Result<SubmitGroupMembershipApplicationResult, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        if !has_existing_receipt(&self.db, &context).await? {
            require_candidate_not_denied(
                &self.db,
                &context,
                request.submission.group_id,
                true,
            )
            .await?;
        }
        GroupApplicationCasCommandPort::submit_group_membership_application_if_current(
            &self.legacy,
            context,
            request,
        )
        .await
    }
}

#[async_trait]
impl GroupApplicationLifecycleReadPort for GroupApplicationService {
    async fn read_my_group_membership_application(
        &self,
        context: PortContext,
        request: ReadMyGroupMembershipApplicationRequest,
    ) -> Result<Option<GroupMembershipApplication>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        require_candidate_not_denied(&self.db, &context, request.group_id, false).await?;
        GroupApplicationLifecycleReadPort::read_my_group_membership_application(
            &self.legacy,
            context,
            request,
        )
        .await
    }
}

#[async_trait]
impl GroupApplicationLifecycleCommandPort for GroupApplicationService {
    async fn cancel_group_membership_application(
        &self,
        context: PortContext,
        request: CancelGroupMembershipApplicationRequest,
    ) -> Result<GroupApplicationLifecycleResult, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        if !has_existing_receipt(&self.db, &context).await? {
            if let Some((group_id, candidate_user_id)) =
                self.application_subject(&context, request.application_id).await?
            {
                let actor_user_id = actor_user_id(&context)?;
                if actor_user_id == candidate_user_id {
                    require_candidate_not_denied(&self.db, &context, group_id, false).await?;
                }
            }
        }
        GroupApplicationLifecycleCommandPort::cancel_group_membership_application(
            &self.legacy,
            context,
            request,
        )
        .await
    }

    async fn reopen_group_membership_application(
        &self,
        context: PortContext,
        request: ReopenGroupMembershipApplicationRequest,
    ) -> Result<GroupApplicationLifecycleResult, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        if !has_existing_receipt(&self.db, &context).await? {
            if let Some((group_id, candidate_user_id)) =
                self.application_subject(&context, request.application_id).await?
            {
                require_effective_manager(
                    &self.db,
                    &context,
                    group_id,
                    GroupManagerCapability::Moderate,
                )
                .await?;
                require_user_not_denied(
                    &self.db,
                    tenant_id(&context)?,
                    group_id,
                    candidate_user_id,
                    true,
                )
                .await?;
            }
        }
        GroupApplicationLifecycleCommandPort::reopen_group_membership_application(
            &self.legacy,
            context,
            request,
        )
        .await
    }
}

#[async_trait]
impl GroupApplicationPolicyManagementReadPort for GroupApplicationService {
    async fn list_group_application_policy_locales(
        &self,
        context: PortContext,
        request: ListGroupApplicationPolicyLocalesRequest,
    ) -> Result<GroupApplicationPolicyLocaleCatalog, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        require_effective_manager(
            &self.db,
            &context,
            request.group_id,
            GroupManagerCapability::ManageSettings,
        )
        .await?;
        GroupApplicationPolicyManagementReadPort::list_group_application_policy_locales(
            &self.legacy,
            context,
            request,
        )
        .await
    }

    async fn read_group_application_policy_for_management(
        &self,
        context: PortContext,
        request: ReadGroupApplicationPolicyForManagementRequest,
    ) -> Result<GroupApplicationPolicyManagementView, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        require_effective_manager(
            &self.db,
            &context,
            request.group_id,
            GroupManagerCapability::ManageSettings,
        )
        .await?;
        GroupApplicationPolicyManagementReadPort::read_group_application_policy_for_management(
            &self.legacy,
            context,
            request,
        )
        .await
    }
}

#[async_trait]
impl GroupApplicationBulkReviewCommandPort for GroupApplicationService {
    async fn bulk_review_group_membership_applications(
        &self,
        context: PortContext,
        request: BulkReviewGroupMembershipApplicationsRequest,
    ) -> Result<BulkReviewGroupMembershipApplicationsResult, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        if !request.confirmed {
            return Err(PortError::validation(
                "groups.bulk_review_confirmation_required",
                "bulk membership application review requires explicit confirmation",
            ));
        }
        if request.application_ids.is_empty() {
            return Err(PortError::validation(
                "groups.bulk_review_empty",
                "bulk membership application review requires at least one application",
            ));
        }
        if request.application_ids.len() > MAX_BULK_REVIEW_ITEMS {
            return Err(PortError::validation(
                "groups.bulk_review_limit_exceeded",
                format!(
                    "bulk membership application review accepts at most {MAX_BULK_REVIEW_ITEMS} applications"
                ),
            ));
        }

        let mut unique_ids = BTreeSet::new();
        for application_id in &request.application_ids {
            if !unique_ids.insert(*application_id) {
                return Err(PortError::validation(
                    "groups.bulk_review_duplicate_application",
                    "bulk membership application review contains duplicate application IDs",
                ));
            }
        }
        let normalized_note = normalize_bulk_note(request.note)?;
        let base_idempotency_key = context
            .idempotency_key
            .as_deref()
            .ok_or_else(|| {
                PortError::validation(
                    "port.idempotency_key_required",
                    "write port calls require a non-empty idempotency key",
                )
            })?
            .to_string();

        let mut items = Vec::with_capacity(request.application_ids.len());
        let mut succeeded = 0_u32;
        let mut failed = 0_u32;
        for application_id in request.application_ids {
            let item_context = context
                .clone()
                .with_causation_id(context.correlation_id.clone())
                .with_idempotency_key(bulk_review_item_idempotency_key(
                    &base_idempotency_key,
                    application_id,
                ));
            let item_request = ReviewGroupMembershipApplicationRequest {
                application_id,
                decision: request.decision,
                note: normalized_note.clone(),
            };
            match self.review_effective(item_context, item_request).await {
                Ok(result) => {
                    succeeded = succeeded.saturating_add(1);
                    items.push(BulkReviewGroupMembershipApplicationItemResult {
                        application_id,
                        result: Some(result),
                        error: None,
                    });
                }
                Err(error) => {
                    failed = failed.saturating_add(1);
                    items.push(BulkReviewGroupMembershipApplicationItemResult {
                        application_id,
                        result: None,
                        error: Some(error),
                    });
                }
            }
        }

        Ok(BulkReviewGroupMembershipApplicationsResult {
            items,
            succeeded,
            failed,
        })
    }
}

fn normalize_bulk_note(note: Option<String>) -> Result<Option<String>, PortError> {
    let note = note.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    });
    if note
        .as_deref()
        .is_some_and(|value| value.chars().count() > MAX_REVIEW_NOTE_CHARS)
    {
        return Err(PortError::validation(
            "groups.validation",
            format!("review note must not exceed {MAX_REVIEW_NOTE_CHARS} characters"),
        ));
    }
    Ok(note)
}

fn bulk_review_item_idempotency_key(base_idempotency_key: &str, application_id: Uuid) -> String {
    let mut hasher = Sha256::new();
    hasher.update(base_idempotency_key.as_bytes());
    hasher.update(application_id.as_bytes());
    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut encoded, "{byte:02x}");
    }
    format!("groups-bulk-review:{encoded}")
}
