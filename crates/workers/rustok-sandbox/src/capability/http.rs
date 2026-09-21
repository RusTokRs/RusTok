use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use super::{CapabilityCall, CapabilityGrant};
use crate::{SandboxError, SandboxResult};

/// Typed policy for the `platform.http` capability.
///
/// A grant must name every allowed host, HTTP method and path prefix. Matching
/// is exact for hosts and methods and prefix-based for paths; there are no
/// implicit wildcards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpCapabilityConstraints {
    pub hosts: Vec<String>,
    pub methods: Vec<String>,
    pub path_prefixes: Vec<String>,
    #[serde(default)]
    pub allow_plain_http: bool,
}

impl HttpCapabilityConstraints {
    pub(crate) fn from_grant(grant: &CapabilityGrant) -> SandboxResult<Self> {
        let constraints =
            serde_json::from_value::<Self>(grant.constraints.clone()).map_err(|error| {
                SandboxError::CapabilityConstraintDenied {
                    capability: grant.name.clone(),
                    reason: format!("invalid HTTP constraints: {error}"),
                }
            })?;
        if constraints.hosts.is_empty()
            || constraints.methods.is_empty()
            || constraints.path_prefixes.is_empty()
        {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason: "HTTP constraints require non-empty hosts, methods, and path_prefixes"
                    .to_string(),
            });
        }
        if constraints.hosts.iter().any(|host| host.trim().is_empty())
            || constraints
                .methods
                .iter()
                .any(|method| method.trim().is_empty())
            || constraints
                .path_prefixes
                .iter()
                .any(|prefix| !prefix.starts_with('/'))
        {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason:
                    "HTTP hosts and methods must be non-empty and path_prefixes must start with `/`"
                        .to_string(),
            });
        }
        Ok(constraints)
    }

    pub(crate) fn validate(&self, call: &CapabilityCall) -> SandboxResult<()> {
        let (method, url) = parse_http_call_inputs(call)?;
        self.validate_url_scheme_and_auth(call, &url)?;
        self.validate_http_rules(call, method, &url)
    }

    fn validate_url_scheme_and_auth(&self, call: &CapabilityCall, url: &Url) -> SandboxResult<()> {
        let scheme = url.scheme();
        if scheme != "https" && !(self.allow_plain_http && scheme == "http") {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: format!(
                    "HTTP url scheme `{scheme}` is not allowed (only https is permitted)"
                ),
            });
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: "HTTP url must not contain userinfo credentials".to_string(),
            });
        }
        Ok(())
    }

    fn validate_http_rules(
        &self,
        call: &CapabilityCall,
        method: &str,
        url: &Url,
    ) -> SandboxResult<()> {
        let host = url
            .host_str()
            .ok_or_else(|| SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: "HTTP url must include a host".to_string(),
            })?;
        if !self
            .hosts
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(host))
        {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: format!("HTTP host `{host}` is not allowed"),
            });
        }
        if !self.methods.iter().any(|allowed| allowed == method) {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: format!("HTTP method `{method}` is not allowed"),
            });
        }
        if !self
            .path_prefixes
            .iter()
            .any(|prefix| url.path().starts_with(prefix))
        {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: format!("HTTP path `{}` is not allowed", url.path()),
            });
        }
        Ok(())
    }
}

fn parse_http_call_inputs(call: &CapabilityCall) -> SandboxResult<(&str, Url)> {
    let input =
        call.input
            .as_object()
            .ok_or_else(|| SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: "HTTP input must be an object".to_string(),
            })?;
    let method = input.get("method").and_then(Value::as_str).ok_or_else(|| {
        SandboxError::CapabilityConstraintDenied {
            capability: call.capability.clone(),
            reason: "HTTP input must contain a string method".to_string(),
        }
    })?;
    let raw_url = input.get("url").and_then(Value::as_str).ok_or_else(|| {
        SandboxError::CapabilityConstraintDenied {
            capability: call.capability.clone(),
            reason: "HTTP input must contain a string url".to_string(),
        }
    })?;
    let url = Url::parse(raw_url).map_err(|_| SandboxError::CapabilityConstraintDenied {
        capability: call.capability.clone(),
        reason: "HTTP url must be absolute".to_string(),
    })?;
    Ok((method, url))
}
