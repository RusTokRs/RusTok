use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_use::use_interval_fn;
use rustok_installer::{
    AdminBootstrap, DatabaseConfig, DatabaseEngine, InstallEnvironment, InstallPlan,
    InstallProfile, ModuleSelection, SecretMode, SecretRef, SecretValue, SeedProfile,
    TenantBootstrap,
};

use crate::features::installer::api;
use crate::shared::ui::PageHeader;

fn local_resource<S, Fut, T>(
    source: impl Fn() -> S + 'static,
    fetcher: impl Fn(S) -> Fut + 'static,
) -> LocalResource<T>
where
    S: 'static,
    Fut: std::future::Future<Output = T> + 'static,
    T: 'static,
{
    LocalResource::new(move || fetcher(source()))
}

#[component]
pub fn InstallerPage() -> impl IntoView {
    let (setup_token, set_setup_token) = signal(String::new());
    let (environment, set_environment) = signal("local".to_string());
    let (profile, set_profile) = signal("dev_local".to_string());
    let (database_engine, set_database_engine) = signal("postgres".to_string());
    let (database_url, set_database_url) = signal(String::new());
    let (database_secret_backend, set_database_secret_backend) = signal("env".to_string());
    let (database_secret_key, set_database_secret_key) = signal("DATABASE_URL".to_string());
    let (create_database, set_create_database) = signal(false);
    let (pg_admin_url, set_pg_admin_url) = signal(String::new());
    let (admin_email, set_admin_email) = signal("admin@local".to_string());
    let (admin_password, set_admin_password) = signal(String::new());
    let (admin_secret_backend, set_admin_secret_backend) = signal("env".to_string());
    let (admin_secret_key, set_admin_secret_key) = signal("SUPERADMIN_PASSWORD".to_string());
    let (tenant_slug, set_tenant_slug) = signal("demo".to_string());
    let (tenant_name, set_tenant_name) = signal("Demo Workspace".to_string());
    let (seed_profile, set_seed_profile) = signal("dev".to_string());
    let (secrets_mode, set_secrets_mode) = signal("dotenv_file".to_string());
    let (enable_modules, set_enable_modules) = signal(String::new());
    let (disable_modules, set_disable_modules) = signal(String::new());
    let (lock_owner, set_lock_owner) = signal("web".to_string());
    let (lock_ttl_secs, set_lock_ttl_secs) = signal("900".to_string());

    let (busy, set_busy) = signal(false);
    let (status_refresh, set_status_refresh) = signal(0_u64);
    let (preflight_result, set_preflight_result) =
        signal(None::<Result<api::InstallPreflightResponse, String>>);
    let (apply_result, set_apply_result) =
        signal(None::<Result<api::InstallApplyJobResponse, String>>);
    let (job_id, set_job_id) = signal(None::<uuid::Uuid>);
    let (job_status, set_job_status) =
        signal(None::<Result<api::InstallJobStatusResponse, String>>);
    let (receipts, set_receipts) = signal(None::<Result<api::InstallReceiptsResponse, String>>);

    let status_resource = local_resource(
        move || status_refresh.get(),
        |_| async move { api::fetch_status().await },
    );
    Effect::new(move |_| {
        let Some(Ok(status)) = status_resource.get() else {
            return;
        };
        if !status.completed {
            return;
        }
        let Some(session_id) = status.session_id else {
            return;
        };
        let token = setup_token.get();
        if token.trim().is_empty() {
            return;
        }
        let loaded = receipts
            .get_untracked()
            .and_then(Result::ok)
            .is_some_and(|response| response.session_id == session_id);
        if loaded {
            return;
        }
        spawn_local(async move {
            set_receipts.set(Some(api::fetch_receipts(session_id, token).await));
        });
    });

    let build_plan = move || {
        build_install_plan(
            environment.get_untracked(),
            profile.get_untracked(),
            database_engine.get_untracked(),
            database_url.get_untracked(),
            database_secret_backend.get_untracked(),
            database_secret_key.get_untracked(),
            create_database.get_untracked(),
            admin_email.get_untracked(),
            admin_password.get_untracked(),
            admin_secret_backend.get_untracked(),
            admin_secret_key.get_untracked(),
            tenant_slug.get_untracked(),
            tenant_name.get_untracked(),
            seed_profile.get_untracked(),
            secrets_mode.get_untracked(),
            enable_modules.get_untracked(),
            disable_modules.get_untracked(),
        )
    };

    let run_preflight = move |_| {
        let plan = match build_plan() {
            Ok(plan) => plan,
            Err(error) => {
                set_preflight_result.set(Some(Err(error)));
                return;
            }
        };
        let token = setup_token.get_untracked();

        set_busy.set(true);
        set_preflight_result.set(None);
        spawn_local(async move {
            let result = api::preflight(plan, token).await;
            set_preflight_result.set(Some(result));
            set_busy.set(false);
        });
    };

    let run_apply = move |_| {
        let plan = match build_plan() {
            Ok(plan) => plan,
            Err(error) => {
                set_apply_result.set(Some(Err(error)));
                return;
            }
        };
        let lock_ttl_secs_value = match lock_ttl_secs.get_untracked().trim().parse::<i64>() {
            Ok(value) if value > 0 => value,
            _ => {
                set_apply_result.set(Some(Err("Lock TTL must be a positive number.".to_string())));
                return;
            }
        };
        let token = setup_token.get_untracked();
        let status_token = token.clone();
        let pg_admin_url_value = pg_admin_url.get_untracked();
        let pg_admin_url = if plan.database.create_if_missing {
            match optional_text(pg_admin_url_value) {
                Some(value) => Some(value),
                None => {
                    set_apply_result.set(Some(Err(
                        "Admin PostgreSQL URL is required when database creation is enabled."
                            .to_string(),
                    )));
                    return;
                }
            }
        } else {
            None
        };
        let request = api::InstallApplyRequest {
            plan,
            lock_owner: optional_text(lock_owner.get_untracked()),
            lock_ttl_secs: Some(lock_ttl_secs_value),
            pg_admin_url,
        };

        set_busy.set(true);
        set_apply_result.set(None);
        set_job_status.set(None);
        set_receipts.set(None);
        spawn_local(async move {
            match api::apply(request, token).await {
                Ok(response) => {
                    set_job_id.set(Some(response.job_id));
                    set_job_status.set(Some(api::fetch_job(response.job_id, status_token).await));
                    set_apply_result.set(Some(Ok(response)));
                }
                Err(error) => {
                    set_apply_result.set(Some(Err(error)));
                }
            }
            set_busy.set(false);
        });
    };

    let refresh_job = move || {
        let Some(current_job_id) = job_id.get_untracked() else {
            return;
        };
        spawn_refresh_job(
            current_job_id,
            setup_token.get_untracked(),
            set_job_status,
            set_receipts,
            set_status_refresh,
        );
    };

    let poller = use_interval_fn(refresh_job, 3000);
    (poller.pause)();
    let pause_polling = poller.pause.clone();
    let resume_polling = poller.resume.clone();
    Effect::new(move |_| {
        if matches!(
            job_status.get(),
            Some(Ok(api::InstallJobStatusResponse {
                status: api::InstallJobState::Running,
                ..
            }))
        ) {
            resume_polling();
        } else {
            pause_polling();
        }
    });

    let apply_disabled = Signal::derive(move || {
        busy.get()
            || status_resource
                .get()
                .and_then(Result::ok)
                .is_some_and(|status| status.completed)
            || matches!(
                job_status.get(),
                Some(Ok(api::InstallJobStatusResponse {
                    status: api::InstallJobState::Running,
                    ..
                }))
            )
    });

    view! {
        <section class="p-4 md:p-8">
            <PageHeader
                title="Hybrid installer".to_string()
                eyebrow="Platform setup".to_string()
                subtitle="CLI remains canonical; this screen submits the same install plan to the server installer API.".to_string()
            />

            <div class="grid gap-6 xl:grid-cols-[minmax(0,1.2fr)_minmax(360px,0.8fr)]">
                <div class="space-y-6">
                    <section class="rounded-lg border border-border bg-card p-5 shadow-sm">
                        <div class="mb-4 flex flex-wrap items-center justify-between gap-3">
                            <div>
                                <h2 class="text-base font-semibold text-card-foreground">"Install plan"</h2>
                                <p class="mt-1 text-sm text-muted-foreground">"The backend validates, redacts, locks and applies this plan."</p>
                            </div>
                            <StatusBadge status_resource=status_resource />
                        </div>

                        <div class="grid gap-4 md:grid-cols-2">
                            <SelectField
                                label="Environment"
                                value=environment
                                set_value=set_environment
                                options=vec![
                                    ("local".to_string(), "Local".to_string()),
                                    ("demo".to_string(), "Demo".to_string()),
                                    ("test".to_string(), "Test".to_string()),
                                    ("production".to_string(), "Production".to_string()),
                                ]
                            />
                            <SelectField
                                label="Profile"
                                value=profile
                                set_value=set_profile
                                options=vec![
                                    ("dev_local".to_string(), "Dev local".to_string()),
                                    ("monolith".to_string(), "Monolith".to_string()),
                                    ("hybrid_admin".to_string(), "Hybrid admin".to_string()),
                                    ("headless_next".to_string(), "Headless Next".to_string()),
                                    ("headless_leptos".to_string(), "Headless Leptos".to_string()),
                                ]
                            />
                            <SelectField
                                label="Seed"
                                value=seed_profile
                                set_value=set_seed_profile
                                options=vec![
                                    ("none".to_string(), "None".to_string()),
                                    ("minimal".to_string(), "Minimal".to_string()),
                                    ("dev".to_string(), "Dev".to_string()),
                                ]
                            />
                            <SelectField
                                label="Secrets mode"
                                value=secrets_mode
                                set_value=set_secrets_mode
                                options=vec![
                                    ("env".to_string(), "Env".to_string()),
                                    ("dotenv_file".to_string(), "Dotenv file".to_string()),
                                    ("mounted_file".to_string(), "Mounted file".to_string()),
                                    ("external_secret".to_string(), "External secret".to_string()),
                                ]
                            />
                        </div>
                    </section>

                    <section class="rounded-lg border border-border bg-card p-5 shadow-sm">
                        <h2 class="mb-4 text-base font-semibold text-card-foreground">"Database"</h2>
                        <div class="grid gap-4 md:grid-cols-2">
                            <SelectField
                                label="Engine"
                                value=database_engine
                                set_value=set_database_engine
                                options=vec![
                                    ("postgres".to_string(), "PostgreSQL".to_string()),
                                    ("sqlite".to_string(), "SQLite".to_string()),
                                ]
                            />
                            <CheckboxField
                                label="Create database if missing"
                                checked=create_database
                                set_checked=set_create_database
                            />
                            <TextField
                                label="Database URL"
                                value=database_url
                                set_value=set_database_url
                                type_="password"
                                placeholder="postgres://..."
                            />
                            <TextField
                                label="Admin PostgreSQL URL"
                                value=pg_admin_url
                                set_value=set_pg_admin_url
                                type_="password"
                                placeholder="required only when creating DB"
                            />
                            <TextField
                                label="DB secret backend"
                                value=database_secret_backend
                                set_value=set_database_secret_backend
                                type_="text"
                                placeholder="env"
                            />
                            <TextField
                                label="DB secret key"
                                value=database_secret_key
                                set_value=set_database_secret_key
                                type_="text"
                                placeholder="DATABASE_URL"
                            />
                        </div>
                    </section>

                    <section class="rounded-lg border border-border bg-card p-5 shadow-sm">
                        <h2 class="mb-4 text-base font-semibold text-card-foreground">"Tenant and admin"</h2>
                        <div class="grid gap-4 md:grid-cols-2">
                            <TextField label="Tenant slug" value=tenant_slug set_value=set_tenant_slug type_="text" placeholder="demo" />
                            <TextField label="Tenant name" value=tenant_name set_value=set_tenant_name type_="text" placeholder="Demo Workspace" />
                            <TextField label="Admin email" value=admin_email set_value=set_admin_email type_="email" placeholder="admin@example.com" />
                            <TextField label="Admin password" value=admin_password set_value=set_admin_password type_="password" placeholder="password or use ref" />
                            <TextField label="Admin secret backend" value=admin_secret_backend set_value=set_admin_secret_backend type_="text" placeholder="env" />
                            <TextField label="Admin secret key" value=admin_secret_key set_value=set_admin_secret_key type_="text" placeholder="SUPERADMIN_PASSWORD" />
                        </div>
                    </section>

                    <section class="rounded-lg border border-border bg-card p-5 shadow-sm">
                        <h2 class="mb-4 text-base font-semibold text-card-foreground">"Modules and lock"</h2>
                        <div class="grid gap-4 md:grid-cols-2">
                            <TextField label="Enable modules" value=enable_modules set_value=set_enable_modules type_="text" placeholder="commerce,pages,blog" />
                            <TextField label="Disable modules" value=disable_modules set_value=set_disable_modules type_="text" placeholder="comma-separated slugs" />
                            <TextField label="Lock owner" value=lock_owner set_value=set_lock_owner type_="text" placeholder="web" />
                            <TextField label="Lock TTL seconds" value=lock_ttl_secs set_value=set_lock_ttl_secs type_="number" placeholder="900" />
                            <TextField label="Setup token" value=setup_token set_value=set_setup_token type_="password" placeholder="RUSTOK_INSTALL_SETUP_TOKEN" />
                        </div>
                    </section>
                </div>

                <aside class="space-y-6">
                    <section class="rounded-lg border border-border bg-card p-5 shadow-sm">
                        <h2 class="text-base font-semibold text-card-foreground">"Run"</h2>
                        <div class="mt-4 flex flex-wrap gap-3">
                            <button
                                type="button"
                                class="inline-flex h-9 items-center rounded-md border border-border bg-background px-4 text-sm font-medium text-foreground transition hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                disabled=move || busy.get()
                                on:click=run_preflight
                            >
                                "Preflight"
                            </button>
                            <button
                                type="button"
                                class="inline-flex h-9 items-center rounded-md bg-primary px-4 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50"
                                disabled=move || apply_disabled.get()
                                on:click=run_apply
                            >
                                "Apply"
                            </button>
                            <button
                                type="button"
                                class="inline-flex h-9 items-center rounded-md border border-border bg-background px-4 text-sm font-medium text-foreground transition hover:bg-accent"
                                on:click=move |_| {
                                    set_status_refresh.update(|value| *value += 1);
                                    if let Some(current_job_id) = job_id.get_untracked() {
                                        spawn_refresh_job(
                                            current_job_id,
                                            setup_token.get_untracked(),
                                            set_job_status,
                                            set_receipts,
                                            set_status_refresh,
                                        );
                                    }
                                }
                            >
                                "Refresh"
                            </button>
                        </div>
                    </section>

                    <PreflightPanel result=preflight_result />
                    <JobPanel result=apply_result status=job_status />
                    <ReceiptsPanel receipts=receipts />
                </aside>
            </div>
        </section>
    }
}

