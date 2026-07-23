use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MODERATION_DECISION_EFFECT_SCHEMA_V1: u16 = 1;
pub const MAX_MODERATION_EFFECT_CAPABILITIES: usize = 32;
pub const MAX_MODERATION_CAPABILITY_KEY_BYTES: usize = 120;

macro_rules! string_enum {
    ($vis:vis enum $name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        $vis enum $name {
            $($variant),+
        }

        impl $name {
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $value),+
                }
            }

            pub fn parse(value: &str) -> Option<Self> {
                match value {
                    $($value => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

string_enum! {
    pub enum ModerationSubjectKind {
        ForumTopic => "forum_topic",
        ForumPost => "forum_post",
        BlogPost => "blog_post",
        Comment => "comment",
        Group => "group",
        GroupMembership => "group_membership",
        Review => "review",
        ReviewResponse => "review_response",
        Product => "product",
        MarketplaceListing => "marketplace_listing",
        SellerProfile => "seller_profile",
        Message => "message",
        MediaAsset => "media_asset",
        UserProfile => "user_profile",
    }
}

string_enum! {
    pub enum ModerationScopeKind {
        Platform => "platform",
        Group => "group",
        Page => "page",
        ForumCategory => "forum_category",
        MarketplaceStore => "marketplace_store",
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

string_enum! {
    pub enum ModerationReasonCode {
        Spam => "spam",
        Fraud => "fraud",
        Harassment => "harassment",
        HateOrAbuse => "hate_or_abuse",
        Threats => "threats",
        SexualContent => "sexual_content",
        Violence => "violence",
        IllegalGoods => "illegal_goods",
        Counterfeit => "counterfeit",
        Misinformation => "misinformation",
        PersonalDataExposure => "personal_data_exposure",
        Copyright => "copyright",
        Impersonation => "impersonation",
        ManipulatedRating => "manipulated_rating",
        OffTopic => "off_topic",
        DuplicateContent => "duplicate_content",
        Other => "other",
    }
}

string_enum! {
    pub enum ModerationDecisionKind {
        NoViolation => "no_violation",
        Warning => "warning",
        Hide => "hide",
        Unpublish => "unpublish",
        Remove => "remove",
        Lock => "lock",
        RestrictInteraction => "restrict_interaction",
        RequireEdit => "require_edit",
        RejectPublication => "reject_publication",
        SuspendSubject => "suspend_subject",
        Escalate => "escalate",
        AccountSanctionRecommended => "account_sanction_recommended",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ModerationCapabilityKey(String);

impl ModerationCapabilityKey {
    pub fn new(value: impl Into<String>) -> Result<Self, ModerationEffectValidationError> {
        let value = value.into();
        if value.is_empty() || value.len() > MAX_MODERATION_CAPABILITY_KEY_BYTES {
            return Err(ModerationEffectValidationError::InvalidCapabilityKey);
        }
        let bytes = value.as_bytes();
        let separator = |byte: u8| matches!(byte, b'.' | b':' | b'_' | b'-');
        let valid = bytes.iter().copied().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || separator(byte)
        }) && !separator(bytes[0])
            && !separator(bytes[bytes.len() - 1]);
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

string_enum! {
    pub enum ModerationVisibilityState {
        Hidden => "hidden",
        Unpublished => "unpublished",
        Removed => "removed",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModerationDecisionEffectAction {
    NoDomainMutation,
    SetVisibility {
        state: ModerationVisibilityState,
    },
    Lock {
        effective_until: Option<DateTime<Utc>>,
    },
    RestrictInteraction {
        capabilities: Vec<ModerationCapabilityKey>,
        effective_until: Option<DateTime<Utc>>,
    },
    RequireEdit,
    RejectPublication,
    SuspendSubject {
        effective_until: Option<DateTime<Utc>>,
    },
    Escalate,
    AccountSanctionRecommended {
        capabilities: Vec<ModerationCapabilityKey>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModerationDecisionEffect {
    pub schema_version: u16,
    pub action: ModerationDecisionEffectAction,
}

impl ModerationDecisionEffect {
    pub fn v1(
        action: ModerationDecisionEffectAction,
    ) -> Result<Self, ModerationEffectValidationError> {
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
    if capabilities
        .windows(2)
        .any(|pair| pair[0].as_str() >= pair[1].as_str())
    {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suspension_requires_matching_decision_kind() {
        let effect = ModerationDecisionEffect::v1(
            ModerationDecisionEffectAction::SuspendSubject {
                effective_until: None,
            },
        )
        .expect("valid effect");
        assert!(
            effect
                .validate_for_decision_kind(ModerationDecisionKind::SuspendSubject)
                .is_ok()
        );
        assert!(
            effect
                .validate_for_decision_kind(ModerationDecisionKind::Warning)
                .is_err()
        );
    }

    #[test]
    fn capability_sets_must_be_canonical() {
        let capabilities = vec![
            ModerationCapabilityKey::new("groups.comment").expect("key"),
            ModerationCapabilityKey::new("groups.post").expect("key"),
        ];
        let effect = ModerationDecisionEffect::v1(
            ModerationDecisionEffectAction::RestrictInteraction {
                capabilities,
                effective_until: None,
            },
        );
        assert!(effect.is_ok());
    }
}
