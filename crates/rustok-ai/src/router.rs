use uuid::Uuid;

use crate::{
    engine::{ProviderFeature, ProviderSlug, provider_catalog_entry},
    error::{AiError, AiResult},
    model::{
        AiRunDecisionTrace, ExecutionMode, ExecutionOverride, ProviderCapability,
        ProviderUsagePolicy, TaskProfile,
    },
};

#[derive(Debug, Clone)]
pub struct RouterProviderProfile {
    pub id: Uuid,
    pub slug: String,
    pub provider_slug: ProviderSlug,
    pub model: String,
    pub capabilities: Vec<ProviderCapability>,
    pub usage_policy: ProviderUsagePolicy,
    pub is_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouterCandidateStatus {
    Selected,
    Eligible,
    Inactive,
    MissingCapability,
    MissingIntegrationFeature,
    NotInTaskAllowList,
    TaskDeniedByProviderPolicy,
    NotInProviderAllowList,
    MissingRequiredActorRole,
}

impl RouterCandidateStatus {
    pub const fn slug(&self) -> &'static str {
        match self {
            Self::Selected => "selected",
            Self::Eligible => "eligible",
            Self::Inactive => "inactive",
            Self::MissingCapability => "missing_capability",
            Self::MissingIntegrationFeature => "missing_integration_feature",
            Self::NotInTaskAllowList => "not_in_task_allow_list",
            Self::TaskDeniedByProviderPolicy => "task_denied_by_provider_policy",
            Self::NotInProviderAllowList => "not_in_provider_allow_list",
            Self::MissingRequiredActorRole => "missing_required_actor_role",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouterCandidateDecision {
    pub provider_profile_id: Uuid,
    pub provider_slug: String,
    pub integration_slug: ProviderSlug,
    pub status: RouterCandidateStatus,
    pub preferred_by_task: bool,
    pub reason: String,
}

#[derive(Debug)]
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

            let candidate_decisions = explain_provider_candidates(
                profile,
                providers,
                actor_role_slugs,
                Some(provider.id),
            );
            for decision in candidate_decisions {
                reasons.push(format!(
                    "Provider candidate `{}` status `{}`: {}",
                    decision.provider_slug,
                    decision.status.slug(),
                    decision.reason
                ));
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
                provider_slug: Some(provider.provider_slug.as_str().to_string()),
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

pub fn explain_provider_candidates(
    task_profile: &TaskProfile,
    providers: &[RouterProviderProfile],
    actor_role_slugs: &[String],
    selected_provider_profile_id: Option<Uuid>,
) -> Vec<RouterCandidateDecision> {
    providers
        .iter()
        .map(|provider| {
            let preferred_by_task = task_profile
                .preferred_provider_profile_ids
                .contains(&provider.id);
            let (status, reason) =
                provider_candidate_status(provider, task_profile, actor_role_slugs);
            let status = if selected_provider_profile_id == Some(provider.id)
                && matches!(status, RouterCandidateStatus::Eligible)
            {
                RouterCandidateStatus::Selected
            } else {
                status
            };
            RouterCandidateDecision {
                provider_profile_id: provider.id,
                provider_slug: provider.slug.clone(),
                integration_slug: provider.provider_slug.clone(),
                status,
                preferred_by_task,
                reason,
            }
        })
        .collect()
}

fn provider_allowed(
    provider: &RouterProviderProfile,
    task_profile: &TaskProfile,
    actor_role_slugs: &[String],
) -> bool {
    matches!(
        provider_candidate_status(provider, task_profile, actor_role_slugs).0,
        RouterCandidateStatus::Eligible
    )
}

fn provider_candidate_status(
    provider: &RouterProviderProfile,
    task_profile: &TaskProfile,
    _actor_role_slugs: &[String],
) -> (RouterCandidateStatus, String) {
    if !provider.is_active {
        return (
            RouterCandidateStatus::Inactive,
            "provider profile is inactive".to_string(),
        );
    }
    if !provider
        .capabilities
        .contains(&task_profile.target_capability)
    {
        return (
            RouterCandidateStatus::MissingCapability,
            format!(
                "provider lacks required `{}` capability",
                task_profile.target_capability.slug()
            ),
        );
    }
    let required_feature = match task_profile.target_capability {
        ProviderCapability::TextGeneration | ProviderCapability::CodeGeneration => {
            ProviderFeature::Chat
        }
        ProviderCapability::StructuredGeneration => ProviderFeature::StructuredOutput,
        ProviderCapability::ImageGeneration => ProviderFeature::Image,
        ProviderCapability::MultimodalUnderstanding => ProviderFeature::Multimodal,
        ProviderCapability::AlloyAssist => ProviderFeature::Tools,
    };
    let supports_feature = provider_catalog_entry(&provider.provider_slug)
        .is_some_and(|descriptor| descriptor.features.contains(&required_feature));
    if !supports_feature {
        return (
            RouterCandidateStatus::MissingIntegrationFeature,
            format!(
                "provider integration `{}` does not implement required {:?} feature",
                provider.provider_slug, required_feature
            ),
        );
    }
    if !task_profile.allowed_provider_profile_ids.is_empty()
        && !task_profile
            .allowed_provider_profile_ids
            .contains(&provider.id)
    {
        return (
            RouterCandidateStatus::NotInTaskAllowList,
            "provider is not listed in the task profile allow-list".to_string(),
        );
    }
    if provider
        .usage_policy
        .denied_task_profiles
        .iter()
        .any(|slug| slug == &task_profile.slug)
    {
        return (
            RouterCandidateStatus::TaskDeniedByProviderPolicy,
            "task profile is denied by provider usage policy".to_string(),
        );
    }
    if !provider.usage_policy.allowed_task_profiles.is_empty()
        && !provider
            .usage_policy
            .allowed_task_profiles
            .iter()
            .any(|slug| slug == &task_profile.slug)
    {
        return (
            RouterCandidateStatus::NotInProviderAllowList,
            "task profile is not listed in provider usage policy allow-list".to_string(),
        );
    }
    if !provider.usage_policy.restricted_role_slugs.is_empty() {
        return (
            RouterCandidateStatus::MissingRequiredActorRole,
            "provider role restriction awaits the platform tenant RBAC catalog".to_string(),
        );
    }
    (
        RouterCandidateStatus::Eligible,
        "provider satisfies task routing policy".to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn provider(
        id: u128,
        slug: &str,
        integration_slug: ProviderSlug,
        capabilities: Vec<ProviderCapability>,
        usage_policy: ProviderUsagePolicy,
    ) -> RouterProviderProfile {
        RouterProviderProfile {
            id: Uuid::from_u128(id),
            slug: slug.to_string(),
            provider_slug: integration_slug,
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
            ProviderSlug::openai_compatible(),
            vec![ProviderCapability::TextGeneration],
            ProviderUsagePolicy::default(),
        );
        let preferred = provider(
            2,
            "anthropic-copy",
            ProviderSlug::anthropic(),
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
        assert!(
            resolved
                .decision_trace
                .reasons
                .iter()
                .any(|reason| reason.contains("Selected preferred provider"))
        );
    }

    #[test]
    fn resolve_skips_restricted_provider_and_falls_back() {
        let restricted = provider(
            1,
            "gemini-vision",
            ProviderSlug::gemini(),
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
            ProviderSlug::openai_compatible(),
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
            ProviderSlug::gemini(),
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

        assert!(
            error
                .to_string()
                .contains("provider `gemini-image` is not allowed for task profile `image_asset`")
        );
    }

    #[test]
    fn candidate_rejects_profile_capability_missing_from_rig_descriptor() {
        let provider = provider(
            1,
            "vertex-vision-claim",
            ProviderSlug::new("vertex_ai").unwrap(),
            vec![ProviderCapability::MultimodalUnderstanding],
            ProviderUsagePolicy::default(),
        );
        let task = task_profile(
            12,
            "vision_analysis",
            ProviderCapability::MultimodalUnderstanding,
            vec![],
            vec![],
        );

        let (status, reason) = provider_candidate_status(&provider, &task, &[]);

        assert_eq!(status, RouterCandidateStatus::MissingIntegrationFeature);
        assert!(reason.contains("vertex_ai"));
        assert!(reason.contains("Multimodal"));
    }

    #[test]
    fn explain_provider_candidates_records_all_policy_reasons() {
        let inactive = RouterProviderProfile {
            is_active: false,
            ..provider(
                1,
                "inactive",
                ProviderSlug::openai_compatible(),
                vec![ProviderCapability::TextGeneration],
                ProviderUsagePolicy::default(),
            )
        };
        let missing_capability = provider(
            2,
            "vision-only",
            ProviderSlug::gemini(),
            vec![ProviderCapability::MultimodalUnderstanding],
            ProviderUsagePolicy::default(),
        );
        let restricted = provider(
            3,
            "restricted",
            ProviderSlug::anthropic(),
            vec![ProviderCapability::TextGeneration],
            ProviderUsagePolicy {
                allowed_task_profiles: vec![],
                denied_task_profiles: vec![],
                restricted_role_slugs: vec!["ai-admin".to_string()],
            },
        );
        let selected = provider(
            4,
            "selected",
            ProviderSlug::openai_compatible(),
            vec![ProviderCapability::TextGeneration],
            ProviderUsagePolicy::default(),
        );
        let task = task_profile(
            13,
            "operator_chat",
            ProviderCapability::TextGeneration,
            vec![restricted.id, selected.id],
            vec![],
        );

        let decisions = explain_provider_candidates(
            &task,
            &[inactive, missing_capability, restricted, selected.clone()],
            &["support-agent".to_string()],
            Some(selected.id),
        );

        assert_eq!(decisions[0].status, RouterCandidateStatus::Inactive);
        assert_eq!(
            decisions[1].status,
            RouterCandidateStatus::MissingCapability
        );
        assert_eq!(
            decisions[2].status,
            RouterCandidateStatus::MissingRequiredActorRole
        );
        assert_eq!(decisions[3].status, RouterCandidateStatus::Selected);
        assert!(decisions[2].preferred_by_task);
    }

    #[test]
    fn resolve_decision_trace_includes_candidate_statuses() {
        let restricted = provider(
            1,
            "restricted",
            ProviderSlug::anthropic(),
            vec![ProviderCapability::TextGeneration],
            ProviderUsagePolicy {
                allowed_task_profiles: vec![],
                denied_task_profiles: vec![],
                restricted_role_slugs: vec!["ai-admin".to_string()],
            },
        );
        let fallback = provider(
            2,
            "fallback",
            ProviderSlug::openai_compatible(),
            vec![ProviderCapability::TextGeneration],
            ProviderUsagePolicy::default(),
        );
        let task = task_profile(
            14,
            "operator_chat",
            ProviderCapability::TextGeneration,
            vec![restricted.id],
            vec![],
        );

        let resolved = AiRouter::resolve(
            Some(&task),
            &[restricted, fallback],
            None,
            None,
            &ExecutionOverride::default(),
            &["support-agent".to_string()],
        )
        .expect("router should resolve to fallback");

        assert!(resolved.decision_trace.reasons.iter().any(|reason| {
            reason.contains("Provider candidate `restricted` status `missing_required_actor_role`")
        }));
        assert!(
            resolved.decision_trace.reasons.iter().any(|reason| {
                reason.contains("Provider candidate `fallback` status `selected`")
            })
        );
    }

    #[test]
    fn resolve_applies_execution_mode_override() {
        let provider = provider(
            1,
            "openai-direct",
            ProviderSlug::openai_compatible(),
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