#[component]
fn StatusBadge(
    status_resource: LocalResource<Result<api::InstallStatusResponse, String>>,
) -> impl IntoView {
    view! {
        <Suspense fallback=|| view! { <span class="rounded-full bg-muted px-3 py-1 text-xs font-medium text-muted-foreground">"checking"</span> }>
            {move || {
                status_resource.get().map(|result| {
                    match result {
                        Ok(status) => {
                            let class = if status.completed {
                                "bg-emerald-500/10 text-emerald-700"
                            } else if status.initialized {
                                "bg-amber-500/10 text-amber-700"
                            } else {
                                "bg-muted text-muted-foreground"
                            };
                            view! {
                                <span class=format!("rounded-full px-3 py-1 text-xs font-medium {class}")>
                                    {status.status}
                                </span>
                            }.into_any()
                        }
                        Err(error) => view! {
                            <span class="rounded-full bg-destructive/10 px-3 py-1 text-xs font-medium text-destructive">
                                {error}
                            </span>
                        }.into_any(),
                    }
                })
            }}
        </Suspense>
    }
}

#[component]
fn PreflightPanel(
    result: ReadSignal<Option<Result<api::InstallPreflightResponse, String>>>,
) -> impl IntoView {
    view! {
        <section class="rounded-lg border border-border bg-card p-5 shadow-sm">
            <h2 class="text-base font-semibold text-card-foreground">"Preflight"</h2>
            <div class="mt-4">
                {move || match result.get() {
                    None => view! { <p class="text-sm text-muted-foreground">"No report yet."</p> }.into_any(),
                    Some(Err(error)) => view! {
                        <div class="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">
                            {error}
                        </div>
                    }.into_any(),
                    Some(Ok(response)) => {
                        let issues = response.report.issues;
                        view! {
                            <div class="space-y-3">
                                <span class=if response.passed {
                                    "inline-flex rounded-full bg-emerald-500/10 px-3 py-1 text-xs font-medium text-emerald-700"
                                } else {
                                    "inline-flex rounded-full bg-destructive/10 px-3 py-1 text-xs font-medium text-destructive"
                                }>
                                    {if response.passed { "passed" } else { "blocked" }}
                                </span>
                                <div class="space-y-2">
                                    {if issues.is_empty() {
                                        view! { <p class="text-sm text-muted-foreground">"No issues."</p> }.into_any()
                                    } else {
                                        issues
                                            .into_iter()
                                            .map(|issue| {
                                                view! {
                                                    <div class="rounded-md border border-border bg-background px-3 py-2">
                                                        <div class="flex items-center justify-between gap-3">
                                                            <span class="text-sm font-medium text-foreground">{issue.code}</span>
                                                            <span class="shrink-0 rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">
                                                                {format!("{:?}", issue.severity).to_lowercase()}
                                                            </span>
                                                        </div>
                                                        <p class="mt-1 text-sm text-muted-foreground">{issue.message}</p>
                                                    </div>
                                                }
                                            })
                                            .collect_view()
                                            .into_any()
                                    }}
                                </div>
                            </div>
                        }.into_any()
                    }
                }}
            </div>
        </section>
    }
}

