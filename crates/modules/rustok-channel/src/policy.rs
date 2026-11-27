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

        match self.action {
            ResolutionAction::ResolveToChannel { channel_id } if channel_id.is_nil() => {
                return Err(ChannelError::InvalidPolicyDefinition(
                    "resolve_to_channel action requires a non-nil channel_id".to_string(),
                ));
            }
            _ => {}
        }

        let predicates = self
            .predicates
            .into_iter()
            .map(ResolutionPredicate::validated)
            .collect::<ChannelResult<Vec<_>>>()?;

        // A rule must be an explicit, unambiguous conjunction: a duplicated
        // predicate is dead configuration, and `host_equals` together with
        // `host_suffix` can never both hold for the values the predicate admits.
        let mut seen_host_equals = false;
        let mut seen_host_suffix = false;
        let mut seen_oauth_app = false;
        let mut seen_surface = false;
        let mut seen_locale = false;
        for predicate in &predicates {
            let already_seen = match predicate {
                ResolutionPredicate::HostEquals(_) => {
                    std::mem::replace(&mut seen_host_equals, true)
                }
                ResolutionPredicate::HostSuffix(_) => {
                    std::mem::replace(&mut seen_host_suffix, true)
                }
                ResolutionPredicate::OAuthAppEquals(_) => {
                    std::mem::replace(&mut seen_oauth_app, true)
                }
                ResolutionPredicate::SurfaceIs(_) => std::mem::replace(&mut seen_surface, true),
                ResolutionPredicate::LocaleEquals(_) => std::mem::replace(&mut seen_locale, true),
            };
            if already_seen {
                return Err(ChannelError::InvalidPolicyDefinition(format!(
                    "resolution rule repeats the `{}` predicate; combine it into a single predicate instead",
                    predicate.kind()
                )));
            }
        }
        if seen_host_equals && seen_host_suffix {
            return Err(ChannelError::InvalidPolicyDefinition(
                "resolution rule cannot combine `host_equals` with `host_suffix`: the exact host and the suffix can never both match"
                    .to_string(),
            ));
        }

        Ok(Self {
            predicates,
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
    /// Stable predicate discriminator used in validation diagnostics.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::HostEquals(_) => "host_equals",
            Self::HostSuffix(_) => "host_suffix",
            Self::OAuthAppEquals(_) => "oauth_app_equals",
            Self::SurfaceIs(_) => "surface_is",
            Self::LocaleEquals(_) => "locale_equals",
        }
    }

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
            Self::OAuthAppEquals(app_id) => {
                if app_id.is_nil() {
                    return Err(ChannelError::InvalidPolicyDefinition(
                        "oauth_app_equals requires a non-nil app_id".to_string(),
                    ));
                }
                Ok(Self::OAuthAppEquals(app_id))
            }
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
                    // `host == suffix` or a strict subdomain: stripping the suffix
                    // must leave a label separator, so `badexample.test` never
                    // matches `example.test`. This runs per rule per request, so it
                    // avoids the per-comparison `format!` allocation.
                    host == expected_suffix
                        || host
                            .strip_suffix(expected_suffix)
                            .is_some_and(|prefix| prefix.ends_with('.'))
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

/// Normalizes a locale predicate to the `ll`/`ll-rr` form facts are matched with.
///
/// The predicate used to accept any non-empty string, so a rule could be stored
/// (and indexed) that no request locale could ever equal.
fn normalize_locale(raw: &str) -> ChannelResult<String> {
    let normalized = raw.trim().replace('_', "-").to_ascii_lowercase();
    let mut parts = normalized.splitn(2, '-');
    let language = parts.next().unwrap_or_default();
    let region = parts.next();
    let language_is_valid =
        (2..=3).contains(&language.len()) && language.chars().all(|ch| ch.is_ascii_alphabetic());
    let region_is_valid = region.is_none_or(|region| {
        (2..=8).contains(&region.len()) && region.chars().all(|ch| ch.is_ascii_alphanumeric())
    });
    if !language_is_valid || !region_is_valid {
        return Err(ChannelError::InvalidPolicyDefinition(format!(
            "locale_equals requires a BCP-47-like locale such as `en` or `en-us`, got `{raw}`"
        )));
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

    #[test]
    fn rejects_nil_channel_id_in_action() {
        let error = ChannelResolutionRuleDefinition {
            predicates: vec![ResolutionPredicate::SurfaceIs(TargetSurface::Http)],
            action: ResolutionAction::ResolveToChannel {
                channel_id: Uuid::nil(),
            },
        }
        .validated()
        .unwrap_err();

        assert!(error.to_string().contains("non-nil channel_id"));
    }

    #[test]
    fn rejects_nil_oauth_app_id_in_predicate() {
        let error = ChannelResolutionRuleDefinition {
            predicates: vec![ResolutionPredicate::OAuthAppEquals(Uuid::nil())],
            action: ResolutionAction::ResolveToChannel {
                channel_id: Uuid::new_v4(),
            },
        }
        .validated()
        .unwrap_err();

        assert!(error.to_string().contains("non-nil app_id"));
    }
}
