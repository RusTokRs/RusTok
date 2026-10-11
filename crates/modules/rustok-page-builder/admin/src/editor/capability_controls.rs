use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use crate::{PageBuilderAdminProviderState, PageBuilderAdminProviderStatus};
use fly_ui::{EditorCapability, EditorProviderState};
use leptos::prelude::*;
use rustok_grid::{
    ColumnAlign, ColumnFilters, FilterValue, GridColumnDef, GridFilterType, GridPagination,
    RowSelection,
};
use rustok_grid_leptos::DataGrid;
use rustok_ui_core::UiRouteContext;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityPolicyRowModel {
    pub capability: EditorCapability,
    pub capability_id: String,
    pub requested: Option<bool>,
    pub tenant: Option<bool>,
    pub permission: Option<bool>,
    pub effective: bool,
    pub effective_label: String,
}

pub fn capability_policy_grid_columns(
    capability_label: &str,
    requested_label: &str,
    tenant_label: &str,
    permission_label: &str,
    effective_label: &str,
    filter_placeholder: &str,
) -> Vec<GridColumnDef> {
    vec![
        GridColumnDef::new("capability", capability_label)
            .min_width(160)
            .align(ColumnAlign::Left)
            .filter(GridFilterType::Text {
                placeholder: Some(filter_placeholder.to_string()),
            }),
        GridColumnDef::new("requested", requested_label)
            .min_width(110)
            .align(ColumnAlign::Center)
            .filter(GridFilterType::Text {
                placeholder: Some(filter_placeholder.to_string()),
            }),
        GridColumnDef::new("tenant", tenant_label)
            .min_width(110)
            .align(ColumnAlign::Center)
            .filter(GridFilterType::Text {
                placeholder: Some(filter_placeholder.to_string()),
            }),
        GridColumnDef::new("permission", permission_label)
            .min_width(110)
            .align(ColumnAlign::Center)
            .filter(GridFilterType::Text {
                placeholder: Some(filter_placeholder.to_string()),
            }),
        GridColumnDef::new("effective", effective_label)
            .min_width(110)
            .align(ColumnAlign::Center)
            .filter(GridFilterType::Text {
                placeholder: Some(filter_placeholder.to_string()),
            }),
    ]
}

pub fn matches_capability_policy_filter(
    row: &CapabilityPolicyRowModel,
    filters: &ColumnFilters,
) -> bool {
    for (col_id, filter_val) in filters.iter() {
        match (col_id.as_str(), filter_val) {
            ("capability", FilterValue::Text(q)) => {
                if !row
                    .capability_id
                    .to_ascii_lowercase()
                    .contains(&q.to_ascii_lowercase())
                {
                    return false;
                }
            }
            ("requested", FilterValue::Text(q)) => {
                let term = q.trim().to_ascii_lowercase();
                let matches_yes = term == "yes" || term == "да" || term == "true";
                let matches_no = term == "no" || term == "нет" || term == "false";
                match row.requested {
                    Some(true) if !matches_yes => return false,
                    Some(false) if !matches_no => return false,
                    None if !term.is_empty() && term != "—" && term != "-" => return false,
                    _ => {}
                }
            }
            ("tenant", FilterValue::Text(q)) => {
                let term = q.trim().to_ascii_lowercase();
                let matches_yes = term == "yes" || term == "да" || term == "true";
                let matches_no = term == "no" || term == "нет" || term == "false";
                match row.tenant {
                    Some(true) if !matches_yes => return false,
                    Some(false) if !matches_no => return false,
                    None if !term.is_empty() && term != "—" && term != "-" => return false,
                    _ => {}
                }
            }
            ("permission", FilterValue::Text(q)) => {
                let term = q.trim().to_ascii_lowercase();
                let matches_yes = term == "yes" || term == "да" || term == "true";
                let matches_no = term == "no" || term == "нет" || term == "false";
                match row.permission {
                    Some(true) if !matches_yes => return false,
                    Some(false) if !matches_no => return false,
                    None if !term.is_empty() && term != "—" && term != "-" => return false,
                    _ => {}
                }
            }
            ("effective", FilterValue::Text(q))
                if !row
                    .effective_label
                    .to_ascii_lowercase()
                    .contains(&q.to_ascii_lowercase()) =>
            {
                return false;
            }
            _ => {}
        }
    }
    true
}

