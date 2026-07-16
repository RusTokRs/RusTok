use crate::i18n::t;
use crate::model::{
    AiAgentDescriptorPayload, AiAgentModelAssignmentPayload, AiAgentPrincipalPayload,
    AiAgentWorkflowPayload, AiProviderProfilePayload, AiTenantRbacPermissionPayload,
    AiTenantRbacRolePayload,
};
use crate::ui::leptos::{Card, TextField};
use leptos::ev::MouseEvent;
use leptos::prelude::*;
use std::collections::BTreeMap;

/// Typed request emitted only from owner and tenant RBAC catalog selections.
#[derive(Clone)]
pub struct AiAgentPrincipalCreateForm {
    pub slug: String,
    pub descriptor_owner: String,
    pub descriptor_slug: String,
    pub role_slugs: Vec<String>,
}

/// Typed role replacement emitted only from tenant RBAC catalog selections.
#[derive(Clone)]
pub struct AiAgentPrincipalUpdateForm {
    pub id: String,
    pub role_slugs: Vec<String>,
    pub is_active: bool,
}

/// Typed assignment request assembled only from principal/provider catalog
/// selections and the closed execution-mode enum.
#[derive(Clone)]
pub struct AiAgentModelAssignmentCreateForm {
    pub agent_principal_id: String,
    pub provider_profile_id: String,
    pub model_override: Option<String>,
    pub execution_mode: String,
}

#[derive(Clone)]
pub struct AiAgentModelAssignmentUpdateForm {
    pub id: String,
    pub model_override: Option<String>,
    pub execution_mode: String,
    pub is_active: bool,
}

