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

impl ModerationSubjectKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ForumTopic => "forum_topic",
            Self::ForumPost => "forum_post",
            Self::BlogPost => "blog_post",
            Self::Comment => "comment",
            Self::Group => "group",
            Self::GroupMembership => "group_membership",
            Self::Review => "review",
            Self::ReviewResponse => "review_response",
            Self::Product => "product",
            Self::MarketplaceListing => "marketplace_listing",
            Self::SellerProfile => "seller_profile",
            Self::Message => "message",
            Self::MediaAsset => "media_asset",
            Self::UserProfile => "user_profile",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "forum_topic" => Some(Self::ForumTopic),
            "forum_post" => Some(Self::ForumPost),
            "blog_post" => Some(Self::BlogPost),
            "comment" => Some(Self::Comment),
            "group" => Some(Self::Group),
            "group_membership" => Some(Self::GroupMembership),
            "review" => Some(Self::Review),
            "review_response" => Some(Self::ReviewResponse),
            "product" => Some(Self::Product),
            "marketplace_listing" => Some(Self::MarketplaceListing),
            "seller_profile" => Some(Self::SellerProfile),
            "message" => Some(Self::Message),
            "media_asset" => Some(Self::MediaAsset),
            "user_profile" => Some(Self::UserProfile),
            _ => None,
        }
    }
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

impl ModerationScopeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Platform => "platform",
            Self::Group => "group",
            Self::Page => "page",
            Self::ForumCategory => "forum_category",
            Self::MarketplaceStore => "marketplace_store",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "platform" => Some(Self::Platform),
            "group" => Some(Self::Group),
            "page" => Some(Self::Page),
            "forum_category" => Some(Self::ForumCategory),
            "marketplace_store" => Some(Self::MarketplaceStore),
            _ => None,
        }
    }
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

impl ModerationReasonCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Spam => "spam",
            Self::Fraud => "fraud",
            Self::Harassment => "harassment",
            Self::HateOrAbuse => "hate_or_abuse",
            Self::Threats => "threats",
            Self::SexualContent => "sexual_content",
            Self::Violence => "violence",
            Self::IllegalGoods => "illegal_goods",
            Self::Counterfeit => "counterfeit",
            Self::Misinformation => "misinformation",
            Self::PersonalDataExposure => "personal_data_exposure",
            Self::Copyright => "copyright",
            Self::Impersonation => "impersonation",
            Self::ManipulatedRating => "manipulated_rating",
            Self::OffTopic => "off_topic",
            Self::DuplicateContent => "duplicate_content",
            Self::Other => "other",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "spam" => Some(Self::Spam),
            "fraud" => Some(Self::Fraud),
            "harassment" => Some(Self::Harassment),
            "hate_or_abuse" => Some(Self::HateOrAbuse),
            "threats" => Some(Self::Threats),
            "sexual_content" => Some(Self::SexualContent),
            "violence" => Some(Self::Violence),
            "illegal_goods" => Some(Self::IllegalGoods),
            "counterfeit" => Some(Self::Counterfeit),
            "misinformation" => Some(Self::Misinformation),
            "personal_data_exposure" => Some(Self::PersonalDataExposure),
            "copyright" => Some(Self::Copyright),
            "impersonation" => Some(Self::Impersonation),
            "manipulated_rating" => Some(Self::ManipulatedRating),
            "off_topic" => Some(Self::OffTopic),
            "duplicate_content" => Some(Self::DuplicateContent),
            "other" => Some(Self::Other),
            _ => None,
        }
    }
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

impl ModerationReporterKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Moderator => "moderator",
            Self::DomainModule => "domain_module",
            Self::AutomatedProvider => "automated_provider",
            Self::System => "system",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "user" => Some(Self::User),
            "moderator" => Some(Self::Moderator),
            "domain_module" => Some(Self::DomainModule),
            "automated_provider" => Some(Self::AutomatedProvider),
            "system" => Some(Self::System),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModerationReportStatus {
    Submitted,
    Attached,
    Dismissed,
    Closed,
}

impl ModerationReportStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Submitted => "submitted",
            Self::Attached => "attached",
            Self::Dismissed => "dismissed",
            Self::Closed => "closed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "submitted" => Some(Self::Submitted),
            "attached" => Some(Self::Attached),
            "dismissed" => Some(Self::Dismissed),
            "closed" => Some(Self::Closed),
            _ => None,
        }
    }
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

impl ModerationCaseStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Assigned => "assigned",
            Self::Investigating => "investigating",
            Self::AwaitingEvidence => "awaiting_evidence",
            Self::Decided => "decided",
            Self::ApplyingDecision => "applying_decision",
            Self::Closed => "closed",
            Self::Escalated => "escalated",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "open" => Some(Self::Open),
            "assigned" => Some(Self::Assigned),
            "investigating" => Some(Self::Investigating),
            "awaiting_evidence" => Some(Self::AwaitingEvidence),
            "decided" => Some(Self::Decided),
            "applying_decision" => Some(Self::ApplyingDecision),
            "closed" => Some(Self::Closed),
            "escalated" => Some(Self::Escalated),
            _ => None,
        }
    }

    pub const fn accepts_assignment(self) -> bool {
        matches!(self, Self::Open | Self::Assigned | Self::Investigating)
    }

    pub const fn accepts_decision(self) -> bool {
        matches!(
            self,
            Self::Open | Self::Assigned | Self::Investigating | Self::AwaitingEvidence
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModerationCasePriority {
    Low,
    Normal,
    High,
    Critical,
}

impl ModerationCasePriority {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "low" => Some(Self::Low),
            "normal" => Some(Self::Normal),
            "high" => Some(Self::High),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }
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

impl ModerationDecisionKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoViolation => "no_violation",
            Self::Warning => "warning",
            Self::Hide => "hide",
            Self::Unpublish => "unpublish",
            Self::Remove => "remove",
            Self::Lock => "lock",
            Self::RestrictInteraction => "restrict_interaction",
            Self::RequireEdit => "require_edit",
            Self::RejectPublication => "reject_publication",
            Self::SuspendSubject => "suspend_subject",
            Self::Escalate => "escalate",
            Self::AccountSanctionRecommended => "account_sanction_recommended",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "no_violation" => Some(Self::NoViolation),
            "warning" => Some(Self::Warning),
            "hide" => Some(Self::Hide),
            "unpublish" => Some(Self::Unpublish),
            "remove" => Some(Self::Remove),
            "lock" => Some(Self::Lock),
            "restrict_interaction" => Some(Self::RestrictInteraction),
            "require_edit" => Some(Self::RequireEdit),
            "reject_publication" => Some(Self::RejectPublication),
            "suspend_subject" => Some(Self::SuspendSubject),
            "escalate" => Some(Self::Escalate),
            "account_sanction_recommended" => Some(Self::AccountSanctionRecommended),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubmitModerationReportCommand {
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
    pub metadata: Value,
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
    pub metadata: Value,
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

impl Default for ModerationQueueFilter {
    fn default() -> Self {
        Self {
            queue_key: None,
            status: None,
            priority: None,
            assigned_moderator_id: None,
            limit: 50,
            cursor: None,
        }
    }
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

    #[test]
    fn stored_enum_values_round_trip() {
        assert_eq!(
            ModerationCaseStatus::parse(ModerationCaseStatus::AwaitingEvidence.as_str()),
            Some(ModerationCaseStatus::AwaitingEvidence)
        );
        assert_eq!(
            ModerationDecisionKind::parse(ModerationDecisionKind::RequireEdit.as_str()),
            Some(ModerationDecisionKind::RequireEdit)
        );
    }
}
