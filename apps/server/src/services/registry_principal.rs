use rustok_api::{AuthPrincipalKind, Permission};
use rustok_api::context::{AuthContext, has_effective_permission};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RegistryPrincipalKind {
    User,
    Runner,
    Legacy,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RegistryPrincipalRef {
    kind: RegistryPrincipalKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<Uuid>,
    subject: String,
    display_label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    legacy_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryAuthority {
    principal: RegistryPrincipalRef,
    can_manage_modules: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryPrincipalWire {
    kind: RegistryPrincipalKind,
    #[serde(default)]
    user_id: Option<Uuid>,
    subject: String,
    display_label: String,
    #[serde(default)]
    legacy_label: Option<String>,
}

impl RegistryPrincipalKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Runner => "runner",
            Self::Legacy => "legacy",
        }
    }
}

impl RegistryPrincipalRef {
    pub fn user(user_id: Uuid) -> Option<Self> {
        if user_id.is_nil() {
            return None;
        }
        let subject = format!("user:{user_id}");
        Some(Self {
            kind: RegistryPrincipalKind::User,
            user_id: Some(user_id),
            subject: subject.clone(),
            display_label: subject,
            legacy_label: None,
        })
    }

    pub fn runner(runner_id: &str) -> Option<Self> {
        let normalized = runner_id.trim();
        if normalized.is_empty()
            || normalized.len() > 128
            || !normalized.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return None;
        }
        let subject = format!("remote-runner:{normalized}");
        Some(Self {
            kind: RegistryPrincipalKind::Runner,
            user_id: None,
            subject: subject.clone(),
            display_label: subject,
            legacy_label: None,
        })
    }

    pub fn legacy(label: &str) -> Option<Self> {
        let normalized = label.trim();
        if normalized.is_empty() {
            return None;
        }
        Some(Self {
            kind: RegistryPrincipalKind::Legacy,
            user_id: None,
            subject: normalized.to_string(),
            display_label: normalized.to_string(),
            legacy_label: Some(normalized.to_string()),
        })
    }

    fn validate(&self) -> Result<(), &'static str> {
        match self.kind {
            RegistryPrincipalKind::User => {
                let user_id = self.user_id.filter(|user_id| !user_id.is_nil())
                    .ok_or("user principal requires a non-nil user_id")?;
                let expected = format!("user:{user_id}");
                if self.subject != expected
                    || self.display_label != expected
                    || self.legacy_label.is_some()
                {
                    return Err("user principal fields are inconsistent");
                }
            }
            RegistryPrincipalKind::Runner => {
                if self.user_id.is_some() || self.legacy_label.is_some() {
                    return Err("runner principal contains user or legacy fields");
                }
                let runner_id = self.subject.strip_prefix("remote-runner:")
                    .ok_or("runner principal has an invalid subject")?;
                if runner_id.is_empty()
                    || runner_id.len() > 128
                    || !runner_id.bytes().all(|byte| byte.is_ascii_graphic())
                    || self.display_label != self.subject
                {
                    return Err("runner principal fields are inconsistent");
                }
            }
            RegistryPrincipalKind::Legacy => {
                if self.user_id.is_some() {
                    return Err("legacy principal contains a user_id");
                }
                let label = self.legacy_label.as_deref()
                    .filter(|label| !label.is_empty() && label.trim() == *label)
                    .ok_or("legacy principal requires a normalized legacy_label")?;
                if self.subject != label || self.display_label != label {
                    return Err("legacy principal fields are inconsistent");
                }
            }
        }
        Ok(())
    }

    pub(crate) fn from_json_value(value: &serde_json::Value) -> Result<Self, String> {
        let wire: RegistryPrincipalWire = serde_json::from_value(value.clone())
            .map_err(|_| "registry principal is not a canonical structured object".to_string())?;
        let principal = Self {
            kind: wire.kind,
            user_id: wire.user_id,
            subject: wire.subject,
            display_label: wire.display_label,
            legacy_label: wire.legacy_label,
        };
        principal.validate().map_err(str::to_string)?;
        Ok(principal)
    }

    pub fn is_user(&self) -> bool {
        self.validate().is_ok() && matches!(self.kind, RegistryPrincipalKind::User)
    }

    pub fn is_legacy(&self) -> bool {
        self.validate().is_ok() && matches!(self.kind, RegistryPrincipalKind::Legacy)
    }

    pub fn user_id(&self) -> Option<Uuid> {
        if self.is_user() { self.user_id } else { None }
    }

    pub fn matches_user(&self, user_id: Uuid) -> bool {
        !user_id.is_nil() && self.is_user() && self.user_id == Some(user_id)
    }

    pub fn label(&self) -> &str {
        self.display_label.as_str()
    }

    pub fn persisted_label(&self) -> &str {
        self.legacy_label
            .as_deref()
            .unwrap_or(self.display_label.as_str())
    }

    pub fn to_json_value(&self) -> serde_json::Value {
        let mut object = serde_json::Map::new();
        object.insert(
            "kind".to_string(),
            serde_json::Value::String(self.kind.as_str().to_string()),
        );
        if let Some(user_id) = self.user_id {
            object.insert(
                "user_id".to_string(),
                serde_json::Value::String(user_id.to_string()),
            );
        }
        object.insert(
            "subject".to_string(),
            serde_json::Value::String(self.subject.clone()),
        );
        object.insert(
            "display_label".to_string(),
            serde_json::Value::String(self.display_label.clone()),
        );
        if let Some(legacy_label) = &self.legacy_label {
            object.insert(
                "legacy_label".to_string(),
                serde_json::Value::String(legacy_label.clone()),
            );
        }
        serde_json::Value::Object(object)
    }
}

