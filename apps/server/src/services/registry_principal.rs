use rustok_api::context::{has_effective_permission, AuthContext, AuthContextExtension};
use rustok_core::Permission;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RegistryPrincipalKind {
    User,
    Runner,
    Legacy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryPrincipalRef {
    pub kind: RegistryPrincipalKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<Uuid>,
    pub subject: String,
    pub display_label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryAuthority {
    pub principal: RegistryPrincipalRef,
    pub can_manage_modules: bool,
}

impl RegistryPrincipalRef {
    pub fn user(user_id: Uuid) -> Self {
        let subject = format!("user:{user_id}");
        Self {
            kind: RegistryPrincipalKind::User,
            user_id: Some(user_id),
            subject: subject.clone(),
            display_label: subject,
            legacy_label: None,
        }
    }

    pub fn runner(runner_id: &str) -> Self {
        let normalized = runner_id.trim();
        let subject = format!("remote-runner:{normalized}");
        Self {
            kind: RegistryPrincipalKind::Runner,
            user_id: None,
            subject: subject.clone(),
            display_label: subject,
            legacy_label: None,
        }
    }

    pub fn legacy(label: &str) -> Self {
        let normalized = label.trim();
        Self {
            kind: RegistryPrincipalKind::Legacy,
            user_id: None,
            subject: normalized.to_string(),
            display_label: normalized.to_string(),
            legacy_label: Some(normalized.to_string()),
        }
    }

    pub fn from_legacy_value(value: &str) -> Self {
        let normalized = value.trim();
        if let Some(raw) = normalized.strip_prefix("user:") {
            if let Ok(user_id) = Uuid::parse_str(raw) {
                return Self::user(user_id);
            }
        }
        if let Some(runner_id) = normalized.strip_prefix("remote-runner:") {
            return Self::runner(runner_id);
        }
        Self::legacy(normalized)
    }

    pub fn is_user(&self) -> bool {
        matches!(self.kind, RegistryPrincipalKind::User) && self.user_id.is_some()
    }

    pub fn is_legacy(&self) -> bool {
        matches!(self.kind, RegistryPrincipalKind::Legacy)
    }

    pub fn user_id(&self) -> Option<Uuid> {
        self.user_id
    }

    pub fn matches_user(&self, user_id: Uuid) -> bool {
        self.user_id == Some(user_id) && matches!(self.kind, RegistryPrincipalKind::User)
    }

    pub fn label(&self) -> &str {
        self.display_label.as_str()
    }

    pub fn persisted_label(&self) -> &str {
        self.legacy_label
            .as_deref()
            .unwrap_or(self.display_label.as_str())
    }

    pub fn from_json_value(value: &serde_json::Value) -> Self {
        serde_json::from_value(value.clone())
            .unwrap_or_else(|_| Self::from_legacy_value(value.as_str().unwrap_or("legacy:unknown")))
    }

    pub fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

impl RegistryAuthority {
    pub fn from_auth(auth: &AuthContextExtension) -> Self {
        Self::from_auth_context(&auth.0)
    }

    pub fn from_auth_context(auth: &AuthContext) -> Self {
        Self {
            principal: RegistryPrincipalRef::user(auth.user_id),
            can_manage_modules: has_effective_permission(
                &auth.permissions,
                &Permission::MODULES_MANAGE,
            ),
        }
    }

    pub fn requires_user_session(&self) -> bool {
        self.principal.is_user()
    }
}
