use leptos::task::spawn_local;
use leptos::{html, prelude::*};
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui::{
    Alert, AlertVariant, Badge, BadgeVariant, Button, ButtonVariant, Card, CardContent,
    CardDescription, CardHeader, CardTitle, Checkbox, Input, Label, Select, SelectOption, Textarea,
};
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_ui_core::UiRouteContext;

use crate::core::{self, ProposalCommand, TranslationAdminTab, operation_receipt_view_model};
use crate::i18n::t;
use crate::model::{
    ActorKind, Glossary, GlossarySummary, InterchangeArtifact, MemoryEntry, MemorySuggestion,
    ReviewerQueueItem, ReviewerWorkload, TranslationAdminOperation, TranslationAdminResponse,
    TranslationAdminTransportContext, TranslationPolicy, TranslationTarget, WorkflowNote,
};
use crate::transport;

type OperationOutcome = Option<Result<TranslationAdminResponse, String>>;

#[component]
pub fn TranslationAdmin() -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let tab_query = use_route_query_value(core::TAB_QUERY_KEY);
    let glossary_query = use_route_query_value(core::GLOSSARY_ID_QUERY_KEY);
    let memory_entry_query = use_route_query_value(core::MEMORY_ENTRY_ID_QUERY_KEY);
    let query_writer = use_route_query_writer();
    let active_tab = Signal::derive(move || core::tab_from_query(tab_query.get().as_deref()));

    let title = t(
        locale.as_deref(),
        "translation.title",
        "Translation control plane",
    );
    let subtitle = t(
        locale.as_deref(),
        "translation.subtitle",
        "Exact-locale coverage, required-target policy, inventory repair, and reviewed owner-safe workflow.",
    );
    let badge = t(locale.as_deref(), "translation.badge", "admin only");
    let tabs_aria_label = t(
        locale.as_deref(),
        "translation.tabs.label",
        "Translation sections",
    );
    let locale_for_tabs = locale.clone();
    let locale_for_content = locale.clone();
    let tab_refs = TranslationAdminTab::ALL.map(|_| NodeRef::<html::Button>::new());

    view! {
        <div class="space-y-6" data-testid="translation-admin">
            <header class="flex flex-col gap-4 rounded-2xl border border-border bg-card p-6 shadow-sm lg:flex-row lg:items-start lg:justify-between">
                <div class="space-y-2">
                    <Badge variant=BadgeVariant::Outline>{badge}</Badge>
                    <h1 class="text-2xl font-semibold text-card-foreground">{title}</h1>
                    <p class="max-w-3xl text-sm text-muted-foreground">{subtitle}</p>
                </div>
            </header>

            <nav
                aria-label=tabs_aria_label
                role="tablist"
                class="flex flex-wrap gap-2"
            >
                {move || {
                    let selected = active_tab.get();
                    let locale = locale_for_tabs.clone();
                    TranslationAdminTab::ALL
                        .into_iter()
                        .map(|tab| {
                            let writer = query_writer.clone();
                            let keyboard_writer = query_writer.clone();
                            let label = tab_label(locale.as_deref(), tab);
                            let is_selected = selected == tab;
                            let tab_id = format!("translation-tab-{}", tab.query_value());
                            let panel_id = format!("translation-panel-{}", tab.query_value());
                            let tab_ref = tab_refs[tab.index()];
                            let focus_refs = tab_refs;
                            view! {
                                <button
                                    type="button"
                                    id=tab_id
                                    node_ref=tab_ref
                                    role="tab"
                                    aria-selected=if is_selected { "true" } else { "false" }
                                    aria-controls=panel_id
                                    tabindex=if is_selected { 0 } else { -1 }
                                    class=translation_tab_class(is_selected)
                                    on:click=move |_| {
                                        writer.apply_query_intent(core::tab_query_intent(tab));
                                    }
                                    on:keydown=move |event| {
                                        let target = match event.key().as_str() {
                                            "ArrowLeft" | "ArrowUp" => Some(selected.previous()),
                                            "ArrowRight" | "ArrowDown" => Some(selected.next()),
                                            "Home" => Some(TranslationAdminTab::Overview),
                                            "End" => Some(TranslationAdminTab::Workflow),
                                            _ => None,
                                        };
                                        if let Some(target) = target {
                                            event.prevent_default();
                                            keyboard_writer.apply_query_intent(
                                                core::tab_query_intent(target),
                                            );
                                            if let Some(button) = focus_refs[target.index()].get() {
                                                let _ = button.focus();
                                            }
                                        }
                                    }
                                >
                                    {label}
                                </button>
                            }
                        })
                        .collect_view()
                }}
            </nav>

            {move || {
                let selected = active_tab.get();
                let panel_id = format!("translation-panel-{}", selected.query_value());
                let tab_id = format!("translation-tab-{}", selected.query_value());
                view! {
                    <section
                        id=panel_id
                        role="tabpanel"
                        aria-labelledby=tab_id
                        tabindex=0
                    >
                        {match selected {
                            TranslationAdminTab::Overview => view! {
                                <OverviewTab token tenant locale=locale_for_content.clone() />
                            }.into_any(),
                            TranslationAdminTab::Jobs => view! {
                                <JobsTab token tenant locale=locale_for_content.clone() />
                            }.into_any(),
                            TranslationAdminTab::Glossaries => view! {
                                <GlossariesTab
                                    token
                                    tenant
                                    locale=locale_for_content.clone()
                                    selected_glossary_id=glossary_query
                                />
                            }.into_any(),
                            TranslationAdminTab::Memory => view! {
                                <MemoryTab
                                    token
                                    tenant
                                    locale=locale_for_content.clone()
                                    selected_memory_entry_id=memory_entry_query
                                />
                            }.into_any(),
                            TranslationAdminTab::Inventory => view! {
                                <InventoryTab token tenant locale=locale_for_content.clone() />
                            }.into_any(),
                            TranslationAdminTab::Workflow => view! {
                                <WorkflowTab token tenant locale=locale_for_content.clone() />
                            }.into_any(),
                        }}
                    </section>
                }
            }}
        </div>
    }
}

fn translation_tab_class(is_selected: bool) -> &'static str {
    if is_selected {
        "inline-flex h-9 items-center justify-center gap-2 whitespace-nowrap rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground shadow ring-offset-background transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
    } else {
        "inline-flex h-9 items-center justify-center gap-2 whitespace-nowrap rounded-md border border-input bg-background px-4 py-2 text-sm font-medium shadow-sm ring-offset-background transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
    }
}

fn tab_label(locale: Option<&str>, tab: TranslationAdminTab) -> String {
    match tab {
        TranslationAdminTab::Overview => t(locale, "translation.tabs.overview", "Overview"),
        TranslationAdminTab::Jobs => t(locale, "translation.tabs.jobs", "Jobs"),
        TranslationAdminTab::Glossaries => t(locale, "translation.tabs.glossaries", "Glossaries"),
        TranslationAdminTab::Memory => t(locale, "translation.tabs.memory", "Memory"),
        TranslationAdminTab::Inventory => t(locale, "translation.tabs.inventory", "Inventory"),
        TranslationAdminTab::Workflow => t(locale, "translation.tabs.workflow", "Workflow"),
    }
}

#[component]
fn OverviewTab(
    token: Signal<Option<String>>,
    tenant: Signal<Option<String>>,
    locale: Option<String>,
) -> impl IntoView {
    let locale_for_resource = locale.clone();
    let bootstrap = LocalResource::new(move || {
        let context =
            core::transport_context(token.get(), tenant.get(), locale_for_resource.clone());
        async move {
            let policy = transport::execute(context.clone(), TranslationAdminOperation::ReadPolicy)
                .await
                .map_err(|error| error.to_string())?;
            let targets = transport::execute(context, TranslationAdminOperation::ListTargets)
                .await
                .map_err(|error| error.to_string())?;

            match (policy, targets) {
                (
                    TranslationAdminResponse::Policy(policy),
                    TranslationAdminResponse::Targets(targets),
                ) => Ok((policy, targets)),
                _ => Err("Translation bootstrap returned an unexpected response".to_string()),
            }
        }
    });

    let loading = t(
        locale.as_deref(),
        "translation.loading",
        "Loading Translation state…",
    );

    view! {
        <Suspense fallback=move || view! {
            <Card>
                <CardContent>
                    <p class="text-sm text-muted-foreground">{loading.clone()}</p>
                </CardContent>
            </Card>
        }>
            {move || {
                let locale = locale.clone();
                bootstrap.get().map(|result| match result {
                    Ok((policy, targets)) => view! {
                        <div class="grid gap-6 xl:grid-cols-[minmax(0,0.9fr)_minmax(0,1.4fr)]">
                            <PolicyCard policy token tenant locale=locale.clone() />
                            <TargetsCard targets locale=locale.clone() />
                        </div>
                    }.into_any(),
                    Err(error) => view! {
                        <Alert
                            variant=AlertVariant::Destructive
                            title=t(locale.as_deref(), "translation.error.load", "Unable to load Translation")
                        >
                            {error}
                        </Alert>
                    }.into_any(),
                })
            }}
        </Suspense>
    }
}

#[component]
fn PolicyCard(
    policy: TranslationPolicy,
    token: Signal<Option<String>>,
    tenant: Signal<Option<String>>,
    locale: Option<String>,
) -> impl IntoView {
    let freshness_variant = if policy.freshness == "current" {
        BadgeVariant::Success
    } else {
        BadgeVariant::Warning
    };
    let policy_freshness = policy.freshness.clone();
    let policy_revision = policy.revision;
    let policy_locales = policy.required_target_locales.clone();
    let disabled_locales = policy.disabled_required_target_locales.clone();
    let (expected_revision, set_expected_revision) = signal(policy_revision.to_string());
    let (required_locales, set_required_locales) = signal(policy_locales.join(", "));
    let (idempotency_key, set_idempotency_key) =
        signal(core::new_idempotency_key("replace-policy"));
    let (busy, set_busy) = signal(false);
    let (outcome, set_outcome) = signal(OperationOutcome::None);
    let title = t(
        locale.as_deref(),
        "translation.policy.title",
        "Required-target policy",
    );
    let description = t(
        locale.as_deref(),
        "translation.policy.description",
        "Locales that contribute to required Translation coverage.",
    );
    let revision_label = t(
        locale.as_deref(),
        "translation.field.revision",
        "Expected revision",
    );
    let locales_label = t(
        locale.as_deref(),
        "translation.field.locales",
        "Required locales",
    );
    let replace_label = t(
        locale.as_deref(),
        "translation.action.replacePolicy",
        "Replace policy",
    );
    let policy_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::replace_policy_operation(
                    &expected_revision.get_untracked(),
                    &required_locales.get_untracked(),
                    &idempotency_key.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(move |response| {
                    if let TranslationAdminResponse::Policy(policy) = response {
                        set_expected_revision.set(policy.revision.to_string());
                        set_required_locales.set(policy.required_target_locales.join(", "));
                    }
                    set_idempotency_key.set(core::new_idempotency_key("replace-policy"));
                }),
            );
        }
    };

    view! {
        <Card>
            <CardHeader>
                <CardTitle>
                    {title}
                </CardTitle>
                <CardDescription>
                    {description}
                </CardDescription>
            </CardHeader>
            <CardContent class="space-y-4">
                <div class="flex flex-wrap items-center gap-2">
                    <Badge variant=freshness_variant>{policy_freshness}</Badge>
                    <span class="text-xs text-muted-foreground">
                        {format!("revision {}", policy_revision)}
                    </span>
                </div>
                <div class="flex flex-wrap gap-2">
                    {policy_locales.into_iter().map(|locale| view! {
                        <Badge variant=BadgeVariant::Secondary>{locale}</Badge>
                    }).collect_view()}
                </div>
                {(!disabled_locales.is_empty()).then(|| view! {
                    <Alert variant=AlertVariant::Warning>
                        {format!(
                            "Disabled required locales: {}",
                            disabled_locales.join(", ")
                        )}
                    </Alert>
                })}
                <div class="grid gap-3">
                    <div class="space-y-2">
                        <Label required=true r#for="expected_revision">{revision_label}</Label>
                        <Input value=expected_revision set_value=set_expected_revision id="expected_revision" name="expected_revision" />
                    </div>
                    <div class="space-y-2">
                        <Label required=true r#for="required_target_locales">{locales_label}</Label>
                        <Input value=required_locales set_value=set_required_locales id="required_target_locales" name="required_target_locales" />
                    </div>
                    <Button on_click=Box::new(policy_action)>{replace_label}</Button>
                    <Show when=move || busy.get()>
                        <p class="text-xs text-muted-foreground">"Operation in progress…"</p>
                    </Show>
                    <OutcomePanel outcome locale=locale.clone() />
                </div>
            </CardContent>
        </Card>
    }
}

#[component]
fn TargetsCard(targets: Vec<TranslationTarget>, locale: Option<String>) -> impl IntoView {
    let title = t(
        locale.as_deref(),
        "translation.targets.title",
        "Translation targets",
    );
    let description = t(
        locale.as_deref(),
        "translation.targets.description",
        "Owner-provided resources currently available to the control plane.",
    );
    let empty = t(
        locale.as_deref(),
        "translation.targets.empty",
        "No owner target providers are registered.",
    );
    let provider_label = t(
        locale.as_deref(),
        "translation.targets.provider",
        "Provider",
    );
    let target_label = t(locale.as_deref(), "translation.targets.target", "Target");
    let capabilities_label = t(
        locale.as_deref(),
        "translation.targets.capabilities",
        "Capabilities",
    );

    view! {
        <Card>
            <CardHeader>
                <CardTitle>
                    {title}
                </CardTitle>
                <CardDescription>
                    {description}
                </CardDescription>
            </CardHeader>
            <CardContent>
                {if targets.is_empty() {
                    view! {
                        <p class="rounded-xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
                            {empty}
                        </p>
                    }.into_any()
                } else {
                    view! {
                        <div class="overflow-x-auto rounded-xl border border-border">
                            <table class="w-full text-sm">
                                <thead class="bg-muted/50 text-left text-xs uppercase tracking-wide text-muted-foreground">
                                    <tr>
                                        <th class="px-4 py-3">{provider_label}</th>
                                        <th class="px-4 py-3">{target_label}</th>
                                        <th class="px-4 py-3">{capabilities_label}</th>
                                    </tr>
                                </thead>
                                <tbody class="divide-y divide-border">
                                    {targets.into_iter().map(|target| view! {
                                        <tr>
                                            <td class="px-4 py-3 font-medium text-foreground">{target.owner_slug}</td>
                                            <td class="px-4 py-3">
                                                <div class="font-medium text-foreground">{target.display_name}</div>
                                                <div class="text-xs text-muted-foreground">{target.resource_kind}</div>
                                            </td>
                                            <td class="px-4 py-3">
                                                <div class="flex flex-wrap gap-1">
                                                    {target.capabilities.into_iter().map(|capability| view! {
                                                        <Badge variant=BadgeVariant::Outline>{capability}</Badge>
                                                    }).collect_view()}
                                                </div>
                                            </td>
                                        </tr>
                                    }).collect_view()}
                                </tbody>
                            </table>
                        </div>
                    }.into_any()
                }}
            </CardContent>
        </Card>
    }
}