/// Owner-catalog and tenant-RBAC-driven agent principal editor. It never
/// exposes free-form role, permission, owner, or descriptor inputs.
#[component]
pub fn AiAgentPanel(
    ui_locale: Option<String>,
    catalog: Vec<AiAgentDescriptorPayload>,
    workflows: Vec<AiAgentWorkflowPayload>,
    principals: Vec<AiAgentPrincipalPayload>,
    assignments: Vec<AiAgentModelAssignmentPayload>,
    providers: Vec<AiProviderProfilePayload>,
    tenant_rbac_roles: Vec<AiTenantRbacRolePayload>,
    tenant_rbac_permissions: Vec<AiTenantRbacPermissionPayload>,
    principal_slug: RwSignal<String>,
    selected_descriptor_slug: RwSignal<String>,
    selected_principal_id: RwSignal<String>,
    selected_role_slugs: RwSignal<Vec<String>>,
    principal_active: RwSignal<bool>,
    on_create_principal: Callback<AiAgentPrincipalCreateForm>,
    on_update_principal: Callback<AiAgentPrincipalUpdateForm>,
    assignment_principal_id: RwSignal<String>,
    assignment_provider_profile_id: RwSignal<String>,
    assignment_model_override: RwSignal<String>,
    assignment_execution_mode: RwSignal<String>,
    assignment_active: RwSignal<bool>,
    selected_assignment_id: RwSignal<String>,
    on_create_assignment: Callback<AiAgentModelAssignmentCreateForm>,
    on_update_assignment: Callback<AiAgentModelAssignmentUpdateForm>,
) -> impl IntoView {
    let catalog_locale = ui_locale.clone();
    let workflows_locale = ui_locale.clone();
    let principals_locale = ui_locale.clone();
    let update_principal_locale = ui_locale.clone();
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
            .iter()
            .cloned()
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
    let descriptor_choices = catalog.clone();
    let principal_choices = principals.clone();
    let assignment_principal_choices = principals.clone();
    let assignment_provider_choices = providers
        .into_iter()
        .filter(|provider| provider.is_active)
        .collect::<Vec<_>>();
    let assignment_choices = assignments.clone();
    let role_choices = tenant_rbac_roles.clone();
    let role_choices_for_create = tenant_rbac_roles.clone();
    let descriptor_catalog_for_create = catalog.clone();
    let role_choices_for_update = tenant_rbac_roles.clone();
    let active_label_for_editor = active_label.clone();
    let inactive_label_for_editor = inactive_label.clone();

    view! {
        <Card title=t(ui_locale.as_deref(), "ai.card.agents", "Agents")>
            <div class="space-y-4 text-sm">
                <form class="space-y-3 rounded border border-border p-3" on:submit=move |event| {
                    event.prevent_default();
                    let descriptor_slug = selected_descriptor_slug.get_untracked();
                    if let Some(descriptor) = descriptor_catalog_for_create
                        .iter()
                        .find(|descriptor| descriptor.slug == descriptor_slug)
                    {
                        on_create_principal.run(AiAgentPrincipalCreateForm {
                            slug: principal_slug.get_untracked(),
                            descriptor_owner: descriptor.owner.clone(),
                            descriptor_slug: descriptor.slug.clone(),
                            role_slugs: selected_role_slugs.get_untracked(),
                        });
                    }
                }>
                    <div class="font-medium text-foreground">{t(ui_locale.as_deref(), "ai.agents.principalEditor", "Agent principal")}</div>
                    <TextField label=t(ui_locale.as_deref(), "ai.field.slug", "Slug") value=principal_slug />
                    <label class="grid gap-1 text-sm text-muted-foreground">
                        <span>{t(ui_locale.as_deref(), "ai.field.agentDescriptor", "Agent descriptor")}</span>
                        <select
                            class="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground"
                            prop:value=move || selected_descriptor_slug.get()
                            on:change=move |event| selected_descriptor_slug.set(event_target_value(&event))
                        >
                            <option value="">{t(ui_locale.as_deref(), "ai.agents.selectDescriptor", "Select an owner descriptor")}</option>
                            <For each=move || descriptor_choices.clone() key=|descriptor| descriptor.slug.clone() let:descriptor>
                                <option value=descriptor.slug.clone()>{format!("{} / {}", descriptor.owner, descriptor.display_name)}</option>
                            </For>
                        </select>
                    </label>
                    <fieldset class="space-y-2">
                        <legend class="text-sm text-muted-foreground">{t(ui_locale.as_deref(), "ai.agents.assignRoles", "Assigned roles")}</legend>
                        <AiAgentRoleSelector
                            roles=role_choices_for_create.clone()
                            selected_role_slugs
                        />
                    </fieldset>
                    <button
                        type="submit"
                        class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:opacity-50"
                        disabled=move || principal_slug.get().trim().is_empty() || selected_descriptor_slug.get().trim().is_empty()
                    >{t(ui_locale.as_deref(), "ai.action.createAgentPrincipal", "Create agent principal")}</button>
                </form>
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
                    <For each=move || principal_choices.clone() key=|principal| principal.id.clone() let:principal>
                        {
                            let selected_id = principal.id.clone();
                            let selected_slug = principal.slug.clone();
                            let selected_descriptor = principal.descriptor_slug.clone();
                            let selected_roles = principal.role_slugs.clone();
                            let selected_active = principal.is_active;
                            view! {
                        <button class="w-full rounded border border-border p-3 text-left text-xs text-muted-foreground hover:bg-muted" on:click=move |_| {
                            selected_principal_id.set(selected_id.clone());
                            principal_slug.set(selected_slug.clone());
                            selected_descriptor_slug.set(selected_descriptor.clone());
                            selected_role_slugs.set(selected_roles.clone());
                            principal_active.set(selected_active);
                        }>
                            <div class="font-medium text-foreground">{principal.slug}</div>
                            <div>{assignment_summaries.get(&principal.id).map(|items| items.join(", ")).unwrap_or_else(|| no_assignment_label.clone())}</div>
                        </button>
                            }
                        }
                    </For>
                </div>
                <form class="space-y-3 rounded border border-border p-3" on:submit=move |event| {
                    event.prevent_default();
                    on_create_assignment.run(AiAgentModelAssignmentCreateForm {
                        agent_principal_id: assignment_principal_id.get_untracked(),
                        provider_profile_id: assignment_provider_profile_id.get_untracked(),
                        model_override: nonempty_value(assignment_model_override.get_untracked()),
                        execution_mode: assignment_execution_mode.get_untracked(),
                    });
                }>
                    <div class="font-medium text-foreground">{t(ui_locale.as_deref(), "ai.agents.assignmentEditor", "Model assignment")}</div>
                    <label class="grid gap-1 text-sm text-muted-foreground">
                        <span>{t(ui_locale.as_deref(), "ai.agents.assignmentPrincipal", "Agent principal")}</span>
                        <select class="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground" prop:value=move || assignment_principal_id.get() on:change=move |event| assignment_principal_id.set(event_target_value(&event))>
                            <option value="">{t(ui_locale.as_deref(), "ai.agents.selectPrincipal", "Select an agent principal")}</option>
                            <For each=move || assignment_principal_choices.clone() key=|principal| principal.id.clone() let:principal>
                                <option value=principal.id.clone() disabled=!principal.is_active>{principal.slug}</option>
                            </For>
                        </select>
                    </label>
                    <label class="grid gap-1 text-sm text-muted-foreground">
                        <span>{t(ui_locale.as_deref(), "ai.agents.assignmentProvider", "Provider profile")}</span>
                        <select class="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground" prop:value=move || assignment_provider_profile_id.get() on:change=move |event| assignment_provider_profile_id.set(event_target_value(&event))>
                            <option value="">{t(ui_locale.as_deref(), "ai.agents.selectProvider", "Select a provider profile")}</option>
                            <For each=move || assignment_provider_choices.clone() key=|provider| provider.id.clone() let:provider>
                                <option value=provider.id.clone()>{format!("{} / {}", provider.display_name, provider.model)}</option>
                            </For>
                        </select>
                    </label>
                    <TextField label=t(ui_locale.as_deref(), "ai.field.modelOverride", "Model override (optional)") value=assignment_model_override />
                    <AiExecutionModeSelector ui_locale=ui_locale.clone() value=assignment_execution_mode />
                    <button type="submit" class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:opacity-50" disabled=move || assignment_principal_id.get().trim().is_empty() || assignment_provider_profile_id.get().trim().is_empty()>{t(ui_locale.as_deref(), "ai.action.createModelAssignment", "Create model assignment")}</button>
                </form>
                <div class="space-y-3 rounded border border-border p-3">
                    <div class="font-medium text-foreground">{t(ui_locale.as_deref(), "ai.agents.updateAssignment", "Update selected model assignment")}</div>
                    <For each=move || assignment_choices.clone() key=|assignment| assignment.id.clone() let:assignment>
                        {
                            let assignment_id = assignment.id.clone();
                            let model_override = assignment.model_override.clone().unwrap_or_default();
                            let execution_mode = assignment.execution_mode.clone();
                            let is_active = assignment.is_active;
                            view! {
                                <button class="w-full rounded border border-border p-2 text-left text-xs text-muted-foreground hover:bg-muted" on:click=move |_| {
                                    selected_assignment_id.set(assignment_id.clone());
                                    assignment_model_override.set(model_override.clone());
                                    assignment_execution_mode.set(execution_mode.clone());
                                    assignment_active.set(is_active);
                                }>{format!("{} / {}", assignment.agent_principal_id, assignment.provider_profile_id)}</button>
                            }
                        }
                    </For>
                    <TextField label=t(ui_locale.as_deref(), "ai.field.modelOverride", "Model override (optional)") value=assignment_model_override />
                    <AiExecutionModeSelector ui_locale=ui_locale.clone() value=assignment_execution_mode />
                    <label class="flex items-center gap-2 text-sm text-muted-foreground">
                        <input type="checkbox" prop:checked=move || assignment_active.get() on:change=move |event| assignment_active.set(event_target_checked(&event)) />
                        {t(ui_locale.as_deref(), "ai.field.active", "Active")}
                    </label>
                    <button type="button" class="rounded-lg border border-border px-4 py-2 text-sm font-medium disabled:opacity-50" disabled=move || selected_assignment_id.get().trim().is_empty() on:click=move |_event: MouseEvent| on_update_assignment.run(AiAgentModelAssignmentUpdateForm {
                        id: selected_assignment_id.get_untracked(),
                        model_override: nonempty_value(assignment_model_override.get_untracked()),
                        execution_mode: assignment_execution_mode.get_untracked(),
                        is_active: assignment_active.get_untracked(),
                    })>{t(ui_locale.as_deref(), "ai.action.updateSelected", "Update selected")}</button>
                </div>
                <div class="space-y-3 rounded border border-border p-3">
                    <div class="font-medium text-foreground">{t(ui_locale.as_deref(), "ai.agents.updatePrincipal", "Update selected principal")}</div>
                    <div class="text-xs text-muted-foreground">{move || {
                        let selected = selected_principal_id.get();
                        if selected.is_empty() {
                            t(update_principal_locale.as_deref(), "ai.agents.noPrincipalSelected", "Select a principal above")
                        } else {
                            format!("{} ({})", principal_slug.get(), if principal_active.get() { active_label_for_editor.clone() } else { inactive_label_for_editor.clone() })
                        }
                    }}</div>
                    <fieldset class="space-y-2">
                        <legend class="text-sm text-muted-foreground">{t(ui_locale.as_deref(), "ai.agents.assignRoles", "Assigned roles")}</legend>
                        <AiAgentRoleSelector
                            roles=role_choices_for_update.clone()
                            selected_role_slugs
                        />
                    </fieldset>
                    <label class="flex items-center gap-2 text-sm text-muted-foreground">
                        <input type="checkbox" prop:checked=move || principal_active.get() on:change=move |event| principal_active.set(event_target_checked(&event)) />
                        {t(ui_locale.as_deref(), "ai.field.active", "Active")}
                    </label>
                    <button type="button" class="rounded-lg border border-border px-4 py-2 text-sm font-medium disabled:opacity-50" disabled=move || selected_principal_id.get().trim().is_empty() on:click=move |_event: MouseEvent| {
                        on_update_principal.run(AiAgentPrincipalUpdateForm {
                            id: selected_principal_id.get_untracked(),
                            role_slugs: selected_role_slugs.get_untracked(),
                            is_active: principal_active.get_untracked(),
                        });
                    }>{t(ui_locale.as_deref(), "ai.action.updateSelected", "Update selected")}</button>
                </div>
                <div class="space-y-2">
                    <div class="font-medium text-foreground">{t(ui_locale.as_deref(), "ai.agents.rbacCatalog", "Tenant RBAC catalog")}</div>
                    <div class="text-xs text-muted-foreground">{format!("{} roles / {} permissions", tenant_rbac_roles.len(), tenant_rbac_permissions.len())}</div>
                    <For each=move || role_choices.clone() key=|role| role.slug.clone() let:role>
                        <div class="rounded border border-border p-3 text-xs text-muted-foreground">
                            <div class="font-medium text-foreground">{role.display_name}</div>
                            <div>{format!("{}: {}", role.slug, role.permission_slugs.join(", "))}</div>
                        </div>
                    </For>
                </div>
            </div>
        </Card>
    }
}

