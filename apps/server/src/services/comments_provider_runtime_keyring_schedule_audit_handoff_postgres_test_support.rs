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
