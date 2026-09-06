use std::collections::BTreeSet;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::applications_legacy_module::{
    BulkReviewGroupMembershipApplicationItemResult, BulkReviewGroupMembershipApplicationsRequest,
    BulkReviewGroupMembershipApplicationsResult, CancelGroupMembershipApplicationRequest,
    GroupApplicationBulkReviewCommandPort, GroupApplicationCasCommandPort,
    GroupApplicationCommandPort, GroupApplicationLifecycleCommandPort,
    GroupApplicationLifecycleReadPort, GroupApplicationLifecycleResult,
    GroupApplicationPolicyLocaleCatalog, GroupApplicationPolicyManagementReadPort,
    GroupApplicationPolicyManagementView, GroupApplicationReadPort,
    GroupApplicationReviewCommandPort, GroupMembershipApplication,
    GroupMembershipApplicationConnection, ListGroupApplicationPolicyLocalesRequest,
    ListGroupMembershipApplicationsRequest, ReadGroupApplicationPolicyForManagementRequest,
    ReadGroupApplicationPolicyRequest, ReadMyGroupMembershipApplicationRequest,
    ReopenGroupMembershipApplicationRequest, ReviewGroupMembershipApplicationRequest,
    ReviewGroupMembershipApplicationResult, SubmitGroupMembershipApplicationIfCurrentRequest,
    SubmitGroupMembershipApplicationRequest, SubmitGroupMembershipApplicationResult,
    UpsertGroupApplicationPolicyIfCurrentRequest, UpsertGroupApplicationPolicyRequest,
    UpsertGroupApplicationPolicyResult,
};
use crate::domain::GroupVisibility;
use crate::effective_membership_guard::{
    GroupManagerCapability, actor_user_id, require_candidate_not_denied, require_effective_manager,
    tenant_id,
};
use crate::entities::group;

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

    /// Preserve application-surface non-disclosure before returning a membership-specific denial.
    /// Secret groups never accept applications, so candidate policy/current-state reads return
    /// not-found even when a historical membership enforcement row exists.
    async fn require_candidate_surface_visible(
        &self,
        context: &PortContext,
        group_id: Uuid,
    ) -> Result<(), PortError> {
        let tenant_id = tenant_id(context)?;
        let model = group::Entity::find()
            .filter(group::Column::TenantId.eq(tenant_id))
            .filter(group::Column::Id.eq(group_id))
            .one(&self.db)
            .await
            .map_err(|error| {
                PortError::unavailable("groups.group_lookup_unavailable", error.to_string())
            })?;
        match model {
            Some(model) if model.visibility == GroupVisibility::Secret.as_str() => Err(
                PortError::not_found("groups.not_found", "group was not found"),
            ),
            _ => Ok(()),
        }
    }

    fn validate_write_context(context: &PortContext) -> Result<(), PortError> {
        context.require_policy(PortCallPolicy::write())?;
        tenant_id(context)?;
        actor_user_id(context)?;
        Ok(())
    }

    async fn review_effective(
        &self,
        context: PortContext,
        request: ReviewGroupMembershipApplicationRequest,
    ) -> Result<ReviewGroupMembershipApplicationResult, PortError> {
        Self::validate_write_context(&context)?;
        self.legacy
            .review_application_effective_owned(&context, request)
            .await
            .map_err(Into::into)
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
        self.require_candidate_surface_visible(&context, request.group_id)
            .await?;
        require_candidate_not_denied(&self.db, &context, request.group_id, false).await?;
        GroupApplicationReadPort::read_group_application_policy(&self.legacy, context, request)
            .await
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
        GroupApplicationReadPort::list_group_membership_applications(&self.legacy, context, request)
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
        Self::validate_write_context(&context)?;
        self.legacy
            .upsert_policy_effective_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn submit_group_membership_application(
        &self,
        context: PortContext,
        request: SubmitGroupMembershipApplicationRequest,
    ) -> Result<SubmitGroupMembershipApplicationResult, PortError> {
        Self::validate_write_context(&context)?;
        self.legacy
            .submit_application_effective_owned(&context, request)
            .await
            .map_err(Into::into)
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
        Self::validate_write_context(&context)?;
        self.legacy
            .upsert_policy_if_current_effective_owned(&context, request)
            .await
            .map_err(crate::applications_legacy_module::map_effective_application_cas_error)
    }

    async fn submit_group_membership_application_if_current(
        &self,
        context: PortContext,
        request: SubmitGroupMembershipApplicationIfCurrentRequest,
    ) -> Result<SubmitGroupMembershipApplicationResult, PortError> {
        Self::validate_write_context(&context)?;
        self.legacy
            .submit_application_if_current_effective_owned(&context, request)
            .await
            .map_err(crate::applications_legacy_module::map_effective_application_cas_error)
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
        self.require_candidate_surface_visible(&context, request.group_id)
            .await?;
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
        Self::validate_write_context(&context)?;
        self.legacy
            .cancel_application_effective_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn reopen_group_membership_application(
        &self,
        context: PortContext,
        request: ReopenGroupMembershipApplicationRequest,
    ) -> Result<GroupApplicationLifecycleResult, PortError> {
        Self::validate_write_context(&context)?;
        self.legacy
            .reopen_application_effective_owned(&context, request)
            .await
            .map_err(Into::into)
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
        Self::validate_write_context(&context)?;
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