#[component]
fn JobPanel(
    result: ReadSignal<Option<Result<api::InstallApplyJobResponse, String>>>,
    status: ReadSignal<Option<Result<api::InstallJobStatusResponse, String>>>,
) -> impl IntoView {
    view! {
        <section class="rounded-lg border border-border bg-card p-5 shadow-sm">
            <h2 class="text-base font-semibold text-card-foreground">"Job"</h2>
            <div class="mt-4 space-y-3">
                {move || match result.get() {
                    None => view! { <p class="text-sm text-muted-foreground">"No job submitted."</p> }.into_any(),
                    Some(Err(error)) => view! {
                        <div class="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">
                            {error}
                        </div>
                    }.into_any(),
                    Some(Ok(job)) => view! {
                        <div class="rounded-md border border-border bg-background px-3 py-2 text-sm">
                            <div class="font-medium text-foreground">{job.job_id.to_string()}</div>
                            <div class="mt-1 text-muted-foreground">{job.status.label()}</div>
                        </div>
                    }.into_any(),
                }}

                {move || match status.get() {
                    None => ().into_any(),
                    Some(Err(error)) => view! {
                        <div class="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">
                            {error}
                        </div>
                    }.into_any(),
                    Some(Ok(status)) => view! {
                        <div class="rounded-md border border-border bg-background px-3 py-2 text-sm">
                            <div class="flex items-center justify-between gap-3">
                                <span class="font-medium text-foreground">{status.status.label()}</span>
                                {status.session_id.map(|id| view! {
                                    <span class="truncate text-xs text-muted-foreground">{id.to_string()}</span>
                                })}
                            </div>
                            {status.error.map(|error| view! {
                                <p class="mt-2 text-destructive">{error}</p>
                            })}
                        </div>
                    }.into_any(),
                }}
            </div>
        </section>
    }
}

