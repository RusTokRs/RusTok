use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ChannelTargetType;
use crate::error::{ChannelError, ChannelResult};
use crate::resolution::{RequestFacts, TargetSurface};

pub const CHANNEL_RESOLUTION_POLICY_SCHEMA_VERSION: i32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "value")]
pub enum ResolutionPredicate {
    HostEquals(String),
    HostSuffix(String),
    OAuthAppEquals(Uuid),
    SurfaceIs(TargetSurface),
    LocaleEquals(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ResolutionAction {
    ResolveToChannel { channel_id: Uuid },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelResolutionRuleDefinition {
    pub predicates: Vec<ResolutionPredicate>,
    pub action: ResolutionAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredChannelResolutionRule {
    pub id: Uuid,
    pub policy_set_id: Uuid,
    pub policy_set_slug: String,
    pub policy_set_name: String,
    pub priority: i32,
    pub action_channel_id: Uuid,
    pub definition: ChannelResolutionRuleDefinition,
}

impl ChannelResolutionRuleDefinition {
    pub fn validated(self) -> ChannelResult<Self> {
        if self.predicates.is_empty() {
            return Err(ChannelError::InvalidPolicyDefinition(
                "resolution rule must contain at least one predicate".to_string(),
            ));
        }

        Ok(Self {
            predicates: self
                .predicates
                .into_iter()
                .map(ResolutionPredicate::validated)
                .collect::<ChannelResult<Vec<_>>>()?,
            action: self.action,
        })
    }

    pub fn matches(&self, facts: &RequestFacts) -> bool {
        self.predicates
            .iter()
            .all(|predicate| predicate.matches(facts))
    }

    pub fn action_channel_id(&self) -> Uuid {
        match self.action {
            ResolutionAction::ResolveToChannel { channel_id } => channel_id,
        }
    }
}

impl ResolutionPredicate {
    fn validated(self) -> ChannelResult<Self> {
        match self {
            Self::HostEquals(value) => Ok(Self::HostEquals(normalize_host_predicate(
                &value,
                "host_equals",
            )?)),
            Self::HostSuffix(value) => Ok(Self::HostSuffix(normalize_host_predicate(
                &value,
                "host_suffix",
            )?)),
            Self::OAuthAppEquals(app_id) => Ok(Self::OAuthAppEquals(app_id)),
            Self::SurfaceIs(surface) => Ok(Self::SurfaceIs(surface)),
            Self::LocaleEquals(locale) => Ok(Self::LocaleEquals(normalize_locale(&locale)?)),
        }
    }

    fn matches(&self, facts: &RequestFacts) -> bool {
        match self {
            Self::HostEquals(expected) => normalized_request_host(facts)
                .as_deref()
                .is_some_and(|host| host == expected),
            Self::HostSuffix(expected_suffix) => normalized_request_host(facts)
                .as_deref()
                .is_some_and(|host| {
                    host == expected_suffix
                        || host.ends_with(format!(".{expected_suffix}").as_str())
                }),
            Self::OAuthAppEquals(app_id) => facts.oauth_app_id == Some(*app_id),
            Self::SurfaceIs(surface) => &facts.surface == surface,
            Self::LocaleEquals(expected) => facts
                .locale
                .as_deref()
                .and_then(|value| normalize_locale(value).ok())
                .is_some_and(|locale| locale == *expected),
        }
    }
}

fn normalize_host_predicate(raw: &str, predicate_name: &str) -> ChannelResult<String> {
    ChannelTargetType::WebDomain
        .normalize_value(raw)
        .ok_or_else(|| {
            ChannelError::InvalidPolicyDefinition(format!(
                "{predicate_name} requires a canonical web_domain-like host value"
            ))
        })
}

fn normalized_request_host(facts: &RequestFacts) -> Option<String> {
    facts
        .host
        .as_deref()
        .and_then(|value| ChannelTargetType::WebDomain.normalize_value(value))
}

fn normalize_locale(raw: &str) -> ChannelResult<String> {
    let normalized = raw.trim().replace('_', "-").to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(ChannelError::InvalidPolicyDefinition(
            "locale_equals requires a non-empty locale".to_string(),
        ));
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::{
        ChannelResolutionRuleDefinition, ResolutionAction, ResolutionPredicate,
        StoredChannelResolutionRule,
    };
    use crate::resolution::{RequestFacts, TargetSurface};
    use uuid::Uuid;

    #[test]
    fn validates_and_normalizes_host_and_locale_predicates() {
        let channel_id = Uuid::new_v4();
        let definition = ChannelResolutionRuleDefinition {
            predicates: vec![
                ResolutionPredicate::HostEquals(" HTTPS://Shop.Example.TEST:443/ ".to_string()),
                ResolutionPredicate::LocaleEquals(" RU_by ".to_string()),
            ],
            action: ResolutionAction::ResolveToChannel { channel_id },
        }
        .validated()
        .expect("definition should be valid");

        assert_eq!(
            definition.predicates,
            vec![
                ResolutionPredicate::HostEquals("shop.example.test".to_string()),
                ResolutionPredicate::LocaleEquals("ru-by".to_string()),
            ]
        );
        assert_eq!(definition.action_channel_id(), channel_id);
    }

    #[test]
    fn host_suffix_matches_only_canonical_subdomains() {
        let rule = StoredChannelResolutionRule {
            id: Uuid::new_v4(),
            policy_set_id: Uuid::new_v4(),
            policy_set_slug: "default".to_string(),
            policy_set_name: "Default".to_string(),
            priority: 10,
            action_channel_id: Uuid::new_v4(),
            definition: ChannelResolutionRuleDefinition {
                predicates: vec![
                    ResolutionPredicate::SurfaceIs(TargetSurface::Http),
                    ResolutionPredicate::HostSuffix("example.test".to_string()),
                ],
                action: ResolutionAction::ResolveToChannel {
                    channel_id: Uuid::new_v4(),
                },
            }
            .validated()
            .expect("definition should be valid"),
        };

        assert!(rule.definition.matches(&RequestFacts {
            tenant_id: Uuid::new_v4(),
            surface: TargetSurface::Http,
            host: Some("shop.example.test".to_string()),
            ..RequestFacts::default()
        }));
        assert!(!rule.definition.matches(&RequestFacts {
            tenant_id: Uuid::new_v4(),
            surface: TargetSurface::Http,
            host: Some("badexample.test".to_string()),
            ..RequestFacts::default()
        }));
    }
}
