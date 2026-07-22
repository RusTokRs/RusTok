use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::policy::{ChannelResolutionRuleDefinition, ResolutionAction, ResolutionPredicate};
use crate::resolution::TargetSurface;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChannelInput {
    pub tenant_id: Uuid,
    pub slug: String,
    pub name: String,
    pub settings: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChannelTargetInput {
    pub target_type: String,
    pub value: String,
    pub is_primary: bool,
    pub settings: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateChannelTargetInput {
    pub target_type: String,
    pub value: String,
    pub is_primary: bool,
    pub settings: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindChannelModuleInput {
    pub module_slug: String,
    pub is_enabled: bool,
    pub settings: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindChannelOauthAppInput {
    pub oauth_app_id: Uuid,
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChannelResolutionPolicySetInput {
    pub tenant_id: Uuid,
    pub slug: String,
    pub name: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChannelResolutionRuleInput {
    pub priority: i32,
    pub is_active: bool,
    pub definition: ChannelResolutionRuleDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateChannelResolutionRuleInput {
    pub priority: Option<i32>,
    pub is_active: Option<bool>,
    pub action_channel_id: Option<Uuid>,
    pub host_equals: Option<String>,
    pub host_suffix: Option<String>,
    pub oauth_app_id: Option<String>,
    pub surface: Option<String>,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReorderChannelResolutionRulesInput {
    pub rule_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelBootstrapResponse<C> {
    pub current_channel: Option<C>,
    pub channels: Vec<ChannelDetailResponse>,
    pub policy_sets: Vec<ChannelResolutionPolicySetDetailResponse>,
    pub available_modules: Vec<AvailableChannelModuleItem>,
    pub oauth_apps: Vec<AvailableChannelOauthAppItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateResolutionPolicySetRequest {
    pub slug: String,
    pub name: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateResolutionRuleRequest {
    pub priority: i32,
    pub is_active: bool,
    pub action_channel_id: Uuid,
    pub host_equals: Option<String>,
    pub host_suffix: Option<String>,
    pub oauth_app_id: Option<Uuid>,
    pub surface: Option<String>,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateResolutionRuleRequest {
    pub priority: Option<i32>,
    pub is_active: Option<bool>,
    pub action_channel_id: Option<Uuid>,
    pub host_equals: Option<String>,
    pub host_suffix: Option<String>,
    pub oauth_app_id: Option<String>,
    pub surface: Option<String>,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReorderResolutionRulesRequest {
    pub rule_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableChannelModuleItem {
    pub slug: String,
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableChannelOauthAppItem {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub app_type: String,
    pub is_active: bool,
}

pub fn create_resolution_policy_set_input(
    tenant_id: Uuid,
    input: CreateResolutionPolicySetRequest,
) -> CreateChannelResolutionPolicySetInput {
    CreateChannelResolutionPolicySetInput {
        tenant_id,
        slug: input.slug,
        name: input.name,
        is_active: input.is_active,
    }
}

pub fn create_resolution_rule_input(
    input: CreateResolutionRuleRequest,
) -> Result<CreateChannelResolutionRuleInput, String> {
    let CreateResolutionRuleRequest {
        priority,
        is_active,
        action_channel_id,
        host_equals,
        host_suffix,
        oauth_app_id,
        surface,
        locale,
    } = input;
    let mut predicates = Vec::new();

    if let Some(host_equals) = normalize_optional_string(host_equals) {
        predicates.push(ResolutionPredicate::HostEquals(host_equals));
    }
    if let Some(host_suffix) = normalize_optional_string(host_suffix) {
        predicates.push(ResolutionPredicate::HostSuffix(host_suffix));
    }
    if let Some(oauth_app_id) = oauth_app_id {
        predicates.push(ResolutionPredicate::OAuthAppEquals(oauth_app_id));
    }
    if let Some(surface) = normalize_optional_string(surface) {
        let surface = parse_target_surface(surface.as_str())?;
        predicates.push(ResolutionPredicate::SurfaceIs(surface));
    }
    if let Some(locale) = normalize_optional_string(locale) {
        predicates.push(ResolutionPredicate::LocaleEquals(locale));
    }

    Ok(CreateChannelResolutionRuleInput {
        priority,
        is_active,
        definition: ChannelResolutionRuleDefinition {
            predicates,
            action: ResolutionAction::ResolveToChannel {
                channel_id: action_channel_id,
            },
        },
    })
}

pub fn update_resolution_rule_input(
    input: UpdateResolutionRuleRequest,
) -> UpdateChannelResolutionRuleInput {
    UpdateChannelResolutionRuleInput {
        priority: input.priority,
        is_active: input.is_active,
        action_channel_id: input.action_channel_id,
        host_equals: normalize_optional_string(input.host_equals),
        host_suffix: normalize_optional_string(input.host_suffix),
        oauth_app_id: normalize_optional_string(input.oauth_app_id),
        surface: normalize_optional_string(input.surface),
        locale: normalize_optional_string(input.locale),
    }
}

fn parse_target_surface(surface: &str) -> Result<TargetSurface, String> {
    match surface {
        "http" => Ok(TargetSurface::Http),
        other => Err(format!(
            "Unsupported surface `{other}`; only `http` is currently supported"
        )),
    }
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub slug: String,
    pub name: String,
    pub is_active: bool,
    pub is_default: bool,
    pub status: String,
    pub settings: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelTargetResponse {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub target_type: String,
    pub value: String,
    pub is_primary: bool,
    pub settings: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelModuleBindingResponse {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub module_slug: String,
    pub is_enabled: bool,
    pub settings: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelOauthAppResponse {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub oauth_app_id: Uuid,
    pub role: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelResolutionPolicySetResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub slug: String,
    pub name: String,
    pub schema_version: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelResolutionRuleResponse {
    pub id: Uuid,
    pub policy_set_id: Uuid,
    pub priority: i32,
    pub is_active: bool,
    pub action_channel_id: Uuid,
    pub definition: ChannelResolutionRuleDefinition,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelResolutionPolicySetDetailResponse {
    pub policy_set: ChannelResolutionPolicySetResponse,
    pub rules: Vec<ChannelResolutionRuleResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelDetailResponse {
    pub channel: ChannelResponse,
    pub targets: Vec<ChannelTargetResponse>,
    pub module_bindings: Vec<ChannelModuleBindingResponse>,
    pub oauth_apps: Vec<ChannelOauthAppResponse>,
}

#[cfg(test)]
mod tests {
    use super::{
        CreateResolutionRuleRequest, UpdateResolutionRuleRequest, create_resolution_rule_input,
        update_resolution_rule_input,
    };
    use crate::{ResolutionPredicate, TargetSurface};
    use uuid::Uuid;

    #[test]
    fn create_resolution_rule_input_returns_normalized_predicates() {
        let channel_id = Uuid::new_v4();
        let payload = create_resolution_rule_input(CreateResolutionRuleRequest {
            priority: 30,
            is_active: true,
            action_channel_id: channel_id,
            host_equals: Some(" SHOP.EXAMPLE.TEST ".to_string()),
            host_suffix: None,
            oauth_app_id: None,
            surface: Some("http".to_string()),
            locale: Some(" RU_BY ".to_string()),
        })
        .expect("definition should be valid");

        assert_eq!(payload.priority, 30);
        assert!(payload.is_active);
        assert_eq!(
            payload.definition.predicates,
            vec![
                ResolutionPredicate::HostEquals("shop.example.test".to_string()),
                ResolutionPredicate::SurfaceIs(TargetSurface::Http),
                ResolutionPredicate::LocaleEquals("ru_by".to_string()),
            ]
        );
    }

    #[test]
    fn create_resolution_rule_input_rejects_unsupported_surface() {
        let error = create_resolution_rule_input(CreateResolutionRuleRequest {
            priority: 10,
            is_active: true,
            action_channel_id: Uuid::new_v4(),
            host_equals: Some("shop.example.test".to_string()),
            host_suffix: None,
            oauth_app_id: None,
            surface: Some("grpc".to_string()),
            locale: None,
        })
        .expect_err("unsupported surface should be rejected");

        assert!(error.contains("Unsupported surface"));
    }

    #[test]
    fn update_resolution_rule_input_trims_patch_fields() {
        let payload = update_resolution_rule_input(UpdateResolutionRuleRequest {
            priority: Some(40),
            is_active: Some(false),
            action_channel_id: Some(Uuid::new_v4()),
            host_equals: Some(" SHOP.EXAMPLE.TEST ".to_string()),
            host_suffix: Some("   ".to_string()),
            oauth_app_id: Some(" 550e8400-e29b-41d4-a716-446655440000 ".to_string()),
            surface: Some(" HTTP ".to_string()),
            locale: Some(" EN_US ".to_string()),
        });

        assert_eq!(payload.priority, Some(40));
        assert_eq!(payload.is_active, Some(false));
        assert_eq!(payload.host_equals.as_deref(), Some("shop.example.test"));
        assert_eq!(payload.host_suffix.as_deref(), None);
        assert_eq!(
            payload.oauth_app_id.as_deref(),
            Some("550e8400-e29b-41d4-a716-446655440000")
        );
        assert_eq!(payload.surface.as_deref(), Some("http"));
        assert_eq!(payload.locale.as_deref(), Some("en_us"));
    }
}
