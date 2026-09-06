#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BulkReviewGroupMembershipApplicationsCommand {
    pub idempotency_key: String,
    pub application_ids: Vec<String>,
    pub decision: GroupsAdminApplicationReviewDecision,
    pub note: Option<String>,
    pub confirmed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GroupsAdminBulkReviewApplicationError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GroupsAdminBulkReviewApplicationItemResult {
    pub application_id: String,
    pub result: Option<GroupsAdminReviewApplicationResult>,
    pub error: Option<GroupsAdminBulkReviewApplicationError>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GroupsAdminBulkReviewApplicationsResult {
    pub items: Vec<GroupsAdminBulkReviewApplicationItemResult>,
    pub succeeded: u32,
    pub failed: u32,
}
