use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub use rustok_moderation_api::{
    ApplyModerationDecisionCommand, ModerationDecisionApplication, ModerationDecisionEffect,
    ModerationDecisionEffectAction, ModerationDecisionKind, ModerationEffectValidationError,
    ModerationReasonCode, ModerationScopeKind, ModerationScopeRef, ModerationSubjectKind,
    ModerationSubjectRef, ModerationVisibilityState,
};

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
    pub effect: ModerationDecisionEffect,
    pub policy_snapshot: Value,
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
    pub effect: Option<ModerationDecisionEffect>,
    pub policy_snapshot: Value,
    pub subject_revision: i64,
    pub decision_hash: String,
    pub decided_by: Uuid,
    pub decided_at: DateTime<Utc>,
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
