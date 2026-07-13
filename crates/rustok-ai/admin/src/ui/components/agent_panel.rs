use crate::i18n::t;
use crate::model::{
    AiAgentDescriptorPayload, AiAgentModelAssignmentPayload, AiAgentPrincipalPayload,
    AiAgentWorkflowPayload,
};
use crate::ui::leptos::Card;
use leptos::prelude::*;
use std::collections::BTreeMap;

/// Read-only owner catalog. Configuration controls are added only when the
/// platform supplies the shared tenant RBAC catalog; this panel never accepts
/// raw role or permission vocabulary.
#[component]
pub fn AiAgentPanel(
    ui_locale: Option<String>,
    catalog: Vec<AiAgentDescriptorPayload>,
    workflows: Vec<AiAgentWorkflowPayload>,
    principals: Vec<AiAgentPrincipalPayload>,
    assignments: Vec<AiAgentModelAssignmentPayload>,
) -> impl IntoView {
    let catalog_locale = ui_locale.clone();
    let workflows_locale = ui_locale.clone();
    let principals_locale = ui_locale.clone();
    let operations_label = t(ui_locale.as_deref(), "ai.agents.operations", "Operations");
    let capabilities_label = t(
        ui_locale.as_deref(),
        "ai.agents.capabilities",
        "Capabilities",
    );
    let required_permissions_label = t(
        ui_locale.as_deref(),
        "ai.agents.requiredPermissions",
        "Required permissions",
    );
    let no_assignment_label = t(
        ui_locale.as_deref(),
        "ai.agents.noModelAssignment",
        "No configured model assignment",
    );
    let active_label = t(ui_locale.as_deref(), "ai.common.active", "active");
    let inactive_label = t(ui_locale.as_deref(), "ai.common.inactive", "inactive");
    let assignment_summaries =
        assignments
            .into_iter()
            .fold(BTreeMap::new(), |mut items, assignment| {
                let summary = format!(
                    "{} / {} / {}{}",
                    assignment.provider_profile_id,
                    assignment.execution_mode,
                    if assignment.is_active {
                        active_label.as_str()
                    } else {
                        inactive_label.as_str()
                    },
                    assignment
                        .model_override
                        .as_deref()
                        .map(|model| format!(" / {model}"))
                        .unwrap_or_default(),
                );
                items
                    .entry(assignment.agent_principal_id)
                    .or_insert_with(Vec::new)
                    .push(summary);
                items
            });

    view! {
        <Card title=t(ui_locale.as_deref(), "ai.card.agents", "Agents")>
            <div class="space-y-4 text-sm">
                <div class="space-y-2">
                    <div class="font-medium text-foreground">{t(catalog_locale.as_deref(), "ai.agents.ownerCatalog", "Owner catalog")}</div>
                    <For each=move || catalog.clone() key=|agent| agent.slug.clone() let:agent>
                        <div class="rounded border border-border p-3">
                            <div class="font-medium">{agent.display_name}</div>
                            <div class="text-xs text-muted-foreground">{format!("{} / {} / {}", agent.owner, agent.kind, agent.slug)}</div>
                            <div class="mt-1 text-muted-foreground">{agent.responsibility}</div>
                            <div class="mt-1 text-xs text-muted-foreground">{format!("{}: {}", operations_label, agent.allowed_operations.join(", "))}</div>
                            <div class="mt-1 text-xs text-muted-foreground">{format!("{}: {}", capabilities_label, agent.required_capabilities.join(", "))}</div>
                            <div class="mt-1 text-xs text-muted-foreground">{format!("{}: {}", required_permissions_label, agent.required_permissions.join(", "))}</div>
                        </div>
                    </For>
                </div>
                <div class="space-y-2">
                    <div class="font-medium text-foreground">{t(workflows_locale.as_deref(), "ai.agents.workflows", "Workflows")}</div>
                    <For each=move || workflows.clone() key=|workflow| workflow.slug.clone() let:workflow>
                        <AiAgentWorkflowCard workflow=workflow />
                    </For>
                </div>
                <div class="text-xs text-muted-foreground">
                    {format!("{}: {}", t(principals_locale.as_deref(), "ai.agents.configuredPrincipals", "Configured principals"), principals.len())}
                </div>
                <div class="space-y-2">
                    <div class="font-medium text-foreground">{t(ui_locale.as_deref(), "ai.agents.modelAssignments", "Model assignments")}</div>
                    <For each=move || principals.clone() key=|principal| principal.id.clone() let:principal>
                        <div class="rounded border border-border p-3 text-xs text-muted-foreground">
                            <div class="font-medium text-foreground">{principal.slug}</div>
                            <div>{assignment_summaries.get(&principal.id).map(|items| items.join(", ")).unwrap_or_else(|| no_assignment_label.clone())}</div>
                        </div>
                    </For>
                </div>
            </div>
        </Card>
    }
}

#[component]
fn AiAgentWorkflowCard(workflow: AiAgentWorkflowPayload) -> impl IntoView {
    let stages = workflow
        .stages
        .iter()
        .map(|stage| {
            format!(
                "{} -> {}{}",
                stage.id,
                stage.agent_slug,
                if stage.requires_approval {
                    " (approval)"
                } else {
                    ""
                }
            )
        })
        .collect::<Vec<_>>();

    view! {
        <div class="rounded border border-border p-3">
            <div class="font-medium">{workflow.display_name}</div>
            <div class="text-xs text-muted-foreground">{workflow.owner}</div>
            <ol class="mt-2 list-decimal space-y-1 pl-5 text-muted-foreground">
                {stages.into_iter().map(|stage| view! { <li>{stage}</li> }).collect_view()}
            </ol>
        </div>
    }
}