/// Registry V2 accepts direct, session-backed users only. Reuse the canonical
/// grant classifier rather than inferring a user from a missing OAuth client.
pub(crate) fn is_registry_session(auth: &AuthContext) -> bool {
    !auth.user_id.is_nil()
        && !auth.tenant_id.is_nil()
        && AuthPrincipalKind::from_authenticated_facts(
            &auth.grant_type,
            auth.client_id,
            auth.session_id,
        )
        .is_some_and(AuthPrincipalKind::is_direct_user)
}

impl RegistryAuthority {
    /// Construct request authority only for a valid direct session.
    pub(crate) fn from_session(auth: &AuthContext) -> Option<Self> {
        if !is_registry_session(auth) {
            return None;
        }
        Some(Self {
            principal: RegistryPrincipalRef::user(auth.user_id)?,
            can_manage_modules: has_effective_permission(
                &auth.permissions,
                &Permission::MODULES_MANAGE,
            ),
        })
    }

    pub fn principal(&self) -> &RegistryPrincipalRef {
        &self.principal
    }

    pub fn can_manage_modules(&self) -> bool {
        self.can_manage_modules
    }

    pub fn requires_user_session(&self) -> bool {
        self.principal.is_user()
    }
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn principal_constructors_reject_nil_or_empty_identities() {
        assert!(RegistryPrincipalRef::user(Uuid::nil()).is_none());
        assert!(RegistryPrincipalRef::runner("").is_none());
        assert!(RegistryPrincipalRef::runner("runner\n01").is_none());
        assert!(RegistryPrincipalRef::legacy("  ").is_none());
        assert!(RegistryPrincipalRef::user(Uuid::new_v4()).unwrap().is_user());
    }

    #[test]
    fn persisted_principals_require_the_canonical_structured_shape() {
        let user = RegistryPrincipalRef::user(Uuid::new_v4()).unwrap();
        assert_eq!(
            RegistryPrincipalRef::from_json_value(&user.to_json_value()).unwrap(),
            user
        );
        let runner = RegistryPrincipalRef::runner("runner-01").unwrap();
        assert_eq!(
            RegistryPrincipalRef::from_json_value(&runner.to_json_value()).unwrap(),
            runner
        );
        let legacy = RegistryPrincipalRef::legacy("historical publisher").unwrap();
        assert_eq!(
            RegistryPrincipalRef::from_json_value(&legacy.to_json_value()).unwrap(),
            legacy
        );

        assert!(RegistryPrincipalRef::from_json_value(&serde_json::json!("user:legacy")).is_err());
        assert!(RegistryPrincipalRef::from_json_value(&serde_json::json!({
            "kind": "user",
            "user_id": Uuid::nil(),
            "subject": format!("user:{}", Uuid::nil()),
            "display_label": format!("user:{}", Uuid::nil())
        })).is_err());
        assert!(RegistryPrincipalRef::from_json_value(&serde_json::json!({
            "kind": "user",
            "user_id": user.user_id().unwrap(),
            "subject": "user:someone-else",
            "display_label": "user:someone-else"
        })).is_err());
        assert!(RegistryPrincipalRef::from_json_value(&serde_json::json!({
            "kind": "runner",
            "subject": "remote-runner:",
            "display_label": "remote-runner:"
        })).is_err());
        assert!(RegistryPrincipalRef::from_json_value(&serde_json::json!({
            "kind": "legacy",
            "subject": "historical publisher",
            "display_label": "historical publisher"
        })).is_err());
        assert!(RegistryPrincipalRef::from_json_value(&serde_json::json!({
            "kind": "user",
            "user_id": user.user_id().unwrap(),
            "subject": format!("user:{}", user.user_id().unwrap()),
            "display_label": format!("user:{}", user.user_id().unwrap()),
            "unexpected": true
        })).is_err());
    }

    #[test]
    fn registry_session_requires_canonical_direct_grant_and_non_nil_identity() {
        let mut auth = AuthContext {
            user_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            client_id: None,
            permissions: vec![Permission::MODULES_MANAGE],
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
        };
        assert!(RegistryAuthority::from_session(&auth).unwrap().can_manage_modules);
        auth.permissions.clear();
        assert!(!RegistryAuthority::from_session(&auth).unwrap().can_manage_modules);

        let valid = auth.clone();
        for (grant, client, session) in [
            ("direct", None, Uuid::nil()),
            ("direct", Some(Uuid::new_v4()), Uuid::new_v4()),
            ("authorization_code", Some(Uuid::new_v4()), Uuid::nil()),
            ("authorization_code", Some(Uuid::new_v4()), Uuid::new_v4()),
            ("client_credentials", Some(Uuid::new_v4()), Uuid::nil()),
            ("client_credentials", None, Uuid::new_v4()),
            ("unknown", None, Uuid::new_v4()),
        ] {
            auth.grant_type = grant.to_string();
            auth.client_id = client;
            auth.session_id = session;
            assert!(RegistryAuthority::from_session(&auth).is_none(), "{grant}");
        }
        auth = valid.clone();
        auth.user_id = Uuid::nil();
        assert!(RegistryAuthority::from_session(&auth).is_none());
        auth = valid;
        auth.tenant_id = Uuid::nil();
        assert!(RegistryAuthority::from_session(&auth).is_none());
    }
}
