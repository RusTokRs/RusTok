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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_reason_is_bounded_and_control_free() {
        assert!(validate_recovery_reason("operator approved retry").is_ok());
        assert!(validate_recovery_reason("").is_err());
        assert!(validate_recovery_reason(" padded").is_err());
        assert!(validate_recovery_reason("line\nbreak").is_err());
        assert!(validate_recovery_reason(&"x".repeat(513)).is_err());
    }

    #[test]
    fn requeue_sql_repeats_the_terminal_inspection_fence() {
        let request = CommentsTcpDelegationScheduleAuditRecoveryRequest::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            8,
            2,
            "operator approved retry",
        )
        .unwrap();
        let sql = requeue_source_statement(&request, 3).sql;
        assert!(sql.contains("handoff_attempt_count = $2"));
        assert!(sql.contains("handoff_recovery_epoch = $3"));
        assert!(sql.contains("handoff_claim_token IS NULL"));
        assert!(sql.contains("handoff_dead_letter_reason = 'attempt_budget_exhausted'"));
        assert!(sql.contains("handoff_attempt_count = 0"));
    }
}
