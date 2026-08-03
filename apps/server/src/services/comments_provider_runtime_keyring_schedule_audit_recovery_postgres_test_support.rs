#[cfg(test)]
impl PostgresCommentsTcpDelegationScheduleAuditRecoveryStore {
    pub(crate) async fn reconcile_requeue_for_test(
        &self,
        audit_id: Uuid,
        request: &CommentsTcpDelegationScheduleAuditRecoveryRequest,
        recovery_epoch: i64,
    ) -> std::result::Result<
        CommentsTcpDelegationScheduleAuditRecoveryOutcome,
        CommentsTcpDelegationScheduleAuditRecoveryError,
    > {
        self.reconcile_requeue(audit_id, request, recovery_epoch)
            .await
    }
}