#[component]
fn ReceiptsPanel(
    receipts: ReadSignal<Option<Result<api::InstallReceiptsResponse, String>>>,
) -> impl IntoView {
    view! {
        <section class="rounded-lg border border-border bg-card p-5 shadow-sm">
            <h2 class="text-base font-semibold text-card-foreground">"Receipts"</h2>
            <div class="mt-4 space-y-2">
                {move || match receipts.get() {
                    None => view! { <p class="text-sm text-muted-foreground">"No receipts loaded."</p> }.into_any(),
                    Some(Err(error)) => view! {
                        <div class="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">
                            {error}
                        </div>
                    }.into_any(),
                    Some(Ok(response)) => {
                        if response.receipts.is_empty() {
                            view! { <p class="text-sm text-muted-foreground">"No receipts persisted."</p> }.into_any()
                        } else {
                            response.receipts.into_iter().map(|receipt| {
                                view! {
                                    <div class="rounded-md border border-border bg-background px-3 py-2">
                                        <div class="flex items-center justify-between gap-3">
                                            <span class="text-sm font-medium text-foreground">{receipt.step}</span>
                                            <span class="shrink-0 rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">{receipt.outcome}</span>
                                        </div>
                                        <p class="mt-1 truncate text-xs text-muted-foreground">{receipt.input_checksum}</p>
                                    </div>
                                }
                            }).collect_view().into_any()
                        }
                    }
                }}
            </div>
        </section>
    }
}

