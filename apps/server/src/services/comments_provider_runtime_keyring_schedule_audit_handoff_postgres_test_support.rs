#[cfg(test)]
impl CommentsTcpDelegationScheduleAuditHandoffClaim {
    pub(crate) fn from_parts_for_test(
        request_id: Uuid,
        claim_token: Uuid,
        attempt_count: i64,
    ) -> Self {
        Self {
            request_id,
            claim_token,
            attempt_count,
        }
    }
}

#[cfg(test)]
impl PostgresCommentsTcpDelegationScheduleAuditCanonicalHandoff {
    pub(crate) async fn reconcile_claim_for_test(
        &self,
        claim_token: Uuid,
    ) -> std::result::Result<
        CommentsTcpDelegationScheduleAuditHandoffClaim,
        CommentsTcpDelegationScheduleAuditHandoffError,
    > {
        self.reconcile_claim(claim_token).await
    }

    pub(crate) async fn reconcile_publication_for_test(
        &self,
        request_id: Uuid,
    ) -> std::result::Result<Uuid, CommentsTcpDelegationScheduleAuditHandoffError> {
        self.reconcile_publication(request_id).await
    }
}
