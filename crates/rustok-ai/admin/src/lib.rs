mod api;
mod model;

use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn AiAdmin() -> impl IntoView {
    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (selected_session, set_selected_session) = signal(Option::<String>::None);
    let (feedback, set_feedback) = signal(Option::<String>::None);
    let (error, set_error) = signal(Option::<String>::None);

    let provider_slug = RwSignal::new(String::new());
    let provider_name = RwSignal::new(String::new());
    let provider_kind = RwSignal::new("openai_compatible".to_string());
    let provider_base_url = RwSignal::new("http://localhost:11434".to_string());
    let provider_model = RwSignal::new("gpt-4.1-mini".to_string());
    let provider_api_key = RwSignal::new(String::new());
    let provider_temperature = RwSignal::new("0.2".to_string());
    let provider_max_tokens = RwSignal::new("1024".to_string());
    let provider_capabilities =
        RwSignal::new("text_generation,structured_generation,code_generation".to_string());
    let provider_allowed_tasks = RwSignal::new(String::new());
    let provider_denied_tasks = RwSignal::new(String::new());
    let provider_restricted_roles = RwSignal::new(String::new());
    let provider_active = RwSignal::new(true);

    let tool_slug = RwSignal::new(String::new());
    let tool_name = RwSignal::new(String::new());
    let tool_description = RwSignal::new(String::new());
    let tool_allowed = RwSignal::new(
        "list_modules,query_modules,module_details,mcp_health,mcp_whoami".to_string(),
    );
    let tool_denied = RwSignal::new(String::new());
    let tool_sensitive = RwSignal::new(
        "alloy_create_script,alloy_update_script,alloy_delete_script,alloy_apply_module_scaffold"
            .to_string(),
    );
    let tool_active = RwSignal::new(true);

    let task_slug = RwSignal::new(String::new());
    let task_name = RwSignal::new(String::new());
    let task_description = RwSignal::new(String::new());
    let task_capability = RwSignal::new("text_generation".to_string());
    let task_system_prompt = RwSignal::new(String::new());
    let task_allowed_providers = RwSignal::new(String::new());
    let task_preferred_providers = RwSignal::new(String::new());
    let task_execution_mode = RwSignal::new("auto".to_string());
    let task_active = RwSignal::new(true);

    let session_title = RwSignal::new(String::new());
    let session_message = RwSignal::new(String::new());
    let selected_provider = RwSignal::new(String::new());
    let selected_task_profile = RwSignal::new(String::new());
    let selected_tool_profile = RwSignal::new(String::new());

    let reply_message = RwSignal::new(String::new());

    let bootstrap = Resource::new(
        move || refresh_nonce.get(),
        move |_| async move { api::fetch_bootstrap().await },
    );

    let session_detail = Resource::new(
        move || (selected_session.get(), refresh_nonce.get()),
        move |(session_id, _)| async move {
            match session_id {
                Some(session_id) => api::fetch_session(session_id).await,
                None => Ok(None),
            }
        },
    );

    let on_create_provider = move |ev: SubmitEvent| {
        ev.prevent_default();
        set_feedback.set(None);
        set_error.set(None);
        spawn_local(async move {
            let result = api::create_provider(
                provider_slug.get_untracked(),
                provider_name.get_untracked(),
                provider_kind.get_untracked(),
                provider_base_url.get_untracked(),
                provider_model.get_untracked(),
                optional_text(provider_api_key.get_untracked()),
                provider_temperature
                    .get_untracked()
                    .trim()
                    .parse::<f32>()
                    .ok(),
                provider_max_tokens
                    .get_untracked()
                    .trim()
                    .parse::<i32>()
                    .ok(),
                parse_csv(provider_capabilities.get_untracked()),
                parse_csv(provider_allowed_tasks.get_untracked()),
                parse_csv(provider_denied_tasks.get_untracked()),
                parse_csv(provider_restricted_roles.get_untracked()),
            )
            .await;
            match result {
                Ok(profile) => {
                    set_feedback.set(Some(format!("Provider `{}` created.", profile.slug)));
                    selected_provider.set(profile.id.clone());
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let reset_provider_form = move || {
        provider_slug.set(String::new());
        provider_name.set(String::new());
        provider_kind.set("openai_compatible".to_string());
        provider_base_url.set("http://localhost:11434".to_string());
        provider_model.set("gpt-4.1-mini".to_string());
        provider_api_key.set(String::new());
        provider_temperature.set("0.2".to_string());
        provider_max_tokens.set("1024".to_string());
        provider_capabilities
            .set("text_generation,structured_generation,code_generation".to_string());
        provider_allowed_tasks.set(String::new());
        provider_denied_tasks.set(String::new());
        provider_restricted_roles.set(String::new());
        provider_active.set(true);
        selected_provider.set(String::new());
    };

    let on_update_provider = move |_| {
        let provider_id = selected_provider.get_untracked();
        if provider_id.trim().is_empty() {
            set_error.set(Some("Select a provider before updating it.".to_string()));
            return;
        }
        set_feedback.set(None);
        set_error.set(None);
        spawn_local(async move {
            let result = api::update_provider(
                provider_id,
                provider_name.get_untracked(),
                provider_base_url.get_untracked(),
                provider_model.get_untracked(),
                provider_temperature
                    .get_untracked()
                    .trim()
                    .parse::<f32>()
                    .ok(),
                provider_max_tokens
                    .get_untracked()
                    .trim()
                    .parse::<i32>()
                    .ok(),
                parse_csv(provider_capabilities.get_untracked()),
                parse_csv(provider_allowed_tasks.get_untracked()),
                parse_csv(provider_denied_tasks.get_untracked()),
                parse_csv(provider_restricted_roles.get_untracked()),
                provider_active.get_untracked(),
            )
            .await;
            match result {
                Ok(profile) => {
                    set_feedback.set(Some(format!("Provider `{}` updated.", profile.slug)));
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let on_test_provider = move |_| {
        let provider_id = selected_provider.get_untracked();
        if provider_id.trim().is_empty() {
            set_error.set(Some("Select a provider before testing it.".to_string()));
            return;
        }
        set_feedback.set(None);
        set_error.set(None);
        spawn_local(async move {
            match api::test_provider(provider_id).await {
                Ok(result) => set_feedback.set(Some(result.message)),
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let on_deactivate_provider = move |_| {
        let provider_id = selected_provider.get_untracked();
        if provider_id.trim().is_empty() {
            set_error.set(Some(
                "Select a provider before deactivating it.".to_string(),
            ));
            return;
        }
        set_feedback.set(None);
        set_error.set(None);
        spawn_local(async move {
            match api::deactivate_provider(provider_id).await {
                Ok(profile) => {
                    provider_active.set(false);
                    set_feedback.set(Some(format!("Provider `{}` deactivated.", profile.slug)));
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let on_create_tool_profile = move |ev: SubmitEvent| {
        ev.prevent_default();
        set_feedback.set(None);
        set_error.set(None);
        spawn_local(async move {
            let result = api::create_tool_profile(
                tool_slug.get_untracked(),
                tool_name.get_untracked(),
                optional_text(tool_description.get_untracked()),
                parse_csv(tool_allowed.get_untracked()),
                parse_csv(tool_denied.get_untracked()),
                parse_csv(tool_sensitive.get_untracked()),
            )
            .await;
            match result {
                Ok(profile) => {
                    set_feedback.set(Some(format!("Tool profile `{}` created.", profile.slug)));
                    selected_tool_profile.set(profile.id.clone());
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let reset_tool_form = move || {
        tool_slug.set(String::new());
        tool_name.set(String::new());
        tool_description.set(String::new());
        tool_allowed
            .set("list_modules,query_modules,module_details,mcp_health,mcp_whoami".to_string());
        tool_denied.set(String::new());
        tool_sensitive.set(
            "alloy_create_script,alloy_update_script,alloy_delete_script,alloy_apply_module_scaffold"
                .to_string(),
        );
        tool_active.set(true);
        selected_tool_profile.set(String::new());
    };

    let on_update_tool_profile = move |_| {
        let tool_profile_id = selected_tool_profile.get_untracked();
        if tool_profile_id.trim().is_empty() {
            set_error.set(Some(
                "Select a tool profile before updating it.".to_string(),
            ));
            return;
        }
        set_feedback.set(None);
        set_error.set(None);
        spawn_local(async move {
            let result = api::update_tool_profile(
                tool_profile_id,
                tool_name.get_untracked(),
                optional_text(tool_description.get_untracked()),
                parse_csv(tool_allowed.get_untracked()),
                parse_csv(tool_denied.get_untracked()),
                parse_csv(tool_sensitive.get_untracked()),
                tool_active.get_untracked(),
            )
            .await;
            match result {
                Ok(profile) => {
                    set_feedback.set(Some(format!("Tool profile `{}` updated.", profile.slug)));
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let on_start_session = move |ev: SubmitEvent| {
        ev.prevent_default();
        set_feedback.set(None);
        set_error.set(None);
        spawn_local(async move {
            let result = api::start_session(
                session_title.get_untracked(),
                optional_text(selected_provider.get_untracked()),
                optional_text(selected_task_profile.get_untracked()),
                optional_text(selected_tool_profile.get_untracked()),
                optional_text(session_message.get_untracked()),
            )
            .await;
            match result {
                Ok(result) => {
                    set_selected_session.set(Some(result.session.session.id.clone()));
                    set_feedback.set(Some(format!(
                        "Session `{}` started.",
                        result.session.session.title
                    )));
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let reset_task_form = move || {
        task_slug.set(String::new());
        task_name.set(String::new());
        task_description.set(String::new());
        task_capability.set("text_generation".to_string());
        task_system_prompt.set(String::new());
        task_allowed_providers.set(String::new());
        task_preferred_providers.set(String::new());
        task_execution_mode.set("auto".to_string());
        task_active.set(true);
        selected_task_profile.set(String::new());
    };

    let on_create_task_profile = move |ev: SubmitEvent| {
        ev.prevent_default();
        set_feedback.set(None);
        set_error.set(None);
        spawn_local(async move {
            let result = api::create_task_profile(
                task_slug.get_untracked(),
                task_name.get_untracked(),
                optional_text(task_description.get_untracked()),
                task_capability.get_untracked(),
                optional_text(task_system_prompt.get_untracked()),
                parse_csv(task_allowed_providers.get_untracked()),
                parse_csv(task_preferred_providers.get_untracked()),
                optional_text(selected_tool_profile.get_untracked()),
                task_execution_mode.get_untracked(),
            )
            .await;
            match result {
                Ok(profile) => {
                    set_feedback.set(Some(format!("Task profile `{}` created.", profile.slug)));
                    selected_task_profile.set(profile.id.clone());
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let on_update_task_profile = move |_| {
        let task_profile_id = selected_task_profile.get_untracked();
        if task_profile_id.trim().is_empty() {
            set_error.set(Some(
                "Select a task profile before updating it.".to_string(),
            ));
            return;
        }
        set_feedback.set(None);
        set_error.set(None);
        spawn_local(async move {
            let result = api::update_task_profile(
                task_profile_id,
                task_name.get_untracked(),
                optional_text(task_description.get_untracked()),
                task_capability.get_untracked(),
                optional_text(task_system_prompt.get_untracked()),
                parse_csv(task_allowed_providers.get_untracked()),
                parse_csv(task_preferred_providers.get_untracked()),
                optional_text(selected_tool_profile.get_untracked()),
                task_execution_mode.get_untracked(),
                task_active.get_untracked(),
            )
            .await;
            match result {
                Ok(profile) => {
                    set_feedback.set(Some(format!("Task profile `{}` updated.", profile.slug)));
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let on_send_message = move |ev: SubmitEvent| {
        ev.prevent_default();
        let Some(session_id) = selected_session.get_untracked() else {
            set_error.set(Some("Select a session first.".to_string()));
            return;
        };
        let content = reply_message.get_untracked();
        if content.trim().is_empty() {
            return;
        }
        set_feedback.set(None);
        set_error.set(None);
        spawn_local(async move {
            let result = api::send_message(session_id, content).await;
            match result {
                Ok(_) => {
                    reply_message.set(String::new());
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    view! {
        <div class="space-y-6">
            <header class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-2">
                    <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                        "capability"
                    </span>
                    <h1 class="text-2xl font-semibold text-card-foreground">"AI Control Plane"</h1>
                    <p class="max-w-3xl text-sm text-muted-foreground">
                        "Provider profiles, tool policies, operator chat sessions, tool traces, and approval gates for rustok-ai."
                    </p>
                </div>
            </header>

            <Show when=move || feedback.get().is_some()>
                <div class="rounded-xl border border-emerald-300 bg-emerald-50 px-4 py-3 text-sm text-emerald-700">
                    {move || feedback.get().unwrap_or_default()}
                </div>
            </Show>
            <Show when=move || error.get().is_some()>
                <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>

            <Suspense fallback=move || view! { <div class="h-32 animate-pulse rounded-2xl bg-muted"></div> }>
                {move || bootstrap.get().map(|result| match result {
                    Ok(bootstrap) => view! {
                        <div class="grid gap-6 xl:grid-cols-[1.2fr_1fr_1.6fr]">
                            <section class="space-y-6">
                                <Card title="Providers">
                                    <form class="space-y-3" on:submit=on_create_provider>
                                        <TextField label="Slug" value=provider_slug />
                                        <TextField label="Display name" value=provider_name />
                                        <TextField label="Provider kind" value=provider_kind />
                                        <TextField label="Base URL" value=provider_base_url />
                                        <TextField label="Model" value=provider_model />
                                        <TextField label="API key" value=provider_api_key />
                                        <TextField label="Temperature" value=provider_temperature />
                                        <TextField label="Max tokens" value=provider_max_tokens />
                                        <TextField label="Capabilities (csv)" value=provider_capabilities />
                                        <TextField label="Allowed tasks (csv)" value=provider_allowed_tasks />
                                        <TextField label="Denied tasks (csv)" value=provider_denied_tasks />
                                        <TextField label="Restricted roles (csv)" value=provider_restricted_roles />
                                        <label class="flex items-center gap-2 text-sm text-muted-foreground">
                                            <input
                                                type="checkbox"
                                                prop:checked=provider_active
                                                on:change=move |ev| provider_active.set(event_target_checked(&ev))
                                            />
                                            "Active"
                                        </label>
                                        <div class="flex flex-wrap gap-2">
                                            <button type="submit" class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground">"Create provider"</button>
                                            <button type="button" class="rounded-lg border border-border px-4 py-2 text-sm font-medium" on:click=on_update_provider>"Update selected"</button>
                                            <button type="button" class="rounded-lg border border-border px-4 py-2 text-sm font-medium" on:click=on_test_provider>"Test selected"</button>
                                            <button type="button" class="rounded-lg border border-destructive/40 px-4 py-2 text-sm font-medium text-destructive" on:click=on_deactivate_provider>"Deactivate"</button>
                                            <button type="button" class="rounded-lg border border-border px-4 py-2 text-sm font-medium" on:click=move |_| reset_provider_form()>"Reset"</button>
                                        </div>
                                    </form>
                                    <div class="mt-4 space-y-2">
                                        {bootstrap.providers.into_iter().map(|provider| {
                                            let provider_id = provider.id.clone();
                                            let provider_slug_value = provider.slug.clone();
                                            let provider_name_value = provider.display_name.clone();
                                            let provider_kind_value = provider.provider_kind.clone();
                                            let provider_base_url_value = provider.base_url.clone();
                                            let provider_model_value = provider.model.clone();
                                            let provider_temperature_value = provider.temperature.map(|value| value.to_string()).unwrap_or_default();
                                            let provider_max_tokens_value = provider.max_tokens.map(|value| value.to_string()).unwrap_or_default();
                                            let provider_capabilities_value = provider.capabilities.join(",");
                                            let provider_allowed_tasks_value = provider.allowed_task_profiles.join(",");
                                            let provider_denied_tasks_value = provider.denied_task_profiles.join(",");
                                            let provider_restricted_roles_value = provider.restricted_role_slugs.join(",");
                                            let provider_active_value = provider.is_active;
                                            view! {
                                                <button
                                                    class="w-full rounded-lg border border-border px-3 py-3 text-left text-sm hover:bg-muted"
                                                    on:click=move |_| {
                                                        selected_provider.set(provider_id.clone());
                                                        provider_slug.set(provider_slug_value.clone());
                                                        provider_name.set(provider_name_value.clone());
                                                        provider_kind.set(provider_kind_value.clone());
                                                        provider_base_url.set(provider_base_url_value.clone());
                                                        provider_model.set(provider_model_value.clone());
                                                        provider_api_key.set(String::new());
                                                        provider_temperature.set(provider_temperature_value.clone());
                                                        provider_max_tokens.set(provider_max_tokens_value.clone());
                                                        provider_capabilities.set(provider_capabilities_value.clone());
                                                        provider_allowed_tasks.set(provider_allowed_tasks_value.clone());
                                                        provider_denied_tasks.set(provider_denied_tasks_value.clone());
                                                        provider_restricted_roles.set(provider_restricted_roles_value.clone());
                                                        provider_active.set(provider_active_value);
                                                    }
                                                >
                                                    <div class="font-medium">{provider.display_name}</div>
                                                    <div class="text-muted-foreground">
                                                        {format!(
                                                            "{} · {} · {} capabilities · {}",
                                                            provider.provider_kind,
                                                            provider.model,
                                                            provider.capabilities.len(),
                                                            if provider.is_active { "active" } else { "inactive" }
                                                        )}
                                                    </div>
                                                </button>
                                            }
                                        }).collect_view()}
                                    </div>
                                </Card>

                                <Card title="Tool Profiles">
                                    <form class="space-y-3" on:submit=on_create_tool_profile>
                                        <TextField label="Slug" value=tool_slug />
                                        <TextField label="Display name" value=tool_name />
                                        <TextField label="Description" value=tool_description />
                                        <TextField label="Allowed tools (csv)" value=tool_allowed />
                                        <TextField label="Denied tools (csv)" value=tool_denied />
                                        <TextField label="Sensitive tools (csv)" value=tool_sensitive />
                                        <label class="flex items-center gap-2 text-sm text-muted-foreground">
                                            <input
                                                type="checkbox"
                                                prop:checked=tool_active
                                                on:change=move |ev| tool_active.set(event_target_checked(&ev))
                                            />
                                            "Active"
                                        </label>
                                        <div class="flex flex-wrap gap-2">
                                            <button type="submit" class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground">"Create tool profile"</button>
                                            <button type="button" class="rounded-lg border border-border px-4 py-2 text-sm font-medium" on:click=on_update_tool_profile>"Update selected"</button>
                                            <button type="button" class="rounded-lg border border-border px-4 py-2 text-sm font-medium" on:click=move |_| reset_tool_form()>"Reset"</button>
                                        </div>
                                    </form>
                                    <div class="mt-4 space-y-2">
                                        {bootstrap.tool_profiles.into_iter().map(|profile| {
                                            let profile_id = profile.id.clone();
                                            let profile_slug_value = profile.slug.clone();
                                            let profile_name_value = profile.display_name.clone();
                                            let profile_description_value = profile.description.clone().unwrap_or_default();
                                            let profile_allowed_value = profile.allowed_tools.join(",");
                                            let profile_denied_value = profile.denied_tools.join(",");
                                            let profile_sensitive_value = profile.sensitive_tools.join(",");
                                            let profile_active_value = profile.is_active;
                                            view! {
                                                <button
                                                    class="w-full rounded-lg border border-border px-3 py-3 text-left text-sm hover:bg-muted"
                                                    on:click=move |_| {
                                                        selected_tool_profile.set(profile_id.clone());
                                                        tool_slug.set(profile_slug_value.clone());
                                                        tool_name.set(profile_name_value.clone());
                                                        tool_description.set(profile_description_value.clone());
                                                        tool_allowed.set(profile_allowed_value.clone());
                                                        tool_denied.set(profile_denied_value.clone());
                                                        tool_sensitive.set(profile_sensitive_value.clone());
                                                        tool_active.set(profile_active_value);
                                                    }
                                                >
                                                    <div class="font-medium">{profile.display_name}</div>
                                                    <div class="text-muted-foreground">
                                                        {format!("allowed: {} · sensitive: {} · {}", profile.allowed_tools.len(), profile.sensitive_tools.len(), if profile.is_active { "active" } else { "inactive" })}
                                                    </div>
                                                </button>
                                            }
                                        }).collect_view()}
                                    </div>
                                </Card>

                                <Card title="Task Profiles">
                                    <form class="space-y-3" on:submit=on_create_task_profile>
                                        <TextField label="Slug" value=task_slug />
                                        <TextField label="Display name" value=task_name />
                                        <TextField label="Description" value=task_description />
                                        <TextField label="Target capability" value=task_capability />
                                        <TextField label="System prompt" value=task_system_prompt />
                                        <TextField label="Allowed provider ids (csv)" value=task_allowed_providers />
                                        <TextField label="Preferred provider ids (csv)" value=task_preferred_providers />
                                        <TextField label="Execution mode" value=task_execution_mode />
                                        <label class="flex items-center gap-2 text-sm text-muted-foreground">
                                            <input
                                                type="checkbox"
                                                prop:checked=task_active
                                                on:change=move |ev| task_active.set(event_target_checked(&ev))
                                            />
                                            "Active"
                                        </label>
                                        <div class="flex flex-wrap gap-2">
                                            <button type="submit" class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground">"Create task profile"</button>
                                            <button type="button" class="rounded-lg border border-border px-4 py-2 text-sm font-medium" on:click=on_update_task_profile>"Update selected"</button>
                                            <button type="button" class="rounded-lg border border-border px-4 py-2 text-sm font-medium" on:click=move |_| reset_task_form()>"Reset"</button>
                                        </div>
                                    </form>
                                    <div class="mt-4 space-y-2">
                                        {bootstrap.task_profiles.into_iter().map(|profile| {
                                            let profile_id = profile.id.clone();
                                            let profile_slug_value = profile.slug.clone();
                                            let profile_name_value = profile.display_name.clone();
                                            let profile_description_value = profile.description.clone().unwrap_or_default();
                                            let profile_capability_value = profile.target_capability.clone();
                                            let profile_system_prompt_value = profile.system_prompt.clone().unwrap_or_default();
                                            let profile_allowed_value = profile.allowed_provider_profile_ids.join(",");
                                            let profile_preferred_value = profile.preferred_provider_profile_ids.join(",");
                                            let profile_execution_mode_value = profile.default_execution_mode.clone();
                                            let profile_active_value = profile.is_active;
                                            let profile_tool_profile_id = profile.tool_profile_id.clone().unwrap_or_default();
                                            view! {
                                                <button
                                                    class="w-full rounded-lg border border-border px-3 py-3 text-left text-sm hover:bg-muted"
                                                    on:click=move |_| {
                                                        selected_task_profile.set(profile_id.clone());
                                                        task_slug.set(profile_slug_value.clone());
                                                        task_name.set(profile_name_value.clone());
                                                        task_description.set(profile_description_value.clone());
                                                        task_capability.set(profile_capability_value.clone());
                                                        task_system_prompt.set(profile_system_prompt_value.clone());
                                                        task_allowed_providers.set(profile_allowed_value.clone());
                                                        task_preferred_providers.set(profile_preferred_value.clone());
                                                        task_execution_mode.set(profile_execution_mode_value.clone());
                                                        task_active.set(profile_active_value);
                                                        if !profile_tool_profile_id.is_empty() {
                                                            selected_tool_profile.set(profile_tool_profile_id.clone());
                                                        }
                                                    }
                                                >
                                                    <div class="font-medium">{profile.display_name}</div>
                                                    <div class="text-muted-foreground">
                                                        {format!("{} В· {} В· {}", profile.target_capability, profile.default_execution_mode, if profile.is_active { "active" } else { "inactive" })}
                                                    </div>
                                                </button>
                                            }
                                        }).collect_view()}
                                    </div>
                                </Card>
                            </section>

                            <section class="space-y-6">
                                <Card title="New Session">
                                    <form class="space-y-3" on:submit=on_start_session>
                                        <TextField label="Title" value=session_title />
                                        <TextField label="Initial message" value=session_message />
                                        <div class="rounded-lg border border-border px-3 py-2 text-sm text-muted-foreground">
                                            {move || format!(
                                                "Provider: {} | Task profile: {} | Tool profile: {}",
                                                selected_provider.get(),
                                                selected_task_profile.get(),
                                                selected_tool_profile.get()
                                            )}
                                        </div>
                                        <button type="submit" class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground">"Start session"</button>
                                    </form>
                                </Card>

                                <Card title="Sessions">
                                    <div class="space-y-2">
                                        {bootstrap.sessions.into_iter().map(|session| {
                                            let session_id = session.id.clone();
                                            view! {
                                                <button
                                                    class="w-full rounded-lg border border-border px-3 py-3 text-left text-sm hover:bg-muted"
                                                    on:click=move |_| set_selected_session.set(Some(session_id.clone()))
                                                >
                                                    <div class="font-medium">{session.title}</div>
                                                    <div class="text-muted-foreground">
                                                        {format!(
                                                            "status: {} · mode: {} · latest: {} · approvals: {}",
                                                            session.status,
                                                            session.execution_mode,
                                                            session.latest_run_status.unwrap_or_else(|| "idle".to_string()),
                                                            session.pending_approvals
                                                        )}
                                                    </div>
                                                </button>
                                            }
                                        }).collect_view()}
                                    </div>
                                </Card>
                            </section>

                            <section>
                                <Card title="Operator Chat">
                                    <Suspense fallback=move || view! { <div class="h-64 animate-pulse rounded-xl bg-muted"></div> }>
                                        {move || session_detail.get().map(|result| match result {
                                            Ok(Some(detail)) => {
                                                let pending_approvals = detail
                                                    .approvals
                                                    .clone()
                                                    .into_iter()
                                                    .filter(|item| item.status == "pending")
                                                    .collect::<Vec<_>>();
                                                view! {
                                                    <div class="space-y-5">
                                                        <div class="rounded-lg border border-border px-3 py-3 text-sm">
                                                            <div class="font-medium">{detail.session.title.clone()}</div>
                                                            <div class="text-muted-foreground">
                                                                {format!(
                                                                    "provider: {} · model: {} · mode: {}",
                                                                    detail.provider_profile.display_name,
                                                                    detail.provider_profile.model,
                                                                    detail.session.execution_mode
                                                                )}
                                                            </div>
                                                        </div>

                                                        <div class="max-h-[380px] space-y-3 overflow-y-auto rounded-xl border border-border p-3">
                                                            {detail.messages.into_iter().map(|message| view! {
                                                                <div class="rounded-lg border border-border px-3 py-3 text-sm">
                                                                    <div class="mb-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                                                                        {message.role.clone()}
                                                                    </div>
                                                                    <div>{message.content.unwrap_or_else(|| "(no textual content)".to_string())}</div>
                                                                </div>
                                                            }).collect_view()}
                                                        </div>

                                                        <form class="space-y-3" on:submit=on_send_message>
                                                            <textarea
                                                                class="min-h-28 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                                                prop:value=reply_message
                                                                on:input=move |ev| reply_message.set(event_target_value(&ev))
                                                            />
                                                            <button type="submit" class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground">"Send"</button>
                                                        </form>

                                                        {if pending_approvals.is_empty() {
                                                            ().into_any()
                                                        } else {
                                                            view! {
                                                                <div class="space-y-3">
                                                                    <div class="text-sm font-semibold">"Pending approvals"</div>
                                                                    {pending_approvals.into_iter().map(|approval| {
                                                                    let approve_id = approval.id.clone();
                                                                    let reject_id = approval.id.clone();
                                                                    view! {
                                                                        <div class="rounded-lg border border-amber-300 bg-amber-50 px-4 py-3 text-sm text-amber-900">
                                                                            <div class="font-medium">{approval.tool_name.clone()}</div>
                                                                            <div class="mt-1 text-amber-800">{approval.reason.unwrap_or_else(|| "Operator approval required".to_string())}</div>
                                                                            <div class="mt-3 flex gap-2">
                                                                                <button
                                                                                    class="rounded-md bg-amber-900 px-3 py-2 text-xs font-semibold text-white"
                                                                                    on:click=move |_| {
                                                                                        let approval_id = approve_id.clone();
                                                                                        spawn_local(async move {
                                                                                            let _ = api::resume_approval(approval_id, true, None).await;
                                                                                            set_refresh_nonce.update(|value| *value += 1);
                                                                                        });
                                                                                    }
                                                                                >
                                                                                    "Approve"
                                                                                </button>
                                                                                <button
                                                                                    class="rounded-md border border-amber-900 px-3 py-2 text-xs font-semibold text-amber-900"
                                                                                    on:click=move |_| {
                                                                                        let approval_id = reject_id.clone();
                                                                                        spawn_local(async move {
                                                                                            let _ = api::resume_approval(approval_id, false, Some("Rejected in admin UI".to_string())).await;
                                                                                            set_refresh_nonce.update(|value| *value += 1);
                                                                                        });
                                                                                    }
                                                                                >
                                                                                    "Reject"
                                                                                </button>
                                                                            </div>
                                                                        </div>
                                                                    }
                                                                    }).collect_view()}
                                                                </div>
                                                            }.into_any()
                                                        }}

                                                        <div class="space-y-3">
                                                            <div class="text-sm font-semibold">"Runs"</div>
                                                            {detail.runs.into_iter().map(|run| {
                                                                let error_message = run.error_message.clone().unwrap_or_default();
                                                                let has_error = !error_message.is_empty();
                                                                view! {
                                                                    <div class="rounded-lg border border-border px-3 py-3 text-sm">
                                                                        <div class="font-medium">{run.model.clone()}</div>
                                                                        <div class="text-muted-foreground">
                                                                            {format!(
                                                                                "{} · {} · path {}",
                                                                                run.status,
                                                                                run.execution_mode,
                                                                                run.execution_path
                                                                            )}
                                                                        </div>
                                                                        <Show when=move || has_error>
                                                                            <div class="mt-2 text-destructive">{error_message.clone()}</div>
                                                                        </Show>
                                                                    </div>
                                                                }
                                                            }).collect_view()}
                                                        </div>

                                                        <div class="space-y-3">
                                                            <div class="text-sm font-semibold">"Tool trace"</div>
                                                            {detail.tool_traces.into_iter().map(|trace| view! {
                                                                <div class="rounded-lg border border-border px-3 py-3 text-sm">
                                                                    <div class="font-medium">{trace.tool_name}</div>
                                                                    <div class="text-muted-foreground">{format!("{} · {} ms", trace.status, trace.duration_ms)}</div>
                                                                </div>
                                                            }).collect_view()}
                                                        </div>
                                                    </div>
                                                }.into_any()
                                            }
                                            Ok(None) => view! {
                                                <div class="rounded-lg border border-dashed border-border px-4 py-8 text-sm text-muted-foreground">
                                                    "Select a session to inspect chat history, traces, and approvals."
                                                </div>
                                            }.into_any(),
                                            Err(err) => view! {
                                                <div class="rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                                    {format!("Failed to load session: {err}")}
                                                </div>
                                            }.into_any(),
                                        })}
                                    </Suspense>
                                </Card>
                            </section>
                        </div>
                    }.into_any(),
                    Err(err) => view! {
                        <div class="rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                            {format!("Failed to load AI bootstrap: {err}")}
                        </div>
                    }.into_any(),
                })}
            </Suspense>
        </div>
    }
}

#[component]
fn Card(title: &'static str, children: Children) -> impl IntoView {
    view! {
        <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
            <h2 class="mb-4 text-lg font-semibold text-card-foreground">{title}</h2>
            {children()}
        </section>
    }
}

#[component]
fn TextField(label: &'static str, value: RwSignal<String>) -> impl IntoView {
    view! {
        <label class="block space-y-1">
            <span class="text-sm text-muted-foreground">{label}</span>
            <input
                type="text"
                class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                prop:value=value
                on:input=move |ev| value.set(event_target_value(&ev))
            />
        </label>
    }
}

fn parse_csv(value: String) -> Vec<String> {
    value
        .split(',')
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn optional_text(value: String) -> Option<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}