#[component]
fn TextField(
    label: &'static str,
    value: ReadSignal<String>,
    set_value: WriteSignal<String>,
    type_: &'static str,
    placeholder: &'static str,
) -> impl IntoView {
    view! {
        <label class="flex flex-col gap-2 text-sm">
            <span class="font-medium text-foreground">{label}</span>
            <input
                type=type_
                class="h-9 rounded-md border border-input bg-background px-3 text-sm text-foreground shadow-sm transition placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                prop:value=value
                placeholder=placeholder
                on:input=move |event| set_value.set(event_target_value(&event))
            />
        </label>
    }
}

#[component]
fn SelectField(
    label: &'static str,
    value: ReadSignal<String>,
    set_value: WriteSignal<String>,
    options: Vec<(String, String)>,
) -> impl IntoView {
    view! {
        <label class="flex flex-col gap-2 text-sm">
            <span class="font-medium text-foreground">{label}</span>
            <select
                class="h-9 rounded-md border border-input bg-background px-3 text-sm text-foreground shadow-sm transition focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                on:change=move |event| set_value.set(event_target_value(&event))
            >
                {move || {
                    let current = value.get();
                    options
                        .iter()
                        .map(|(option_value, label)| {
                            let selected = option_value == &current;
                            view! {
                                <option value=option_value.clone() selected=selected>{label.clone()}</option>
                            }
                        })
                        .collect_view()
                }}
            </select>
        </label>
    }
}

