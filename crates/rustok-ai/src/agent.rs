use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AiError, AiResult, ProviderCapability};

/// Stable classification of an AI principal. It is independent from a model
/// provider and from the RBAC role names assigned by a deployment.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    Domain,
    Code,
    Orchestrator,
    Review,
}

/// Tenant-scoped non-human principal used for an AI run.
///
/// The runtime must intersect this principal's permissions with those of the
/// initiating subject. An agent therefore cannot elevate the initiator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentPrincipal {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub agent_slug: String,
    #[serde(default)]
    pub role_slugs: BTreeSet<String>,
    #[serde(default)]
    pub permission_slugs: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentDescriptor {
    pub slug: String,
    pub display_name: String,
    pub owner: String,
    pub kind: AgentKind,
    pub responsibility: String,
    #[serde(default)]
    pub required_permissions: BTreeSet<String>,
    #[serde(default)]
    pub allowed_operations: BTreeSet<String>,
    #[serde(default)]
    pub required_capabilities: Vec<ProviderCapability>,
    pub can_orchestrate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentWorkflowStage {
    pub id: String,
    pub agent_slug: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub requires_approval: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentWorkflowDescriptor {
    pub slug: String,
    pub display_name: String,
    pub owner: String,
    #[serde(default)]
    pub stages: Vec<AgentWorkflowStage>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentWorkflowStatus {
    Queued,
    Running,
    WaitingApproval,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentStageStatus {
    Pending,
    Ready,
    Running,
    WaitingApproval,
    Completed,
    Failed,
    Cancelled,
}

impl AgentStageStatus {
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

/// Owner-contributed catalog consumed by configuration and orchestration
/// surfaces. It intentionally contains no provider endpoints or credentials.
#[derive(Debug, Clone, Default)]
pub struct AgentCatalog {
    descriptors: Vec<AgentDescriptor>,
    workflows: Vec<AgentWorkflowDescriptor>,
}

impl AgentCatalog {
    pub fn new(
        descriptors: Vec<AgentDescriptor>,
        workflows: Vec<AgentWorkflowDescriptor>,
    ) -> AiResult<Self> {
        let catalog = Self {
            descriptors,
            workflows,
        };
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn descriptors(&self) -> &[AgentDescriptor] {
        &self.descriptors
    }

    pub fn workflows(&self) -> &[AgentWorkflowDescriptor] {
        &self.workflows
    }

    pub fn descriptor(&self, slug: &str) -> Option<&AgentDescriptor> {
        self.descriptors.iter().find(|descriptor| descriptor.slug == slug)
    }

    pub fn effective_permissions(
        &self,
        initiator_permissions: &BTreeSet<String>,
        principal: &AgentPrincipal,
    ) -> AiResult<BTreeSet<String>> {
        let descriptor = self.descriptor(&principal.agent_slug).ok_or_else(|| {
            AiError::Validation(format!("unknown agent descriptor `{}`", principal.agent_slug))
        })?;
        let effective = initiator_permissions
            .intersection(&principal.permission_slugs)
            .filter(|permission| descriptor.required_permissions.contains(*permission))
            .cloned()
            .collect::<BTreeSet<_>>();
        if effective != descriptor.required_permissions {
            return Err(AiError::Validation(format!(
                "agent `{}` lacks one or more required permissions in the initiator intersection",
                descriptor.slug
            )));
        }
        Ok(effective)
    }

    /// Resolves only stages whose declared dependencies completed. An approval
    /// gate is expressed as `WaitingApproval`, never as a scheduler bypass.
    pub fn ready_stages(
        &self,
        workflow_slug: &str,
        states: &std::collections::BTreeMap<String, AgentStageStatus>,
    ) -> AiResult<Vec<String>> {
        let workflow = self
            .workflows
            .iter()
            .find(|workflow| workflow.slug == workflow_slug)
            .ok_or_else(|| AiError::Validation(format!("unknown agent workflow `{workflow_slug}`")))?;
        Ok(workflow
            .stages
            .iter()
            .filter(|stage| states.get(&stage.id).copied().unwrap_or(AgentStageStatus::Pending) == AgentStageStatus::Pending)
            .filter(|stage| {
                stage.depends_on.iter().all(|dependency| {
                    states.get(dependency).copied() == Some(AgentStageStatus::Completed)
                })
            })
            .map(|stage| stage.id.clone())
            .collect())
    }

    fn validate(&self) -> AiResult<()> {
        let descriptor_slugs = self
            .descriptors
            .iter()
            .map(|descriptor| descriptor.slug.as_str())
            .collect::<BTreeSet<_>>();
        if descriptor_slugs.len() != self.descriptors.len() {
            return Err(AiError::Validation(
                "agent descriptor slugs must be unique".to_string(),
            ));
        }
        for workflow in &self.workflows {
            let stage_ids = workflow
                .stages
                .iter()
                .map(|stage| stage.id.as_str())
                .collect::<BTreeSet<_>>();
            if stage_ids.len() != workflow.stages.len() {
                return Err(AiError::Validation(format!(
                    "agent workflow `{}` has duplicate stage ids",
                    workflow.slug
                )));
            }
            for stage in &workflow.stages {
                if !descriptor_slugs.contains(stage.agent_slug.as_str()) {
                    return Err(AiError::Validation(format!(
                        "agent workflow `{}` references unknown agent `{}`",
                        workflow.slug, stage.agent_slug
                    )));
                }
                if stage.depends_on.iter().any(|dependency| dependency == &stage.id)
                    || stage
                        .depends_on
                        .iter()
                        .any(|dependency| !stage_ids.contains(dependency.as_str()))
                {
                    return Err(AiError::Validation(format!(
                        "agent workflow `{}` has an invalid dependency for stage `{}`",
                        workflow.slug, stage.id
                    )));
                }
            }
        }
        Ok(())
    }
}

#[cfg(feature = "server")]
/// Maps Alloy's owner-owned descriptors into the generic runtime catalog.
/// `rustok-ai-alloy` never depends on this crate, preventing a dependency cycle.
pub fn alloy_agent_catalog() -> AiResult<AgentCatalog> {
    let descriptors = rustok_ai_alloy::alloy_code_agents()
        .iter()
        .map(|descriptor| AgentDescriptor {
            slug: descriptor.slug.to_string(),
            display_name: descriptor.display_name.to_string(),
            owner: "rustok-ai-alloy".to_string(),
            kind: if descriptor.slug.contains("reviewer") {
                AgentKind::Review
            } else {
                AgentKind::Code
            },
            responsibility: descriptor.responsibility.to_string(),
            required_permissions: descriptor
                .required_permissions
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            allowed_operations: descriptor
                .allowed_operations
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            required_capabilities: vec![
                ProviderCapability::CodeGeneration,
                ProviderCapability::AlloyAssist,
            ],
            can_orchestrate: false,
        })
        .collect();
    let workflows = rustok_ai_alloy::alloy_swarm_workflows()
        .iter()
        .map(|workflow| AgentWorkflowDescriptor {
            slug: workflow.slug.to_string(),
            display_name: workflow.display_name.to_string(),
            owner: "rustok-ai-alloy".to_string(),
            stages: workflow
                .stages
                .iter()
                .map(|stage| AgentWorkflowStage {
                    id: stage.id.to_string(),
                    agent_slug: stage.agent_slug.to_string(),
                    depends_on: stage
                        .depends_on
                        .iter()
                        .map(|value| (*value).to_string())
                        .collect(),
                    requires_approval: stage.requires_approval,
                })
                .collect(),
        })
        .collect();
    AgentCatalog::new(descriptors, workflows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn descriptor() -> AgentDescriptor {
        AgentDescriptor {
            slug: "catalog_enricher".to_string(),
            display_name: "Catalog enricher".to_string(),
            owner: "rustok-ai-product".to_string(),
            kind: AgentKind::Domain,
            responsibility: "Enrich product data".to_string(),
            required_permissions: BTreeSet::from(["product.write".to_string()]),
            allowed_operations: BTreeSet::from(["product.update_generated".to_string()]),
            required_capabilities: vec![ProviderCapability::StructuredGeneration],
            can_orchestrate: false,
        }
    }

    #[test]
    fn permission_intersection_cannot_elevate_initiator() {
        let catalog = AgentCatalog::new(vec![descriptor()], vec![]).unwrap();
        let principal = AgentPrincipal {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            agent_slug: "catalog_enricher".to_string(),
            role_slugs: BTreeSet::new(),
            permission_slugs: BTreeSet::from([
                "product.write".to_string(),
                "product.delete".to_string(),
            ]),
        };
        assert!(catalog
            .effective_permissions(&BTreeSet::from(["product.read".to_string()]), &principal)
            .is_err());
        assert_eq!(
            catalog
                .effective_permissions(
                    &BTreeSet::from(["product.write".to_string(), "product.delete".to_string()]),
                    &principal,
                )
                .unwrap(),
            BTreeSet::from(["product.write".to_string()])
        );
    }

    #[test]
    fn scheduler_only_releases_dependency_ready_stages() {
        let workflow = AgentWorkflowDescriptor {
            slug: "catalog_review".to_string(),
            display_name: "Catalog review".to_string(),
            owner: "rustok-ai-product".to_string(),
            stages: vec![
                AgentWorkflowStage {
                    id: "draft".to_string(),
                    agent_slug: "catalog_enricher".to_string(),
                    depends_on: vec![],
                    requires_approval: false,
                },
                AgentWorkflowStage {
                    id: "review".to_string(),
                    agent_slug: "catalog_enricher".to_string(),
                    depends_on: vec!["draft".to_string()],
                    requires_approval: true,
                },
            ],
        };
        let catalog = AgentCatalog::new(vec![descriptor()], vec![workflow]).unwrap();
        assert_eq!(
            catalog.ready_stages("catalog_review", &BTreeMap::new()).unwrap(),
            vec!["draft"]
        );
        assert_eq!(
            catalog
                .ready_stages(
                    "catalog_review",
                    &BTreeMap::from([("draft".to_string(), AgentStageStatus::Completed)]),
                )
                .unwrap(),
            vec!["review"]
        );
    }

    #[cfg(feature = "server")]
    #[test]
    fn maps_alloy_owned_code_agents_without_leaking_runtime_ownership() {
        let catalog = alloy_agent_catalog().unwrap();
        assert_eq!(catalog.descriptors().len(), 4);
        assert_eq!(catalog.workflows()[0].owner, "rustok-ai-alloy");
        assert!(catalog
            .descriptor("alloy_code_reviewer")
            .is_some_and(|descriptor| descriptor.kind == AgentKind::Review));
    }
}