#[component]
fn JobsTab(
    token: Signal<Option<String>>,
    tenant: Signal<Option<String>>,
    locale: Option<String>,
) -> impl IntoView {
    let (source_locale, set_source_locale) = signal("en".to_string());
    let (target_locale, set_target_locale) = signal("de".to_string());
    let (glossary_id, set_glossary_id) = signal(String::new());
    let (glossary_revision, set_glossary_revision) = signal(String::new());
    let (job_id, set_job_id) = signal(String::new());
    let (reviewer_assignee_kind, set_reviewer_assignee_kind) = signal("user".to_string());
    let (reviewer_assignee_id, set_reviewer_assignee_id) = signal(String::new());
    let (reviewer_include_unassigned, set_reviewer_include_unassigned) = signal(true);
    let (reviewer_queue_limit, set_reviewer_queue_limit) = signal("50".to_string());
    let (reviewer_queue, set_reviewer_queue) = signal(Vec::<ReviewerQueueItem>::new());
    let (reviewer_workloads, set_reviewer_workloads) = signal(Vec::<ReviewerWorkload>::new());
    let (max_export_items, set_max_export_items) = signal("200".to_string());
    let (export_document, set_export_document) = signal(String::new());
    let (import_document, set_import_document) = signal(String::new());
    let (interchange_artifacts, set_interchange_artifacts) =
        signal(Vec::<InterchangeArtifact>::new());
    let (interchange_artifact_id, set_interchange_artifact_id) = signal(String::new());
    let (interchange_artifact_document, set_interchange_artifact_document) = signal(String::new());
    let (interchange_artifact_expiry, set_interchange_artifact_expiry) =
        signal("86400".to_string());
    let (interchange_artifact_include_expired, set_interchange_artifact_include_expired) =
        signal(false);
    let (busy, set_busy) = signal(false);
    let (outcome, set_outcome) = signal(OperationOutcome::None);
    let (create_key, set_create_key) = signal(core::new_idempotency_key("create-job"));
    let (rebuild_key, set_rebuild_key) = signal(core::new_idempotency_key("rebuild-job-progress"));
    let (import_key, set_import_key) = signal(core::new_idempotency_key("import-item"));
    let (create_interchange_artifact_key, set_create_interchange_artifact_key) = signal(
        core::new_idempotency_key("create-interchange-export-artifact"),
    );
    let (store_interchange_artifact_key, set_store_interchange_artifact_key) = signal(
        core::new_idempotency_key("store-interchange-import-artifact"),
    );
    let (process_interchange_artifact_key, set_process_interchange_artifact_key) = signal(
        core::new_idempotency_key("process-interchange-import-artifact"),
    );

    let create_title = t(
        locale.as_deref(),
        "translation.jobs.create",
        "Create translation job",
    );
    let create_description = t(
        locale.as_deref(),
        "translation.jobs.createDescription",
        "Start a tenant-scoped manual workflow for one exact locale pair.",
    );
    let inspect_title = t(
        locale.as_deref(),
        "translation.jobs.inspect",
        "Inspect or rebuild job progress",
    );
    let inspect_description = t(
        locale.as_deref(),
        "translation.jobs.inspectDescription",
        "Job selection remains explicit until a list contract is available.",
    );
    let source_label = t(
        locale.as_deref(),
        "translation.field.sourceLocale",
        "Source locale",
    );
    let target_label = t(
        locale.as_deref(),
        "translation.field.targetLocale",
        "Target locale",
    );
    let job_id_label = t(locale.as_deref(), "translation.field.jobId", "Job ID");
    let glossary_id_label = t(
        locale.as_deref(),
        "translation.field.glossaryId",
        "Glossary ID",
    );
    let glossary_revision_label = t(
        locale.as_deref(),
        "translation.field.glossaryRevision",
        "Glossary revision",
    );
    let create_label = t(
        locale.as_deref(),
        "translation.action.createJob",
        "Create job",
    );
    let read_label = t(
        locale.as_deref(),
        "translation.action.readProgress",
        "Read progress",
    );
    let rebuild_label = t(
        locale.as_deref(),
        "translation.action.rebuildProgress",
        "Rebuild projection",
    );
    let reviewer_title = t(
        locale.as_deref(),
        "translation.jobs.reviewers",
        "Reviewer queue and workload",
    );
    let reviewer_description = t(
        locale.as_deref(),
        "translation.jobs.reviewersDescription",
        "Review work is derived from submitted, unapproved proposals and current assignments.",
    );
    let reviewer_queue_label = t(
        locale.as_deref(),
        "translation.action.readReviewerQueue",
        "Load reviewer queue",
    );
    let reviewer_workload_label = t(
        locale.as_deref(),
        "translation.action.readReviewerWorkload",
        "Load workload",
    );
    let reviewer_workload_button_label = reviewer_workload_label.clone();
    let reviewer_kind_label = t(
        locale.as_deref(),
        "translation.field.assigneeKind",
        "Assignee kind",
    );
    let reviewer_id_label = t(
        locale.as_deref(),
        "translation.field.assigneeId",
        "Assignee ID",
    );
    let reviewer_user_label = t(locale.as_deref(), "translation.field.assigneeUser", "User");
    let reviewer_service_label = t(
        locale.as_deref(),
        "translation.field.assigneeService",
        "Service",
    );
    let reviewer_limit_label = t(
        locale.as_deref(),
        "translation.field.reviewerQueueLimit",
        "Queue limit",
    );
    let include_unassigned_label = t(
        locale.as_deref(),
        "translation.field.includeUnassigned",
        "Include unassigned",
    );
    let reviewer_label = t(locale.as_deref(), "translation.field.reviewer", "Reviewer");
    let queue_item_label = t(
        locale.as_deref(),
        "translation.field.queueItems",
        "Queue items",
    );
    let item_label = t(locale.as_deref(), "translation.field.itemId", "Item ID");
    let status_label = t(locale.as_deref(), "translation.field.status", "Status");
    let submitted_at_label = t(
        locale.as_deref(),
        "translation.field.submittedAt",
        "Submitted at",
    );
    let open_items_label = t(
        locale.as_deref(),
        "translation.field.openItems",
        "Open items",
    );
    let in_review_items_label = t(
        locale.as_deref(),
        "translation.field.inReviewItems",
        "In review",
    );
    let approved_items_label = t(
        locale.as_deref(),
        "translation.field.approvedItems",
        "Approved",
    );
    let rebase_required_label = t(
        locale.as_deref(),
        "translation.field.rebaseRequiredItems",
        "Rebase required",
    );
    let blocked_items_label = t(
        locale.as_deref(),
        "translation.field.blockedItems",
        "Blocked items",
    );
    let source_characters_label = t(
        locale.as_deref(),
        "translation.field.sourceCharacters",
        "Source characters",
    );
    let unassigned_label = t(
        locale.as_deref(),
        "translation.field.unassigned",
        "Unassigned",
    );
    let reviewer_empty_label = t(
        locale.as_deref(),
        "translation.jobs.reviewersEmpty",
        "No reviewer data has been loaded.",
    );
    let workload_reviewer_label = reviewer_label.clone();
    let queue_unassigned_label = unassigned_label.clone();
    let workload_unassigned_label = unassigned_label.clone();
    let workload_empty_label = reviewer_empty_label.clone();
    let interchange_title = t(
        locale.as_deref(),
        "translation.jobs.interchange",
        "Bounded interchange",
    );
    let interchange_description = t(
        locale.as_deref(),
        "translation.jobs.interchangeDescription",
        "Export immutable job snapshots and import one translated item through canonical QA.",
    );
    let max_items_label = t(
        locale.as_deref(),
        "translation.field.maxExportItems",
        "Maximum export items",
    );
    let export_document_label = t(
        locale.as_deref(),
        "translation.field.exportDocument",
        "Export document",
    );
    let import_document_label = t(
        locale.as_deref(),
        "translation.field.importDocument",
        "Import item JSON",
    );
    let export_label = t(
        locale.as_deref(),
        "translation.action.exportJob",
        "Export job",
    );
    let import_label = t(
        locale.as_deref(),
        "translation.action.importItem",
        "Import item",
    );
    let interchange_artifacts_title = t(
        locale.as_deref(),
        "translation.jobs.interchangeArtifacts",
        "Expiring interchange artifacts",
    );
    let interchange_artifacts_description = t(
        locale.as_deref(),
        "translation.jobs.interchangeArtifactsDescription",
        "Store bounded interchange documents in private object storage, then inspect or process their aggregate conflict report.",
    );
    let interchange_artifacts_empty_label = t(
        locale.as_deref(),
        "translation.jobs.interchangeArtifactsEmpty",
        "No interchange artifacts have been loaded.",
    );
    let interchange_artifact_id_label = t(
        locale.as_deref(),
        "translation.field.interchangeArtifactId",
        "Artifact ID",
    );
    let interchange_artifact_expiry_label = t(
        locale.as_deref(),
        "translation.field.interchangeArtifactExpiry",
        "Artifact expiry (seconds)",
    );
    let interchange_artifact_document_label = t(
        locale.as_deref(),
        "translation.field.interchangeArtifactDocument",
        "Artifact document JSON",
    );
    let include_expired_label = t(
        locale.as_deref(),
        "translation.field.includeExpired",
        "Include expired",
    );
    let direction_label = t(
        locale.as_deref(),
        "translation.field.interchangeDirection",
        "Direction",
    );
    let expires_at_label = t(
        locale.as_deref(),
        "translation.field.expiresAt",
        "Expires at",
    );
    let accepted_items_label = t(
        locale.as_deref(),
        "translation.field.acceptedItems",
        "Accepted items",
    );
    let conflict_items_label = t(
        locale.as_deref(),
        "translation.field.conflictItems",
        "Conflict items",
    );
    let create_interchange_artifact_label = t(
        locale.as_deref(),
        "translation.action.createInterchangeExportArtifact",
        "Create export artifact",
    );
    let list_interchange_artifacts_label = t(
        locale.as_deref(),
        "translation.action.listInterchangeArtifacts",
        "Load artifacts",
    );
    let read_interchange_artifact_label = t(
        locale.as_deref(),
        "translation.action.readInterchangeArtifact",
        "Read artifact",
    );
    let store_interchange_artifact_label = t(
        locale.as_deref(),
        "translation.action.storeInterchangeImportArtifact",
        "Store import artifact",
    );
    let process_interchange_artifact_label = t(
        locale.as_deref(),
        "translation.action.processInterchangeImportArtifact",
        "Process import artifact",
    );

    let create_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::create_job_with_glossary_operation(
                    &source_locale.get_untracked(),
                    &target_locale.get_untracked(),
                    &glossary_id.get_untracked(),
                    &glossary_revision.get_untracked(),
                    &create_key.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(move |_| {
                    set_create_key.set(core::new_idempotency_key("create-job"));
                }),
            );
        }
    };
    let read_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::read_job_progress_operation(&job_id.get_untracked()),
                set_busy,
                set_outcome,
                Callback::new(|_| {}),
            );
        }
    };
    let rebuild_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::rebuild_job_progress_operation(
                    &job_id.get_untracked(),
                    &rebuild_key.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(move |_| {
                    set_rebuild_key.set(core::new_idempotency_key("rebuild-job-progress"));
                }),
            );
        }
    };
    let reviewer_queue_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::read_reviewer_queue_operation(core::ReviewerQueueOperationInput {
                    job_id: &job_id.get_untracked(),
                    assignee_kind: &reviewer_assignee_kind.get_untracked(),
                    assignee_id: &reviewer_assignee_id.get_untracked(),
                    include_unassigned: reviewer_include_unassigned.get_untracked(),
                    limit: &reviewer_queue_limit.get_untracked(),
                }),
                set_busy,
                set_outcome,
                Callback::new(move |response| {
                    if let TranslationAdminResponse::ReviewerQueue(queue) = response {
                        set_reviewer_queue.set(queue);
                    }
                }),
            );
        }
    };
    let reviewer_workload_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::read_reviewer_workload_operation(&job_id.get_untracked()),
                set_busy,
                set_outcome,
                Callback::new(move |response| {
                    if let TranslationAdminResponse::ReviewerWorkloads(workloads) = response {
                        set_reviewer_workloads.set(workloads);
                    }
                }),
            );
        }
    };
    let export_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::export_job_operation(
                    &job_id.get_untracked(),
                    &max_export_items.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(move |response| {
                    if let TranslationAdminResponse::InterchangeDocument(document) = response {
                        match core::interchange_document_json(&document) {
                            Ok(json) => set_export_document.set(json),
                            Err(error) => set_outcome.set(Some(Err(error.to_string()))),
                        }
                    }
                }),
            );
        }
    };
    let import_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::import_item_operation(
                    &import_document.get_untracked(),
                    &import_key.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(move |_| {
                    set_import_key.set(core::new_idempotency_key("import-item"));
                }),
            );
        }
    };
    let create_interchange_artifact_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::create_interchange_export_artifact_operation(
                    &job_id.get_untracked(),
                    &max_export_items.get_untracked(),
                    &interchange_artifact_expiry.get_untracked(),
                    &create_interchange_artifact_key.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(move |response| {
                    if let TranslationAdminResponse::InterchangeArtifact(artifact) = response {
                        set_interchange_artifact_id.set(artifact.id.clone());
                        set_interchange_artifacts.update(|artifacts| {
                            artifacts.retain(|current| current.id != artifact.id);
                            artifacts.push(artifact);
                            artifacts.sort_by(|left, right| right.created_at.cmp(&left.created_at));
                        });
                    }
                    set_create_interchange_artifact_key.set(core::new_idempotency_key(
                        "create-interchange-export-artifact",
                    ));
                }),
            );
        }
    };
    let list_interchange_artifacts_action = {
        let locale = locale.clone();
        move || {
            let current_job_id = job_id.get_untracked();
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::list_interchange_artifacts_operation(
                    core::ListInterchangeArtifactsOperationInput {
                        job_id: (!current_job_id.trim().is_empty())
                            .then_some(current_job_id.as_str()),
                        include_expired: interchange_artifact_include_expired.get_untracked(),
                        limit: "50",
                    },
                ),
                set_busy,
                set_outcome,
                Callback::new(move |response| {
                    if let TranslationAdminResponse::InterchangeArtifacts(artifacts) = response {
                        set_interchange_artifacts.set(artifacts);
                    }
                }),
            );
        }
    };
    let read_interchange_artifact_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::read_interchange_artifact_operation(&interchange_artifact_id.get_untracked()),
                set_busy,
                set_outcome,
                Callback::new(move |response| {
                    if let TranslationAdminResponse::InterchangeArtifactContent(content) = response
                    {
                        set_interchange_artifact_id.set(content.artifact.id.clone());
                        match core::interchange_document_json(&content.document) {
                            Ok(document) => set_interchange_artifact_document.set(document),
                            Err(error) => set_outcome.set(Some(Err(error.to_string()))),
                        }
                        set_interchange_artifacts.update(|artifacts| {
                            artifacts.retain(|current| current.id != content.artifact.id);
                            artifacts.push(content.artifact);
                            artifacts.sort_by(|left, right| right.created_at.cmp(&left.created_at));
                        });
                    }
                }),
            );
        }
    };
    let store_interchange_artifact_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::store_interchange_import_artifact_operation(
                    &job_id.get_untracked(),
                    &interchange_artifact_document.get_untracked(),
                    &interchange_artifact_expiry.get_untracked(),
                    &store_interchange_artifact_key.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(move |response| {
                    if let TranslationAdminResponse::InterchangeArtifact(artifact) = response {
                        set_interchange_artifact_id.set(artifact.id.clone());
                        set_interchange_artifacts.update(|artifacts| {
                            artifacts.retain(|current| current.id != artifact.id);
                            artifacts.push(artifact);
                            artifacts.sort_by(|left, right| right.created_at.cmp(&left.created_at));
                        });
                    }
                    set_store_interchange_artifact_key.set(core::new_idempotency_key(
                        "store-interchange-import-artifact",
                    ));
                }),
            );
        }
    };
    let process_interchange_artifact_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::process_interchange_import_artifact_operation(
                    &interchange_artifact_id.get_untracked(),
                    &process_interchange_artifact_key.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(move |response| {
                    if let TranslationAdminResponse::InterchangeArtifact(artifact) = response {
                        set_interchange_artifact_id.set(artifact.id.clone());
                        set_interchange_artifacts.update(|artifacts| {
                            artifacts.retain(|current| current.id != artifact.id);
                            artifacts.push(artifact);
                            artifacts.sort_by(|left, right| right.created_at.cmp(&left.created_at));
                        });
                    }
                    set_process_interchange_artifact_key.set(core::new_idempotency_key(
                        "process-interchange-import-artifact",
                    ));
                }),
            );
        }
    };
    let interchange_artifact_id_input_label = interchange_artifact_id_label.clone();
    let interchange_artifact_status_label = status_label.clone();

    view! {
        <div class="grid gap-6 xl:grid-cols-2">
            <Card>
                <CardHeader>
                    <CardTitle>{create_title}</CardTitle>
                    <CardDescription>{create_description}</CardDescription>
                </CardHeader>
                <CardContent class="space-y-4">
                    <div class="grid gap-4 sm:grid-cols-2">
                        <div class="space-y-2">
                            <Label required=true r#for="source_locale">{source_label}</Label>
                            <Input value=source_locale set_value=set_source_locale id="source_locale" name="source_locale" />
                        </div>
                        <div class="space-y-2">
                            <Label required=true r#for="target_locale">{target_label}</Label>
                            <Input value=target_locale set_value=set_target_locale id="target_locale" name="target_locale" />
                        </div>
                        <div class="space-y-2">
                            <Label r#for="glossary_id">{glossary_id_label}</Label>
                            <Input value=glossary_id set_value=set_glossary_id id="glossary_id" name="glossary_id" />
                        </div>
                        <div class="space-y-2">
                            <Label r#for="glossary_revision">{glossary_revision_label}</Label>
                            <Input value=glossary_revision set_value=set_glossary_revision id="glossary_revision" name="glossary_revision" />
                        </div>
                    </div>
                    <Button on_click=Box::new(create_action)>{create_label}</Button>
                </CardContent>
            </Card>

            <Card>
                <CardHeader>
                    <CardTitle>{inspect_title}</CardTitle>
                    <CardDescription>{inspect_description}</CardDescription>
                </CardHeader>
                <CardContent class="space-y-4">
                    <div class="space-y-2">
                        <Label required=true r#for="job_id">{job_id_label}</Label>
                        <Input value=job_id set_value=set_job_id id="job_id" name="job_id" />
                    </div>
                    <div class="flex flex-wrap gap-2">
                        <Button variant=ButtonVariant::Outline on_click=Box::new(read_action)>{read_label}</Button>
                        <Button variant=ButtonVariant::Secondary on_click=Box::new(rebuild_action)>{rebuild_label}</Button>
                    </div>
                    <Show when=move || busy.get()>
                        <p class="text-xs text-muted-foreground">"Operation in progress…"</p>
                    </Show>
                </CardContent>
            </Card>

            <Card class="xl:col-span-2">
                <CardHeader>
                    <CardTitle>{reviewer_title}</CardTitle>
                    <CardDescription>{reviewer_description}</CardDescription>
                </CardHeader>
                <CardContent class="space-y-5">
                    <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
                        <div class="space-y-2">
                            <Label r#for="reviewer_assignee_kind">{reviewer_kind_label}</Label>
                            <Select
                                options=vec![
                                    SelectOption::new("user", reviewer_user_label),
                                    SelectOption::new("service", reviewer_service_label),
                                ]
                                value=reviewer_assignee_kind
                                set_value=set_reviewer_assignee_kind
                                id="reviewer_assignee_kind"
                                name="reviewer_assignee_kind"
                            />
                        </div>
                        <div class="space-y-2">
                            <Label r#for="reviewer_assignee_id">{reviewer_id_label}</Label>
                            <Input value=reviewer_assignee_id set_value=set_reviewer_assignee_id id="reviewer_assignee_id" name="reviewer_assignee_id" />
                        </div>
                        <div class="space-y-2">
                            <Label required=true r#for="reviewer_queue_limit">{reviewer_limit_label}</Label>
                            <Input value=reviewer_queue_limit set_value=set_reviewer_queue_limit id="reviewer_queue_limit" name="reviewer_queue_limit" />
                        </div>
                        <label class="flex items-center gap-2 pt-8 text-sm text-foreground">
                            <Checkbox checked=reviewer_include_unassigned set_checked=set_reviewer_include_unassigned name="reviewer_include_unassigned" />
                            {include_unassigned_label}
                        </label>
                    </div>
                    <div class="flex flex-wrap gap-2">
                        <Button variant=ButtonVariant::Outline on_click=Box::new(reviewer_queue_action)>{reviewer_queue_label}</Button>
                        <Button variant=ButtonVariant::Secondary on_click=Box::new(reviewer_workload_action)>{reviewer_workload_button_label}</Button>
                    </div>
                    <div class="grid gap-5 xl:grid-cols-2">
                        <div class="space-y-2">
                            <h3 class="text-sm font-medium text-foreground">{queue_item_label}</h3>
                            {move || {
                                let queue = reviewer_queue.get();
                                if queue.is_empty() {
                                    view! {
                                        <p class="rounded-xl border border-dashed border-border p-4 text-sm text-muted-foreground">
                                            {reviewer_empty_label.clone()}
                                        </p>
                                    }
                                    .into_any()
                                } else {
                                    let unassigned_label = queue_unassigned_label.clone();
                                    view! {
                                        <div class="overflow-x-auto rounded-xl border border-border">
                                            <table class="w-full text-sm">
                                                <thead class="bg-muted/50 text-left text-xs uppercase tracking-wide text-muted-foreground">
                                                    <tr>
                                                        <th class="px-3 py-2">{item_label.clone()}</th>
                                                        <th class="px-3 py-2">{reviewer_label.clone()}</th>
                                                        <th class="px-3 py-2">{status_label.clone()}</th>
                                                        <th class="px-3 py-2">{submitted_at_label.clone()}</th>
                                                    </tr>
                                                </thead>
                                                <tbody class="divide-y divide-border">
                                                    {queue.into_iter().map(move |entry| {
                                                        let assignee = entry.item.assignee.map(|actor| {
                                                            let kind = match actor.kind {
                                                                ActorKind::User => "user",
                                                                ActorKind::Service => "service",
                                                            };
                                                            format!("{kind}:{}", actor.id)
                                                        }).unwrap_or_else(|| unassigned_label.clone());
                                                        view! {
                                                            <tr>
                                                                <td class="px-3 py-2 font-mono text-xs">{entry.item.id}</td>
                                                                <td class="px-3 py-2">{assignee}</td>
                                                                <td class="px-3 py-2">{entry.item.status}</td>
                                                                <td class="px-3 py-2 whitespace-nowrap text-xs text-muted-foreground">{entry.submitted_at}</td>
                                                            </tr>
                                                        }
                                                    }).collect_view()}
                                                </tbody>
                                            </table>
                                        </div>
                                    }
                                    .into_any()
                                }
                            }}
                        </div>
                        <div class="space-y-2">
                            <h3 class="text-sm font-medium text-foreground">{reviewer_workload_label}</h3>
                            {move || {
                                let workloads = reviewer_workloads.get();
                                if workloads.is_empty() {
                                    view! {
                                        <p class="rounded-xl border border-dashed border-border p-4 text-sm text-muted-foreground">
                                            {workload_empty_label.clone()}
                                        </p>
                                    }
                                    .into_any()
                                } else {
                                    let unassigned_label = workload_unassigned_label.clone();
                                    view! {
                                        <div class="overflow-x-auto rounded-xl border border-border">
                                            <table class="w-full text-sm">
                                                <thead class="bg-muted/50 text-left text-xs uppercase tracking-wide text-muted-foreground">
                                                    <tr>
                                                        <th class="px-3 py-2">{workload_reviewer_label.clone()}</th>
                                                        <th class="px-3 py-2">{open_items_label.clone()}</th>
                                                        <th class="px-3 py-2">{in_review_items_label.clone()}</th>
                                                        <th class="px-3 py-2">{approved_items_label.clone()}</th>
                                                        <th class="px-3 py-2">{rebase_required_label.clone()}</th>
                                                        <th class="px-3 py-2">{blocked_items_label.clone()}</th>
                                                        <th class="px-3 py-2">{source_characters_label.clone()}</th>
                                                    </tr>
                                                </thead>
                                                <tbody class="divide-y divide-border">
                                                    {workloads.into_iter().map(move |workload| {
                                                        let assignee = workload.assignee.map(|actor| {
                                                            let kind = match actor.kind {
                                                                ActorKind::User => "user",
                                                                ActorKind::Service => "service",
                                                            };
                                                            format!("{kind}:{}", actor.id)
                                                        }).unwrap_or_else(|| unassigned_label.clone());
                                                        view! {
                                                            <tr>
                                                                <td class="px-3 py-2">{assignee}</td>
                                                                <td class="px-3 py-2">{workload.open_items}</td>
                                                                <td class="px-3 py-2">{workload.in_review_items}</td>
                                                                <td class="px-3 py-2">{workload.approved_items}</td>
                                                                <td class="px-3 py-2">{workload.rebase_required_items}</td>
                                                                <td class="px-3 py-2">{workload.blocked_items}</td>
                                                                <td class="px-3 py-2">{workload.source_characters}</td>
                                                            </tr>
                                                        }
                                                    }).collect_view()}
                                                </tbody>
                                            </table>
                                        </div>
                                    }
                                    .into_any()
                                }
                            }}
                        </div>
                    </div>
                </CardContent>
            </Card>

            <Card class="xl:col-span-2">
                <CardHeader>
                    <CardTitle>{interchange_title}</CardTitle>
                    <CardDescription>{interchange_description}</CardDescription>
                </CardHeader>
                <CardContent class="space-y-4">
                    <div class="grid gap-4 lg:grid-cols-2">
                        <div class="space-y-3">
                            <div class="space-y-2">
                                <Label r#for="max_export_items">{max_items_label}</Label>
                                <Input value=max_export_items set_value=set_max_export_items id="max_export_items" name="max_export_items" />
                            </div>
                            <Button variant=ButtonVariant::Outline on_click=Box::new(export_action)>{export_label}</Button>
                            <div class="space-y-2">
                                <Label r#for="export_document">{export_document_label}</Label>
                                <Textarea value=export_document set_value=set_export_document id="export_document" name="export_document" rows=14 />
                            </div>
                        </div>
                        <div class="space-y-3">
                            <div class="space-y-2">
                                <Label required=true r#for="import_document">{import_document_label}</Label>
                                <Textarea value=import_document set_value=set_import_document id="import_document" name="import_document" rows=18 />
                            </div>
                            <Button on_click=Box::new(import_action)>{import_label}</Button>
                        </div>
                    </div>
                </CardContent>
            </Card>

            <Card class="xl:col-span-2">
                <CardHeader>
                    <CardTitle>{interchange_artifacts_title}</CardTitle>
                    <CardDescription>{interchange_artifacts_description}</CardDescription>
                </CardHeader>
                <CardContent class="space-y-5">
                    <div class="grid gap-4 lg:grid-cols-3">
                        <div class="space-y-2">
                            <Label required=true r#for="interchange_artifact_expiry">{interchange_artifact_expiry_label}</Label>
                            <Input value=interchange_artifact_expiry set_value=set_interchange_artifact_expiry id="interchange_artifact_expiry" name="interchange_artifact_expiry" />
                        </div>
                        <div class="space-y-2">
                            <Label r#for="interchange_artifact_id">{interchange_artifact_id_input_label}</Label>
                            <Input value=interchange_artifact_id set_value=set_interchange_artifact_id id="interchange_artifact_id" name="interchange_artifact_id" />
                        </div>
                        <label class="flex items-center gap-2 pt-8 text-sm text-foreground">
                            <Checkbox checked=interchange_artifact_include_expired set_checked=set_interchange_artifact_include_expired name="interchange_artifact_include_expired" />
                            {include_expired_label}
                        </label>
                    </div>
                    <div class="flex flex-wrap gap-2">
                        <Button on_click=Box::new(create_interchange_artifact_action)>{create_interchange_artifact_label}</Button>
                        <Button variant=ButtonVariant::Outline on_click=Box::new(list_interchange_artifacts_action)>{list_interchange_artifacts_label}</Button>
                        <Button variant=ButtonVariant::Outline on_click=Box::new(read_interchange_artifact_action)>{read_interchange_artifact_label}</Button>
                        <Button variant=ButtonVariant::Secondary on_click=Box::new(process_interchange_artifact_action)>{process_interchange_artifact_label}</Button>
                    </div>
                    <div class="space-y-2">
                        <Label required=true r#for="interchange_artifact_document">{interchange_artifact_document_label}</Label>
                        <Textarea value=interchange_artifact_document set_value=set_interchange_artifact_document id="interchange_artifact_document" name="interchange_artifact_document" rows=14 />
                        <Button on_click=Box::new(store_interchange_artifact_action)>{store_interchange_artifact_label}</Button>
                    </div>
                    {move || {
                        let artifacts = interchange_artifacts.get();
                        if artifacts.is_empty() {
                            view! {
                                <p class="rounded-xl border border-dashed border-border p-4 text-sm text-muted-foreground">
                                    {interchange_artifacts_empty_label.clone()}
                                </p>
                            }
                            .into_any()
                        } else {
                            let artifact_id_label = interchange_artifact_id_label.clone();
                            let direction_label = direction_label.clone();
                            let status_label = interchange_artifact_status_label.clone();
                            let expires_at_label = expires_at_label.clone();
                            let accepted_items_label = accepted_items_label.clone();
                            let conflict_items_label = conflict_items_label.clone();
                            view! {
                                <div class="overflow-x-auto rounded-xl border border-border">
                                    <table class="w-full text-sm" data-testid="translation-interchange-artifacts">
                                        <thead class="bg-muted/50 text-left text-xs uppercase tracking-wide text-muted-foreground">
                                            <tr>
                                                <th class="px-3 py-2">{artifact_id_label}</th>
                                                <th class="px-3 py-2">{direction_label}</th>
                                                <th class="px-3 py-2">{status_label}</th>
                                                <th class="px-3 py-2">{expires_at_label}</th>
                                                <th class="px-3 py-2">{accepted_items_label}</th>
                                                <th class="px-3 py-2">{conflict_items_label}</th>
                                            </tr>
                                        </thead>
                                        <tbody class="divide-y divide-border">
                                            {artifacts.into_iter().map(|artifact| {
                                                let accepted = artifact.report.as_ref().map(|report| report.accepted_items).unwrap_or_default();
                                                let conflicts = artifact.report.as_ref().map(|report| report.conflict_items).unwrap_or_default();
                                                view! {
                                                    <tr>
                                                        <td class="px-3 py-2 font-mono text-xs">{artifact.id}</td>
                                                        <td class="px-3 py-2">{artifact.direction}</td>
                                                        <td class="px-3 py-2">{artifact.status}</td>
                                                        <td class="px-3 py-2 whitespace-nowrap text-xs text-muted-foreground">{artifact.expires_at}</td>
                                                        <td class="px-3 py-2">{accepted}</td>
                                                        <td class="px-3 py-2">{conflicts}</td>
                                                    </tr>
                                                }
                                            }).collect_view()}
                                        </tbody>
                                    </table>
                                </div>
                            }
                            .into_any()
                        }
                    }}
                </CardContent>
            </Card>

            <div class="xl:col-span-2">
                <OutcomePanel outcome locale=locale.clone() />
            </div>
        </div>
    }
}

