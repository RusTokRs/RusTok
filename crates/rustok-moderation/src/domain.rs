use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModerationSubjectKind {
    ForumTopic,
    ForumPost,
    BlogPost,
    Comment,
    Group,
    GroupMembership,
    Review,
    ReviewResponse,
    Product,
    MarketplaceListing,
    SellerProfile,
    Message,
    MediaAsset,
    UserProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModerationScopeKind {
    Platform,
    Group,
    Page,
    ForumCategory,
    MarketplaceStore,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModerationScopeRef {
    pub kind: ModerationScopeKind,
    pub id: Option<Uuid>,
}

impl ModerationScopeRef {
    pub const fn platform() -> Self {
        Self {
            kind: ModerationScopeKind::Platform,
            id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModerationSubjectRef {
    pub module: String,
    pub kind: ModerationSubjectKind,
    pub id: Uuid,
    pub revision: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModerationReasonCode {
    Spam,
    Fraud,
    Harassment,
    HateOrAbuse,
    Threats,
    SexualContent,
    Violence,
    IllegalGoods,
    Counterfeit,
    Misinformation,
    PersonalDataExposure,
    Copyright,
    Impersonation,
    ManipulatedRating,
    OffTopic,
    DuplicateContent,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModerationReporterKind {
    User,
    Moderator,
    DomainModule,
    AutomatedProvider,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModerationReportStatus {
    Submitted,
    Attached,
    Dismissed,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModerationCaseStatus {
    Open,
    Assigned,
    Investigating,
    AwaitingEvidence,
    Decided,
    ApplyingDecision,
    Closed,
    Escalated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModerationCasePriority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModerationDecisionKind {
    NoViolation,
    Warning,
    Hide,
    Unpublish,
    Remove,
    Lock,
    RestrictInteraction,
    RequireEdit,
    RejectPublication,
    SuspendSubject,
    Escalate,
    AccountSanctionRecommended,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubmitModerationReportCommand {
    pub idempotency_key: String,
    pub request_hash: String,
    pub scope: ModerationScopeRef,
    pub subject: ModerationSubjectRef,
    pub reporter_kind: ModerationReporterKind,
    pub reporter_id: Option<Uuid>,
    pub reason_code: ModerationReasonCode,
    pub description_reference: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenModerationCaseCommand {
    pub idempotency_key: String,
    pub request_hash: String,
    pub scope: ModerationScopeRef,
    pub subject: ModerationSubjectRef,
    pub queue_key: String,
    pub priority: ModerationCasePriority,
    pub policy_id: Option<Uuid>,
    pub policy_version: i32,
    pub report_ids: Vec<Uuid>,
    pub metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssignModerationCaseCommand {
    pub case_id: Uuid,
    pub expected_revision: i64,
    pub moderator_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecideModerationCaseCommand {
    pub case_id: Uuid,
    pub expected_revision: i64,
    pub decision_kind: ModerationDecisionKind,
    pub reason_code: ModerationReasonCode,
    pub policy_snapshot: Value,
    pub decision_hash: String,
    pub decided_by: Uuid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApplyModerationDecisionCommand {
    pub decision_id: Uuid,
    pub subject: ModerationSubjectRef,
    pub decision_kind: ModerationDecisionKind,
    pub reason_code: ModerationReasonCode,
    pub decision_hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModerationReportRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub scope: ModerationScopeRef,
    pub subject: ModerationSubjectRef,
    pub reporter_kind: ModerationReporterKind,
    pub reporter_id: Option<Uuid>,
    pub reason_code: ModerationReasonCode,
    pub description_reference: Option<String>,
    pub status: ModerationReportStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModerationCaseRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub scope: ModerationScopeRef,
    pub subject: ModerationSubjectRef,
    pub queue_key: String,
    pub policy_id: Option<Uuid>,
    pub policy_version: i32,
    pub priority: ModerationCasePriority,
    pub status: ModerationCaseStatus,
    pub assigned_moderator_id: Option<Uuid>,
    pub revision: i64,
    pub opened_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub decided_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModerationDecisionRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub case_id: Uuid,
    pub decision_kind: ModerationDecisionKind,
    pub reason_code: ModerationReasonCode,
    pub policy_snapshot: Value,
    pub subject_revision: i64,
    pub decision_hash: String,
    pub decided_by: Uuid,
    pub decided_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModerationDecisionApplication {
    pub decision_id: Uuid,
    pub subject: ModerationSubjectRef,
    pub applied_revision: i64,
    pub applied_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModerationQueueFilter {
    pub queue_key: Option<String>,
    pub status: Option<ModerationCaseStatus>,
    pub priority: Option<ModerationCasePriority>,
    pub assigned_moderator_id: Option<Uuid>,
    pub limit: u32,
    pub cursor: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_scope_has_no_domain_id() {
        let scope = ModerationScopeRef::platform();
        assert_eq!(scope.kind, ModerationScopeKind::Platform);
        assert!(scope.id.is_none());
    }

    #[test]
    fn subject_reference_carries_source_revision() {
        let subject = ModerationSubjectRef {
            module: "forum".to_string(),
            kind: ModerationSubjectKind::ForumPost,
            id: Uuid::new_v4(),
            revision: 7,
        };

        assert_eq!(subject.revision, 7);
    }
}
