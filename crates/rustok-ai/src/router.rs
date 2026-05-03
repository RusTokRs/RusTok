use uuid::Uuid;

use crate::{
    error::{AiError, AiResult},
    model::{
        AiRunDecisionTrace, ExecutionMode, ExecutionOverride, ProviderCapability, ProviderKind,
        ProviderUsagePolicy, TaskProfile,
    },
};

#[derive(Debug, Clone)]
pub struct RouterProviderProfile {
    pub id: Uuid,
    pub slug: String,
    pub provider_kind: ProviderKind,
    pub model: String,
    pub capabilities: Vec<ProviderCapability>,
    pub usage_policy: ProviderUsagePolicy,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct ResolvedExecutionPlan {
    pub provider_profile_id: Uuid,
    pub task_profile_id: Option<Uuid>,
    pub tool_profile_id: Option<Uuid>,
    pub model: String,
    pub execution_mode: ExecutionMode,
    pub system_prompt: Option<String>,
    pub decision_trace: AiRunDecisionTrace,
}

pub struct AiRouter;

impl AiRouter {
    pub fn resolve(
        task_profile: Option<&TaskProfile>,
        providers: &[RouterProviderProfile],
        explicit_provider_profile_id: Option<Uuid>,
        explicit_tool_profile_id: Option<Uuid>,
        override_config: &ExecutionOverride,
        actor_role_slugs: &[String],
    ) -> AiResult<ResolvedExecutionPlan> {
        let mut reasons = Vec::new();
        let mut used_override = false;

        let execution_mode = if let Some(mode) = override_config.execution_mode {
            used_override = true;
            reasons.push(format!("Execution mode overridden to `{}`", mode.slug()));
            mode
        } else if let Some(profile) = task_profile {
            reasons.push(format!(
                "Execution mode inherited from task profile `{}`",
                profile.slug
            ));
            profile.default_execution_mode
        } else if explicit_tool_profile_id.is_some() {
            reasons.push("MCP tooling selected because a tool profile is attached".to_string());
            ExecutionMode::McpTooling
        } else {
            reasons
                .push("Direct execution selected because no tool profile is attached".to_string());
            ExecutionMode::Direct
        };

        let provider = if let Some(provider_id) = override_config
            .provider_profile_id
            .or(explicit_provider_profile_id)
        {
            if override_config.provider_profile_id.is_some() {
                used_override = true;
                reasons.push("Provider profile selected via explicit override".to_string());
            } else {
                reasons.push("Provider profile supplied explicitly by the caller".to_string());
            }
            providers
                .iter()
                .find(|candidate| candidate.id == provider_id)
                .ok_or_else(|| AiError::NotFound("AI provider profile not found".to_string()))?
        } else {
            let profile = task_profile.ok_or_else(|| {
                AiError::Validation(
                    "task profile is required when provider_profile_id is not provided".to_string(),
                )
            })?;

            let preferred = profile
                .preferred_provider_profile_ids
                .iter()
                .filter_map(|id| providers.iter().find(|candidate| candidate.id == *id))
                .find(|candidate| provider_allowed(candidate, profile, actor_role_slugs));

            if let Some(candidate) = preferred {
                reasons.push(format!(
                    "Selected preferred provider `{}` from task profile `{}`",
                    candidate.slug, profile.slug
                ));
                candidate
            } else {
                providers
                    .iter()
                    .filter(|candidate| provider_allowed(candidate, profile, actor_role_slugs))
                    .find(|candidate| candidate.capabilities.contains(&profile.target_capability))
                    .ok_or_else(|| {
                        AiError::Validation(format!(
                            "no active provider profile can satisfy task profile `{}`",
                            profile.slug
                        ))
                    })?
            }
        };

        if let Some(profile) = task_profile {
            if !provider_allowed(provider, profile, actor_role_slugs) {
                return Err(AiError::Validation(format!(
                    "provider `{}` is not allowed for task profile `{}`",
                    provider.slug, profile.slug
                )));
            }
        }

        let model = override_config
            .model
            .clone()
            .unwrap_or_else(|| provider.model.clone());
        if override_config.model.is_some() {
            used_override = true;
            reasons.push(format!("Model override selected `{model}`"));
        } else {
            reasons.push(format!("Using provider default model `{model}`"));
        }

        Ok(ResolvedExecutionPlan {
            provider_profile_id: provider.id,
            task_profile_id: task_profile.map(|profile| profile.id),
            tool_profile_id: explicit_tool_profile_id
                .or_else(|| task_profile.and_then(|profile| profile.tool_profile_id)),
            model: model.clone(),
            execution_mode,
            system_prompt: task_profile.and_then(|profile| profile.system_prompt.clone()),
            decision_trace: AiRunDecisionTrace {
                task_profile_id: task_profile.map(|profile| profile.id),
                task_profile_slug: task_profile.map(|profile| profile.slug.clone()),
                provider_profile_id: Some(provider.id),
                provider_slug: Some(provider.slug.clone()),
                provider_kind: Some(provider.provider_kind),
                selected_model: Some(model),
                execution_mode: Some(execution_mode),
                execution_target: None,
                requested_locale: None,
                resolved_locale: None,
                reasons,
                used_override,
            },
        })
    }
}

fn provider_allowed(
    provider: &RouterProviderProfile,
    task_profile: &TaskProfile,
    actor_role_slugs: &[String],
) -> bool {
    if !provider.is_active {
        return false;
    }
    if !provider
        .capabilities
        .contains(&task_profile.target_capability)
    {
        return false;
    }
    if !task_profile.allowed_provider_profile_ids.is_empty()
        && !task_profile
            .allowed_provider_profile_ids
            .contains(&provider.id)
    {
        return false;
    }
    if !provider.usage_policy.allowed_task_profiles.is_empty()
        && !provider
            .usage_policy
            .allowed_task_profiles
            .iter()
            .any(|slug| slug == &task_profile.slug)
    {
        return false;
    }
    if provider
        .usage_policy
        .denied_task_profiles
        .iter()
        .any(|slug| slug == &task_profile.slug)
    {
        return false;
    }
    if !provider.usage_policy.restricted_role_slugs.is_empty()
        && !actor_role_slugs.iter().any(|role| {
            provider
                .usage_policy
                .restricted_role_slugs
                .iter()
                .any(|allowed| allowed == role)
        })
    {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn provider(
        id: u128,
        slug: &str,
        kind: ProviderKind,
        capabilities: Vec<ProviderCapability>,
        usage_policy: ProviderUsagePolicy,
    ) -> RouterProviderProfile {
        RouterProviderProfile {
            id: Uuid::from_u128(id),
            slug: slug.to_string(),
            provider_kind: kind,
            model: format!("{slug}-model"),
            capabilities,
            usage_policy,
            is_active: true,
        }
    }

    fn task_profile(
        id: u128,
        slug: &str,
        capability: ProviderCapability,
        preferred_provider_profile_ids: Vec<Uuid>,
        allowed_provider_profile_ids: Vec<Uuid>,
    ) -> TaskProfile {
        TaskProfile {
            id: Uuid::from_u128(id),
            slug: slug.to_string(),
            display_name: slug.to_string(),
            description: None,
            target_capability: capability,
            system_prompt: Some(format!("system::{slug}")),
            allowed_provider_profile_ids,
            preferred_provider_profile_ids,
            fallback_strategy: "ordered".to_string(),
            tool_profile_id: None,
            approval_policy: json!({}),
            default_execution_mode: ExecutionMode::Auto,
            is_active: true,
            metadata: json!({}),
        }
    }

    #[test]
    fn resolve_prefers_preferred_provider_when_allowed() {
        let first = provider(
            1,
            "openai-default",
            ProviderKind::OpenAiCompatible,
            vec![ProviderCapability::TextGeneration],
            ProviderUsagePolicy::default(),
        );
        let preferred = provider(
            2,
            "anthropic-copy",
            ProviderKind::Anthropic,
            vec![
                ProviderCapability::TextGeneration,
                ProviderCapability::CodeGeneration,
            ],
            ProviderUsagePolicy::default(),
        );
        let task = task_profile(
            10,
            "blog_draft",
            ProviderCapability::TextGeneration,
            vec![preferred.id],
            vec![],
        );

        let resolved = AiRouter::resolve(
            Some(&task),
            &[first, preferred.clone()],
            None,
            None,
            &ExecutionOverride::default(),
            &[],
        )
        .expect("router should resolve");

        assert_eq!(resolved.provider_profile_id, preferred.id);
        assert_eq!(resolved.model, preferred.model);
        assert!(resolved
            .decision_trace
            .reasons
            .iter()
            .any(|reason| reason.contains("Selected preferred provider")));
    }

    #[test]
    fn resolve_skips_restricted_provider_and_falls_back() {
        let restricted = provider(
            1,
            "gemini-vision",
            ProviderKind::Gemini,
            vec![ProviderCapability::TextGeneration],
            ProviderUsagePolicy {
                allowed_task_profiles: vec![],
                denied_task_profiles: vec![],
                restricted_role_slugs: vec!["ai-admin".to_string()],
            },
        );
        let fallback = provider(
            2,
            "openai-general",
            ProviderKind::OpenAiCompatible,
            vec![ProviderCapability::TextGeneration],
            ProviderUsagePolicy::default(),
        );
        let task = task_profile(
            11,
            "operator_chat",
            ProviderCapability::TextGeneration,
            vec![restricted.id],
            vec![],
        );

        let resolved = AiRouter::resolve(
            Some(&task),
            &[restricted, fallback.clone()],
            None,
            None,
            &ExecutionOverride::default(),
            &["support-agent".to_string()],
        )
        .expect("router should fall back to unrestricted provider");

        assert_eq!(resolved.provider_profile_id, fallback.id);
    }

    #[test]
    fn resolve_rejects_override_when_provider_denied_for_task() {
        let denied = provider(
            1,
            "gemini-image",
            ProviderKind::Gemini,
            vec![ProviderCapability::ImageGeneration],
            ProviderUsagePolicy {
                allowed_task_profiles: vec![],
                denied_task_profiles: vec!["image_asset".to_string()],
                restricted_role_slugs: vec![],
            },
        );
        let task = task_profile(
            12,
            "image_asset",
            ProviderCapability::ImageGeneration,
            vec![],
            vec![],
        );

        let error = AiRouter::resolve(
            Some(&task),
            std::slice::from_ref(&denied),
            Some(denied.id),
            None,
            &ExecutionOverride::default(),
            &[],
        )
        .expect_err("denied provider must not be selected");

        assert!(error
            .to_string()
            .contains("provider `gemini-image` is not allowed for task profile `image_asset`"));
    }

    #[test]
    fn resolve_applies_execution_mode_override() {
        let provider = provider(
            1,
            "openai-direct",
            ProviderKind::OpenAiCompatible,
            vec![ProviderCapability::TextGeneration],
            ProviderUsagePolicy::default(),
        );

        let resolved = AiRouter::resolve(
            None,
            std::slice::from_ref(&provider),
            Some(provider.id),
            None,
            &ExecutionOverride {
                provider_profile_id: None,
                model: Some("override-model".to_string()),
                execution_mode: Some(ExecutionMode::McpTooling),
            },
            &[],
        )
        .expect("router should honor override");

        assert_eq!(resolved.execution_mode, ExecutionMode::McpTooling);
        assert_eq!(resolved.model, "override-model");
        assert!(resolved.decision_trace.used_override);
    }
}