#[component]
fn GlossariesTab(
    token: Signal<Option<String>>,
    tenant: Signal<Option<String>>,
    locale: Option<String>,
    selected_glossary_id: Signal<Option<String>>,
) -> impl IntoView {
    let query_writer = use_route_query_writer();
    let (refresh_revision, set_refresh_revision) = signal(0_u64);
    let (busy, set_busy) = signal(false);
    let (outcome, set_outcome) = signal(OperationOutcome::None);

    let (name, set_name) = signal(String::new());
    let (description, set_description) = signal(String::new());
    let (source_locale, set_source_locale) = signal("en".to_string());
    let (target_locale, set_target_locale) = signal("de".to_string());
    let (owner_slug, set_owner_slug) = signal(String::new());
    let (resource_kind, set_resource_kind) = signal(String::new());
    let (field_key, set_field_key) = signal(String::new());
    let (create_key, set_create_key) = signal(core::new_idempotency_key("create-glossary"));

    let (selected_id, set_selected_id) = signal(String::new());
    let (selected_revision, set_selected_revision) = signal("1".to_string());
    let (selected_active, set_selected_active) = signal(true);
    let (edit_name, set_edit_name) = signal(String::new());
    let (edit_description, set_edit_description) = signal(String::new());
    let (concepts_json, set_concepts_json) = signal("[]".to_string());
    let (update_key, set_update_key) = signal(core::new_idempotency_key("update-glossary"));
    let (terms_key, set_terms_key) = signal(core::new_idempotency_key("replace-glossary-terms"));
    let (active_key, set_active_key) = signal(core::new_idempotency_key("set-glossary-active"));

    let locale_for_resource = locale.clone();
    let glossaries = LocalResource::new(move || {
        refresh_revision.get();
        let context =
            core::transport_context(token.get(), tenant.get(), locale_for_resource.clone());
        let selected = selected_glossary_id.get();
        async move {
            let list = transport::execute(
                context.clone(),
                TranslationAdminOperation::ListGlossaries { limit: 200 },
            )
            .await
            .map_err(|error| error.to_string())?;
            let list = match list {
                TranslationAdminResponse::Glossaries(glossaries) => glossaries,
                _ => return Err("Glossary list returned an unexpected response".to_string()),
            };
            let selected = match selected {
                Some(glossary_id) => {
                    let response = transport::execute(
                        context,
                        TranslationAdminOperation::ReadGlossary {
                            glossary_id,
                            revision: None,
                        },
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                    match response {
                        TranslationAdminResponse::Glossary(glossary) => Some(glossary),
                        _ => {
                            return Err("Glossary read returned an unexpected response".to_string());
                        }
                    }
                }
                None => None,
            };
            Ok::<(Vec<GlossarySummary>, Option<Glossary>), String>((list, selected))
        }
    });

    Effect::new(move |_| {
        if let Some(Ok((_, selected))) = glossaries.get() {
            if let Some(glossary) = selected {
                set_selected_id.set(glossary.id.clone());
                set_selected_revision.set(glossary.revision.to_string());
                set_selected_active.set(glossary.is_active);
                set_edit_name.set(glossary.name.clone());
                set_edit_description.set(glossary.description.clone());
                set_concepts_json.set(
                    serde_json::to_string_pretty(&glossary.concepts)
                        .unwrap_or_else(|_| "[]".to_string()),
                );
            } else {
                set_selected_id.set(String::new());
                set_selected_revision.set("1".to_string());
                set_selected_active.set(true);
                set_edit_name.set(String::new());
                set_edit_description.set(String::new());
                set_concepts_json.set("[]".to_string());
            }
        }
    });

    let create_action = Callback::new({
        let locale = locale.clone();
        move |_: ()| {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::create_glossary_operation(core::CreateGlossaryOperationInput {
                    name: &name.get_untracked(),
                    description: &description.get_untracked(),
                    source_locale: &source_locale.get_untracked(),
                    target_locale: &target_locale.get_untracked(),
                    owner_slug: &owner_slug.get_untracked(),
                    resource_kind: &resource_kind.get_untracked(),
                    field_key: &field_key.get_untracked(),
                    idempotency_key: &create_key.get_untracked(),
                }),
                set_busy,
                set_outcome,
                Callback::new(move |_| {
                    set_create_key.set(core::new_idempotency_key("create-glossary"));
                    set_refresh_revision.update(|revision| *revision = revision.saturating_add(1));
                }),
            );
        }
    });
    let update_action = Callback::new({
        let locale = locale.clone();
        move |_: ()| {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::update_glossary_operation(
                    &selected_id.get_untracked(),
                    &selected_revision.get_untracked(),
                    &edit_name.get_untracked(),
                    &edit_description.get_untracked(),
                    &update_key.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(move |_| {
                    set_update_key.set(core::new_idempotency_key("update-glossary"));
                    set_refresh_revision.update(|revision| *revision = revision.saturating_add(1));
                }),
            );
        }
    });
    let terms_action = Callback::new({
        let locale = locale.clone();
        move |_: ()| {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::replace_glossary_terms_operation(
                    &selected_id.get_untracked(),
                    &selected_revision.get_untracked(),
                    &concepts_json.get_untracked(),
                    &terms_key.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(move |_| {
                    set_terms_key.set(core::new_idempotency_key("replace-glossary-terms"));
                    set_refresh_revision.update(|revision| *revision = revision.saturating_add(1));
                }),
            );
        }
    });
    let active_action = Callback::new({
        let locale = locale.clone();
        move |_: ()| {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::set_glossary_active_operation(
                    &selected_id.get_untracked(),
                    &selected_revision.get_untracked(),
                    !selected_active.get_untracked(),
                    &active_key.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(move |_| {
                    set_active_key.set(core::new_idempotency_key("set-glossary-active"));
                    set_refresh_revision.update(|revision| *revision = revision.saturating_add(1));
                }),
            );
        }
    });

    let loading = t(
        locale.as_deref(),
        "translation.glossary.loading",
        "Loading glossaries…",
    );
    let create_title = t(
        locale.as_deref(),
        "translation.glossary.create",
        "Create glossary",
    );
    let create_description = t(
        locale.as_deref(),
        "translation.glossary.createDescription",
        "Create a tenant-scoped, locale-pair terminology policy.",
    );
    let name_label = t(locale.as_deref(), "translation.field.name", "Name");
    let description_label = t(
        locale.as_deref(),
        "translation.field.description",
        "Description",
    );
    let source_label = t(
        locale.as_deref(),
        "translation.field.sourceLocale",
        "Source locale",
    );
    let target_label = t(
        locale.as_deref(),
        "translation.field.targetLocale",
        "Target locale",
    );
    let owner_label = t(
        locale.as_deref(),
        "translation.field.ownerSlug",
        "Owner slug",
    );
    let resource_label = t(
        locale.as_deref(),
        "translation.field.resourceKind",
        "Resource kind",
    );
    let field_label = t(locale.as_deref(), "translation.field.fieldKey", "Field key");
    let create_button_label = t(
        locale.as_deref(),
        "translation.action.createGlossary",
        "Create glossary",
    );
    let list_title = t(locale.as_deref(), "translation.glossary.list", "Glossaries");
    let list_description = t(
        locale.as_deref(),
        "translation.glossary.listDescription",
        "Selection is URL-owned and no glossary is selected implicitly.",
    );
    let empty_label = t(
        locale.as_deref(),
        "translation.glossary.empty",
        "No glossaries exist yet.",
    );
    let metadata_title = t(
        locale.as_deref(),
        "translation.glossary.metadata",
        "Glossary metadata",
    );
    let metadata_description = t(
        locale.as_deref(),
        "translation.glossary.metadataDescription",
        "Metadata and lifecycle changes use compare-and-set revisions.",
    );
    let update_label = t(
        locale.as_deref(),
        "translation.action.updateGlossary",
        "Update metadata",
    );
    let deactivate_label = t(
        locale.as_deref(),
        "translation.action.deactivateGlossary",
        "Deactivate",
    );
    let activate_label = t(
        locale.as_deref(),
        "translation.action.activateGlossary",
        "Activate",
    );
    let clear_label = t(
        locale.as_deref(),
        "translation.action.clearSelection",
        "Clear selection",
    );
    let terms_title = t(
        locale.as_deref(),
        "translation.glossary.terms",
        "Versioned terms",
    );
    let terms_description = t(
        locale.as_deref(),
        "translation.glossary.termsDescription",
        "Replace the complete concept snapshot; prior revisions remain readable by jobs.",
    );
    let concepts_label = t(
        locale.as_deref(),
        "translation.field.conceptsJson",
        "Concepts JSON",
    );
    let replace_terms_label = t(
        locale.as_deref(),
        "translation.action.replaceGlossaryTerms",
        "Replace term snapshot",
    );
    let error_title = t(
        locale.as_deref(),
        "translation.glossary.error",
        "Unable to load glossaries",
    );
    let locale_for_view = locale.clone();

    view! {
        <div class="space-y-6">
            <Suspense fallback=move || view! {
                <Card>
                    <CardContent>
                        <p class="text-sm text-muted-foreground">{loading.clone()}</p>
                    </CardContent>
                </Card>
            }>
                {move || {
                    let _locale = locale_for_view.clone();
                    let create_action = create_action;
                    let update_action = update_action;
                    let terms_action = terms_action;
                    let active_action = active_action;
                    let query_writer_for_list = query_writer.clone();
                    let query_writer_for_clear = query_writer.clone();
                    let create_title = create_title.clone();
                    let create_description = create_description.clone();
                    let create_name_label = name_label.clone();
                    let edit_name_label = name_label.clone();
                    let create_description_label = description_label.clone();
                    let edit_description_label = description_label.clone();
                    let source_label = source_label.clone();
                    let target_label = target_label.clone();
                    let owner_label = owner_label.clone();
                    let resource_label = resource_label.clone();
                    let field_label = field_label.clone();
                    let create_button_label = create_button_label.clone();
                    let list_title = list_title.clone();
                    let list_description = list_description.clone();
                    let empty_label = empty_label.clone();
                    let metadata_title = metadata_title.clone();
                    let metadata_description = metadata_description.clone();
                    let update_label = update_label.clone();
                    let deactivate_label = deactivate_label.clone();
                    let activate_label = activate_label.clone();
                    let clear_label = clear_label.clone();
                    let terms_title = terms_title.clone();
                    let terms_description = terms_description.clone();
                    let concepts_label = concepts_label.clone();
                    let replace_terms_label = replace_terms_label.clone();
                    let error_title = error_title.clone();
                    glossaries.get().map(|result| match result {
                        Ok((items, selected)) => view! {
                            <div class="space-y-6">
                                <div class="grid gap-6 xl:grid-cols-[minmax(0,0.9fr)_minmax(0,1.3fr)]">
                                    <Card>
                                        <CardHeader>
                                            <CardTitle>{create_title}</CardTitle>
                                            <CardDescription>{create_description}</CardDescription>
                                        </CardHeader>
                                        <CardContent class="space-y-4">
                                            <div class="space-y-2"><Label required=true r#for="glossary_name">{create_name_label}</Label><Input value=name set_value=set_name id="glossary_name" name="glossary_name" /></div>
                                            <div class="space-y-2"><Label r#for="glossary_description">{create_description_label}</Label><Textarea value=description set_value=set_description id="glossary_description" name="glossary_description" /></div>
                                            <div class="grid gap-4 sm:grid-cols-2">
                                                <div class="space-y-2"><Label required=true r#for="glossary_source_locale">{source_label}</Label><Input value=source_locale set_value=set_source_locale id="glossary_source_locale" name="glossary_source_locale" /></div>
                                                <div class="space-y-2"><Label required=true r#for="glossary_target_locale">{target_label}</Label><Input value=target_locale set_value=set_target_locale id="glossary_target_locale" name="glossary_target_locale" /></div>
                                                <div class="space-y-2"><Label r#for="glossary_owner_slug">{owner_label}</Label><Input value=owner_slug set_value=set_owner_slug id="glossary_owner_slug" name="glossary_owner_slug" /></div>
                                                <div class="space-y-2"><Label r#for="glossary_resource_kind">{resource_label}</Label><Input value=resource_kind set_value=set_resource_kind id="glossary_resource_kind" name="glossary_resource_kind" /></div>
                                                <div class="space-y-2"><Label r#for="glossary_field_key">{field_label}</Label><Input value=field_key set_value=set_field_key id="glossary_field_key" name="glossary_field_key" /></div>
                                            </div>
                                            <Button on_click=Box::new(move || create_action.run(()))>{create_button_label}</Button>
                                        </CardContent>
                                    </Card>

                                    <Card>
                                        <CardHeader>
                                            <CardTitle>{list_title}</CardTitle>
                                            <CardDescription>{list_description}</CardDescription>
                                        </CardHeader>
                                        <CardContent class="space-y-3">
                                            {if items.is_empty() {
                                                view! {
                                                    <p class="rounded-xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
                                                        {empty_label}
                                                    </p>
                                                }.into_any()
                                            } else {
                                                items.into_iter().map(|item| {
                                                    let writer = query_writer_for_list.clone();
                                                    let item_id = item.id.clone();
                                                    let is_selected = selected_glossary_id.get().as_deref() == Some(item.id.as_str());
                                                    view! {
                                                        <button
                                                            type="button"
                                                            class=if is_selected {
                                                                "w-full rounded-xl border border-primary bg-primary/5 p-4 text-left"
                                                            } else {
                                                                "w-full rounded-xl border border-border p-4 text-left hover:bg-muted/50"
                                                            }
                                                            on:click=move |_| writer.apply_query_intent(core::glossary_selection_intent(Some(&item_id)))
                                                        >
                                                            <div class="flex flex-wrap items-center justify-between gap-2">
                                                                <span class="font-medium text-foreground">{item.name}</span>
                                                                <div class="flex gap-2">
                                                                    <Badge variant=if item.is_active { BadgeVariant::Success } else { BadgeVariant::Secondary }>
                                                                        {if item.is_active { "active" } else { "inactive" }}
                                                                    </Badge>
                                                                    <Badge variant=BadgeVariant::Outline>{format!("v{}", item.revision)}</Badge>
                                                                </div>
                                                            </div>
                                                            <p class="mt-2 text-xs text-muted-foreground">{format!("{} → {}", item.source_locale, item.target_locale)}</p>
                                                        </button>
                                                    }
                                                }).collect_view().into_any()
                                            }}
                                        </CardContent>
                                    </Card>
                                </div>

                                {selected.map(|selected| view! {
                                    <div class="grid gap-6 xl:grid-cols-2">
                                        <Card>
                                            <CardHeader>
                                                <CardTitle>{metadata_title}</CardTitle>
                                                <CardDescription>{metadata_description}</CardDescription>
                                            </CardHeader>
                                            <CardContent class="space-y-4">
                                                <div class="flex flex-wrap gap-2">
                                                    <Badge variant=BadgeVariant::Outline>{format!("v{}", selected.revision)}</Badge>
                                                    <Badge variant=if selected.is_active { BadgeVariant::Success } else { BadgeVariant::Secondary }>{if selected.is_active { "active" } else { "inactive" }}</Badge>
                                                    <Badge variant=BadgeVariant::Outline>{format!("{} concepts", selected.concepts.len())}</Badge>
                                                </div>
                                                <div class="space-y-2"><Label required=true r#for="edit_glossary_name">{edit_name_label}</Label><Input value=edit_name set_value=set_edit_name id="edit_glossary_name" name="edit_glossary_name" /></div>
                                                <div class="space-y-2"><Label r#for="edit_glossary_description">{edit_description_label}</Label><Textarea value=edit_description set_value=set_edit_description id="edit_glossary_description" name="edit_glossary_description" /></div>
                                                <div class="flex flex-wrap gap-2">
                                                    <Button on_click=Box::new(move || update_action.run(()))>{update_label}</Button>
                                                    <Button variant=ButtonVariant::Secondary on_click=Box::new(move || active_action.run(()))>
                                                        {if selected.is_active {
                                                            deactivate_label
                                                        } else {
                                                            activate_label
                                                        }}
                                                    </Button>
                                                    <Button
                                                        variant=ButtonVariant::Outline
                                                        on_click=Box::new({
                                                            let writer = query_writer_for_clear.clone();
                                                            move || writer.apply_query_intent(core::glossary_selection_intent(None))
                                                        })
                                                    >
                                                        {clear_label}
                                                    </Button>
                                                </div>
                                            </CardContent>
                                        </Card>
                                        <Card>
                                            <CardHeader>
                                                <CardTitle>{terms_title}</CardTitle>
                                                <CardDescription>{terms_description}</CardDescription>
                                            </CardHeader>
                                            <CardContent class="space-y-4">
                                                <div class="space-y-2">
                                                    <Label required=true r#for="glossary_concepts_json">{concepts_label}</Label>
                                                    <Textarea value=concepts_json set_value=set_concepts_json id="glossary_concepts_json" name="glossary_concepts_json" />
                                                </div>
                                                <Button on_click=Box::new(move || terms_action.run(()))>{replace_terms_label}</Button>
                                            </CardContent>
                                        </Card>
                                    </div>
                                })}
                            </div>
                        }.into_any(),
                        Err(error) => view! {
                            <Alert
                                variant=AlertVariant::Destructive
                                title=error_title
                            >
                                {error}
                            </Alert>
                        }.into_any(),
                    })
                }}
            </Suspense>
            <Show when=move || busy.get()>
                <p class="text-xs text-muted-foreground">"Operation in progress…"</p>
            </Show>
            <OutcomePanel outcome locale=locale.clone() />
        </div>
    }
}