#[component]
fn CheckboxField(
    label: &'static str,
    checked: ReadSignal<bool>,
    set_checked: WriteSignal<bool>,
) -> impl IntoView {
    view! {
        <label class="flex min-h-9 items-center gap-3 rounded-md border border-input bg-background px-3 text-sm shadow-sm">
            <input
                type="checkbox"
                class="h-4 w-4 rounded border-input"
                prop:checked=checked
                on:change=move |event| set_checked.set(event_target_checked(&event))
            />
            <span class="font-medium text-foreground">{label}</span>
        </label>
    }
}

#[allow(clippy::too_many_arguments)]
fn build_install_plan(
    environment: String,
    profile: String,
    database_engine: String,
    database_url: String,
    database_secret_backend: String,
    database_secret_key: String,
    create_database: bool,
    admin_email: String,
    admin_password: String,
    admin_secret_backend: String,
    admin_secret_key: String,
    tenant_slug: String,
    tenant_name: String,
    seed_profile: String,
    secrets_mode: String,
    enable_modules: String,
    disable_modules: String,
) -> Result<InstallPlan, String> {
    let tenant_slug = require_text("Tenant slug", tenant_slug)?;
    let tenant_name = require_text("Tenant name", tenant_name)?;
    let admin_email = require_text("Admin email", admin_email)?;
    require_secret("Database URL", &database_url, &database_secret_key)?;
    require_secret("Admin password", &admin_password, &admin_secret_key)?;

    Ok(InstallPlan {
        environment: parse_environment(&environment)?,
        profile: parse_profile(&profile)?,
        database: DatabaseConfig {
            engine: parse_database_engine(&database_engine)?,
            url: secret_value(database_url, database_secret_backend, database_secret_key),
            create_if_missing: create_database,
        },
        tenant: TenantBootstrap {
            slug: tenant_slug,
            name: tenant_name,
        },
        admin: AdminBootstrap {
            email: admin_email,
            password: secret_value(admin_password, admin_secret_backend, admin_secret_key),
        },
        modules: ModuleSelection {
            enable: parse_module_list(&enable_modules),
            disable: parse_module_list(&disable_modules),
        },
        seed_profile: parse_seed_profile(&seed_profile)?,
        secrets_mode: parse_secret_mode(&secrets_mode)?,
    })
}

