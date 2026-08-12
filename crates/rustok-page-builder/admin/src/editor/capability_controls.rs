use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use crate::{PageBuilderAdminProviderState, PageBuilderAdminProviderStatus};
use fly_ui::{EditorCapability, EditorProviderState};
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

#[component]
pub(crate) fn CapabilityFieldset(
    runtime: AdminEditorRuntime,
    capability: EditorCapability,
    children: Children,
) -> impl IntoView {
    let disabled_runtime = runtime.clone();
    let enabled_runtime = runtime;
    let capability_id = capability.as_str();
    view! {
        <fieldset
            class="contents disabled:opacity-60"
            disabled=move || !disabled_runtime.capability_enabled(capability)
            aria-disabled=move || (!enabled_runtime.capability_enabled(capability)).to_string()
            data-fly-capability=capability_id
        >
            {children()}
        </fieldset>
    }
}

#[component]
pub(crate) fn CapabilityPolicyPanel(
    runtime: AdminEditorRuntime,
    provider_status: Option<PageBuilderAdminProviderStatus>,
) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let title = t(
        locale.as_deref(),
        "page_builder.capabilityPolicy.title",
        "Editor access policy",
    );
    let policy_note = t(
        locale.as_deref(),
        "page_builder.capabilityPolicy.summary",
        "No detailed host policy was supplied. The effective profile still remains enforced by the state machine.",
    );
    let provider_control_label = t(
        locale.as_deref(),
        "page_builder.capabilityPolicy.providerControl",
        "Provider control state",
    );
    let observed_health_label = t(
        locale.as_deref(),
        "page_builder.capabilityPolicy.observedHealth",
        "Observed health",
    );
    let host_policy_label = t(
        locale.as_deref(),
        "page_builder.capabilityPolicy.hostProviderPolicy",
        "Host provider policy",
    );
    let rollout_label = t(
        locale.as_deref(),
        "page_builder.capabilityPolicy.rollout",
        "Rollout flags",
    );
    let degradation_label = t(
        locale.as_deref(),
        "page_builder.capabilityPolicy.degradationReasons",
        "Degradation reasons",
    );
    let capability_label = t(
        locale.as_deref(),
        "page_builder.capabilityPolicy.capability",
        "Capability",
    );
    let requested_label = t(
        locale.as_deref(),
        "page_builder.capabilityPolicy.requested",
        "Requested",
    );
    let tenant_label = t(
        locale.as_deref(),
        "page_builder.capabilityPolicy.tenant",
        "Tenant",
    );
    let permission_label = t(
        locale.as_deref(),
        "page_builder.capabilityPolicy.permission",
        "Permission",
    );
    let effective_label = t(
        locale.as_deref(),
        "page_builder.capabilityPolicy.effective",
        "Effective",
    );
    let enabled_label = t(
        locale.as_deref(),
        "page_builder.capabilityPolicy.enabled",
        "enabled",
    );
    let disabled_label = t(
        locale.as_deref(),
        "page_builder.capabilityPolicy.disabled",
        "disabled",
    );
    let yes_label = t(
        locale.as_deref(),
        "page_builder.capabilityPolicy.yes",
        "yes",
    );
    let no_label = t(locale.as_deref(), "page_builder.capabilityPolicy.no", "no");
    let unobserved_label = t(
        locale.as_deref(),
        "page_builder.capabilityPolicy.unobserved",
        "unobserved",
    );
    let none_label = t(
        locale.as_deref(),
        "page_builder.capabilityPolicy.none",
        "none",
    );

    let evaluation = runtime.editor_capability_evaluation.clone();
    let host_provider = evaluation
        .as_ref()
        .map(|evaluation| evaluation.provider_state);
    let provider_control_state = provider_status
        .as_ref()
        .map(PageBuilderAdminProviderStatus::state)
        .unwrap_or(PageBuilderAdminProviderState::Unobserved);
    let provider_class = match provider_control_state {
        PageBuilderAdminProviderState::Ready => {
            "rounded bg-emerald-100 px-2 py-1 text-xs text-emerald-900"
        }
        PageBuilderAdminProviderState::Degraded => {
            "rounded bg-amber-100 px-2 py-1 text-xs text-amber-900"
        }
        PageBuilderAdminProviderState::Unavailable => {
            "rounded bg-destructive/10 px-2 py-1 text-xs text-destructive"
        }
        PageBuilderAdminProviderState::Unobserved => {
            "rounded bg-muted px-2 py-1 text-xs text-muted-foreground"
        }
    };
    let observed_health_state = provider_status
        .as_ref()
        .and_then(|status| status.health.as_ref())
        .map(|health| health.state.as_str())
        .unwrap_or("unobserved");
    let observed_health_display = if observed_health_state == "unobserved" {
        unobserved_label.clone()
    } else {
        observed_health_state.to_string()
    };
    let host_provider_display = host_provider
        .map(EditorProviderState::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| unobserved_label.clone());
    let rollout = provider_status.as_ref().map(|status| {
        format!(
            "builder={} preview={} properties={} publish={}",
            status.flags.builder_enabled,
            status.flags.preview_enabled,
            status.flags.properties_enabled,
            status.flags.publish_enabled,
        )
    });
    let degradation_reasons = provider_status
        .as_ref()
        .and_then(|status| status.health.as_ref())
        .map(|health| {
            health
                .degradation_reasons
                .iter()
                .map(|reason| reason.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|reasons| !reasons.is_empty())
        .unwrap_or_else(|| none_label.clone());

    view! {
        <section
            class="space-y-3 rounded-xl border border-border bg-card p-3"
            data-fly-capability-policy="true"
            data-fly-provider-control-state=provider_control_state.as_str()
            data-fly-provider-health=observed_health_state
        >
            <div class="flex items-center justify-between gap-2">
                <h2 class="font-semibold">{title}</h2>
                <span class=provider_class>{format!("{provider_control_label}: {}", provider_control_state.as_str())}</span>
            </div>
            <div class="grid gap-1 rounded bg-muted/40 px-2 py-2 text-xs text-muted-foreground">
                <p>{format!("{observed_health_label}: {observed_health_display}")}</p>
                <p>{format!("{host_policy_label}: {host_provider_display}")}</p>
                <p>{format!("{rollout_label}: {}", rollout.unwrap_or_else(|| unobserved_label.clone()))}</p>
                <p>{format!("{degradation_label}: {degradation_reasons}")}</p>
            </div>
            {evaluation.is_none().then(|| view! {
                <p class="rounded bg-muted/50 px-2 py-1 text-xs text-muted-foreground" role="status">
                    {policy_note}
                </p>
            })}
            <div class="overflow-x-auto">
                <table class="w-full text-left text-xs">
                    <thead>
                        <tr class="border-b border-border text-muted-foreground">
                            <th class="px-1 py-1">{capability_label}</th>
                            <th class="px-1 py-1">{requested_label}</th>
                            <th class="px-1 py-1">{tenant_label}</th>
                            <th class="px-1 py-1">{permission_label}</th>
                            <th class="px-1 py-1">{effective_label}</th>
                        </tr>
                    </thead>
                    <tbody>
                        {EditorCapability::ALL.into_iter().map(|capability| {
                            let row_runtime = runtime.clone();
                            let evaluation = evaluation.clone();
                            let enabled_label = enabled_label.clone();
                            let disabled_label = disabled_label.clone();
                            let yes_label = yes_label.clone();
                            let no_label = no_label.clone();
                            let requested = evaluation
                                .as_ref()
                                .map(|evaluation| evaluation.requested_allows(capability));
                            let tenant = evaluation
                                .as_ref()
                                .map(|evaluation| evaluation.tenant_allows(capability));
                            let permission = evaluation
                                .as_ref()
                                .map(|evaluation| evaluation.permission_allows(capability));
                            view! {
                                <tr class="border-b border-border/60" data-fly-capability-row=capability.as_str()>
                                    <th class="px-1 py-1 font-medium"><code>{capability.as_str()}</code></th>
                                    <CapabilitySourceCell value=requested yes_label=yes_label.clone() no_label=no_label.clone() />
                                    <CapabilitySourceCell value=tenant yes_label=yes_label.clone() no_label=no_label.clone() />
                                    <CapabilitySourceCell value=permission yes_label=yes_label no_label=no_label />
                                    <td class="px-1 py-1">
                                        {move || if row_runtime.capability_enabled(capability) {
                                            enabled_label.clone()
                                        } else {
                                            disabled_label.clone()
                                        }}
                                    </td>
                                </tr>
                            }
                        }).collect_view()}
                    </tbody>
                </table>
            </div>
        </section>
    }
}

#[component]
fn CapabilitySourceCell(value: Option<bool>, yes_label: String, no_label: String) -> impl IntoView {
    let (label, class) = match value {
        Some(true) => (yes_label, "px-1 py-1 text-emerald-700"),
        Some(false) => (no_label, "px-1 py-1 text-destructive"),
        None => ("—".to_string(), "px-1 py-1 text-muted-foreground"),
    };
    view! { <td class=class>{label}</td> }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_identifiers_are_safe_data_attributes() {
        for capability in EditorCapability::ALL {
            assert!(
                capability
                    .as_str()
                    .chars()
                    .all(|character| character.is_ascii_lowercase() || character == '_')
            );
        }
    }
}