#[component]
fn MemoryTab(
    token: Signal<Option<String>>,
    tenant: Signal<Option<String>>,
    locale: Option<String>,
    selected_memory_entry_id: Signal<Option<String>>,
) -> impl IntoView {
    let query_writer = use_route_query_writer();
    let query_writer_for_purge = query_writer.clone();
    let (refresh_revision, set_refresh_revision) = signal(0_u64);
    let (busy, set_busy) = signal(false);
    let (outcome, set_outcome) = signal(OperationOutcome::None);
    let (suggestions, set_suggestions) = signal(Vec::<MemorySuggestion>::new());

    let (list_source_locale, set_list_source_locale) = signal(String::new());
    let (list_target_locale, set_list_target_locale) = signal(String::new());
    let (include_tombstoned, set_include_tombstoned) = signal(false);
    let (list_limit, set_list_limit) = signal("200".to_string());

    let (lookup_source_locale, set_lookup_source_locale) = signal("en".to_string());
    let (lookup_target_locale, set_lookup_target_locale) = signal("de".to_string());
    let (lookup_owner_slug, set_lookup_owner_slug) = signal("media".to_string());
    let (lookup_resource_kind, set_lookup_resource_kind) = signal("asset".to_string());
    let (lookup_resource_id, set_lookup_resource_id) = signal(String::new());
    let (lookup_subresource_id, set_lookup_subresource_id) = signal(String::new());
    let (lookup_field_key, set_lookup_field_key) = signal("alt".to_string());
    let (lookup_source_text, set_lookup_source_text) = signal(String::new());
    let (lookup_minimum_score, set_lookup_minimum_score) = signal("8500".to_string());
    let (lookup_limit, set_lookup_limit) = signal("10".to_string());

    let (selected_id, set_selected_id) = signal(String::new());
    let (selected_revision, set_selected_revision) = signal("1".to_string());
    let (retention_policy, set_retention_policy) = signal("owner_lifecycle".to_string());
    let (retain_until, set_retain_until) = signal(String::new());
    let (retention_key, set_retention_key) =
        signal(core::new_idempotency_key("set-memory-retention"));
    let (tombstone_key, set_tombstone_key) =
        signal(core::new_idempotency_key("tombstone-memory-entry"));
    let (purge_key, set_purge_key) = signal(core::new_idempotency_key("purge-memory-entry"));

    let locale_for_resource = locale.clone();
    let memory_entries = LocalResource::new(move || {
        refresh_revision.get();
        let context =
            core::transport_context(token.get(), tenant.get(), locale_for_resource.clone());
        let selected = selected_memory_entry_id.get();
        let list_operation = core::list_memory_entries_operation(
            &list_source_locale.get(),
            &list_target_locale.get(),
            include_tombstoned.get(),
            &list_limit.get(),
        );
        async move {
            let list = transport::execute(
                context.clone(),
                list_operation.map_err(|error| error.to_string())?,
            )
            .await
            .map_err(|error| error.to_string())?;
            let list = match list {
                TranslationAdminResponse::MemoryEntries(entries) => entries,
                _ => return Err("Memory list returned an unexpected response".to_string()),
            };
            let selected = match selected {
                Some(entry_id) => {
                    let response = transport::execute(
                        context,
                        core::read_memory_entry_operation(&entry_id)
                            .map_err(|error| error.to_string())?,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                    match response {
                        TranslationAdminResponse::MemoryEntry(entry) => Some(entry),
                        _ => {
                            return Err(
                                "Memory entry read returned an unexpected response".to_string()
                            );
                        }
                    }
                }
                None => None,
            };
            Ok::<(Vec<MemoryEntry>, Option<MemoryEntry>), String>((list, selected))
        }
    });

    Effect::new(move |_| {
        if let Some(Ok((_, selected))) = memory_entries.get() {
            if let Some(entry) = selected {
                set_selected_id.set(entry.id.clone());
                set_selected_revision.set(entry.revision.to_string());
                set_retention_policy.set(match entry.retention_policy {
                    crate::model::MemoryRetentionPolicy::OwnerLifecycle => {
                        "owner_lifecycle".to_string()
                    }
                    crate::model::MemoryRetentionPolicy::RetainUntil => "retain_until".to_string(),
                    crate::model::MemoryRetentionPolicy::LegalHold => "legal_hold".to_string(),
                });
                set_retain_until.set(entry.retain_until.clone().unwrap_or_default());
            } else {
                set_selected_id.set(String::new());
                set_selected_revision.set("1".to_string());
                set_retention_policy.set("owner_lifecycle".to_string());
                set_retain_until.set(String::new());
            }
        }
    });

    let lookup_action = Callback::new({
        let locale = locale.clone();
        move |_: ()| {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::lookup_memory_operation(core::MemoryLookupInput {
                    source_locale: &lookup_source_locale.get_untracked(),
                    target_locale: &lookup_target_locale.get_untracked(),
                    owner_slug: &lookup_owner_slug.get_untracked(),
                    resource_kind: &lookup_resource_kind.get_untracked(),
                    resource_id: &lookup_resource_id.get_untracked(),
                    subresource_id: &lookup_subresource_id.get_untracked(),
                    field_key: &lookup_field_key.get_untracked(),
                    source_text: &lookup_source_text.get_untracked(),
                    minimum_similarity_basis_points: &lookup_minimum_score.get_untracked(),
                    limit: &lookup_limit.get_untracked(),
                }),
                set_busy,
                set_outcome,
                Callback::new(move |response| {
                    if let TranslationAdminResponse::MemorySuggestions(values) = response {
                        set_suggestions.set(values);
                    }
                }),
            );
        }
    });
    let retention_action = Callback::new({
        let locale = locale.clone();
        move |_: ()| {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::set_memory_retention_operation(
                    &selected_id.get_untracked(),
                    &selected_revision.get_untracked(),
                    &retention_policy.get_untracked(),
                    &retain_until.get_untracked(),
                    &retention_key.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(move |_| {
                    set_retention_key.set(core::new_idempotency_key("set-memory-retention"));
                    set_refresh_revision.update(|revision| *revision = revision.saturating_add(1));
                }),
            );
        }
    });
    let tombstone_action = Callback::new({
        let locale = locale.clone();
        move |_: ()| {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::tombstone_memory_entry_operation(
                    &selected_id.get_untracked(),
                    &selected_revision.get_untracked(),
                    &tombstone_key.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(move |_| {
                    set_tombstone_key.set(core::new_idempotency_key("tombstone-memory-entry"));
                    set_refresh_revision.update(|revision| *revision = revision.saturating_add(1));
                }),
            );
        }
    });
    let purge_action = Callback::new({
        let locale = locale.clone();
        move |_: ()| {
            let writer = query_writer_for_purge.clone();
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::purge_memory_entry_operation(
                    &selected_id.get_untracked(),
                    &selected_revision.get_untracked(),
                    &purge_key.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(move |_| {
                    set_purge_key.set(core::new_idempotency_key("purge-memory-entry"));
                    writer.apply_query_intent(core::memory_selection_intent(None));
                    set_refresh_revision.update(|revision| *revision = revision.saturating_add(1));
                }),
            );
        }
    });

    let loading = t(
        locale.as_deref(),
        "translation.memory.loading",
        "Loading translation memory...",
    );
    let load_error = t(
        locale.as_deref(),
        "translation.memory.error",
        "Unable to load translation memory",
    );
    let list_title = t(
        locale.as_deref(),
        "translation.memory.list",
        "Memory entries",
    );
    let list_description = t(
        locale.as_deref(),
        "translation.memory.listDescription",
        "Entries are created only from reviewed owner applies; selection is URL-owned.",
    );
    let lookup_title = t(
        locale.as_deref(),
        "translation.memory.lookup",
        "Context-aware lookup",
    );
    let lookup_description = t(
        locale.as_deref(),
        "translation.memory.lookupDescription",
        "Find exact and deterministic fuzzy suggestions for one owner field.",
    );
    let lifecycle_title = t(
        locale.as_deref(),
        "translation.memory.lifecycle",
        "Retention and lifecycle",
    );
    let lifecycle_description = t(
        locale.as_deref(),
        "translation.memory.lifecycleDescription",
        "All mutations use compare-and-set revisions and replay-safe receipts.",
    );
    let empty_label = t(
        locale.as_deref(),
        "translation.memory.empty",
        "No matching memory entries exist.",
    );
    let no_selection = t(
        locale.as_deref(),
        "translation.memory.select",
        "Select an entry to inspect or manage it.",
    );
    let no_suggestions = t(
        locale.as_deref(),
        "translation.memory.noSuggestions",
        "No suggestions have been requested.",
    );
    let source_locale_label = t(
        locale.as_deref(),
        "translation.field.sourceLocale",
        "Source locale",
    );
    let target_locale_label = t(
        locale.as_deref(),
        "translation.field.targetLocale",
        "Target locale",
    );
    let owner_slug_label = t(
        locale.as_deref(),
        "translation.field.ownerSlug",
        "Owner slug",
    );
    let resource_kind_label = t(
        locale.as_deref(),
        "translation.field.resourceKind",
        "Resource kind",
    );
    let resource_id_label = t(
        locale.as_deref(),
        "translation.field.resourceId",
        "Resource ID",
    );
    let subresource_id_label = t(
        locale.as_deref(),
        "translation.field.subresourceId",
        "Subresource ID",
    );
    let field_key_label = t(locale.as_deref(), "translation.field.fieldKey", "Field key");
    let minimum_score_label = t(
        locale.as_deref(),
        "translation.field.minimumScore",
        "Minimum score (basis points)",
    );
    let source_text_label = t(
        locale.as_deref(),
        "translation.field.sourceText",
        "Source text",
    );
    let target_text_label = t(
        locale.as_deref(),
        "translation.field.targetText",
        "Target text",
    );
    let lookup_limit_label = t(
        locale.as_deref(),
        "translation.field.lookupLimit",
        "Lookup limit",
    );
    let list_limit_label = t(
        locale.as_deref(),
        "translation.field.listLimit",
        "List limit",
    );
    let source_filter_label = t(
        locale.as_deref(),
        "translation.field.sourceLocaleFilter",
        "Source locale filter",
    );
    let target_filter_label = t(
        locale.as_deref(),
        "translation.field.targetLocaleFilter",
        "Target locale filter",
    );
    let include_tombstoned_label = t(
        locale.as_deref(),
        "translation.field.includeTombstoned",
        "Include tombstoned",
    );
    let lookup_label = t(
        locale.as_deref(),
        "translation.action.lookupMemory",
        "Find suggestions",
    );
    let refresh_label = t(
        locale.as_deref(),
        "translation.action.refreshMemory",
        "Refresh entries",
    );
    let entries_title = t(locale.as_deref(), "translation.memory.entries", "Entries");
    let selection_description = t(
        locale.as_deref(),
        "translation.memory.selectionDescription",
        "No entry is selected implicitly.",
    );
    let selected_title = t(
        locale.as_deref(),
        "translation.memory.selected",
        "Selected memory entry",
    );
    let active_label = t(locale.as_deref(), "translation.memory.active", "active");
    let tombstoned_label = t(
        locale.as_deref(),
        "translation.memory.tombstoned",
        "tombstoned",
    );
    let resource_label = t(locale.as_deref(), "translation.field.resource", "Resource");
    let reviewer_label = t(locale.as_deref(), "translation.field.reviewer", "Reviewer");
    let proposal_label = t(
        locale.as_deref(),
        "translation.field.proposalId",
        "Proposal ID",
    );
    let apply_receipt_label = t(
        locale.as_deref(),
        "translation.field.providerReceipt",
        "Apply receipt",
    );
    let retention_policy_label = t(
        locale.as_deref(),
        "translation.field.retentionPolicy",
        "Retention policy",
    );
    let retain_until_label = t(
        locale.as_deref(),
        "translation.field.retainUntil",
        "Retain until (RFC 3339)",
    );
    let owner_lifecycle_label = t(
        locale.as_deref(),
        "translation.memory.retention.ownerLifecycle",
        "Owner lifecycle",
    );
    let retain_until_option_label = t(
        locale.as_deref(),
        "translation.memory.retention.retainUntil",
        "Retain until",
    );
    let legal_hold_label = t(
        locale.as_deref(),
        "translation.memory.retention.legalHold",
        "Legal hold",
    );
    let update_retention_label = t(
        locale.as_deref(),
        "translation.action.updateMemoryRetention",
        "Update retention",
    );
    let tombstone_label = t(
        locale.as_deref(),
        "translation.action.tombstoneMemory",
        "Tombstone",
    );
    let purge_label = t(
        locale.as_deref(),
        "translation.action.purgeMemory",
        "Purge content",
    );
    let clear_label = t(
        locale.as_deref(),
        "translation.action.clearSelection",
        "Clear selection",
    );
    let lookup_source_text_label = source_text_label.clone();

    view! {
        <div class="space-y-6">
            <Card>
                <CardHeader>
                    <CardTitle>{lookup_title}</CardTitle>
                    <CardDescription>{lookup_description}</CardDescription>
                </CardHeader>
                <CardContent class="space-y-4">
                    <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
                        <div class="space-y-2"><Label required=true r#for="memory_lookup_source_locale">{source_locale_label}</Label><Input value=lookup_source_locale set_value=set_lookup_source_locale id="memory_lookup_source_locale" name="memory_lookup_source_locale" /></div>
                        <div class="space-y-2"><Label required=true r#for="memory_lookup_target_locale">{target_locale_label}</Label><Input value=lookup_target_locale set_value=set_lookup_target_locale id="memory_lookup_target_locale" name="memory_lookup_target_locale" /></div>
                        <div class="space-y-2"><Label required=true r#for="memory_lookup_owner_slug">{owner_slug_label}</Label><Input value=lookup_owner_slug set_value=set_lookup_owner_slug id="memory_lookup_owner_slug" name="memory_lookup_owner_slug" /></div>
                        <div class="space-y-2"><Label required=true r#for="memory_lookup_resource_kind">{resource_kind_label}</Label><Input value=lookup_resource_kind set_value=set_lookup_resource_kind id="memory_lookup_resource_kind" name="memory_lookup_resource_kind" /></div>
                        <div class="space-y-2"><Label required=true r#for="memory_lookup_resource_id">{resource_id_label}</Label><Input value=lookup_resource_id set_value=set_lookup_resource_id id="memory_lookup_resource_id" name="memory_lookup_resource_id" /></div>
                        <div class="space-y-2"><Label r#for="memory_lookup_subresource_id">{subresource_id_label}</Label><Input value=lookup_subresource_id set_value=set_lookup_subresource_id id="memory_lookup_subresource_id" name="memory_lookup_subresource_id" /></div>
                        <div class="space-y-2"><Label required=true r#for="memory_lookup_field_key">{field_key_label}</Label><Input value=lookup_field_key set_value=set_lookup_field_key id="memory_lookup_field_key" name="memory_lookup_field_key" /></div>
                        <div class="space-y-2"><Label required=true r#for="memory_lookup_minimum_score">{minimum_score_label}</Label><Input value=lookup_minimum_score set_value=set_lookup_minimum_score id="memory_lookup_minimum_score" name="memory_lookup_minimum_score" /></div>
                    </div>
                    <div class="space-y-2"><Label required=true r#for="memory_lookup_source_text">{lookup_source_text_label}</Label><Textarea value=lookup_source_text set_value=set_lookup_source_text id="memory_lookup_source_text" name="memory_lookup_source_text" /></div>
                    <div class="flex flex-wrap items-end gap-3">
                        <div class="w-32 space-y-2"><Label required=true r#for="memory_lookup_limit">{lookup_limit_label}</Label><Input value=lookup_limit set_value=set_lookup_limit id="memory_lookup_limit" name="memory_lookup_limit" /></div>
                        <Button on_click=Box::new(move || lookup_action.run(()))>{lookup_label}</Button>
                    </div>
                    {move || {
                        let values = suggestions.get();
                        if values.is_empty() {
                            view! { <p class="text-sm text-muted-foreground">{no_suggestions.clone()}</p> }.into_any()
                        } else {
                            view! {
                                <div class="grid gap-3">
                                    {values.into_iter().map(|suggestion| view! {
                                        <div class="rounded-xl border border-border p-4">
                                            <div class="flex flex-wrap items-center justify-between gap-2">
                                                <span class="font-medium text-foreground">{suggestion.target_text}</span>
                                                <Badge variant=BadgeVariant::Outline>{format!("{} bp", suggestion.evidence.final_similarity_basis_points)}</Badge>
                                            </div>
                                            <p class="mt-2 text-sm text-muted-foreground">{suggestion.source_text}</p>
                                            <p class="mt-2 text-xs text-muted-foreground">{format!("{}/{} | {} | {:?}", suggestion.owner_slug, suggestion.resource_kind, suggestion.field_key, suggestion.evidence.kind)}</p>
                                        </div>
                                    }).collect_view()}
                                </div>
                            }.into_any()
                        }
                    }}
                </CardContent>
            </Card>

            <Card>
                <CardHeader>
                    <CardTitle>{list_title}</CardTitle>
                    <CardDescription>{list_description}</CardDescription>
                </CardHeader>
                <CardContent class="space-y-4">
                    <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
                        <div class="space-y-2"><Label r#for="memory_source_locale_filter">{source_filter_label}</Label><Input value=list_source_locale set_value=set_list_source_locale id="memory_source_locale_filter" name="memory_source_locale_filter" /></div>
                        <div class="space-y-2"><Label r#for="memory_target_locale_filter">{target_filter_label}</Label><Input value=list_target_locale set_value=set_list_target_locale id="memory_target_locale_filter" name="memory_target_locale_filter" /></div>
                        <div class="space-y-2"><Label required=true r#for="memory_list_limit">{list_limit_label}</Label><Input value=list_limit set_value=set_list_limit id="memory_list_limit" name="memory_list_limit" /></div>
                        <label class="flex items-center gap-2 pt-8 text-sm text-foreground">
                            <Checkbox checked=include_tombstoned set_checked=set_include_tombstoned name="memory_include_tombstoned" />
                            {include_tombstoned_label}
                        </label>
                    </div>
                    <Button
                        variant=ButtonVariant::Outline
                        on_click=Box::new(move || set_refresh_revision.update(|revision| *revision = revision.saturating_add(1)))
                    >
                        {refresh_label}
                    </Button>
                </CardContent>
            </Card>

            <Suspense fallback=move || view! {
                <Card><CardContent class="p-6"><p class="text-sm text-muted-foreground">{loading.clone()}</p></CardContent></Card>
            }>
                {move || {
                    let empty_label = empty_label.clone();
                    let no_selection = no_selection.clone();
                    let load_error = load_error.clone();
                    let lifecycle_title = lifecycle_title.clone();
                    let lifecycle_description = lifecycle_description.clone();
                    let entries_title = entries_title.clone();
                    let selection_description = selection_description.clone();
                    let selected_title = selected_title.clone();
                    let active_label = active_label.clone();
                    let tombstoned_label = tombstoned_label.clone();
                    let source_text_label = source_text_label.clone();
                    let target_text_label = target_text_label.clone();
                    let resource_label = resource_label.clone();
                    let reviewer_label = reviewer_label.clone();
                    let proposal_label = proposal_label.clone();
                    let apply_receipt_label = apply_receipt_label.clone();
                    let retention_policy_label = retention_policy_label.clone();
                    let retain_until_label = retain_until_label.clone();
                    let owner_lifecycle_label = owner_lifecycle_label.clone();
                    let retain_until_option_label = retain_until_option_label.clone();
                    let legal_hold_label = legal_hold_label.clone();
                    let update_retention_label = update_retention_label.clone();
                    let tombstone_label = tombstone_label.clone();
                    let purge_label = purge_label.clone();
                    let clear_label = clear_label.clone();
                    let list_query_writer = query_writer.clone();
                    let clear_query_writer = query_writer.clone();
                    memory_entries.get().map(move |result| match result {
                        Ok((entries, selected)) => view! {
                            <div class="grid gap-6 xl:grid-cols-[minmax(0,0.9fr)_minmax(0,1.4fr)]">
                                <Card>
                                    <CardHeader>
                                        <CardTitle>{entries_title}</CardTitle>
                                        <CardDescription>{selection_description}</CardDescription>
                                    </CardHeader>
                                    <CardContent class="space-y-3">
                                        {if entries.is_empty() {
                                            view! { <p class="rounded-xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">{empty_label.clone()}</p> }.into_any()
                                        } else {
                                            entries.into_iter().map(|entry| {
                                                let entry_id = entry.id.clone();
                                                let is_selected = selected_memory_entry_id.get().as_deref() == Some(entry.id.as_str());
                                                let active_label = active_label.clone();
                                                let tombstoned_label = tombstoned_label.clone();
                                                view! {
                                                    <button
                                                        type="button"
                                                        class=if is_selected {
                                                            "w-full rounded-xl border border-primary bg-primary/5 p-4 text-left"
                                                        } else {
                                                            "w-full rounded-xl border border-border p-4 text-left hover:bg-muted/50"
                                                        }
                                                        on:click={
                                                            let writer = list_query_writer.clone();
                                                            move |_| writer.apply_query_intent(core::memory_selection_intent(Some(&entry_id)))
                                                        }
                                                    >
                                                        <div class="flex flex-wrap items-center justify-between gap-2">
                                                            <span class="font-medium text-foreground">{format!("{}/{} | {}", entry.owner_slug, entry.resource_kind, entry.field_key)}</span>
                                                            <div class="flex gap-2">
                                                                <Badge variant=if entry.tombstoned_at.is_some() { BadgeVariant::Secondary } else { BadgeVariant::Success }>
                                                                    {if entry.tombstoned_at.is_some() { tombstoned_label.clone() } else { active_label.clone() }}
                                                                </Badge>
                                                                <Badge variant=BadgeVariant::Outline>{format!("v{}", entry.revision)}</Badge>
                                                            </div>
                                                        </div>
                                                        <p class="mt-2 text-xs text-muted-foreground">{format!("{} -> {} | {}", entry.source_locale, entry.target_locale, entry.resource_id)}</p>
                                                    </button>
                                                }
                                            }).collect_view().into_any()
                                        }}
                                    </CardContent>
                                </Card>

                                {match selected {
                                    Some(entry) => {
                                        let is_tombstoned = entry.tombstoned_at.is_some();
                                        view! {
                                        <div class="space-y-6">
                                            <Card>
                                                <CardHeader>
                                                    <CardTitle>{selected_title}</CardTitle>
                                                    <CardDescription>{entry.id.clone()}</CardDescription>
                                                </CardHeader>
                                                <CardContent class="space-y-4">
                                                    <div class="flex flex-wrap gap-2">
                                                        <Badge variant=BadgeVariant::Outline>{format!("{} -> {}", entry.source_locale, entry.target_locale)}</Badge>
                                                        <Badge variant=BadgeVariant::Outline>{entry.quality_state}</Badge>
                                                        <Badge variant=BadgeVariant::Outline>{format!("v{}", entry.revision)}</Badge>
                                                    </div>
                                                    <div>
                                                        <p class="text-xs font-medium uppercase tracking-wide text-muted-foreground">{source_text_label}</p>
                                                        <p class="mt-1 whitespace-pre-wrap text-sm text-foreground">{entry.source_text}</p>
                                                    </div>
                                                    <div>
                                                        <p class="text-xs font-medium uppercase tracking-wide text-muted-foreground">{target_text_label}</p>
                                                        <p class="mt-1 whitespace-pre-wrap text-sm text-foreground">{entry.target_text}</p>
                                                    </div>
                                                    <dl class="grid gap-3 text-xs sm:grid-cols-2">
                                                        <div><dt class="text-muted-foreground">{resource_label}</dt><dd class="mt-1 break-all font-mono">{format!("{}/{}/{}", entry.owner_slug, entry.resource_kind, entry.resource_id)}</dd></div>
                                                        <div><dt class="text-muted-foreground">{reviewer_label}</dt><dd class="mt-1 break-all font-mono">{format!("{}:{}", entry.reviewer_actor_kind, entry.reviewer_actor_id)}</dd></div>
                                                        <div><dt class="text-muted-foreground">{proposal_label}</dt><dd class="mt-1 break-all font-mono">{entry.proposal_id}</dd></div>
                                                        <div><dt class="text-muted-foreground">{apply_receipt_label}</dt><dd class="mt-1 break-all font-mono">{entry.apply_receipt_id}</dd></div>
                                                    </dl>
                                                </CardContent>
                                            </Card>
                                            <Card>
                                                <CardHeader>
                                                    <CardTitle>{lifecycle_title.clone()}</CardTitle>
                                                    <CardDescription>{lifecycle_description.clone()}</CardDescription>
                                                </CardHeader>
                                                <CardContent class="space-y-4">
                                                    <div class="grid gap-4 md:grid-cols-2">
                                                        <div class="space-y-2">
                                                            <Label required=true r#for="memory_retention_policy">{retention_policy_label}</Label>
                                                            <Select
                                                                options=vec![
                                                                    SelectOption::new("owner_lifecycle", owner_lifecycle_label),
                                                                    SelectOption::new("retain_until", retain_until_option_label),
                                                                    SelectOption::new("legal_hold", legal_hold_label),
                                                                ]
                                                                value=retention_policy
                                                                set_value=set_retention_policy
                                                                id="memory_retention_policy" name="memory_retention_policy"
                                                            />
                                                        </div>
                                                        <div class="space-y-2"><Label r#for="memory_retain_until">{retain_until_label}</Label><Input value=retain_until set_value=set_retain_until id="memory_retain_until" name="memory_retain_until" /></div>
                                                    </div>
                                                    <div class="flex flex-wrap gap-2">
                                                        <Button on_click=Box::new(move || retention_action.run(()))>{update_retention_label}</Button>
                                                        <Button variant=ButtonVariant::Secondary on_click=Box::new(move || tombstone_action.run(())) disabled=is_tombstoned>{tombstone_label}</Button>
                                                        {is_tombstoned.then(|| view! {
                                                            <Button variant=ButtonVariant::Destructive on_click=Box::new(move || purge_action.run(()))>{purge_label}</Button>
                                                        })}
                                                        <Button
                                                            variant=ButtonVariant::Outline
                                                            on_click=Box::new({
                                                                let writer = clear_query_writer.clone();
                                                                move || writer.apply_query_intent(core::memory_selection_intent(None))
                                                            })
                                                        >
                                                            {clear_label}
                                                        </Button>
                                                    </div>
                                                </CardContent>
                                            </Card>
                                        </div>
                                        }.into_any()
                                    }
                                    None => view! {
                                        <Card><CardContent class="p-8"><p class="text-sm text-muted-foreground">{no_selection.clone()}</p></CardContent></Card>
                                    }.into_any(),
                                }}
                            </div>
                        }.into_any(),
                        Err(error) => view! {
                            <Alert variant=AlertVariant::Destructive title=load_error.clone()>{error}</Alert>
                        }.into_any(),
                    })
                }}
            </Suspense>
            <Show when=move || busy.get()>
                <p class="text-xs text-muted-foreground">"Operation in progress..."</p>
            </Show>
            <OutcomePanel outcome locale=locale.clone() />
        </div>
    }
}

