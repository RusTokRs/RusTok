#[async_trait]
pub trait GroupApplicationReviewCommandPort: Send + Sync {
    async fn review_group_membership_application(
        &self,
        context: PortContext,
        request: ReviewGroupMembershipApplicationRequest,
    ) -> Result<ReviewGroupMembershipApplicationResult, PortError>;
}

#[async_trait]
impl GroupApplicationReviewCommandPort for GroupApplicationService {
    async fn review_group_membership_application(
        &self,
        context: PortContext,
        request: ReviewGroupMembershipApplicationRequest,
    ) -> Result<ReviewGroupMembershipApplicationResult, PortError> {
        self.review_application_owned(&context, request)
            .await
            .map_err(Into::into)
    }
}
