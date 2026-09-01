impl PostgresCommentsTcpDelegationScheduleAuditCanonicalHandoff {
    /// Claims one source row that is ready under the durable retry policy.
    ///
    /// Deferred rows remain invisible until their retry timestamp is due,
    /// terminal source dead letters are excluded, and rows at the configured
    /// attempt budget are left for the bounded exhaustion sweep. Claiming clears
    /// the retry timestamp in the same statement so the retry/unclaimed database
    /// constraint remains true.
    pub async fn claim_next_retry_ready(
        &self,
        max_attempts: u32,
    ) -> std::result::Result<
        Option<CommentsTcpDelegationScheduleAuditHandoffClaim>,
        CommentsTcpDelegationScheduleAuditHandoffError,
    > {
        if max_attempts == 0 {
            return Err(CommentsTcpDelegationScheduleAuditHandoffError::Unavailable);
        }
        let max_attempts = i64::from(max_attempts);
        let claim_token = Uuid::new_v4();
        let transaction = self.database.begin().await.map_err(unavailable)?;
        let row = transaction
            .query_one_raw(claim_next_retry_ready_statement(
                claim_token,
                self.claim_ttl_seconds,
                max_attempts,
            ))
            .await
            .map_err(unavailable)?;
        let Some(row) = row else {
            transaction.commit().await.map_err(unavailable)?;
            return Ok(None);
        };
        let claim = decode_claim(&row, claim_token)?;
        match transaction.commit().await {
            Ok(()) => Ok(Some(claim)),
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    claim_token = %claim_token,
                    "Comments schedule audit retry-aware claim commit acknowledgement was ambiguous"
                );
                self.reconcile_claim(claim_token).await.map(Some)
            }
        }
    }
}

fn claim_next_retry_ready_statement(
    claim_token: Uuid,
    claim_ttl_seconds: i64,
    max_attempts: i64,
) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "WITH candidate AS ( \
                 SELECT request_id \
                 FROM {table} \
                 WHERE published_at IS NULL \
                   AND canonical_envelope_id IS NULL \
                   AND handoff_dead_lettered_at IS NULL \
                   AND handoff_attempt_count < $3 \
                   AND (handoff_next_attempt_at IS NULL OR handoff_next_attempt_at <= NOW()) \
                   AND (handoff_claim_token IS NULL OR handoff_claim_expires_at <= NOW()) \
                 ORDER BY created_at ASC, request_id ASC \
                 FOR UPDATE SKIP LOCKED \
                 LIMIT 1 \
             ) \
             UPDATE {table} AS audit \
             SET handoff_claim_token = $1, \
                 handoff_claim_expires_at = NOW() + ($2::bigint * INTERVAL '1 second'), \
                 handoff_next_attempt_at = NULL, \
                 handoff_attempt_count = handoff_attempt_count + 1 \
             FROM candidate \
             WHERE audit.request_id = candidate.request_id \
             RETURNING audit.request_id, audit.handoff_attempt_count",
            table = postgres_audit::COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_OUTBOX_TABLE,
        ),
        vec![
            claim_token.into(),
            claim_ttl_seconds.into(),
            max_attempts.into(),
        ],
    )
}

#[cfg(test)]
mod retry_ready_tests {
    use super::*;

    #[test]
    fn retry_ready_claim_excludes_deferred_exhausted_and_dead_letter_rows() {
        let statement = claim_next_retry_ready_statement(Uuid::new_v4(), 60, 8);
        let sql = statement.sql.as_str();
        assert!(sql.contains("handoff_dead_lettered_at IS NULL"));
        assert!(sql.contains("handoff_attempt_count < $3"));
        assert!(
            sql.contains("handoff_next_attempt_at IS NULL OR handoff_next_attempt_at <= NOW()")
        );
        assert!(sql.contains("handoff_next_attempt_at = NULL"));
        assert!(sql.contains("FOR UPDATE SKIP LOCKED"));
        assert!(sql.contains("LIMIT 1"));
    }
}