#[component]
fn InventoryTab(
    token: Signal<Option<String>>,
    tenant: Signal<Option<String>>,
    locale: Option<String>,
) -> impl IntoView {
    let (owner_slug, set_owner_slug) = signal("media".to_string());
    let (resource_kind, set_resource_kind) = signal("asset".to_string());
    let (source_locale, set_source_locale) = signal("en".to_string());
    let (target_locale, set_target_locale) = signal("de".to_string());
    let (limit, set_limit) = signal("100".to_string());
    let (page_size, set_page_size) = signal("100".to_string());
    let (busy, set_busy) = signal(false);
    let (outcome, set_outcome) = signal(OperationOutcome::None);
    let title = t(
        locale.as_deref(),
        "translation.inventory.title",
        "Provider inventory and coverage",
    );
    let description = t(
        locale.as_deref(),
        "translation.inventory.description",
        "Operate only through an owner-provided target contract; no owner tables are queried here.",
    );
    let owner_label = t(
        locale.as_deref(),
        "translation.field.ownerSlug",
        "Owner slug",
    );
    let kind_label = t(
        locale.as_deref(),
        "translation.field.resourceKind",
        "Resource kind",
    );
    let source_label = t(
        locale.as_deref(),
        "translation.field.sourceLocale",
        "Source locale",
    );
    let target_label = t(
        locale.as_deref(),
        "translation.field.targetLocale",
        "Target locale",
    );
    let limit_label = t(locale.as_deref(), "translation.field.limit", "Sync limit");
    let page_size_label = t(
        locale.as_deref(),
        "translation.field.pageSize",
        "Rebuild page size",
    );
    let sync_label = t(
        locale.as_deref(),
        "translation.action.syncInventory",
        "Sync changes",
    );
    let rebuild_label = t(
        locale.as_deref(),
        "translation.action.rebuildInventory",
        "Full rebuild",
    );
    let progress_label = t(
        locale.as_deref(),
        "translation.action.readCoverage",
        "Read exact coverage",
    );
    let required_label = t(
        locale.as_deref(),
        "translation.action.readRequiredCoverage",
        "Read required-target coverage",
    );

    let sync_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::sync_inventory_operation(
                    &owner_slug.get_untracked(),
                    &resource_kind.get_untracked(),
                    &limit.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(|_| {}),
            );
        }
    };
    let rebuild_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::rebuild_inventory_operation(
                    &owner_slug.get_untracked(),
                    &resource_kind.get_untracked(),
                    &source_locale.get_untracked(),
                    &target_locale.get_untracked(),
                    &page_size.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(|_| {}),
            );
        }
    };
    let progress_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::read_provider_progress_operation(
                    &owner_slug.get_untracked(),
                    &resource_kind.get_untracked(),
                    &source_locale.get_untracked(),
                    &target_locale.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(|_| {}),
            );
        }
    };
    let required_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::read_required_provider_progress_operation(
                    &owner_slug.get_untracked(),
                    &resource_kind.get_untracked(),
                    &source_locale.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(|_| {}),
            );
        }
    };

    view! {
        <div class="space-y-6">
            <Card>
                <CardHeader>
                    <CardTitle>{title}</CardTitle>
                    <CardDescription>{description}</CardDescription>
                </CardHeader>
                <CardContent class="space-y-4">
                    <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
                        <div class="space-y-2"><Label required=true r#for="owner_slug">{owner_label}</Label><Input value=owner_slug set_value=set_owner_slug id="owner_slug" name="owner_slug" /></div>
                        <div class="space-y-2"><Label required=true r#for="resource_kind">{kind_label}</Label><Input value=resource_kind set_value=set_resource_kind id="resource_kind" name="resource_kind" /></div>
                        <div class="space-y-2"><Label required=true r#for="source_locale">{source_label}</Label><Input value=source_locale set_value=set_source_locale id="source_locale" name="source_locale" /></div>
                        <div class="space-y-2"><Label required=true r#for="target_locale">{target_label}</Label><Input value=target_locale set_value=set_target_locale id="target_locale" name="target_locale" /></div>
                    </div>
                    <div class="grid gap-4 md:grid-cols-2">
                        <div class="space-y-2"><Label r#for="limit">{limit_label}</Label><Input value=limit set_value=set_limit id="limit" name="limit" /></div>
                        <div class="space-y-2"><Label r#for="page_size">{page_size_label}</Label><Input value=page_size set_value=set_page_size id="page_size" name="page_size" /></div>
                    </div>
                    <div class="flex flex-wrap gap-2">
                        <Button on_click=Box::new(sync_action)>{sync_label}</Button>
                        <Button variant=ButtonVariant::Secondary on_click=Box::new(rebuild_action)>{rebuild_label}</Button>
                        <Button variant=ButtonVariant::Outline on_click=Box::new(progress_action)>{progress_label}</Button>
                        <Button variant=ButtonVariant::Outline on_click=Box::new(required_action)>{required_label}</Button>
                    </div>
                    <Show when=move || busy.get()>
                        <p class="text-xs text-muted-foreground">"Operation in progress…"</p>
                    </Show>
                </CardContent>
            </Card>
            <OutcomePanel outcome locale=locale.clone() />
        </div>
    }
}

