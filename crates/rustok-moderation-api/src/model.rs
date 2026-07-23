use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MODERATION_DECISION_EFFECT_SCHEMA_V1: u16 = 1;
pub const MAX_MODERATION_EFFECT_CAPABILITIES: usize = 32;
pub const MAX_MODERATION_CAPABILITY_KEY_BYTES: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ModerationCapabilityKey(String);

impl ModerationCapabilityKey {
    pub fn new(value: impl Into<String>) -> Result<Self, ModerationEffectValidationError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= MAX_MODERATION_CAPABILITY_KEY_BYTES
            && value.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b':' | b'_' | b'-')
            })
            && !value.starts_with(['.', ':', '_', '-'])
            && !value.ends_with(['.', ':', '_', '-']);
        if !valid {
            return Err(ModerationEffectValidationError::InvalidCapabilityKey);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl TryFrom<String> for ModerationCapabilityKey {
    type Error = ModerationEffectValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ModerationCapabilityKey> for String {
    fn from(value: ModerationCapabilityKey) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModerationVisibilityState {
    Hidden,
    Unpublished,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModerationDecisionEffectAction {
    NoDomainMutation,
    SetVisibility { state: ModerationVisibilityState },
    Lock { effective_until: Option<DateTime<Utc>> },
    RestrictInteraction {
        capabilities: Vec<ModerationCapabilityKey>,
        effective_until: Option<DateTime<Utc>>,
    },
    RequireEdit,
    RejectPublication,
    SuspendSubject { effective_until: Option<DateTime<Utc>> },
    Escalate,
    AccountSanctionRecommended { capabilities: Vec<ModerationCapabilityKey> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModerationDecisionEffect {
    pub schema_version: u16,
    pub action: ModerationDecisionEffectAction,
}

impl ModerationDecisionEffect {
    pub fn v1(action: ModerationDecisionEffectAction) -> Result<Self, ModerationEffectValidationError> {
        let effect = Self {
            schema_version: MODERATION_DECISION_EFFECT_SCHEMA_V1,
            action,
        };
        effect.validate()?;
        Ok(effect)
    }

    pub fn validate(&self) -> Result<(), ModerationEffectValidationError> {
        if self.schema_version != MODERATION_DECISION_EFFECT_SCHEMA_V1 {
            return Err(ModerationEffectValidationError::UnsupportedSchemaVersion);
        }
        match &self.action {
            ModerationDecisionEffectAction::RestrictInteraction { capabilities, .. }
            | ModerationDecisionEffectAction::AccountSanctionRecommended { capabilities } => {
                validate_capabilities(capabilities)?;
            }
            _ => {}
        }
        Ok(())
    }

    pub fn validate_for_decision_kind(
        &self,
        kind: ModerationDecisionKind,
    ) -> Result<(), ModerationEffectValidationError> {
        self.validate()?;
        let compatible = matches!(
            (kind, &self.action),
            (
                ModerationDecisionKind::NoViolation | ModerationDecisionKind::Warning,
                ModerationDecisionEffectAction::NoDomainMutation
            )
                | (
                    ModerationDecisionKind::Hide,
                    ModerationDecisionEffectAction::SetVisibility {
                        state: ModerationVisibilityState::Hidden
                    }
                )
                | (
                    ModerationDecisionKind::Unpublish,
                    ModerationDecisionEffectAction::SetVisibility {
                        state: ModerationVisibilityState::Unpublished
                    }
                )
                | (
                    ModerationDecisionKind::Remove,
                    ModerationDecisionEffectAction::SetVisibility {
                        state: ModerationVisibilityState::Removed
                    }
                )
                | (ModerationDecisionKind::Lock, ModerationDecisionEffectAction::Lock { .. })
                | (
                    ModerationDecisionKind::RestrictInteraction,
                    ModerationDecisionEffectAction::RestrictInteraction { .. }
                )
                | (
                    ModerationDecisionKind::RequireEdit,
                    ModerationDecisionEffectAction::RequireEdit
                )
                | (
                    ModerationDecisionKind::RejectPublication,
                    ModerationDecisionEffectAction::RejectPublication
                )
                | (
                    ModerationDecisionKind::SuspendSubject,
                    ModerationDecisionEffectAction::SuspendSubject { .. }
                )
                | (ModerationDecisionKind::Escalate, ModerationDecisionEffectAction::Escalate)
                | (
                    ModerationDecisionKind::AccountSanctionRecommended,
                    ModerationDecisionEffectAction::AccountSanctionRecommended { .. }
                )
        );
        if compatible {
            Ok(())
        } else {
            Err(ModerationEffectValidationError::DecisionKindMismatch)
        }
    }
}

fn validate_capabilities(
    capabilities: &[ModerationCapabilityKey],
) -> Result<(), ModerationEffectValidationError> {
    if capabilities.is_empty() || capabilities.len() > MAX_MODERATION_EFFECT_CAPABILITIES {
        return Err(ModerationEffectValidationError::InvalidCapabilityCount);
    }
    let unique = capabilities.iter().collect::<BTreeSet<_>>();
    if unique.len() != capabilities.len() {
        return Err(ModerationEffectValidationError::DuplicateCapability);
    }
    if capabilities.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ModerationEffectValidationError::CapabilitiesNotCanonical);
    }
    Ok(())
}

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModerationEffectValidationError {
    #[error("unsupported moderation decision effect schema version")]
    UnsupportedSchemaVersion,
    #[error("moderation decision kind and effect do not match")]
    DecisionKindMismatch,
    #[error("invalid moderation capability key")]
    InvalidCapabilityKey,
    #[error("moderation capability count is outside the allowed range")]
    InvalidCapabilityCount,
    #[error("moderation capabilities contain duplicates")]
    DuplicateCapability,
    #[error("moderation capabilities are not in canonical order")]
    CapabilitiesNotCanonical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyModerationDecisionCommand {
    pub decision_id: Uuid,
    pub subject: ModerationSubjectRef,
    pub decision_kind: ModerationDecisionKind,
    pub reason_code: ModerationReasonCode,
    pub effect: ModerationDecisionEffect,
    pub decision_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModerationDecisionApplication {
    pub decision_id: Uuid,
    pub subject: ModerationSubjectRef,
    pub applied_revision: i64,
    pub applied_at: DateTime<Utc>,
}
