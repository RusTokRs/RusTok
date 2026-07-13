use crate::i18n::t;
use crate::model::{
    AiAgentDescriptorPayload, AiAgentPrincipalPayload, AiAgentWorkflowPayload,
};
use crate::ui::leptos::Card;
use leptos::prelude::*;

/// Read-only owner catalog. Configuration controls are added only when the
/// platform supplies the shared tenant RBAC catalog; this panel never accepts
/// raw role or permission vocabulary.
#[component]
pub fn AiAgentPanel(
    ui_locale: Option<String>,
    catalog: Vec<AiAgentDescriptorPayload>,
    workflows: Vec<AiAgentWorkflowPayload>,
    principals: Vec<AiAgentPrincipalPayload>,
) -> impl IntoView {
    let catalog_locale = ui_locale.clone();
    let workflows_locale = ui_locale.clone();
    let principals_locale = ui_locale.clone();

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
                            <div class="mt-1 text-xs text-muted-foreground">{format!("Operations: {}", agent.allowed_operations.join(", "))}</div>
                            <div class="mt-1 text-xs text-muted-foreground">{format!("Capabilities: {}", agent.required_capabilities.join(", "))}</div>
                            <div class="mt-1 text-xs text-muted-foreground">{format!("Required permissions: {}", agent.required_permissions.join(", "))}</div>
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