#[component]
fn WorkflowTab(
    token: Signal<Option<String>>,
    tenant: Signal<Option<String>>,
    locale: Option<String>,
) -> impl IntoView {
    let (job_id, set_job_id) = signal(String::new());
    let (owner_slug, set_owner_slug) = signal("media".to_string());
    let (resource_kind, set_resource_kind) = signal("asset".to_string());
    let (resource_id, set_resource_id) = signal(String::new());
    let (subresource_id, set_subresource_id) = signal(String::new());
    let (item_id, set_item_id) = signal(String::new());
    let (proposal_id, set_proposal_id) = signal(String::new());
    let (field_key, set_field_key) = signal("alt".to_string());
    let (field_value, set_field_value) = signal(String::new());
    let (machine_field_keys, set_machine_field_keys) = signal("alt".to_string());
    let (machine_minimum_similarity, set_machine_minimum_similarity) = signal("8500".to_string());
    let (machine_tone, set_machine_tone) = signal(String::new());
    let (machine_domain, set_machine_domain) = signal(String::new());
    let (machine_style, set_machine_style) = signal(String::new());
    let (machine_operation_id, set_machine_operation_id) = signal(String::new());
    let (machine_expected_updated_at, set_machine_expected_updated_at) = signal(String::new());
    let (machine_reason, set_machine_reason) = signal(String::new());
    let (item_revision, set_item_revision) = signal("1".to_string());
    let (assignee_kind, set_assignee_kind) = signal("user".to_string());
    let (assignee_id, set_assignee_id) = signal(String::new());
    let (item_retry_reason, set_item_retry_reason) = signal(String::new());
    let (job_revision, set_job_revision) = signal("1".to_string());
    let (apply_operation_id, set_apply_operation_id) = signal(String::new());
    let (expected_attempt_count, set_expected_attempt_count) = signal("1".to_string());
    let (recovery_reason, set_recovery_reason) = signal(String::new());
    let (workflow_note_body, set_workflow_note_body) = signal(String::new());
    let (workflow_note_limit, set_workflow_note_limit) = signal("50".to_string());
    let (workflow_note_include_resolved, set_workflow_note_include_resolved) = signal(false);
    let (workflow_notes, set_workflow_notes) = signal(Vec::<WorkflowNote>::new());
    let (busy, set_busy) = signal(false);
    let (outcome, set_outcome) = signal(OperationOutcome::None);
    let (add_key, set_add_key) = signal(core::new_idempotency_key("add-item"));
    let (save_key, set_save_key) = signal(core::new_idempotency_key("save-proposal"));
    let (submit_key, set_submit_key) = signal(core::new_idempotency_key("submit-proposal"));
    let (approve_key, set_approve_key) = signal(core::new_idempotency_key("approve-proposal"));
    let (apply_key, set_apply_key) = signal(core::new_idempotency_key("apply-proposal"));
    let (estimate_machine_key, set_estimate_machine_key) =
        signal(core::new_idempotency_key("estimate-machine-translation"));
    let (generate_machine_key, set_generate_machine_key) =
        signal(core::new_idempotency_key("generate-machine-proposal"));
    let (cancel_machine_key, set_cancel_machine_key) =
        signal(core::new_idempotency_key("cancel-machine-operation"));
    let (recover_machine_key, set_recover_machine_key) =
        signal(core::new_idempotency_key("recover-machine-operation"));
    let (assign_key, set_assign_key) = signal(core::new_idempotency_key("assign-item"));
    let (unassign_key, set_unassign_key) = signal(core::new_idempotency_key("unassign-item"));
    let (retry_key, set_retry_key) = signal(core::new_idempotency_key("retry-item"));
    let (cancel_job_key, set_cancel_job_key) = signal(core::new_idempotency_key("cancel-job"));
    let (recover_apply_key, set_recover_apply_key) =
        signal(core::new_idempotency_key("recover-apply"));
    let (create_workflow_note_key, set_create_workflow_note_key) =
        signal(core::new_idempotency_key("create-workflow-note"));
    let (resolve_workflow_note_key, set_resolve_workflow_note_key) =
        signal(core::new_idempotency_key("resolve-workflow-note"));

    let admit_title = t(
        locale.as_deref(),
        "translation.workflow.admit",
        "Admit owner resource",
    );
    let admit_description = t(
        locale.as_deref(),
        "translation.workflow.admitDescription",
        "Snapshot one provider-authorized resource into an existing job.",
    );
    let proposal_title = t(
        locale.as_deref(),
        "translation.workflow.proposal",
        "Manual proposal",
    );
    let proposal_description = t(
        locale.as_deref(),
        "translation.workflow.proposalDescription",
        "Save one exact field value; deterministic and owner QA run before review.",
    );
    let machine_title = t(
        locale.as_deref(),
        "translation.workflow.machine",
        "Machine translation",
    );
    let machine_description = t(
        locale.as_deref(),
        "translation.workflow.machineDescription",
        "Estimate the upper bound before creating a review-required machine proposal.",
    );
    let machine_control_title = t(
        locale.as_deref(),
        "translation.workflow.machineControl",
        "Machine operation control",
    );
    let machine_control_description = t(
        locale.as_deref(),
        "translation.workflow.machineControlDescription",
        "Read the durable operation state, cancel a request, or recover an observed stuck save.",
    );
    let assignment_title = t(
        locale.as_deref(),
        "translation.workflow.assignment",
        "Assignment and item recovery",
    );
    let assignment_description = t(
        locale.as_deref(),
        "translation.workflow.assignmentDescription",
        "Assign or unassign an item at its observed revision, or retry a blocked item with a reason.",
    );
    let job_control_title = t(
        locale.as_deref(),
        "translation.workflow.jobControl",
        "Job and owner apply control",
    );
    let job_control_description = t(
        locale.as_deref(),
        "translation.workflow.jobControlDescription",
        "Cancel a job at its observed revision or recover a durable owner-apply operation.",
    );
    let review_title = t(
        locale.as_deref(),
        "translation.workflow.review",
        "Review and owner apply",
    );
    let review_description = t(
        locale.as_deref(),
        "translation.workflow.reviewDescription",
        "Each transition is explicit, idempotent, and never retries through another protocol.",
    );
    let workflow_notes_title = t(
        locale.as_deref(),
        "translation.workflow.notes",
        "Private workflow notes",
    );
    let workflow_notes_description = t(
        locale.as_deref(),
        "translation.workflow.notesDescription",
        "Leave private job or item context for translators and reviewers; note bodies never enter memory, AI, owner data, or events.",
    );
    let job_id_label = t(locale.as_deref(), "translation.field.jobId", "Job ID");
    let job_control_job_id_label = job_id_label.clone();
    let workflow_note_job_id_label = job_id_label.clone();
    let resource_id_label = t(
        locale.as_deref(),
        "translation.field.resourceId",
        "Resource ID",
    );
    let owner_label = t(
        locale.as_deref(),
        "translation.field.ownerSlug",
        "Owner slug",
    );
    let kind_label = t(
        locale.as_deref(),
        "translation.field.resourceKind",
        "Resource kind",
    );
    let subresource_label = t(
        locale.as_deref(),
        "translation.field.subresourceId",
        "Subresource ID",
    );
    let item_id_label = t(locale.as_deref(), "translation.field.itemId", "Item ID");
    let review_item_id_label = item_id_label.clone();
    let machine_item_id_label = item_id_label.clone();
    let assignment_item_id_label = item_id_label.clone();
    let item_revision_label = t(
        locale.as_deref(),
        "translation.field.itemRevision",
        "Observed item revision",
    );
    let assignee_kind_label = t(
        locale.as_deref(),
        "translation.field.assigneeKind",
        "Assignee kind",
    );
    let assignee_id_label = t(
        locale.as_deref(),
        "translation.field.assigneeId",
        "Assignee ID",
    );
    let assignee_user_label = t(locale.as_deref(), "translation.field.assigneeUser", "User");
    let assignee_service_label = t(
        locale.as_deref(),
        "translation.field.assigneeService",
        "Service",
    );
    let field_key_label = t(locale.as_deref(), "translation.field.fieldKey", "Field key");
    let field_keys_label = t(
        locale.as_deref(),
        "translation.field.fieldKeys",
        "Field keys (comma-separated)",
    );
    let minimum_memory_similarity_label = t(
        locale.as_deref(),
        "translation.field.minimumMemorySimilarity",
        "Minimum memory similarity (basis points)",
    );
    let tone_label = t(locale.as_deref(), "translation.field.tone", "Tone");
    let domain_label = t(locale.as_deref(), "translation.field.domain", "Domain");
    let style_label = t(locale.as_deref(), "translation.field.style", "Style");
    let operation_id_label = t(
        locale.as_deref(),
        "translation.field.operationId",
        "Operation ID",
    );
    let apply_operation_id_label = operation_id_label.clone();
    let expected_updated_at_label = t(
        locale.as_deref(),
        "translation.field.expectedUpdatedAt",
        "Observed updated at (RFC 3339)",
    );
    let machine_reason_label = t(locale.as_deref(), "translation.field.reason", "Reason");
    let item_retry_reason_label = machine_reason_label.clone();
    let recovery_reason_label = machine_reason_label.clone();
    let job_revision_label = t(
        locale.as_deref(),
        "translation.field.jobRevision",
        "Observed job revision",
    );
    let expected_attempt_count_label = t(
        locale.as_deref(),
        "translation.field.expectedAttemptCount",
        "Expected apply attempt count",
    );
    let value_label = t(
        locale.as_deref(),
        "translation.field.value",
        "Translated value",
    );
    let proposal_id_label = t(
        locale.as_deref(),
        "translation.field.proposalId",
        "Proposal ID",
    );
    let workflow_note_body_label = t(
        locale.as_deref(),
        "translation.field.workflowNoteBody",
        "Workflow note",
    );
    let workflow_note_item_label = t(
        locale.as_deref(),
        "translation.field.workflowNoteItemId",
        "Item ID (optional)",
    );
    let workflow_note_limit_label = t(
        locale.as_deref(),
        "translation.field.workflowNoteLimit",
        "Notes to load",
    );
    let workflow_note_include_resolved_label = t(
        locale.as_deref(),
        "translation.field.includeResolved",
        "Include resolved",
    );
    let add_label = t(locale.as_deref(), "translation.action.addItem", "Add item");
    let save_label = t(
        locale.as_deref(),
        "translation.action.saveProposal",
        "Save proposal",
    );
    let submit_label = t(
        locale.as_deref(),
        "translation.action.submitProposal",
        "Submit for review",
    );
    let approve_label = t(
        locale.as_deref(),
        "translation.action.approveProposal",
        "Approve",
    );
    let apply_label = t(
        locale.as_deref(),
        "translation.action.applyProposal",
        "Apply through owner",
    );
    let estimate_machine_label = t(
        locale.as_deref(),
        "translation.action.estimateMachineTranslation",
        "Estimate machine translation",
    );
    let generate_machine_label = t(
        locale.as_deref(),
        "translation.action.generateMachineProposal",
        "Generate machine proposal",
    );
    let read_machine_status_label = t(
        locale.as_deref(),
        "translation.action.readMachineStatus",
        "Read machine status",
    );
    let cancel_machine_label = t(
        locale.as_deref(),
        "translation.action.cancelMachineOperation",
        "Cancel machine operation",
    );
    let recover_machine_label = t(
        locale.as_deref(),
        "translation.action.recoverMachineOperation",
        "Recover machine operation",
    );
    let assign_label = t(
        locale.as_deref(),
        "translation.action.assignItem",
        "Assign item",
    );
    let unassign_label = t(
        locale.as_deref(),
        "translation.action.unassignItem",
        "Unassign item",
    );
    let retry_label = t(
        locale.as_deref(),
        "translation.action.retryItem",
        "Retry item",
    );
    let cancel_job_label = t(
        locale.as_deref(),
        "translation.action.cancelJob",
        "Cancel job",
    );
    let recover_apply_label = t(
        locale.as_deref(),
        "translation.action.recoverApply",
        "Recover owner apply",
    );
    let load_workflow_notes_label = t(
        locale.as_deref(),
        "translation.action.loadWorkflowNotes",
        "Load notes",
    );
    let create_workflow_note_label = t(
        locale.as_deref(),
        "translation.action.createWorkflowNote",
        "Add private note",
    );
    let resolve_workflow_note_label = t(
        locale.as_deref(),
        "translation.action.resolveWorkflowNote",
        "Resolve note",
    );
    let workflow_notes_empty_label = t(
        locale.as_deref(),
        "translation.workflow.notesEmpty",
        "No workflow notes have been loaded.",
    );
    let workflow_note_open_label = t(locale.as_deref(), "translation.workflow.noteOpen", "open");
    let workflow_note_resolved_label = t(
        locale.as_deref(),
        "translation.workflow.noteResolved",
        "resolved",
    );

    let add_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::add_item_operation(
                    &job_id.get_untracked(),
                    &owner_slug.get_untracked(),
                    &resource_kind.get_untracked(),
                    &resource_id.get_untracked(),
                    &subresource_id.get_untracked(),
                    &add_key.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(move |response| {
                    set_add_key.set(core::new_idempotency_key("add-item"));
                    if let TranslationAdminResponse::Item(item) = response {
                        set_job_id.set(item.job_id);
                        set_item_id.set(item.id);
                        set_item_revision.set(item.revision.to_string());
                    }
                }),
            );
        }
    };
    let save_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::save_proposal_operation(
                    &item_id.get_untracked(),
                    &field_key.get_untracked(),
                    &field_value.get_untracked(),
                    &save_key.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(move |_| {
                    set_save_key.set(core::new_idempotency_key("save-proposal"));
                }),
            );
        }
    };
    let submit_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::proposal_command_operation(
                    ProposalCommand::Submit,
                    &item_id.get_untracked(),
                    &proposal_id.get_untracked(),
                    &submit_key.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(move |_| {
                    set_submit_key.set(core::new_idempotency_key("submit-proposal"));
                }),
            );
        }
    };
    let approve_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::proposal_command_operation(
                    ProposalCommand::Approve,
                    &item_id.get_untracked(),
                    &proposal_id.get_untracked(),
                    &approve_key.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(move |_| {
                    set_approve_key.set(core::new_idempotency_key("approve-proposal"));
                }),
            );
        }
    };
    let apply_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::proposal_command_operation(
                    ProposalCommand::Apply,
                    &item_id.get_untracked(),
                    &proposal_id.get_untracked(),
                    &apply_key.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(move |response| {
                    set_apply_key.set(core::new_idempotency_key("apply-proposal"));
                    if let TranslationAdminResponse::Apply(apply) = response {
                        set_apply_operation_id.set(apply.operation_id);
                        set_item_id.set(apply.item_id);
                        set_proposal_id.set(apply.proposal_id);
                    }
                }),
            );
        }
    };
    let estimate_machine_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::machine_proposal_operation(
                    core::MachineProposalCommand::Estimate,
                    core::MachineProposalInput {
                        item_id: &item_id.get_untracked(),
                        field_keys: &machine_field_keys.get_untracked(),
                        minimum_memory_similarity_basis_points: &machine_minimum_similarity
                            .get_untracked(),
                        tone: &machine_tone.get_untracked(),
                        domain: &machine_domain.get_untracked(),
                        style: &machine_style.get_untracked(),
                    },
                    &estimate_machine_key.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(move |_| {
                    set_estimate_machine_key
                        .set(core::new_idempotency_key("estimate-machine-translation"));
                }),
            );
        }
    };
    let generate_machine_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::machine_proposal_operation(
                    core::MachineProposalCommand::Generate,
                    core::MachineProposalInput {
                        item_id: &item_id.get_untracked(),
                        field_keys: &machine_field_keys.get_untracked(),
                        minimum_memory_similarity_basis_points: &machine_minimum_similarity
                            .get_untracked(),
                        tone: &machine_tone.get_untracked(),
                        domain: &machine_domain.get_untracked(),
                        style: &machine_style.get_untracked(),
                    },
                    &generate_machine_key.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(move |response| {
                    set_generate_machine_key
                        .set(core::new_idempotency_key("generate-machine-proposal"));
                    if let TranslationAdminResponse::MachineProposal(proposal) = response {
                        set_machine_operation_id.set(proposal.operation_id);
                        set_machine_expected_updated_at.set(proposal.updated_at);
                        set_proposal_id.set(proposal.proposal_id);
                    }
                }),
            );
        }
    };
    let read_machine_status_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::read_machine_operation_status_operation(
                    &machine_operation_id.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(move |response| {
                    if let TranslationAdminResponse::MachineOperationStatus(status) = response {
                        set_machine_operation_id.set(status.operation_id);
                        set_machine_expected_updated_at.set(status.updated_at);
                        set_item_id.set(status.item_id);
                    }
                }),
            );
        }
    };
    let cancel_machine_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::cancel_machine_operation(core::MachineCancellationInput {
                    operation_id: &machine_operation_id.get_untracked(),
                    reason: &machine_reason.get_untracked(),
                    idempotency_key: &cancel_machine_key.get_untracked(),
                }),
                set_busy,
                set_outcome,
                Callback::new(move |_| {
                    set_cancel_machine_key
                        .set(core::new_idempotency_key("cancel-machine-operation"));
                }),
            );
        }
    };
    let recover_machine_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::recover_machine_operation(core::MachineRecoveryInput {
                    operation_id: &machine_operation_id.get_untracked(),
                    expected_updated_at: &machine_expected_updated_at.get_untracked(),
                    proposal: core::MachineProposalInput {
                        item_id: &item_id.get_untracked(),
                        field_keys: &machine_field_keys.get_untracked(),
                        minimum_memory_similarity_basis_points: &machine_minimum_similarity
                            .get_untracked(),
                        tone: &machine_tone.get_untracked(),
                        domain: &machine_domain.get_untracked(),
                        style: &machine_style.get_untracked(),
                    },
                    reason: &machine_reason.get_untracked(),
                    idempotency_key: &recover_machine_key.get_untracked(),
                }),
                set_busy,
                set_outcome,
                Callback::new(move |response| {
                    set_recover_machine_key
                        .set(core::new_idempotency_key("recover-machine-operation"));
                    if let TranslationAdminResponse::MachineProposal(proposal) = response {
                        set_machine_operation_id.set(proposal.operation_id);
                        set_machine_expected_updated_at.set(proposal.updated_at);
                        set_proposal_id.set(proposal.proposal_id);
                    }
                }),
            );
        }
    };
    let assign_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::assign_item_operation(core::AssignmentInput {
                    item_id: &item_id.get_untracked(),
                    expected_revision: &item_revision.get_untracked(),
                    assignee_kind: &assignee_kind.get_untracked(),
                    assignee_id: &assignee_id.get_untracked(),
                    idempotency_key: &assign_key.get_untracked(),
                }),
                set_busy,
                set_outcome,
                Callback::new(move |response| {
                    set_assign_key.set(core::new_idempotency_key("assign-item"));
                    if let TranslationAdminResponse::Assignment(assignment) = response {
                        set_item_id.set(assignment.item_id);
                        set_item_revision.set(assignment.item_revision.to_string());
                        match assignment.assignee {
                            Some(assignee) => {
                                set_assignee_kind.set(
                                    match assignee.kind {
                                        ActorKind::User => "user",
                                        ActorKind::Service => "service",
                                    }
                                    .to_string(),
                                );
                                set_assignee_id.set(assignee.id);
                            }
                            None => set_assignee_id.set(String::new()),
                        }
                    }
                }),
            );
        }
    };
    let unassign_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::unassign_item_operation(core::UnassignmentInput {
                    item_id: &item_id.get_untracked(),
                    expected_revision: &item_revision.get_untracked(),
                    idempotency_key: &unassign_key.get_untracked(),
                }),
                set_busy,
                set_outcome,
                Callback::new(move |response| {
                    set_unassign_key.set(core::new_idempotency_key("unassign-item"));
                    if let TranslationAdminResponse::Assignment(assignment) = response {
                        set_item_id.set(assignment.item_id);
                        set_item_revision.set(assignment.item_revision.to_string());
                        set_assignee_id.set(String::new());
                    }
                }),
            );
        }
    };
    let retry_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::retry_item_operation(core::ItemRetryInput {
                    item_id: &item_id.get_untracked(),
                    expected_revision: &item_revision.get_untracked(),
                    reason: &item_retry_reason.get_untracked(),
                    idempotency_key: &retry_key.get_untracked(),
                }),
                set_busy,
                set_outcome,
                Callback::new(move |response| {
                    set_retry_key.set(core::new_idempotency_key("retry-item"));
                    if let TranslationAdminResponse::Retry(retry) = response {
                        set_item_id.set(retry.item_id);
                        set_item_revision.set(retry.item_revision.to_string());
                    }
                }),
            );
        }
    };
    let cancel_job_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::cancel_job_operation(core::JobCancellationInput {
                    job_id: &job_id.get_untracked(),
                    expected_revision: &job_revision.get_untracked(),
                    reason: &recovery_reason.get_untracked(),
                    idempotency_key: &cancel_job_key.get_untracked(),
                }),
                set_busy,
                set_outcome,
                Callback::new(move |response| {
                    set_cancel_job_key.set(core::new_idempotency_key("cancel-job"));
                    if let TranslationAdminResponse::Cancellation(cancellation) = response {
                        set_job_id.set(cancellation.job_id);
                        set_job_revision.set(cancellation.job_revision.to_string());
                    }
                }),
            );
        }
    };
    let recover_apply_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::recover_apply_operation(core::ApplyRecoveryInput {
                    operation_id: &apply_operation_id.get_untracked(),
                    expected_attempt_count: &expected_attempt_count.get_untracked(),
                    reason: &recovery_reason.get_untracked(),
                    idempotency_key: &recover_apply_key.get_untracked(),
                }),
                set_busy,
                set_outcome,
                Callback::new(move |response| {
                    set_recover_apply_key.set(core::new_idempotency_key("recover-apply"));
                    if let TranslationAdminResponse::Apply(apply) = response {
                        set_apply_operation_id.set(apply.operation_id);
                        set_item_id.set(apply.item_id);
                        set_proposal_id.set(apply.proposal_id);
                    }
                }),
            );
        }
    };
    let load_workflow_notes_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::list_workflow_notes_operation(core::WorkflowNotesOperationInput {
                    job_id: &job_id.get_untracked(),
                    item_id: &item_id.get_untracked(),
                    include_resolved: workflow_note_include_resolved.get_untracked(),
                    limit: &workflow_note_limit.get_untracked(),
                }),
                set_busy,
                set_outcome,
                Callback::new(move |response| {
                    if let TranslationAdminResponse::WorkflowNotes(notes) = response {
                        set_workflow_notes.set(notes);
                    }
                }),
            );
        }
    };
    let create_workflow_note_action = {
        let locale = locale.clone();
        move || {
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::create_workflow_note_operation(core::CreateWorkflowNoteOperationInput {
                    job_id: &job_id.get_untracked(),
                    item_id: &item_id.get_untracked(),
                    body: &workflow_note_body.get_untracked(),
                    idempotency_key: &create_workflow_note_key.get_untracked(),
                }),
                set_busy,
                set_outcome,
                Callback::new(move |response| {
                    set_create_workflow_note_key
                        .set(core::new_idempotency_key("create-workflow-note"));
                    if let TranslationAdminResponse::WorkflowNote(note) = response {
                        set_workflow_note_body.set(String::new());
                        set_workflow_notes.update(|notes| {
                            notes.retain(|existing| existing.id != note.id);
                            notes.insert(0, note);
                        });
                    }
                }),
            );
        }
    };
    let resolve_workflow_note_action = {
        let locale = locale.clone();
        Callback::new(move |(note_id, expected_revision): (String, i64)| {
            let expected_revision = expected_revision.to_string();
            run_operation(
                core::transport_context(token.get(), tenant.get(), locale.clone()),
                core::resolve_workflow_note_operation(
                    &note_id,
                    &expected_revision,
                    &resolve_workflow_note_key.get_untracked(),
                ),
                set_busy,
                set_outcome,
                Callback::new(move |response| {
                    set_resolve_workflow_note_key
                        .set(core::new_idempotency_key("resolve-workflow-note"));
                    if let TranslationAdminResponse::WorkflowNote(note) = response {
                        set_workflow_notes.update(|notes| {
                            if let Some(existing) =
                                notes.iter_mut().find(|existing| existing.id == note.id)
                            {
                                *existing = note;
                            } else {
                                notes.insert(0, note);
                            }
                        });
                    }
                }),
            );
        })
    };

    view! {
        <div class="space-y-6">
            <div class="grid gap-6 xl:grid-cols-2">
                <Card>
                    <CardHeader>
                        <CardTitle>{admit_title}</CardTitle>
                        <CardDescription>{admit_description}</CardDescription>
                    </CardHeader>
                    <CardContent class="space-y-4">
                        <div class="grid gap-4 sm:grid-cols-2">
                            <div class="space-y-2"><Label required=true r#for="job_id">{job_id_label.clone()}</Label><Input value=job_id set_value=set_job_id id="job_id" name="job_id" /></div>
                            <div class="space-y-2"><Label required=true r#for="resource_id">{resource_id_label}</Label><Input value=resource_id set_value=set_resource_id id="resource_id" name="resource_id" /></div>
                            <div class="space-y-2"><Label required=true r#for="owner_slug">{owner_label}</Label><Input value=owner_slug set_value=set_owner_slug id="owner_slug" name="owner_slug" /></div>
                            <div class="space-y-2"><Label required=true r#for="resource_kind">{kind_label}</Label><Input value=resource_kind set_value=set_resource_kind id="resource_kind" name="resource_kind" /></div>
                            <div class="space-y-2 sm:col-span-2"><Label r#for="subresource_id">{subresource_label}</Label><Input value=subresource_id set_value=set_subresource_id id="subresource_id" name="subresource_id" /></div>
                        </div>
                        <Button on_click=Box::new(add_action)>{add_label}</Button>
                    </CardContent>
                </Card>

                <Card>
                    <CardHeader>
                        <CardTitle>{proposal_title}</CardTitle>
                        <CardDescription>{proposal_description}</CardDescription>
                    </CardHeader>
                    <CardContent class="space-y-4">
                        <div class="grid gap-4 sm:grid-cols-2">
                            <div class="space-y-2"><Label required=true r#for="item_id">{item_id_label}</Label><Input value=item_id set_value=set_item_id id="item_id" name="item_id" /></div>
                            <div class="space-y-2"><Label required=true r#for="field_key">{field_key_label}</Label><Input value=field_key set_value=set_field_key id="field_key" name="field_key" /></div>
                            <div class="space-y-2 sm:col-span-2"><Label required=true r#for="field_value">{value_label}</Label><Textarea value=field_value set_value=set_field_value id="field_value" name="field_value" rows=5 /></div>
                        </div>
                        <Button on_click=Box::new(save_action)>{save_label}</Button>
                    </CardContent>
                </Card>

                <Card>
                    <CardHeader>
                        <CardTitle>{machine_title}</CardTitle>
                        <CardDescription>{machine_description}</CardDescription>
                    </CardHeader>
                    <CardContent class="space-y-4">
                        <div class="grid gap-4 sm:grid-cols-2">
                            <div class="space-y-2"><Label required=true r#for="machine_item_id">{machine_item_id_label}</Label><Input value=item_id set_value=set_item_id id="machine_item_id" name="machine_item_id" /></div>
                            <div class="space-y-2"><Label required=true r#for="machine_field_keys">{field_keys_label}</Label><Input value=machine_field_keys set_value=set_machine_field_keys id="machine_field_keys" name="machine_field_keys" /></div>
                            <div class="space-y-2"><Label required=true r#for="machine_minimum_similarity">{minimum_memory_similarity_label}</Label><Input value=machine_minimum_similarity set_value=set_machine_minimum_similarity id="machine_minimum_similarity" name="machine_minimum_similarity" /></div>
                            <div class="space-y-2"><Label r#for="machine_tone">{tone_label}</Label><Input value=machine_tone set_value=set_machine_tone id="machine_tone" name="machine_tone" /></div>
                            <div class="space-y-2"><Label r#for="machine_domain">{domain_label}</Label><Input value=machine_domain set_value=set_machine_domain id="machine_domain" name="machine_domain" /></div>
                            <div class="space-y-2"><Label r#for="machine_style">{style_label}</Label><Input value=machine_style set_value=set_machine_style id="machine_style" name="machine_style" /></div>
                        </div>
                        <div class="flex flex-wrap gap-2">
                            <Button variant=ButtonVariant::Outline on_click=Box::new(estimate_machine_action)>{estimate_machine_label}</Button>
                            <Button on_click=Box::new(generate_machine_action)>{generate_machine_label}</Button>
                        </div>
                    </CardContent>
                </Card>

                <Card>
                    <CardHeader>
                        <CardTitle>{assignment_title}</CardTitle>
                        <CardDescription>{assignment_description}</CardDescription>
                    </CardHeader>
                    <CardContent class="space-y-4">
                        <div class="grid gap-4 sm:grid-cols-2">
                            <div class="space-y-2"><Label required=true r#for="assignment_item_id">{assignment_item_id_label}</Label><Input value=item_id set_value=set_item_id id="assignment_item_id" name="assignment_item_id" /></div>
                            <div class="space-y-2"><Label required=true r#for="item_revision">{item_revision_label}</Label><Input value=item_revision set_value=set_item_revision id="item_revision" name="item_revision" /></div>
                            <div class="space-y-2">
                                <Label r#for="assignee_kind">{assignee_kind_label}</Label>
                                <Select
                                    options=vec![
                                        SelectOption::new("user", assignee_user_label),
                                        SelectOption::new("service", assignee_service_label),
                                    ]
                                    value=assignee_kind
                                    set_value=set_assignee_kind
                                    id="assignee_kind" name="assignee_kind"
                                />
                            </div>
                            <div class="space-y-2"><Label r#for="assignee_id">{assignee_id_label}</Label><Input value=assignee_id set_value=set_assignee_id id="assignee_id" name="assignee_id" /></div>
                            <div class="space-y-2 sm:col-span-2"><Label r#for="item_retry_reason">{item_retry_reason_label}</Label><Textarea value=item_retry_reason set_value=set_item_retry_reason id="item_retry_reason" name="item_retry_reason" rows=3 /></div>
                        </div>
                        <div class="flex flex-wrap gap-2">
                            <Button on_click=Box::new(assign_action)>{assign_label}</Button>
                            <Button variant=ButtonVariant::Secondary on_click=Box::new(unassign_action)>{unassign_label}</Button>
                            <Button variant=ButtonVariant::Outline on_click=Box::new(retry_action)>{retry_label}</Button>
                        </div>
                    </CardContent>
                </Card>
            </div>

            <Card>
                <CardHeader>
                    <CardTitle>{machine_control_title}</CardTitle>
                    <CardDescription>{machine_control_description}</CardDescription>
                </CardHeader>
                <CardContent class="space-y-4">
                    <div class="grid gap-4 sm:grid-cols-2">
                        <div class="space-y-2"><Label required=true r#for="machine_operation_id">{operation_id_label}</Label><Input value=machine_operation_id set_value=set_machine_operation_id id="machine_operation_id" name="machine_operation_id" /></div>
                        <div class="space-y-2"><Label r#for="machine_expected_updated_at">{expected_updated_at_label}</Label><Input value=machine_expected_updated_at set_value=set_machine_expected_updated_at id="machine_expected_updated_at" name="machine_expected_updated_at" /></div>
                        <div class="space-y-2 sm:col-span-2"><Label r#for="machine_reason">{machine_reason_label}</Label><Textarea value=machine_reason set_value=set_machine_reason id="machine_reason" name="machine_reason" rows=3 /></div>
                    </div>
                    <div class="flex flex-wrap gap-2">
                        <Button variant=ButtonVariant::Outline on_click=Box::new(read_machine_status_action)>{read_machine_status_label}</Button>
                        <Button variant=ButtonVariant::Secondary on_click=Box::new(cancel_machine_action)>{cancel_machine_label}</Button>
                        <Button on_click=Box::new(recover_machine_action)>{recover_machine_label}</Button>
                    </div>
                </CardContent>
            </Card>

            <Card>
                <CardHeader>
                    <CardTitle>{job_control_title}</CardTitle>
                    <CardDescription>{job_control_description}</CardDescription>
                </CardHeader>
                <CardContent class="space-y-4">
                    <div class="grid gap-4 sm:grid-cols-2">
                        <div class="space-y-2"><Label r#for="job_control_id">{job_control_job_id_label}</Label><Input value=job_id set_value=set_job_id id="job_control_id" name="job_control_id" /></div>
                        <div class="space-y-2"><Label r#for="job_revision">{job_revision_label}</Label><Input value=job_revision set_value=set_job_revision id="job_revision" name="job_revision" /></div>
                        <div class="space-y-2"><Label r#for="apply_operation_id">{apply_operation_id_label}</Label><Input value=apply_operation_id set_value=set_apply_operation_id id="apply_operation_id" name="apply_operation_id" /></div>
                        <div class="space-y-2"><Label r#for="expected_attempt_count">{expected_attempt_count_label}</Label><Input value=expected_attempt_count set_value=set_expected_attempt_count id="expected_attempt_count" name="expected_attempt_count" /></div>
                        <div class="space-y-2 sm:col-span-2"><Label required=true r#for="recovery_reason">{recovery_reason_label}</Label><Textarea value=recovery_reason set_value=set_recovery_reason id="recovery_reason" name="recovery_reason" rows=3 /></div>
                    </div>
                    <div class="flex flex-wrap gap-2">
                        <Button variant=ButtonVariant::Secondary on_click=Box::new(cancel_job_action)>{cancel_job_label}</Button>
                        <Button on_click=Box::new(recover_apply_action)>{recover_apply_label}</Button>
                    </div>
                </CardContent>
            </Card>

            <Card>
                <CardHeader>
                    <CardTitle>{review_title}</CardTitle>
                    <CardDescription>{review_description}</CardDescription>
                </CardHeader>
                <CardContent class="space-y-4">
                    <div class="grid gap-4 sm:grid-cols-2">
                        <div class="space-y-2"><Label required=true r#for="review_item_id">{review_item_id_label}</Label><Input value=item_id set_value=set_item_id id="review_item_id" name="review_item_id" /></div>
                        <div class="space-y-2"><Label required=true r#for="proposal_id">{proposal_id_label}</Label><Input value=proposal_id set_value=set_proposal_id id="proposal_id" name="proposal_id" /></div>
                    </div>
                    <div class="flex flex-wrap gap-2">
                        <Button variant=ButtonVariant::Outline on_click=Box::new(submit_action)>{submit_label}</Button>
                        <Button variant=ButtonVariant::Secondary on_click=Box::new(approve_action)>{approve_label}</Button>
                        <Button on_click=Box::new(apply_action)>{apply_label}</Button>
                    </div>
                    <Show when=move || busy.get()>
                        <p class="text-xs text-muted-foreground">"Operation in progress…"</p>
                    </Show>
                </CardContent>
            </Card>

            <Card>
                <CardHeader>
                    <CardTitle>{workflow_notes_title}</CardTitle>
                    <CardDescription>{workflow_notes_description}</CardDescription>
                </CardHeader>
                <CardContent class="space-y-4">
                    <div class="grid gap-4 sm:grid-cols-2">
                        <div class="space-y-2"><Label required=true r#for="workflow_note_job_id">{workflow_note_job_id_label}</Label><Input value=job_id set_value=set_job_id id="workflow_note_job_id" name="workflow_note_job_id" /></div>
                        <div class="space-y-2"><Label r#for="workflow_note_item_id">{workflow_note_item_label}</Label><Input value=item_id set_value=set_item_id id="workflow_note_item_id" name="workflow_note_item_id" /></div>
                        <div class="space-y-2"><Label r#for="workflow_note_limit">{workflow_note_limit_label}</Label><Input value=workflow_note_limit set_value=set_workflow_note_limit id="workflow_note_limit" name="workflow_note_limit" /></div>
                        <label class="flex items-end gap-2 pb-1 text-sm text-foreground"><Checkbox checked=workflow_note_include_resolved set_checked=set_workflow_note_include_resolved name="workflow_note_include_resolved" />{workflow_note_include_resolved_label}</label>
                        <div class="space-y-2 sm:col-span-2"><Label required=true r#for="workflow_note_body">{workflow_note_body_label}</Label><Textarea value=workflow_note_body set_value=set_workflow_note_body id="workflow_note_body" name="workflow_note_body" rows=4 /></div>
                    </div>
                    <div class="flex flex-wrap gap-2">
                        <Button variant=ButtonVariant::Outline on_click=Box::new(load_workflow_notes_action)>{load_workflow_notes_label}</Button>
                        <Button on_click=Box::new(create_workflow_note_action)>{create_workflow_note_label}</Button>
                    </div>
                    {move || {
                        let notes = workflow_notes.get();
                        if notes.is_empty() {
                            view! {
                                <p class="rounded-xl border border-dashed border-border p-4 text-sm text-muted-foreground">
                                    {workflow_notes_empty_label.clone()}
                                </p>
                            }
                            .into_any()
                        } else {
                            let resolve_action = resolve_workflow_note_action;
                            let resolve_label = resolve_workflow_note_label.clone();
                            let open_label = workflow_note_open_label.clone();
                            let resolved_label = workflow_note_resolved_label.clone();
                            notes
                                .into_iter()
                                .map(move |note| {
                                    let note_id = note.id.clone();
                                    let note_revision = note.revision;
                                    let is_resolved = note.resolved_at.is_some();
                                    let state = if is_resolved {
                                        resolved_label.clone()
                                    } else {
                                        open_label.clone()
                                    };
                                    let scope_id = note
                                        .item_id
                                        .clone()
                                        .unwrap_or_else(|| note.job_id.clone());
                                    let author_kind = match note.author.kind {
                                        ActorKind::User => "user",
                                        ActorKind::Service => "service",
                                    };
                                    let author = format!("{author_kind}:{}", note.author.id);
                                    let action = resolve_action;
                                    let resolve_label = resolve_label.clone();
                                    view! {
                                        <article class="space-y-3 rounded-xl border border-border p-4">
                                            <div class="flex flex-wrap items-center justify-between gap-2">
                                                <div class="flex flex-wrap items-center gap-2">
                                                    <Badge variant=if is_resolved { BadgeVariant::Secondary } else { BadgeVariant::Outline }>{state}</Badge>
                                                    <span class="font-mono text-xs text-muted-foreground">{scope_id}</span>
                                                </div>
                                                <span class="text-xs text-muted-foreground">{format!("{author} · {}", note.created_at)}</span>
                                            </div>
                                            <p class="whitespace-pre-wrap text-sm text-foreground">{note.body}</p>
                                            {(!is_resolved).then(|| {
                                                let note_id = note_id.clone();
                                                let resolve_label = resolve_label.clone();
                                                view! {
                                                    <Button variant=ButtonVariant::Outline on_click=Box::new(move || action.run((note_id.clone(), note_revision)))>{resolve_label}</Button>
                                                }
                                            })}
                                        </article>
                                    }
                                })
                                .collect_view()
                                .into_any()
                        }
                    }}
                </CardContent>
            </Card>
            <OutcomePanel outcome locale=locale.clone() />
        </div>
    }
}

