const MAX_BULK_REVIEW_ITEMS: usize = 50;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkReviewGroupMembershipApplicationsRequest {
    pub application_ids: Vec<Uuid>,
    pub decision: GroupApplicationReviewDecision,
    pub note: Option<String>,
    pub confirmed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkReviewGroupMembershipApplicationItemResult {
    pub application_id: Uuid,
    pub result: Option<ReviewGroupMembershipApplicationResult>,
    pub error: Option<PortError>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkReviewGroupMembershipApplicationsResult {
    pub items: Vec<BulkReviewGroupMembershipApplicationItemResult>,
    pub succeeded: u32,
    pub failed: u32,
}

#[async_trait]
pub trait GroupApplicationBulkReviewCommandPort: Send + Sync {
    async fn bulk_review_group_membership_applications(
        &self,
        context: PortContext,
        request: BulkReviewGroupMembershipApplicationsRequest,
    ) -> Result<BulkReviewGroupMembershipApplicationsResult, PortError>;
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
        let normalized_note = normalize_optional_note(request.note).map_err(PortError::from)?;

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
            match self.review_application_owned(&item_context, item_request).await {
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
                        error: Some(error.into()),
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

fn bulk_review_item_idempotency_key(
    base_idempotency_key: &str,
    application_id: Uuid,
) -> String {
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
