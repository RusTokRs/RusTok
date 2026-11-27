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
        validate_http_request_shape(call, method)?;
        self.validate_url_scheme_and_auth(call, &url)?;
        self.validate_http_rules(call, &method.to_ascii_uppercase(), &url)
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
            .any(|prefix| path_matches_prefix(url.path(), prefix))
        {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: format!("HTTP path `{}` is not allowed", url.path()),
            });
        }
        Ok(())
    }
}

fn path_matches_prefix(path: &str, prefix: &str) -> bool {
    if prefix == "/" || path == prefix {
        return true;
    }
    if prefix.ends_with('/') {
        return path.starts_with(prefix);
    }
    path.strip_prefix(prefix)
        .is_some_and(|remainder| remainder.starts_with('/'))
}

fn validate_http_request_shape(call: &CapabilityCall, method: &str) -> SandboxResult<()> {
    let input = call
        .input
        .as_object()
        .ok_or_else(|| SandboxError::CapabilityConstraintDenied {
            capability: call.capability.clone(),
            reason: "HTTP input must be an object".to_string(),
        })?;
    if input
        .keys()
        .any(|field| !matches!(field.as_str(), "method" | "url" | "headers" | "body" | "idempotency_key"))
    {
        return Err(SandboxError::CapabilityConstraintDenied {
            capability: call.capability.clone(),
            reason: "HTTP input contains an unsupported field".to_string(),
        });
    }
    if input
        .get("headers")
        .is_some_and(|headers| {
            headers.as_object().is_none_or(|headers| {
                headers.iter().any(|(name, value)| {
                    name.eq_ignore_ascii_case("idempotency-key") || !value.is_string()
                })
            })
        })
    {
        return Err(SandboxError::CapabilityConstraintDenied {
            capability: call.capability.clone(),
            reason: "HTTP headers must be string values and must not set Idempotency-Key".to_string(),
        });
    }

    if matches!(method.to_ascii_uppercase().as_str(), "POST" | "PUT" | "PATCH" | "DELETE") {
        let key = input
            .get("idempotency_key")
            .and_then(Value::as_str)
            .and_then(|value| uuid::Uuid::parse_str(value).ok())
            .filter(|key| !key.is_nil());
        if key.is_none() || call.context.idempotency_key.is_none_or(|key| key.is_nil()) {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: "mutating HTTP requests require an execution idempotency key and a per-request idempotency_key UUID".to_string(),
            });
        }
    } else if input
        .get("idempotency_key")
        .is_some_and(|value| {
            value.as_str()
                .and_then(|key| uuid::Uuid::parse_str(key).ok())
                .is_none_or(|key| key.is_nil())
        })
    {
        return Err(SandboxError::CapabilityConstraintDenied {
            capability: call.capability.clone(),
            reason: "HTTP idempotency_key must be a non-nil UUID".to_string(),
        });
    }
    Ok(())
}

fn parse_http_call_inputs(call: &CapabilityCall) -> SandboxResult<(&str, Url)> {
    let input = call
        .input
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