fn require_text(label: &str, value: String) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(format!("{label} is required."))
    } else {
        Ok(trimmed.to_string())
    }
}

fn require_secret(label: &str, value: &str, key: &str) -> Result<(), String> {
    if optional_text(value.to_string()).is_some() || optional_text(key.to_string()).is_some() {
        Ok(())
    } else {
        Err(format!("{label} or secret key is required."))
    }
}

fn optional_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn spawn_refresh_job(
    job_id: uuid::Uuid,
    setup_token: String,
    set_job_status: WriteSignal<Option<Result<api::InstallJobStatusResponse, String>>>,
    set_receipts: WriteSignal<Option<Result<api::InstallReceiptsResponse, String>>>,
    set_status_refresh: WriteSignal<u64>,
) {
    spawn_local(async move {
        let result = api::fetch_job(job_id, setup_token.clone()).await;
        if let Ok(status) = &result {
            if let Some(session_id) = status.session_id.filter(|_| {
                matches!(
                    status.status,
                    api::InstallJobState::Succeeded | api::InstallJobState::Failed
                )
            }) {
                set_receipts.set(Some(api::fetch_receipts(session_id, setup_token).await));
                set_status_refresh.update(|value| *value += 1);
            }
        }
        set_job_status.set(Some(result));
    });
}

fn secret_value(value: String, backend: String, key: String) -> SecretValue {
    if let Some(key) = optional_text(key) {
        SecretValue::Reference {
            reference: SecretRef {
                backend: optional_text(backend).unwrap_or_else(|| "env".to_string()),
                key,
            },
        }
    } else {
        SecretValue::Plaintext { value }
    }
}

fn parse_module_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|slug| !slug.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn parse_environment(value: &str) -> Result<InstallEnvironment, String> {
    match value {
        "local" => Ok(InstallEnvironment::Local),
        "demo" => Ok(InstallEnvironment::Demo),
        "test" => Ok(InstallEnvironment::Test),
        "production" => Ok(InstallEnvironment::Production),
        _ => Err(format!("Unknown environment `{value}`.")),
    }
}

fn parse_profile(value: &str) -> Result<InstallProfile, String> {
    match value {
        "dev_local" => Ok(InstallProfile::DevLocal),
        "monolith" => Ok(InstallProfile::Monolith),
        "hybrid_admin" => Ok(InstallProfile::HybridAdmin),
        "headless_next" => Ok(InstallProfile::HeadlessNext),
        "headless_leptos" => Ok(InstallProfile::HeadlessLeptos),
        _ => Err(format!("Unknown profile `{value}`.")),
    }
}

fn parse_database_engine(value: &str) -> Result<DatabaseEngine, String> {
    match value {
        "postgres" => Ok(DatabaseEngine::Postgres),
        "sqlite" => Ok(DatabaseEngine::Sqlite),
        _ => Err(format!("Unknown database engine `{value}`.")),
    }
}

fn parse_seed_profile(value: &str) -> Result<SeedProfile, String> {
    match value {
        "none" => Ok(SeedProfile::None),
        "minimal" => Ok(SeedProfile::Minimal),
        "dev" => Ok(SeedProfile::Dev),
        _ => Err(format!("Unknown seed profile `{value}`.")),
    }
}

fn parse_secret_mode(value: &str) -> Result<SecretMode, String> {
    match value {
        "env" => Ok(SecretMode::Env),
        "dotenv_file" => Ok(SecretMode::DotenvFile),
        "mounted_file" => Ok(SecretMode::MountedFile),
        "external_secret" => Ok(SecretMode::ExternalSecret),
        _ => Err(format!("Unknown secret mode `{value}`.")),
    }
}
