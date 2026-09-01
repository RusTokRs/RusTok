impl PostgresCommentsTcpDelegationScheduleAuditSourceRetryPolicy {
    /// Records a failed publication only while the exact claim is still active.
    ///
    /// This is the runner composition entry point. The expiry fence prevents a
    /// worker that resumed after its lease expired from scheduling a retry or
    /// terminalizing a row before a newer claimant takes ownership.
    pub async fn record_active_failure(
        &self,
        claim: CommentsTcpDelegationScheduleAuditHandoffClaim,
        failure: CommentsTcpDelegationScheduleAuditHandoffError,
    ) -> std::result::Result<
        CommentsTcpDelegationScheduleAuditSourceFailureTransition,
        CommentsTcpDelegationScheduleAuditSourceRetryPolicyError,
    > {
        validate_claim(claim)?;
        let row = self
            .database
            .query_one_raw(record_active_failure_statement(
                claim,
                failure.into(),
                self.max_attempts,
                self.retry_delay_seconds,
            ))
            .await
            .map_err(|_| CommentsTcpDelegationScheduleAuditSourceRetryPolicyError::Unavailable)?;
        let Some(row) = row else {
            return Ok(CommentsTcpDelegationScheduleAuditSourceFailureTransition::StaleClaim);
        };
        decode_failure_transition(&row)
    }
}

fn record_active_failure_statement(
    claim: CommentsTcpDelegationScheduleAuditHandoffClaim,
    failure_code: CommentsTcpDelegationScheduleAuditSourceFailureCode,
    max_attempts: i64,
    retry_delay_seconds: i64,
) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "UPDATE {table} \
             SET handoff_last_failure_at = NOW(), \
                 handoff_last_failure_code = $4, \
                 handoff_claim_token = NULL, \
                 handoff_claim_expires_at = NULL, \
                 handoff_next_attempt_at = CASE \
                     WHEN handoff_attempt_count >= $5 THEN NULL \
                     ELSE NOW() + ($6::bigint * INTERVAL '1 second') \
                 END, \
                 handoff_dead_lettered_at = CASE \
                     WHEN handoff_attempt_count >= $5 THEN NOW() \
                     ELSE NULL \
                 END, \
                 handoff_dead_letter_reason = CASE \
                     WHEN handoff_attempt_count >= $5 THEN '{dead_letter_reason}' \
                     ELSE NULL \
                 END \
             WHERE request_id = $1 \
               AND published_at IS NULL \
               AND canonical_envelope_id IS NULL \
               AND handoff_dead_lettered_at IS NULL \
               AND handoff_claim_token = $2 \
               AND handoff_attempt_count = $3 \
               AND handoff_claim_expires_at > NOW() \
             RETURNING request_id, handoff_attempt_count, \
                       handoff_dead_lettered_at IS NOT NULL AS dead_lettered",
            table = postgres_audit::COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_OUTBOX_TABLE,
            dead_letter_reason = DEAD_LETTER_REASON_ATTEMPT_BUDGET_EXHAUSTED,
        ),
        vec![
            claim.request_id().into(),
            claim.claim_token().into(),
            claim.attempt_count().into(),
            failure_code.as_str().into(),
            max_attempts.into(),
            retry_delay_seconds.into(),
        ],
    )
}

#[cfg(test)]
mod active_failure_tests {
    use super::*;

    #[test]
    fn runner_failure_recording_repeats_the_active_claim_fence() {
        let claim = CommentsTcpDelegationScheduleAuditHandoffClaim::from_parts_for_test(
            Uuid::new_v4(),
            Uuid::new_v4(),
            3,
        );
        let statement = record_active_failure_statement(
            claim,
            CommentsTcpDelegationScheduleAuditSourceFailureCode::Unavailable,
            8,
            30,
        );
        let sql = statement.sql.as_str();
        assert!(sql.contains("handoff_claim_token = $2"));
        assert!(sql.contains("handoff_attempt_count = $3"));
        assert!(sql.contains("handoff_claim_expires_at > NOW()"));
        assert!(sql.contains("handoff_next_attempt_at"));
        assert!(sql.contains("attempt_budget_exhausted"));
    }
}
