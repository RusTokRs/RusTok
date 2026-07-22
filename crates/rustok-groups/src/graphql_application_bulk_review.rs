#[derive(Default)]
pub struct GroupsApplicationBulkReviewMutation;

#[async_graphql::Object]
impl GroupsApplicationBulkReviewMutation {
    async fn bulk_review_group_membership_applications(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        input: BulkReviewGroupMembershipApplicationsInputGql,
    ) -> Result<BulkReviewGroupMembershipApplicationsResultGql> {
        let auth = require_authenticated(ctx)?;
        let result = crate::GroupApplicationBulkReviewCommandPort::bulk_review_group_membership_applications(
            &application_service(ctx)?,
            port_context(ctx, auth, Some(idempotency_key))?,
            crate::BulkReviewGroupMembershipApplicationsRequest {
                application_ids: input.application_ids,
                decision: input.decision.into(),
                note: input.note,
                confirmed: input.confirmed,
            },
        )
        .await
        .map_err(map_port_error)?;
        Ok(result.into())
    }
}

#[derive(async_graphql::InputObject)]
pub struct BulkReviewGroupMembershipApplicationsInputGql {
    pub application_ids: Vec<Uuid>,
    pub decision: GroupApplicationReviewDecisionGql,
    pub note: Option<String>,
    pub confirmed: bool,
}

#[derive(async_graphql::SimpleObject)]
pub struct BulkReviewGroupMembershipApplicationErrorGql {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(async_graphql::SimpleObject)]
pub struct BulkReviewGroupMembershipApplicationItemResultGql {
    pub application_id: Uuid,
    pub result: Option<ReviewGroupMembershipApplicationResultGql>,
    pub error: Option<BulkReviewGroupMembershipApplicationErrorGql>,
}

#[derive(async_graphql::SimpleObject)]
pub struct BulkReviewGroupMembershipApplicationsResultGql {
    pub items: Vec<BulkReviewGroupMembershipApplicationItemResultGql>,
    pub succeeded: u32,
    pub failed: u32,
}

impl From<crate::BulkReviewGroupMembershipApplicationsResult>
    for BulkReviewGroupMembershipApplicationsResultGql
{
    fn from(value: crate::BulkReviewGroupMembershipApplicationsResult) -> Self {
        Self {
            items: value
                .items
                .into_iter()
                .map(|item| BulkReviewGroupMembershipApplicationItemResultGql {
                    application_id: item.application_id,
                    result: item.result.map(Into::into),
                    error: item.error.map(|error| {
                        BulkReviewGroupMembershipApplicationErrorGql {
                            code: error.code,
                            message: error.message,
                            retryable: error.retryable,
                        }
                    }),
                })
                .collect(),
            succeeded: value.succeeded,
            failed: value.failed,
        }
    }
}