fn nonempty_value(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

#[component]
fn AiExecutionModeSelector(ui_locale: Option<String>, value: RwSignal<String>) -> impl IntoView {
    view! {
        <label class="grid gap-1 text-sm text-muted-foreground">
            <span>{t(ui_locale.as_deref(), "ai.field.executionMode", "Execution mode")}</span>
            <select class="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground" prop:value=move || value.get() on:change=move |event| value.set(event_target_value(&event))>
                <option value="auto">{t(ui_locale.as_deref(), "ai.common.auto", "auto")}</option>
                <option value="direct">{t(ui_locale.as_deref(), "ai.common.direct", "direct")}</option>
                <option value="mcp_tooling">{t(ui_locale.as_deref(), "ai.common.mcpTooling", "MCP tooling")}</option>
            </select>
        </label>
    }
}

#[component]
fn AiAgentRoleSelector(
    roles: Vec<AiTenantRbacRolePayload>,
    selected_role_slugs: RwSignal<Vec<String>>,
) -> impl IntoView {
    view! {
        <For each=move || roles.clone() key=|role| role.slug.clone() let:role>
            {
                let role_slug = role.slug.clone();
                let checked_slug = role.slug.clone();
                view! {
                    <label class="flex items-start gap-2 text-xs text-muted-foreground">
                        <input
                            type="checkbox"
                            prop:checked=move || selected_role_slugs.get().contains(&checked_slug)
                            on:change=move |event| {
                                let checked = event_target_checked(&event);
                                selected_role_slugs.update(|selected| {
                                    selected.retain(|slug| slug != &role_slug);
                                    if checked {
                                        selected.push(role_slug.clone());
                                        selected.sort();
                                    }
                                });
                            }
                        />
                        <span>{format!("{} ({})", role.display_name, role.permission_slugs.join(", "))}</span>
                    </label>
                }
            }
        </For>
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