#[component]
fn OutcomePanel(outcome: ReadSignal<OperationOutcome>, locale: Option<String>) -> impl IntoView {
    view! {
        {move || {
            let locale = locale.clone();
            outcome.get().map(|result| match result {
                Ok(response) => {
                    let receipt = operation_receipt_view_model(&response);
                    let title = t(
                        locale.as_deref(),
                        receipt.title_key,
                        receipt.fallback_title,
                    );
                    let facts_locale = locale.clone();
                    view! {
                        <Alert variant=AlertVariant::Success title=title>
                            <dl class="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
                                {receipt.facts.into_iter().map(|fact| {
                                    let label = t(
                                        facts_locale.as_deref(),
                                        fact.label_key,
                                        fact.fallback_label,
                                    );
                                    view! {
                                        <div>
                                            <dt class="text-xs font-medium uppercase tracking-wide opacity-70">
                                                {label}
                                            </dt>
                                            <dd class="mt-1 break-all font-mono text-xs">{fact.value}</dd>
                                        </div>
                                    }
                                }).collect_view()}
                            </dl>
                        </Alert>
                    }.into_any()
                }
                Err(error) => {
                    let title = t(
                        locale.as_deref(),
                        "translation.error.operation",
                        "Operation failed",
                    );
                    view! {
                        <Alert variant=AlertVariant::Destructive title=title>
                            {error}
                        </Alert>
                    }.into_any()
                }
            })
        }}
    }
}

fn run_operation(
    context: TranslationAdminTransportContext,
    operation: Result<TranslationAdminOperation, core::CommandInputError>,
    set_busy: WriteSignal<bool>,
    set_outcome: WriteSignal<OperationOutcome>,
    on_success: Callback<TranslationAdminResponse>,
) {
    let operation = match operation {
        Ok(operation) => operation,
        Err(error) => {
            set_outcome.set(Some(Err(error.to_string())));
            return;
        }
    };

    if set_busy.try_update(|busy| {
        if *busy {
            false
        } else {
            *busy = true;
            true
        }
    }) != Some(true)
    {
        return;
    }

    spawn_local(async move {
        match transport::execute(context, operation).await {
            Ok(response) => {
                on_success.run(response.clone());
                set_outcome.set(Some(Ok(response)));
            }
            Err(error) => set_outcome.set(Some(Err(error.to_string()))),
        }
        set_busy.set(false);
    });
}
