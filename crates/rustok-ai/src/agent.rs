use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AiError, AiResult, ProviderCapability};

/// Stable classification of an AI principal. It is independent from a model
/// provider and from the RBAC role names assigned by a deployment.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    Product,
    Code,
    Orchestrator,
    Review,
}

impl AgentKind {
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Product => "product",
            Self::Code => "code",
            Self::Orchestrator => "orchestrator",
            Self::Review => "review",
        }
    }
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

/// Owner-validated task selection passed to the canonical AI task-run path.
/// It contains no provider settings or secrets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentStageExecution {
    pub task_slug: String,
}

/// Internal composition binding between an owner-contributed descriptor and
/// its owner-level input validator. It is deliberately not persisted or
/// exposed through transport contracts.
#[cfg(feature = "server")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentStageValidator {
    Alloy,
    Product,
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
    #[cfg(feature = "server")]
    stage_validators: BTreeMap<String, AgentStageValidator>,
}

impl AgentCatalog {
    pub fn new(
        descriptors: Vec<AgentDescriptor>,
        workflows: Vec<AgentWorkflowDescriptor>,
    ) -> AiResult<Self> {
        let catalog = Self {
            descriptors,
            workflows,
            #[cfg(feature = "server")]
            stage_validators: BTreeMap::new(),
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
        self.descriptors
            .iter()
            .find(|descriptor| descriptor.slug == slug)
    }

    #[cfg(feature = "server")]
    pub fn validate_stage_execution(
        &self,
        agent_slug: &str,
        payload: &serde_json::Value,
    ) -> AiResult<AgentStageExecution> {
        self.descriptor(agent_slug).ok_or_else(|| {
            AiError::Validation(format!("unknown agent descriptor `{agent_slug}`"))
        })?;
        let validator = self.stage_validators.get(agent_slug).ok_or_else(|| {
            AiError::Validation(format!(
                "agent `{agent_slug}` has no registered stage execution validator"
            ))
        })?;
        let task_slug = match validator {
            AgentStageValidator::Alloy => {
                rustok_ai_alloy::validate_stage_execution_input(agent_slug, payload)
                    .map_err(AiError::Validation)?
                    .task_slug
            }
            AgentStageValidator::Product => {
                rustok_ai_product::validate_product_agent_stage_input(agent_slug, payload)
                    .map_err(AiError::Validation)?
            }
        };
        Ok(AgentStageExecution {
            task_slug: task_slug.to_string(),
        })
    }

    #[cfg(feature = "server")]
    fn with_stage_validators(
        mut self,
        stage_validators: BTreeMap<String, AgentStageValidator>,
    ) -> AiResult<Self> {
        for agent_slug in stage_validators.keys() {
            if self.descriptor(agent_slug).is_none() {
                return Err(AiError::Validation(format!(
                    "stage execution validator references unknown agent `{agent_slug}`"
                )));
            }
        }
        for descriptor in &self.descriptors {
            if !stage_validators.contains_key(&descriptor.slug) {
                return Err(AiError::Validation(format!(
                    "agent `{}` has no registered stage execution validator",
                    descriptor.slug
                )));
            }
        }
        self.stage_validators = stage_validators;
        Ok(self)
    }

    pub fn effective_permissions(
        &self,
        initiator_permissions: &BTreeSet<String>,
        principal: &AgentPrincipal,
    ) -> AiResult<BTreeSet<String>> {
        let descriptor = self.descriptor(&principal.agent_slug).ok_or_else(|| {
            AiError::Validation(format!(
                "unknown agent descriptor `{}`",
                principal.agent_slug
            ))
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
            .ok_or_else(|| {
                AiError::Validation(format!("unknown agent workflow `{workflow_slug}`"))
            })?;
        Ok(workflow
            .stages
            .iter()
            .filter(|stage| {
                states
                    .get(&stage.id)
                    .copied()
                    .unwrap_or(AgentStageStatus::Pending)
                    == AgentStageStatus::Pending
            })
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
            let descriptor_by_slug = self
                .descriptors
                .iter()
                .map(|descriptor| (descriptor.slug.as_str(), descriptor))
                .collect::<std::collections::BTreeMap<_, _>>();
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
                if descriptor_by_slug
                    .get(stage.agent_slug.as_str())
                    .is_some_and(|descriptor| descriptor.owner != workflow.owner)
                {
                    return Err(AiError::Validation(format!(
                        "agent workflow `{}` cannot use agent `{}` owned by another module",
                        workflow.slug, stage.agent_slug
                    )));
                }
                if stage
                    .depends_on
                    .iter()
                    .any(|dependency| dependency == &stage.id)
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
            ensure_workflow_is_acyclic(workflow)?;
        }
        Ok(())
    }
}

fn ensure_workflow_is_acyclic(workflow: &AgentWorkflowDescriptor) -> AiResult<()> {
    let mut remaining = workflow
        .stages
        .iter()
        .map(|stage| stage.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut completed = BTreeSet::new();

    while !remaining.is_empty() {
        let ready = workflow
            .stages
            .iter()
            .filter(|stage| remaining.contains(stage.id.as_str()))
            .filter(|stage| {
                stage
                    .depends_on
                    .iter()
                    .all(|dependency| completed.contains(dependency.as_str()))
            })
            .map(|stage| stage.id.as_str())
            .collect::<Vec<_>>();
        if ready.is_empty() {
            return Err(AiError::Validation(format!(
                "agent workflow `{}` contains a dependency cycle",
                workflow.slug
            )));
        }
        for stage_id in ready {
            remaining.remove(stage_id);
            completed.insert(stage_id);
        }
    }
    Ok(())
}

#[cfg(feature = "server")]
/// Builds the generic runtime catalog from owner-contributed agent descriptors.
/// Alloy is the first contributor; owner support crates never depend on this
/// crate, preventing a dependency cycle.
pub fn agent_catalog() -> AiResult<AgentCatalog> {
    let mut descriptors: Vec<AgentDescriptor> = rustok_ai_alloy::alloy_code_agents()
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
    let product_descriptors = rustok_ai_product::product_ai_agents()
        .iter()
        .map(|agent| {
            let required_capabilities = agent
                .required_capabilities
                .iter()
                .map(|capability| product_agent_capability(capability))
                .collect::<AiResult<Vec<_>>>()?;
            Ok(AgentDescriptor {
                slug: agent.slug.to_string(),
                display_name: agent.display_name.to_string(),
                owner: "rustok-ai-product".to_string(),
                kind: AgentKind::Product,
                responsibility: agent.responsibility.to_string(),
                required_permissions: agent
                    .required_permissions
                    .iter()
                    .map(|value| (*value).to_string())
                    .collect(),
                allowed_operations: [agent.task_slug.to_string()].into_iter().collect(),
                required_capabilities,
                can_orchestrate: false,
            })
        })
        .collect::<AiResult<Vec<_>>>()?;
    descriptors.extend(product_descriptors);
    let mut workflows: Vec<AgentWorkflowDescriptor> = rustok_ai_alloy::alloy_swarm_workflows()
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
    workflows.extend(
        rustok_ai_product::product_ai_workflows()
            .iter()
            .map(|workflow| AgentWorkflowDescriptor {
                slug: workflow.slug.to_string(),
                display_name: workflow.display_name.to_string(),
                owner: "rustok-ai-product".to_string(),
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
            }),
    );
    let mut stage_validators = BTreeMap::new();
    for agent in rustok_ai_alloy::alloy_code_agents() {
        stage_validators.insert(agent.slug.to_string(), AgentStageValidator::Alloy);
    }
    for agent in rustok_ai_product::product_ai_agents() {
        stage_validators.insert(agent.slug.to_string(), AgentStageValidator::Product);
    }
    AgentCatalog::new(descriptors, workflows)?.with_stage_validators(stage_validators)
}

#[cfg(feature = "server")]
fn product_agent_capability(value: &str) -> AiResult<ProviderCapability> {
    match value {
        "text_generation" => Ok(ProviderCapability::TextGeneration),
        "structured_generation" => Ok(ProviderCapability::StructuredGeneration),
        other => Err(AiError::Validation(format!(
            "product agent declares unsupported provider capability `{other}`"
        ))),
    }
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
            kind: AgentKind::Product,
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
        assert!(
            catalog
                .effective_permissions(&BTreeSet::from(["product.read".to_string()]), &principal)
                .is_err()
        );
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
            catalog
                .ready_stages("catalog_review", &BTreeMap::new())
                .unwrap(),
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

    #[test]
    fn catalog_rejects_cross_owner_agents_and_dependency_cycles() {
        let mut foreign_agent = descriptor();
        foreign_agent.slug = "foreign_agent".to_string();
        foreign_agent.owner = "rustok-ai-content".to_string();
        let foreign_workflow = AgentWorkflowDescriptor {
            slug: "invalid_owner".to_string(),
            display_name: "Invalid owner".to_string(),
            owner: "rustok-ai-product".to_string(),
            stages: vec![AgentWorkflowStage {
                id: "run".to_string(),
                agent_slug: foreign_agent.slug.clone(),
                depends_on: vec![],
                requires_approval: false,
            }],
        };
        assert!(AgentCatalog::new(vec![foreign_agent], vec![foreign_workflow]).is_err());

        let cyclic_workflow = AgentWorkflowDescriptor {
            slug: "invalid_cycle".to_string(),
            display_name: "Invalid cycle".to_string(),
            owner: "rustok-ai-product".to_string(),
            stages: vec![
                AgentWorkflowStage {
                    id: "first".to_string(),
                    agent_slug: "catalog_enricher".to_string(),
                    depends_on: vec!["second".to_string()],
                    requires_approval: false,
                },
                AgentWorkflowStage {
                    id: "second".to_string(),
                    agent_slug: "catalog_enricher".to_string(),
                    depends_on: vec!["first".to_string()],
                    requires_approval: false,
                },
            ],
        };
        assert!(AgentCatalog::new(vec![descriptor()], vec![cyclic_workflow]).is_err());
    }

    #[cfg(feature = "server")]
    #[test]
    fn maps_alloy_owned_code_agents_without_leaking_runtime_ownership() {
        let catalog = agent_catalog().unwrap();
        assert_eq!(catalog.descriptors().len(), 6);
        assert_eq!(catalog.workflows()[0].owner, "rustok-ai-alloy");
        assert!(
            catalog
                .descriptor("alloy_code_reviewer")
                .is_some_and(|descriptor| descriptor.kind == AgentKind::Review)
        );
        assert!(
            catalog
                .descriptor("product_copywriter")
                .is_some_and(|descriptor| descriptor.kind == AgentKind::Product)
        );
        assert!(
            catalog
                .validate_stage_execution(
                    "product_copywriter",
                    &serde_json::json!({"product_id":"00000000-0000-0000-0000-000000000001"}),
                )
                .is_ok()
        );
    }

    #[test]
    fn agent_kind_slugs_are_closed_and_transport_stable() {
        assert_eq!(AgentKind::Product.slug(), "product");
        assert_eq!(AgentKind::Code.slug(), "code");
        assert_eq!(AgentKind::Orchestrator.slug(), "orchestrator");
        assert_eq!(AgentKind::Review.slug(), "review");
    }

    #[cfg(feature = "server")]
    #[test]
    fn executable_catalog_requires_one_validator_per_agent() {
        let catalog = AgentCatalog::new(vec![descriptor()], vec![]).unwrap();
        assert!(catalog.with_stage_validators(BTreeMap::new()).is_err());
    }

    #[cfg(feature = "server")]
    #[test]
    fn owner_stage_binding_resolves_to_a_registered_direct_handler() {
        let catalog = agent_catalog().unwrap();
        let handlers = crate::DirectExecutionRegistry::with_defaults();

        let alloy = catalog
            .validate_stage_execution(
                "alloy_code_verifier",
                &serde_json::json!({"operation":"run_script"}),
            )
            .unwrap();
        assert!(handlers.handler(&alloy.task_slug).is_some());
        assert!(
            catalog
                .validate_stage_execution(
                    "alloy_code_verifier",
                    &serde_json::json!({"operation":"validate_script"}),
                )
                .is_err()
        );

        let product = catalog
            .validate_stage_execution(
                "product_copywriter",
                &serde_json::json!({"product_id":"00000000-0000-0000-0000-000000000001"}),
            )
            .unwrap();
        assert!(handlers.handler(&product.task_slug).is_some());
    }
}