pub fn filter_capability_policy_rows(
    rows: &[CapabilityPolicyRowModel],
    filters: &ColumnFilters,
    search: Option<&str>,
) -> Vec<CapabilityPolicyRowModel> {
    rows.iter()
        .filter(|row| {
            if let Some(q) = search {
                let term = q.trim().to_ascii_lowercase();
                if !term.is_empty()
                    && !row.capability_id.to_ascii_lowercase().contains(&term)
                    && !row.effective_label.to_ascii_lowercase().contains(&term)
                {
                    return false;
                }
            }
            matches_capability_policy_filter(row, filters)
        })
        .cloned()
        .collect()
}

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
    let is_ru = locale
        .as_deref()
        .map(|l| l.starts_with("ru"))
        .unwrap_or(false);
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
    let empty_message = t(
        locale.as_deref(),
        "page_builder.capabilityPolicy.empty",
        "No capabilities found",
    );
    let filter_placeholder = if is_ru {
        "Фильтр..."
    } else {
        "Filter..."
    };

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

    let columns = capability_policy_grid_columns(
        &capability_label,
        &requested_label,
        &tenant_label,
        &permission_label,
        &effective_label,
        filter_placeholder,
    );

    let search = RwSignal::new(String::new());
    let filters = RwSignal::new(ColumnFilters::default());
    let selection = RwSignal::new(RowSelection::default());
    let pagination = RwSignal::new(GridPagination::new(
        1,
        10,
        EditorCapability::ALL.len() as u64,
    ));

    let rows = Memo::new({
        let runtime = runtime.clone();
        let evaluation = evaluation.clone();
        let enabled_label = enabled_label.clone();
        let disabled_label = disabled_label.clone();
        move |_| {
            EditorCapability::ALL
                .into_iter()
                .map(|capability| {
                    let requested = evaluation
                        .as_ref()
                        .map(|evaluation| evaluation.requested_allows(capability));
                    let tenant = evaluation
                        .as_ref()
                        .map(|evaluation| evaluation.tenant_allows(capability));
                    let permission = evaluation
                        .as_ref()
                        .map(|evaluation| evaluation.permission_allows(capability));
                    let effective = runtime.capability_enabled(capability);
                    let effective_label = if effective {
                        enabled_label.clone()
                    } else {
                        disabled_label.clone()
                    };
                    CapabilityPolicyRowModel {
                        capability,
                        capability_id: capability.as_str().to_string(),
                        requested,
                        tenant,
                        permission,
                        effective,
                        effective_label,
                    }
                })
                .collect::<Vec<_>>()
        }
    });

    let filtered_rows = Memo::new(move |_| {
        let all = rows.get();
        let q = search.get();
        let f = filters.get();
        filter_capability_policy_rows(&all, &f, if q.trim().is_empty() { None } else { Some(&q) })
    });

    Effect::new(move |_| {
        let count = filtered_rows.get().len() as u64;
        pagination.update(|p| p.set_total(count));
    });

    let paged_rows = Memo::new(move |_| {
        let list = filtered_rows.get();
        let p = pagination.get();
        let start = (p.page.saturating_sub(1)) * p.page_size;
        list.into_iter()
            .skip(start)
            .take(p.page_size)
            .collect::<Vec<_>>()
    });

    let on_filters_change = Callback::new(move |new_filters: ColumnFilters| {
        filters.set(new_filters);
    });

    let cell_renderer = {
        let yes_label = yes_label.clone();
        let no_label = no_label.clone();
        Callback::new(move |(row, col_id): (CapabilityPolicyRowModel, String)| {
            match col_id.as_str() {
                "capability" => view! {
                    <span data-fly-capability-row=row.capability.as_str() class="font-medium text-xs">
                        <code>{row.capability.as_str()}</code>
                    </span>
                }
                .into_any(),
                "requested" => view! {
                    <CapabilitySourceCell
                        value=row.requested
                        yes_label=yes_label.clone()
                        no_label=no_label.clone()
                    />
                }
                .into_any(),
                "tenant" => view! {
                    <CapabilitySourceCell
                        value=row.tenant
                        yes_label=yes_label.clone()
                        no_label=no_label.clone()
                    />
                }
                .into_any(),
                "permission" => view! {
                    <CapabilitySourceCell
                        value=row.permission
                        yes_label=yes_label.clone()
                        no_label=no_label.clone()
                    />
                }
                .into_any(),
                "effective" => {
                    let class = if row.effective {
                        "px-1 py-1 text-xs text-emerald-700 font-medium"
                    } else {
                        "px-1 py-1 text-xs text-destructive font-medium"
                    };
                    view! { <span class=class>{row.effective_label}</span> }.into_any()
                }
                _ => ().into_any(),
            }
        })
    };

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
            <div class="space-y-3">
                <div class="flex flex-col sm:flex-row gap-3 items-center justify-between">
                    <input
                        type="text"
                        placeholder=if is_ru { "Поиск по возможностям..." } else { "Search capabilities..." }
                        prop:value=move || search.get()
                        on:input=move |ev| search.set(event_target_value(&ev))
                        class="text-xs rounded-lg border border-border bg-background px-2.5 py-1 text-foreground outline-none focus:border-primary w-64 font-normal"
                    />
                    <div class="text-xs text-muted-foreground">
                        {move || format!("{} {}", filtered_rows.get().len(), if is_ru { "возможностей" } else { "capabilities" })}
                    </div>
                </div>

                <DataGrid
                    columns=columns
                    data=Signal::derive(move || paged_rows.get())
                    key_fn=|row: &CapabilityPolicyRowModel| row.capability_id.clone()
                    cell_renderer=cell_renderer
                    empty_message=empty_message
                    selection=selection
                    pagination=pagination
                    filters=filters
                    on_filter_change=on_filters_change
                    on_row_click=Callback::new(|_| ())
                />
            </div>
        </section>
    }
}

#[component]
fn CapabilitySourceCell(value: Option<bool>, yes_label: String, no_label: String) -> impl IntoView {
    let (label, class) = match value {
        Some(true) => (yes_label, "px-1 py-1 text-xs text-emerald-700 font-medium"),
        Some(false) => (no_label, "px-1 py-1 text-xs text-destructive font-medium"),
        None => ("—".to_string(), "px-1 py-1 text-xs text-muted-foreground"),
    };
    view! { <span class=class>{label}</span> }
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

    #[test]
    fn capability_policy_grid_columns_initialization() {
        let cols = capability_policy_grid_columns(
            "Capability",
            "Requested",
            "Tenant",
            "Permission",
            "Effective",
            "Filter...",
        );
        assert_eq!(cols.len(), 5);
        assert_eq!(cols[0].id.as_str(), "capability");
        assert_eq!(cols[1].id.as_str(), "requested");
        assert_eq!(cols[2].id.as_str(), "tenant");
        assert_eq!(cols[3].id.as_str(), "permission");
        assert_eq!(cols[4].id.as_str(), "effective");
    }

    #[test]
    fn capability_policy_filtering_by_search_and_column() {
        let rows = vec![
            CapabilityPolicyRowModel {
                capability: EditorCapability::Edit,
                capability_id: "edit".to_string(),
                requested: Some(true),
                tenant: Some(true),
                permission: Some(true),
                effective: true,
                effective_label: "enabled".to_string(),
            },
            CapabilityPolicyRowModel {
                capability: EditorCapability::Publish,
                capability_id: "publish".to_string(),
                requested: Some(false),
                tenant: Some(true),
                permission: Some(false),
                effective: false,
                effective_label: "disabled".to_string(),
            },
        ];

        let mut filters = ColumnFilters::default();
        let result = filter_capability_policy_rows(&rows, &filters, Some("edit"));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].capability_id, "edit");

        filters.set("effective", FilterValue::Text("disabled".to_string()));
        let result2 = filter_capability_policy_rows(&rows, &filters, None);
        assert_eq!(result2.len(), 1);
        assert_eq!(result2[0].capability_id, "publish");
    }
}
