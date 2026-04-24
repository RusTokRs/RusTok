mod api;
mod i18n;
mod model;

use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
#[cfg(not(target_arch = "wasm32"))]
use model::AiLiveStreamStatePayload;
#[cfg(target_arch = "wasm32")]
use model::{AiLiveStreamStatePayload, AiRunStreamEventKindPayload, AiSessionSubscriptionEnvelope};
use model::{
    AiMetricBucketPayload, AiProviderProfilePayload, AiTaskProfilePayload, AiToolProfilePayload,
};
use rustok_api::{AdminQueryKey, UiRouteContext};
#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{closure::Closure, JsCast};
#[cfg(target_arch = "wasm32")]
use web_sys::{CloseEvent, ErrorEvent, Event, MessageEvent, WebSocket};

use crate::i18n::t;

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
pub fn AiAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let tab_query = use_route_query_value(AdminQueryKey::Tab.as_str());
    let session_query = use_route_query_value(AdminQueryKey::SessionId.as_str());
    let provider_slug_query = use_route_query_value(AdminQueryKey::ProviderSlug.as_str());
    let tool_profile_slug_query = use_route_query_value(AdminQueryKey::ToolProfileSlug.as_str());
    let task_profile_slug_query = use_route_query_value(AdminQueryKey::TaskProfileSlug.as_str());
    let query_writer = use_route_query_writer();
    let token = use_token();
    let tenant = use_tenant();
    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (selected_session, set_selected_session) = signal(Option::<String>::None);
    let (live_stream, set_live_stream) = signal(Option::<AiLiveStreamStatePayload>::None);
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
    let provider_capabilities = RwSignal::new(
        "text_generation,structured_generation,image_generation,code_generation".to_string(),
    );
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
    let session_locale = RwSignal::new(String::new());
    let selected_provider = RwSignal::new(String::new());
    let selected_task_profile = RwSignal::new(String::new());
    let selected_tool_profile = RwSignal::new(String::new());
    let alloy_title = RwSignal::new(t(ui_locale.as_deref(), "ai.job.alloyTitle", "Alloy Assist"));
    let alloy_locale = RwSignal::new(String::new());
    let alloy_operation = RwSignal::new("list_scripts".to_string());
    let alloy_script_id = RwSignal::new(String::new());
    let alloy_script_name = RwSignal::new(String::new());
    let alloy_script_source = RwSignal::new(String::new());
    let alloy_runtime_payload = RwSignal::new(String::new());
    let alloy_prompt = RwSignal::new(String::new());
    let image_title = RwSignal::new(t(ui_locale.as_deref(), "ai.job.imageTitle", "Media Image"));
    let image_locale = RwSignal::new(String::new());
    let image_prompt = RwSignal::new(String::new());
    let image_negative_prompt = RwSignal::new(String::new());
    let image_file_name = RwSignal::new(String::new());
    let image_asset_title = RwSignal::new(String::new());
    let image_alt_text = RwSignal::new(String::new());
    let image_caption = RwSignal::new(String::new());
    let image_size = RwSignal::new("1024x1024".to_string());
    let image_assistant_prompt = RwSignal::new(String::new());
    let product_title = RwSignal::new(t(
        ui_locale.as_deref(),
        "ai.job.productTitle",
        "Product Copy",
    ));
    let product_locale = RwSignal::new(String::new());
    let product_id = RwSignal::new(String::new());
    let product_source_locale = RwSignal::new(String::new());
    let product_source_title = RwSignal::new(String::new());
    let product_source_description = RwSignal::new(String::new());
    let product_source_meta_title = RwSignal::new(String::new());
    let product_source_meta_description = RwSignal::new(String::new());
    let product_copy_instructions = RwSignal::new(String::new());
    let product_assistant_prompt = RwSignal::new(String::new());
    let blog_title = RwSignal::new(t(ui_locale.as_deref(), "ai.job.blogTitle", "Blog Draft"));
    let blog_locale = RwSignal::new(String::new());
    let blog_post_id = RwSignal::new(String::new());
    let blog_source_locale = RwSignal::new(String::new());
    let blog_source_title = RwSignal::new(String::new());
    let blog_source_body = RwSignal::new(String::new());
    let blog_source_excerpt = RwSignal::new(String::new());
    let blog_source_seo_title = RwSignal::new(String::new());
    let blog_source_seo_description = RwSignal::new(String::new());
    let blog_tags = RwSignal::new(String::new());
    let blog_category_id = RwSignal::new(String::new());
    let blog_featured_image_url = RwSignal::new(String::new());
    let blog_copy_instructions = RwSignal::new(String::new());
    let blog_assistant_prompt = RwSignal::new(String::new());

    let reply_message = RwSignal::new(String::new());

    let bootstrap = local_resource(
        move || refresh_nonce.get(),
        move |_| async move { api::fetch_bootstrap().await },
    );

    let session_detail = local_resource(
        move || (selected_session.get(), refresh_nonce.get()),
        move |(session_id, _)| async move {
            match session_id {
                Some(session_id) => api::fetch_session(session_id).await,
                None => Ok(None),
            }
        },
    );
    let diagnostics_only =
        Signal::derive(move || matches!(tab_query.get().as_deref(), Some("diagnostics")));
    let badge_label = t(ui_locale.as_deref(), "ai.badge", "capability");
    let page_title_label = t(ui_locale.as_deref(), "ai.title", "AI Control Plane");
    let page_subtitle_label = t(
        ui_locale.as_deref(),
        "ai.subtitle",
        "Provider profiles, tool policies, operator chat sessions, tool traces, and approval gates for rustok-ai.",
    );
    let overview_label = t(ui_locale.as_deref(), "ai.tab.overview", "Overview");
    let diagnostics_label = t(ui_locale.as_deref(), "ai.tab.diagnostics", "Diagnostics");
    let provider_created_template = t(
        ui_locale.as_deref(),
        "ai.feedback.providerCreated",
        "Provider `{slug}` created.",
    );
    let provider_updated_template = t(
        ui_locale.as_deref(),
        "ai.feedback.providerUpdated",
        "Provider `{slug}` updated.",
    );
    let provider_deactivated_template = t(
        ui_locale.as_deref(),
        "ai.feedback.providerDeactivated",
        "Provider `{slug}` deactivated.",
    );
    let tool_created_template = t(
        ui_locale.as_deref(),
        "ai.feedback.toolProfileCreated",
        "Tool profile `{slug}` created.",
    );
    let tool_updated_template = t(
        ui_locale.as_deref(),
        "ai.feedback.toolProfileUpdated",
        "Tool profile `{slug}` updated.",
    );
    let task_created_template = t(
        ui_locale.as_deref(),
        "ai.feedback.taskProfileCreated",
        "Task profile `{slug}` created.",
    );
    let task_updated_template = t(
        ui_locale.as_deref(),
        "ai.feedback.taskProfileUpdated",
        "Task profile `{slug}` updated.",
    );
    let session_started_template = t(
        ui_locale.as_deref(),
        "ai.feedback.sessionStarted",
        "Session `{title}` started.",
    );
    let alloy_completed_template = t(
        ui_locale.as_deref(),
        "ai.feedback.alloyCompleted",
        "Alloy job `{title}` completed.",
    );
    let image_completed_template = t(
        ui_locale.as_deref(),
        "ai.feedback.imageCompleted",
        "Image job `{title}` completed.",
    );
    let product_completed_template = t(
        ui_locale.as_deref(),
        "ai.feedback.productCompleted",
        "Product copy job `{title}` completed.",
    );
    let blog_completed_template = t(
        ui_locale.as_deref(),
        "ai.feedback.blogCompleted",
        "Blog draft job `{title}` completed.",
    );
    let err_select_provider_update = t(
        ui_locale.as_deref(),
        "ai.error.selectProviderBeforeUpdate",
        "Select a provider before updating it.",
    );
    let err_select_provider_test = t(
        ui_locale.as_deref(),
        "ai.error.selectProviderBeforeTest",
        "Select a provider before testing it.",
    );
    let err_select_provider_deactivate = t(
        ui_locale.as_deref(),
        "ai.error.selectProviderBeforeDeactivate",
        "Select a provider before deactivating it.",
    );
    let err_select_tool_update = t(
        ui_locale.as_deref(),
        "ai.error.selectToolProfileBeforeUpdate",
        "Select a tool profile before updating it.",
    );
    let err_select_task_update = t(
        ui_locale.as_deref(),
        "ai.error.selectTaskProfileBeforeUpdate",
        "Select a task profile before updating it.",
    );
    let err_select_session = t(
        ui_locale.as_deref(),
        "ai.error.selectSessionFirst",
        "Select a session first.",
    );
    let err_select_alloy_task = t(
        ui_locale.as_deref(),
        "ai.error.selectAlloyTaskProfile",
        "Select the `alloy_code` task profile before running Alloy Assist.",
    );
    let err_select_image_task = t(
        ui_locale.as_deref(),
        "ai.error.selectImageTaskProfile",
        "Select the `image_asset` task profile before generating a media image.",
    );
    let err_select_product_task = t(
        ui_locale.as_deref(),
        "ai.error.selectProductTaskProfile",
        "Select the `product_copy` task profile before generating localized product copy.",
    );
    let err_select_blog_task = t(
        ui_locale.as_deref(),
        "ai.error.selectBlogTaskProfile",
        "Select the `blog_draft` task profile before generating blog draft content.",
    );
    let err_alloy_payload = t(
        ui_locale.as_deref(),
        "ai.error.assembleAlloyPayload",
        "Failed to assemble Alloy task payload. Check the runtime payload JSON.",
    );
    let err_image_payload = t(
        ui_locale.as_deref(),
        "ai.error.assembleImagePayload",
        "Failed to assemble image task payload. Check prompt and size fields.",
    );
    let err_product_payload = t(
        ui_locale.as_deref(),
        "ai.error.assembleProductPayload",
        "Failed to assemble product copy payload. Check the product id.",
    );
    let err_blog_payload = t(
        ui_locale.as_deref(),
        "ai.error.assembleBlogPayload",
        "Failed to assemble blog draft payload. Check post/category ids.",
    );

    let session_query_writer = query_writer.clone();
    let provider_query_writer = query_writer.clone();
    let tool_query_writer = query_writer.clone();
    let task_query_writer = query_writer.clone();
    let overview_tab_query_writer = query_writer.clone();
    let diagnostics_tab_query_writer = query_writer.clone();
    let reset_provider_query_writer = query_writer.clone();
    let reset_tool_query_writer = query_writer.clone();
    let reset_task_query_writer = query_writer.clone();
    let create_provider_query_writer = query_writer.clone();
    let update_provider_query_writer = query_writer.clone();
    let deactivate_provider_query_writer = query_writer.clone();
    let create_tool_query_writer = query_writer.clone();
    let update_tool_query_writer = query_writer.clone();
    let create_task_query_writer = query_writer.clone();
    let update_task_query_writer = query_writer.clone();
    let start_session_query_writer = query_writer.clone();
    let alloy_session_query_writer = query_writer.clone();
    let image_session_query_writer = query_writer.clone();
    let product_session_query_writer = query_writer.clone();
    let blog_session_query_writer = query_writer.clone();

    Effect::new(
        move |_| match session_query.get().map(|value| value.trim().to_string()) {
            Some(session_id) if !session_id.is_empty() => {
                set_selected_session.set(Some(session_id))
            }
            _ => set_selected_session.set(None),
        },
    );

    Effect::new(move |_| {
        let requested_provider_slug = provider_slug_query.get();
        let requested_tool_slug = tool_profile_slug_query.get();
        let requested_task_slug = task_profile_slug_query.get();
        let requested_session_id = session_query.get();
        match bootstrap.get() {
            Some(Ok(bootstrap)) => {
                match requested_provider_slug
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                {
                    Some(slug) => {
                        if let Some(profile) = bootstrap
                            .providers
                            .iter()
                            .find(|profile| profile.slug == slug)
                        {
                            apply_provider_profile(
                                selected_provider,
                                provider_slug,
                                provider_name,
                                provider_kind,
                                provider_base_url,
                                provider_model,
                                provider_api_key,
                                provider_temperature,
                                provider_max_tokens,
                                provider_capabilities,
                                provider_allowed_tasks,
                                provider_denied_tasks,
                                provider_restricted_roles,
                                provider_active,
                                profile,
                            );
                        } else {
                            clear_provider_profile(
                                selected_provider,
                                provider_slug,
                                provider_name,
                                provider_kind,
                                provider_base_url,
                                provider_model,
                                provider_api_key,
                                provider_temperature,
                                provider_max_tokens,
                                provider_capabilities,
                                provider_allowed_tasks,
                                provider_denied_tasks,
                                provider_restricted_roles,
                                provider_active,
                            );
                            provider_query_writer.clear_key(AdminQueryKey::ProviderSlug.as_str());
                        }
                    }
                    None => clear_provider_profile(
                        selected_provider,
                        provider_slug,
                        provider_name,
                        provider_kind,
                        provider_base_url,
                        provider_model,
                        provider_api_key,
                        provider_temperature,
                        provider_max_tokens,
                        provider_capabilities,
                        provider_allowed_tasks,
                        provider_denied_tasks,
                        provider_restricted_roles,
                        provider_active,
                    ),
                }

                match requested_tool_slug
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                {
                    Some(slug) => {
                        if let Some(profile) = bootstrap
                            .tool_profiles
                            .iter()
                            .find(|profile| profile.slug == slug)
                        {
                            apply_tool_profile(
                                selected_tool_profile,
                                tool_slug,
                                tool_name,
                                tool_description,
                                tool_allowed,
                                tool_denied,
                                tool_sensitive,
                                tool_active,
                                profile,
                            );
                        } else {
                            clear_tool_profile(
                                selected_tool_profile,
                                tool_slug,
                                tool_name,
                                tool_description,
                                tool_allowed,
                                tool_denied,
                                tool_sensitive,
                                tool_active,
                            );
                            tool_query_writer.clear_key(AdminQueryKey::ToolProfileSlug.as_str());
                        }
                    }
                    None => clear_tool_profile(
                        selected_tool_profile,
                        tool_slug,
                        tool_name,
                        tool_description,
                        tool_allowed,
                        tool_denied,
                        tool_sensitive,
                        tool_active,
                    ),
                }

                match requested_task_slug
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                {
                    Some(slug) => {
                        if let Some(profile) = bootstrap
                            .task_profiles
                            .iter()
                            .find(|profile| profile.slug == slug)
                        {
                            apply_task_profile(
                                selected_task_profile,
                                task_slug,
                                task_name,
                                task_description,
                                task_capability,
                                task_system_prompt,
                                task_allowed_providers,
                                task_preferred_providers,
                                task_execution_mode,
                                task_active,
                                profile,
                            );
                        } else {
                            clear_task_profile(
                                selected_task_profile,
                                task_slug,
                                task_name,
                                task_description,
                                task_capability,
                                task_system_prompt,
                                task_allowed_providers,
                                task_preferred_providers,
                                task_execution_mode,
                                task_active,
                            );
                            task_query_writer.clear_key(AdminQueryKey::TaskProfileSlug.as_str());
                        }
                    }
                    None => clear_task_profile(
                        selected_task_profile,
                        task_slug,
                        task_name,
                        task_description,
                        task_capability,
                        task_system_prompt,
                        task_allowed_providers,
                        task_preferred_providers,
                        task_execution_mode,
                        task_active,
                    ),
                }

                if let Some(session_id) = requested_session_id
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                {
                    if !bootstrap
                        .sessions
                        .iter()
                        .any(|session| session.id == session_id)
                    {
                        set_selected_session.set(None);
                        session_query_writer.clear_key(AdminQueryKey::SessionId.as_str());
                    }
                }
            }
            _ => {}
        }
    });

    #[cfg(target_arch = "wasm32")]
    let live_ui_locale = ui_locale.clone();
    Effect::new(move |_| {
        let session_id = selected_session.get();
        let token_value = token.get();
        let tenant_value = tenant.get();
        #[cfg(target_arch = "wasm32")]
        let ui_locale_value = live_ui_locale.clone();
        if session_id.is_none() {
            set_live_stream.set(None);
            #[cfg(target_arch = "wasm32")]
            replace_live_subscription(None);
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = (token_value, tenant_value);
            set_live_stream.set(None);
        }

        #[cfg(target_arch = "wasm32")]
        {
            let Some(session_id) = session_id else {
                set_live_stream.set(None);
                replace_live_subscription(None);
                return;
            };
            let Some(token_value) = token_value else {
                set_live_stream.set(None);
                replace_live_subscription(None);
                return;
            };
            let Some(tenant_value) = tenant_value else {
                set_live_stream.set(None);
                replace_live_subscription(None);
                return;
            };

            let generation = next_live_subscription_generation();

            set_live_stream.set(Some(AiLiveStreamStatePayload {
                run_id: String::new(),
                status: "CONNECTING".to_string(),
                content: String::new(),
                error_message: None,
                connected: false,
            }));

            let ws = match WebSocket::new_with_str(&graphql_ws_url(), "graphql-transport-ws") {
                Ok(ws) => ws,
                Err(_) => {
                    set_live_stream.set(None);
                    replace_live_subscription(None);
                    return;
                }
            };

            let init_message = serde_json::json!({
                "type": "connection_init",
                "payload": {
                    "token": token_value,
                    "tenantSlug": tenant_value,
                    "locale": browser_admin_locale(ui_locale_value.as_deref()),
                }
            })
            .to_string();
            let subscribe_message = serde_json::json!({
                "id": "ai-session-events",
                "type": "subscribe",
                "payload": {
                    "query": AI_SESSION_EVENTS_SUBSCRIPTION,
                    "variables": {
                        "sessionId": session_id,
                    }
                }
            })
            .to_string();

            let ws_for_open = ws.clone();
            let on_open = Closure::<dyn FnMut(Event)>::new(move |_| {
                let _ = ws_for_open.send_with_str(&init_message);
            });

            let ws_for_message = ws.clone();
            let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
                let Some(text) = event.data().as_string() else {
                    return;
                };

                let Ok(message) = serde_json::from_str::<AiSessionSubscriptionEnvelope>(&text)
                else {
                    return;
                };

                match message {
                    AiSessionSubscriptionEnvelope::ConnectionAck => {
                        let _ = ws_for_message.send_with_str(&subscribe_message);
                        set_live_stream.update(|state| {
                            if let Some(state) = state.as_mut() {
                                state.connected = true;
                            }
                        });
                    }
                    AiSessionSubscriptionEnvelope::Next { payload } => {
                        if payload
                            .errors
                            .as_ref()
                            .is_some_and(|errors| !errors.is_empty())
                        {
                            set_live_stream.update(|state| {
                                if let Some(state) = state.as_mut() {
                                    state.connected = false;
                                    state.status = "ERROR".to_string();
                                }
                            });
                            return;
                        }

                        if let Some(event) = payload.data.and_then(|data| data.ai_session_events) {
                            let status = match event.event_kind {
                                AiRunStreamEventKindPayload::Started => "STARTED",
                                AiRunStreamEventKindPayload::Delta => "STREAMING",
                                AiRunStreamEventKindPayload::Completed => "COMPLETED",
                                AiRunStreamEventKindPayload::Failed => "FAILED",
                                AiRunStreamEventKindPayload::WaitingApproval => "WAITING_APPROVAL",
                            }
                            .to_string();
                            let content = event
                                .accumulated_content
                                .or(event.content_delta)
                                .unwrap_or_default();
                            let is_terminal = matches!(
                                event.event_kind,
                                AiRunStreamEventKindPayload::Completed
                                    | AiRunStreamEventKindPayload::Failed
                                    | AiRunStreamEventKindPayload::WaitingApproval
                            );

                            set_live_stream.set(Some(AiLiveStreamStatePayload {
                                run_id: event.run_id,
                                status,
                                content,
                                error_message: event.error_message,
                                connected: true,
                            }));

                            if is_terminal {
                                set_refresh_nonce.update(|value| *value += 1);
                            }
                        }
                    }
                    AiSessionSubscriptionEnvelope::Error { payload } => {
                        let message = payload
                            .into_iter()
                            .find(|item| !item.message.trim().is_empty())
                            .map(|item| item.message);
                        set_live_stream.update(|state| {
                            if let Some(state) = state.as_mut() {
                                state.connected = false;
                                state.status = "ERROR".to_string();
                                state.error_message = message.clone();
                            } else {
                                *state = Some(AiLiveStreamStatePayload {
                                    run_id: String::new(),
                                    status: "ERROR".to_string(),
                                    content: String::new(),
                                    error_message: message.clone(),
                                    connected: false,
                                });
                            }
                        });
                    }
                    AiSessionSubscriptionEnvelope::Ping { payload } => {
                        let pong = serde_json::json!({
                            "type": "pong",
                            "payload": payload,
                        })
                        .to_string();
                        let _ = ws_for_message.send_with_str(&pong);
                    }
                    AiSessionSubscriptionEnvelope::Complete => {
                        set_live_stream.update(|state| {
                            if let Some(state) = state.as_mut() {
                                state.connected = false;
                            }
                        });
                    }
                    AiSessionSubscriptionEnvelope::Pong => {}
                }
            });

            let on_error = Closure::<dyn FnMut(ErrorEvent)>::new(move |_| {
                set_live_stream.update(|state| {
                    if let Some(state) = state.as_mut() {
                        state.connected = false;
                        state.status = "ERROR".to_string();
                    }
                });
            });

            let on_close = Closure::<dyn FnMut(CloseEvent)>::new(move |_| {
                set_live_stream.update(|state| {
                    if let Some(state) = state.as_mut() {
                        state.connected = false;
                    }
                });
            });

            ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
            ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
            ws.set_onerror(Some(on_error.as_ref().unchecked_ref()));
            ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));

            replace_live_subscription(Some(AiLiveSubscriptionHandle {
                generation,
                ws: ws.clone(),
                on_open,
                on_message,
                on_error,
                on_close,
            }));

            on_cleanup(move || {
                clear_live_subscription_generation(generation);
            });
        }
    });

    let on_create_provider = move |ev: SubmitEvent| {
        ev.prevent_default();
        set_feedback.set(None);
        set_error.set(None);
        let provider_created_template = provider_created_template.clone();
        let create_provider_query_writer = create_provider_query_writer.clone();
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
                    set_feedback.set(Some(
                        provider_created_template.replace("{slug}", profile.slug.as_str()),
                    ));
                    selected_provider.set(profile.id.clone());
                    create_provider_query_writer
                        .replace_value(AdminQueryKey::ProviderSlug.as_str(), profile.slug.clone());
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let reset_provider_form = move || {
        reset_provider_query_writer.clear_key(AdminQueryKey::ProviderSlug.as_str());
        clear_provider_profile(
            selected_provider,
            provider_slug,
            provider_name,
            provider_kind,
            provider_base_url,
            provider_model,
            provider_api_key,
            provider_temperature,
            provider_max_tokens,
            provider_capabilities,
            provider_allowed_tasks,
            provider_denied_tasks,
            provider_restricted_roles,
            provider_active,
        );
    };

    let on_update_provider = move |_| {
        let provider_id = selected_provider.get_untracked();
        if provider_id.trim().is_empty() {
            set_error.set(Some(err_select_provider_update.clone()));
            return;
        }
        set_feedback.set(None);
        set_error.set(None);
        let provider_updated_template = provider_updated_template.clone();
        let update_provider_query_writer = update_provider_query_writer.clone();
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
                    set_feedback.set(Some(
                        provider_updated_template.replace("{slug}", profile.slug.as_str()),
                    ));
                    update_provider_query_writer
                        .replace_value(AdminQueryKey::ProviderSlug.as_str(), profile.slug.clone());
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let on_test_provider = move |_| {
        let provider_id = selected_provider.get_untracked();
        if provider_id.trim().is_empty() {
            set_error.set(Some(err_select_provider_test.clone()));
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
            set_error.set(Some(err_select_provider_deactivate.clone()));
            return;
        }
        set_feedback.set(None);
        set_error.set(None);
        let provider_deactivated_template = provider_deactivated_template.clone();
        let deactivate_provider_query_writer = deactivate_provider_query_writer.clone();
        spawn_local(async move {
            match api::deactivate_provider(provider_id).await {
                Ok(profile) => {
                    provider_active.set(false);
                    set_feedback.set(Some(
                        provider_deactivated_template.replace("{slug}", profile.slug.as_str()),
                    ));
                    deactivate_provider_query_writer
                        .replace_value(AdminQueryKey::ProviderSlug.as_str(), profile.slug.clone());
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
        let tool_created_template = tool_created_template.clone();
        let create_tool_query_writer = create_tool_query_writer.clone();
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
                    set_feedback.set(Some(
                        tool_created_template.replace("{slug}", profile.slug.as_str()),
                    ));
                    selected_tool_profile.set(profile.id.clone());
                    create_tool_query_writer.replace_value(
                        AdminQueryKey::ToolProfileSlug.as_str(),
                        profile.slug.clone(),
                    );
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let reset_tool_form = move || {
        reset_tool_query_writer.clear_key(AdminQueryKey::ToolProfileSlug.as_str());
        clear_tool_profile(
            selected_tool_profile,
            tool_slug,
            tool_name,
            tool_description,
            tool_allowed,
            tool_denied,
            tool_sensitive,
            tool_active,
        );
    };

    let on_update_tool_profile = move |_| {
        let tool_profile_id = selected_tool_profile.get_untracked();
        if tool_profile_id.trim().is_empty() {
            set_error.set(Some(err_select_tool_update.clone()));
            return;
        }
        set_feedback.set(None);
        set_error.set(None);
        let tool_updated_template = tool_updated_template.clone();
        let update_tool_query_writer = update_tool_query_writer.clone();
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
                    set_feedback.set(Some(
                        tool_updated_template.replace("{slug}", profile.slug.as_str()),
                    ));
                    update_tool_query_writer.replace_value(
                        AdminQueryKey::ToolProfileSlug.as_str(),
                        profile.slug.clone(),
                    );
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
        let session_started_template = session_started_template.clone();
        let start_session_query_writer = start_session_query_writer.clone();
        spawn_local(async move {
            let result = api::start_session(
                session_title.get_untracked(),
                optional_text(selected_provider.get_untracked()),
                optional_text(selected_task_profile.get_untracked()),
                optional_text(selected_tool_profile.get_untracked()),
                optional_text(session_locale.get_untracked()),
                optional_text(session_message.get_untracked()),
            )
            .await;
            match result {
                Ok(result) => {
                    let session_id = result.session.session.id.clone();
                    set_selected_session.set(Some(session_id.clone()));
                    start_session_query_writer
                        .replace_value(AdminQueryKey::SessionId.as_str(), session_id);
                    set_feedback.set(Some(
                        session_started_template
                            .replace("{title}", result.session.session.title.as_str()),
                    ));
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let on_run_alloy_job = move |ev: SubmitEvent| {
        ev.prevent_default();
        let task_profile_id = selected_task_profile.get_untracked();
        if task_profile_id.trim().is_empty() {
            set_error.set(Some(err_select_alloy_task.clone()));
            return;
        }

        let payload = alloy_task_payload(
            alloy_operation.get_untracked(),
            optional_text(alloy_script_id.get_untracked()),
            optional_text(alloy_script_name.get_untracked()),
            optional_text(alloy_script_source.get_untracked()),
            optional_text(alloy_runtime_payload.get_untracked()),
            optional_text(alloy_prompt.get_untracked()),
        );
        let Ok(payload) = payload else {
            set_error.set(Some(err_alloy_payload.clone()));
            return;
        };

        set_feedback.set(None);
        set_error.set(None);
        let alloy_completed_template = alloy_completed_template.clone();
        let alloy_session_query_writer = alloy_session_query_writer.clone();
        spawn_local(async move {
            let result = api::run_task_job(
                alloy_title.get_untracked(),
                optional_text(selected_provider.get_untracked()),
                task_profile_id,
                Some("direct".to_string()),
                optional_text(alloy_locale.get_untracked()),
                payload,
            )
            .await;
            match result {
                Ok(result) => {
                    let session_id = result.session.session.id.clone();
                    set_selected_session.set(Some(session_id.clone()));
                    alloy_session_query_writer
                        .replace_value(AdminQueryKey::SessionId.as_str(), session_id);
                    set_feedback.set(Some(
                        alloy_completed_template
                            .replace("{title}", result.session.session.title.as_str()),
                    ));
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let on_run_image_job = move |ev: SubmitEvent| {
        ev.prevent_default();
        let task_profile_id = selected_task_profile.get_untracked();
        if task_profile_id.trim().is_empty() {
            set_error.set(Some(err_select_image_task.clone()));
            return;
        }

        let payload = image_task_payload(
            image_prompt.get_untracked(),
            optional_text(image_negative_prompt.get_untracked()),
            optional_text(image_asset_title.get_untracked()),
            optional_text(image_alt_text.get_untracked()),
            optional_text(image_caption.get_untracked()),
            optional_text(image_file_name.get_untracked()),
            optional_text(image_size.get_untracked()),
            optional_text(image_assistant_prompt.get_untracked()),
        );
        let Ok(payload) = payload else {
            set_error.set(Some(err_image_payload.clone()));
            return;
        };

        set_feedback.set(None);
        set_error.set(None);
        let image_completed_template = image_completed_template.clone();
        let image_session_query_writer = image_session_query_writer.clone();
        spawn_local(async move {
            let result = api::run_task_job(
                image_title.get_untracked(),
                optional_text(selected_provider.get_untracked()),
                task_profile_id,
                Some("direct".to_string()),
                optional_text(image_locale.get_untracked()),
                payload,
            )
            .await;
            match result {
                Ok(result) => {
                    let session_id = result.session.session.id.clone();
                    set_selected_session.set(Some(session_id.clone()));
                    image_session_query_writer
                        .replace_value(AdminQueryKey::SessionId.as_str(), session_id);
                    set_feedback.set(Some(
                        image_completed_template
                            .replace("{title}", result.session.session.title.as_str()),
                    ));
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let on_run_product_job = move |ev: SubmitEvent| {
        ev.prevent_default();
        let task_profile_id = selected_task_profile.get_untracked();
        if task_profile_id.trim().is_empty() {
            set_error.set(Some(err_select_product_task.clone()));
            return;
        }

        let payload = product_task_payload(
            product_id.get_untracked(),
            optional_text(product_source_locale.get_untracked()),
            optional_text(product_source_title.get_untracked()),
            optional_text(product_source_description.get_untracked()),
            optional_text(product_source_meta_title.get_untracked()),
            optional_text(product_source_meta_description.get_untracked()),
            optional_text(product_copy_instructions.get_untracked()),
            optional_text(product_assistant_prompt.get_untracked()),
        );
        let Ok(payload) = payload else {
            set_error.set(Some(err_product_payload.clone()));
            return;
        };

        set_feedback.set(None);
        set_error.set(None);
        let product_completed_template = product_completed_template.clone();
        let product_session_query_writer = product_session_query_writer.clone();
        spawn_local(async move {
            let result = api::run_task_job(
                product_title.get_untracked(),
                optional_text(selected_provider.get_untracked()),
                task_profile_id,
                Some("direct".to_string()),
                optional_text(product_locale.get_untracked()),
                payload,
            )
            .await;
            match result {
                Ok(result) => {
                    let session_id = result.session.session.id.clone();
                    set_selected_session.set(Some(session_id.clone()));
                    product_session_query_writer
                        .replace_value(AdminQueryKey::SessionId.as_str(), session_id);
                    set_feedback.set(Some(
                        product_completed_template
                            .replace("{title}", result.session.session.title.as_str()),
                    ));
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let on_run_blog_job = move |ev: SubmitEvent| {
        ev.prevent_default();
        let task_profile_id = selected_task_profile.get_untracked();
        if task_profile_id.trim().is_empty() {
            set_error.set(Some(err_select_blog_task.clone()));
            return;
        }

        let payload = blog_task_payload(
            optional_text(blog_post_id.get_untracked()),
            optional_text(blog_source_locale.get_untracked()),
            optional_text(blog_source_title.get_untracked()),
            optional_text(blog_source_body.get_untracked()),
            optional_text(blog_source_excerpt.get_untracked()),
            optional_text(blog_source_seo_title.get_untracked()),
            optional_text(blog_source_seo_description.get_untracked()),
            parse_csv(blog_tags.get_untracked()),
            optional_text(blog_category_id.get_untracked()),
            optional_text(blog_featured_image_url.get_untracked()),
            optional_text(blog_copy_instructions.get_untracked()),
            optional_text(blog_assistant_prompt.get_untracked()),
        );
        let Ok(payload) = payload else {
            set_error.set(Some(err_blog_payload.clone()));
            return;
        };

        set_feedback.set(None);
        set_error.set(None);
        let blog_completed_template = blog_completed_template.clone();
        let blog_session_query_writer = blog_session_query_writer.clone();
        spawn_local(async move {
            let result = api::run_task_job(
                blog_title.get_untracked(),
                optional_text(selected_provider.get_untracked()),
                task_profile_id,
                Some("direct".to_string()),
                optional_text(blog_locale.get_untracked()),
                payload,
            )
            .await;
            match result {
                Ok(result) => {
                    let session_id = result.session.session.id.clone();
                    set_selected_session.set(Some(session_id.clone()));
                    blog_session_query_writer
                        .replace_value(AdminQueryKey::SessionId.as_str(), session_id);
                    set_feedback.set(Some(
                        blog_completed_template
                            .replace("{title}", result.session.session.title.as_str()),
                    ));
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let reset_task_form = move || {
        reset_task_query_writer.clear_key(AdminQueryKey::TaskProfileSlug.as_str());
        clear_task_profile(
            selected_task_profile,
            task_slug,
            task_name,
            task_description,
            task_capability,
            task_system_prompt,
            task_allowed_providers,
            task_preferred_providers,
            task_execution_mode,
            task_active,
        );
    };

    let on_create_task_profile = move |ev: SubmitEvent| {
        ev.prevent_default();
        set_feedback.set(None);
        set_error.set(None);
        let task_created_template = task_created_template.clone();
        let create_task_query_writer = create_task_query_writer.clone();
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
                    set_feedback.set(Some(
                        task_created_template.replace("{slug}", profile.slug.as_str()),
                    ));
                    selected_task_profile.set(profile.id.clone());
                    create_task_query_writer.replace_value(
                        AdminQueryKey::TaskProfileSlug.as_str(),
                        profile.slug.clone(),
                    );
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let on_update_task_profile = move |_| {
        let task_profile_id = selected_task_profile.get_untracked();
        if task_profile_id.trim().is_empty() {
            set_error.set(Some(err_select_task_update.clone()));
            return;
        }
        set_feedback.set(None);
        set_error.set(None);
        let task_updated_template = task_updated_template.clone();
        let update_task_query_writer = update_task_query_writer.clone();
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
                    set_feedback.set(Some(
                        task_updated_template.replace("{slug}", profile.slug.as_str()),
                    ));
                    update_task_query_writer.replace_value(
                        AdminQueryKey::TaskProfileSlug.as_str(),
                        profile.slug.clone(),
                    );
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let on_send_message = move |ev: SubmitEvent| {
        ev.prevent_default();
        let Some(session_id) = selected_session.get_untracked() else {
            set_error.set(Some(err_select_session.clone()));
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
                        {badge_label.clone()}
                    </span>
                    <h1 class="text-2xl font-semibold text-card-foreground">{page_title_label.clone()}</h1>
                    <p class="max-w-3xl text-sm text-muted-foreground">
                        {page_subtitle_label.clone()}
                    </p>
                </div>
                <div class="mt-4 flex flex-wrap gap-2 text-sm">
                    <button
                        type="button"
                        class=move || {
                            if diagnostics_only.get() {
                                "rounded-full border border-border px-3 py-1.5 text-muted-foreground"
                            } else {
                                "rounded-full border border-primary bg-primary/10 px-3 py-1.5 font-medium text-primary"
                            }
                        }
                        on:click=move |_| overview_tab_query_writer.replace_value(AdminQueryKey::Tab.as_str(), "overview")
                    >
                        {overview_label.clone()}
                    </button>
                    <button
                        type="button"
                        class=move || {
                            if diagnostics_only.get() {
                                "rounded-full border border-primary bg-primary/10 px-3 py-1.5 font-medium text-primary"
                            } else {
                                "rounded-full border border-border px-3 py-1.5 text-muted-foreground"
                            }
                        }
                        on:click=move |_| diagnostics_tab_query_writer.replace_value(AdminQueryKey::Tab.as_str(), "diagnostics")
                    >
                        {diagnostics_label.clone()}
                    </button>
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
                {move || {
                    let ui_locale = ui_locale.clone();
                    let ui_locale_providers = ui_locale.clone();
                    let ui_locale_tools = ui_locale.clone();
                    let ui_locale_tasks = ui_locale.clone();
                    let ui_locale_diagnostics = ui_locale.clone();
                    let ui_locale_blog = ui_locale.clone();
                    let ui_locale_product = ui_locale.clone();
                    let ui_locale_image = ui_locale.clone();
                    let ui_locale_alloy = ui_locale.clone();
                    let ui_locale_new_session = ui_locale.clone();
                    let ui_locale_sessions = ui_locale.clone();
                    let ui_locale_operator = ui_locale.clone();
                    let on_create_provider = on_create_provider.clone();
                    let on_update_provider = on_update_provider.clone();
                    let on_test_provider = on_test_provider.clone();
                    let on_deactivate_provider = on_deactivate_provider.clone();
                    let reset_provider_form = reset_provider_form.clone();
                    let on_create_tool_profile = on_create_tool_profile.clone();
                    let on_update_tool_profile = on_update_tool_profile.clone();
                    let reset_tool_form = reset_tool_form.clone();
                    let on_create_task_profile = on_create_task_profile.clone();
                    let on_update_task_profile = on_update_task_profile.clone();
                    let reset_task_form = reset_task_form.clone();
                    let on_run_blog_job = on_run_blog_job.clone();
                    let on_run_product_job = on_run_product_job.clone();
                    let on_run_image_job = on_run_image_job.clone();
                    let on_run_alloy_job = on_run_alloy_job.clone();
                    let on_start_session = on_start_session.clone();
                    let on_send_message = on_send_message.clone();
                    let blog_transport_locale = ui_locale.clone();
                    let product_transport_locale = ui_locale.clone();
                    let image_transport_locale = ui_locale.clone();
                    let alloy_transport_locale = ui_locale.clone();
                    let session_transport_locale = ui_locale.clone();
                    let select_provider_query_writer = query_writer.clone();
                    let select_tool_query_writer = query_writer.clone();
                    let select_task_query_writer = query_writer.clone();
                    let select_session_query_writer = query_writer.clone();
                    bootstrap.get().map(|result| match result {
                    Ok(bootstrap) => view! {
                        <div class=if diagnostics_only.get() {
                            "grid gap-6 xl:grid-cols-[1fr_1.6fr]".to_string()
                        } else {
                            "grid gap-6 xl:grid-cols-[1.2fr_1fr_1.6fr]".to_string()
                        }>
                            {if !diagnostics_only.get() { view! {
                            <section class="space-y-6">
                                <Card title=t(ui_locale_providers.as_deref(), "ai.card.providers", "Providers")>
                                    <form class="space-y-3" on:submit=on_create_provider.clone()>
                                        <TextField label=t(ui_locale_providers.as_deref(), "ai.field.slug", "Slug") value=provider_slug />
                                        <TextField label=t(ui_locale_providers.as_deref(), "ai.field.displayName", "Display name") value=provider_name />
                                        <TextField label=t(ui_locale_providers.as_deref(), "ai.field.providerKind", "Provider kind") value=provider_kind />
                                        <TextField label=t(ui_locale_providers.as_deref(), "ai.field.baseUrl", "Base URL") value=provider_base_url />
                                        <TextField label=t(ui_locale_providers.as_deref(), "ai.field.model", "Model") value=provider_model />
                                        <TextField label=t(ui_locale_providers.as_deref(), "ai.field.apiKey", "API key") value=provider_api_key />
                                        <TextField label=t(ui_locale_providers.as_deref(), "ai.field.temperature", "Temperature") value=provider_temperature />
                                        <TextField label=t(ui_locale_providers.as_deref(), "ai.field.maxTokens", "Max tokens") value=provider_max_tokens />
                                        <TextField label=t(ui_locale_providers.as_deref(), "ai.field.capabilitiesCsv", "Capabilities (csv)") value=provider_capabilities />
                                        <TextField label=t(ui_locale_providers.as_deref(), "ai.field.allowedTasksCsv", "Allowed tasks (csv)") value=provider_allowed_tasks />
                                        <TextField label=t(ui_locale_providers.as_deref(), "ai.field.deniedTasksCsv", "Denied tasks (csv)") value=provider_denied_tasks />
                                        <TextField label=t(ui_locale_providers.as_deref(), "ai.field.restrictedRolesCsv", "Restricted roles (csv)") value=provider_restricted_roles />
                                        <label class="flex items-center gap-2 text-sm text-muted-foreground">
                                            <input
                                                type="checkbox"
                                                prop:checked=provider_active
                                                on:change=move |ev| provider_active.set(event_target_checked(&ev))
                                            />
                                            {t(ui_locale_providers.as_deref(), "ai.field.active", "Active")}
                                        </label>
                                        <div class="flex flex-wrap gap-2">
                                            <button type="submit" class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground">{t(ui_locale_providers.as_deref(), "ai.action.createProvider", "Create provider")}</button>
                                            <button type="button" class="rounded-lg border border-border px-4 py-2 text-sm font-medium" on:click=on_update_provider.clone()>{t(ui_locale_providers.as_deref(), "ai.action.updateSelected", "Update selected")}</button>
                                            <button type="button" class="rounded-lg border border-border px-4 py-2 text-sm font-medium" on:click=on_test_provider.clone()>{t(ui_locale_providers.as_deref(), "ai.action.testSelected", "Test selected")}</button>
                                            <button type="button" class="rounded-lg border border-destructive/40 px-4 py-2 text-sm font-medium text-destructive" on:click=on_deactivate_provider.clone()>{t(ui_locale_providers.as_deref(), "ai.action.deactivate", "Deactivate")}</button>
                                            <button type="button" class="rounded-lg border border-border px-4 py-2 text-sm font-medium" on:click=move |_| reset_provider_form()>{t(ui_locale_providers.as_deref(), "ai.action.reset", "Reset")}</button>
                                        </div>
                                    </form>
                                    <div class="mt-4 space-y-2">
                                        {bootstrap.providers.into_iter().map(|provider| {
                                            let provider_slug_value = provider.slug.clone();
                                            let provider_query_writer = select_provider_query_writer.clone();
                                            view! {
                                                <button
                                                    class="w-full rounded-lg border border-border px-3 py-3 text-left text-sm hover:bg-muted"
                                                    on:click=move |_| {
                                                        provider_query_writer.replace_value(
                                                            AdminQueryKey::ProviderSlug.as_str(),
                                                            provider_slug_value.clone(),
                                                        );
                                                    }
                                                >
                                                    <div class="font-medium">{provider.display_name}</div>
                                                    <div class="text-muted-foreground">
                                                        {provider_profile_summary(
                                                            ui_locale_providers.as_deref(),
                                                            provider.provider_kind.as_str(),
                                                            provider.model.as_str(),
                                                            provider.capabilities.len(),
                                                            provider.is_active,
                                                        )}
                                                    </div>
                                                </button>
                                            }
                                        }).collect_view()}
                                    </div>
                                </Card>

                                <Card title=t(ui_locale_tools.as_deref(), "ai.card.toolProfiles", "Tool Profiles")>
                                    <form class="space-y-3" on:submit=on_create_tool_profile.clone()>
                                        <TextField label=t(ui_locale_tools.as_deref(), "ai.field.slug", "Slug") value=tool_slug />
                                        <TextField label=t(ui_locale_tools.as_deref(), "ai.field.displayName", "Display name") value=tool_name />
                                        <TextField label=t(ui_locale_tools.as_deref(), "ai.field.description", "Description") value=tool_description />
                                        <TextField label=t(ui_locale_tools.as_deref(), "ai.field.allowedToolsCsv", "Allowed tools (csv)") value=tool_allowed />
                                        <TextField label=t(ui_locale_tools.as_deref(), "ai.field.deniedToolsCsv", "Denied tools (csv)") value=tool_denied />
                                        <TextField label=t(ui_locale_tools.as_deref(), "ai.field.sensitiveToolsCsv", "Sensitive tools (csv)") value=tool_sensitive />
                                        <label class="flex items-center gap-2 text-sm text-muted-foreground">
                                            <input
                                                type="checkbox"
                                                prop:checked=tool_active
                                                on:change=move |ev| tool_active.set(event_target_checked(&ev))
                                            />
                                            {t(ui_locale_tools.as_deref(), "ai.field.active", "Active")}
                                        </label>
                                        <div class="flex flex-wrap gap-2">
                                            <button type="submit" class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground">{t(ui_locale_tools.as_deref(), "ai.action.createToolProfile", "Create tool profile")}</button>
                                            <button type="button" class="rounded-lg border border-border px-4 py-2 text-sm font-medium" on:click=on_update_tool_profile.clone()>{t(ui_locale_tools.as_deref(), "ai.action.updateSelected", "Update selected")}</button>
                                            <button type="button" class="rounded-lg border border-border px-4 py-2 text-sm font-medium" on:click=move |_| reset_tool_form()>{t(ui_locale_tools.as_deref(), "ai.action.reset", "Reset")}</button>
                                        </div>
                                    </form>
                                    <div class="mt-4 space-y-2">
                                        {bootstrap.tool_profiles.into_iter().map(|profile| {
                                            let profile_slug_value = profile.slug.clone();
                                            let tool_query_writer = select_tool_query_writer.clone();
                                            view! {
                                                <button
                                                    class="w-full rounded-lg border border-border px-3 py-3 text-left text-sm hover:bg-muted"
                                                    on:click=move |_| {
                                                        tool_query_writer.replace_value(
                                                            AdminQueryKey::ToolProfileSlug.as_str(),
                                                            profile_slug_value.clone(),
                                                        );
                                                    }
                                                >
                                                    <div class="font-medium">{profile.display_name}</div>
                                                    <div class="text-muted-foreground">
                                                        {tool_profile_summary(
                                                            ui_locale_tools.as_deref(),
                                                            profile.allowed_tools.len(),
                                                            profile.sensitive_tools.len(),
                                                            profile.is_active,
                                                        )}
                                                    </div>
                                                </button>
                                            }
                                        }).collect_view()}
                                    </div>
                                </Card>

                                <Card title=t(ui_locale_tasks.as_deref(), "ai.card.taskProfiles", "Task Profiles")>
                                    <form class="space-y-3" on:submit=on_create_task_profile.clone()>
                                        <TextField label=t(ui_locale_tasks.as_deref(), "ai.field.slug", "Slug") value=task_slug />
                                        <TextField label=t(ui_locale_tasks.as_deref(), "ai.field.displayName", "Display name") value=task_name />
                                        <TextField label=t(ui_locale_tasks.as_deref(), "ai.field.description", "Description") value=task_description />
                                        <TextField label=t(ui_locale_tasks.as_deref(), "ai.field.targetCapability", "Target capability") value=task_capability />
                                        <TextField label=t(ui_locale_tasks.as_deref(), "ai.field.systemPrompt", "System prompt") value=task_system_prompt />
                                        <TextField label=t(ui_locale_tasks.as_deref(), "ai.field.allowedProviderIdsCsv", "Allowed provider ids (csv)") value=task_allowed_providers />
                                        <TextField label=t(ui_locale_tasks.as_deref(), "ai.field.preferredProviderIdsCsv", "Preferred provider ids (csv)") value=task_preferred_providers />
                                        <TextField label=t(ui_locale_tasks.as_deref(), "ai.field.executionMode", "Execution mode") value=task_execution_mode />
                                        <label class="flex items-center gap-2 text-sm text-muted-foreground">
                                            <input
                                                type="checkbox"
                                                prop:checked=task_active
                                                on:change=move |ev| task_active.set(event_target_checked(&ev))
                                            />
                                            {t(ui_locale_tasks.as_deref(), "ai.field.active", "Active")}
                                        </label>
                                        <div class="flex flex-wrap gap-2">
                                            <button type="submit" class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground">{t(ui_locale_tasks.as_deref(), "ai.action.createTaskProfile", "Create task profile")}</button>
                                            <button type="button" class="rounded-lg border border-border px-4 py-2 text-sm font-medium" on:click=on_update_task_profile.clone()>{t(ui_locale_tasks.as_deref(), "ai.action.updateSelected", "Update selected")}</button>
                                            <button type="button" class="rounded-lg border border-border px-4 py-2 text-sm font-medium" on:click=move |_| reset_task_form()>{t(ui_locale_tasks.as_deref(), "ai.action.reset", "Reset")}</button>
                                        </div>
                                    </form>
                                    <div class="mt-4 space-y-2">
                                        {bootstrap.task_profiles.into_iter().map(|profile| {
                                            let profile_slug_value = profile.slug.clone();
                                            let task_query_writer = select_task_query_writer.clone();
                                            view! {
                                                <button
                                                    class="w-full rounded-lg border border-border px-3 py-3 text-left text-sm hover:bg-muted"
                                                    on:click=move |_| {
                                                        task_query_writer.replace_value(
                                                            AdminQueryKey::TaskProfileSlug.as_str(),
                                                            profile_slug_value.clone(),
                                                        );
                                                    }
                                                >
                                                    <div class="font-medium">{profile.display_name}</div>
                                                    <div class="text-muted-foreground">
                                                        {task_profile_summary(
                                                            ui_locale_tasks.as_deref(),
                                                            profile.target_capability.as_str(),
                                                            profile.default_execution_mode.as_str(),
                                                            profile.is_active,
                                                        )}
                                                    </div>
                                                </button>
                                            }
                                        }).collect_view()}
                                    </div>
                                </Card>
                            </section>
                            }.into_any() } else { ().into_any() }}

                            <section class="space-y-6">
                                <Card title=t(ui_locale_diagnostics.as_deref(), "ai.card.diagnostics", "Diagnostics")>
                                    <div class="grid gap-3 sm:grid-cols-2">
                                        <InfoItem
                                            label=t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.routerResolutions", "Router resolutions")
                                            value=bootstrap.metrics.router_resolutions_total.to_string()
                                        />
                                        <InfoItem
                                            label=t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.overrides", "Overrides")
                                            value=bootstrap.metrics.router_overrides_total.to_string()
                                        />
                                        <InfoItem
                                            label=t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.completedRuns", "Completed runs")
                                            value=bootstrap.metrics.completed_runs_total.to_string()
                                        />
                                        <InfoItem
                                            label=t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.failedRuns", "Failed runs")
                                            value=bootstrap.metrics.failed_runs_total.to_string()
                                        />
                                        <InfoItem
                                            label=t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.waitingApproval", "Waiting approval")
                                            value=bootstrap.metrics.waiting_approval_runs_total.to_string()
                                        />
                                        <InfoItem
                                            label=t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.localeFallbacks", "Locale fallbacks")
                                            value=bootstrap.metrics.locale_fallback_total.to_string()
                                        />
                                        <InfoItem
                                            label=t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.directSelected", "Direct selected")
                                            value=bootstrap.metrics.selected_direct_total.to_string()
                                        />
                                        <InfoItem
                                            label=t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.mcpSelected", "MCP selected")
                                            value=bootstrap.metrics.selected_mcp_total.to_string()
                                        />
                                    </div>
                                    <div class="mt-4 space-y-3 text-sm text-muted-foreground">
                                        <div>
                                            {average_run_latency_summary(
                                                ui_locale_diagnostics.as_deref(),
                                                if bootstrap.metrics.run_latency_samples == 0 {
                                                    0
                                                } else {
                                                    bootstrap.metrics.run_latency_ms_total / bootstrap.metrics.run_latency_samples
                                                }
                                            )}
                                        </div>
                                        <div>
                                            <div class="font-medium text-foreground">{t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.providerBuckets", "Provider buckets")}</div>
                                            <div>{bucket_summary(ui_locale_diagnostics.as_deref(), &bootstrap.metrics.provider_kind_totals)}</div>
                                        </div>
                                        <div>
                                            <div class="font-medium text-foreground">{t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.executionTargets", "Execution targets")}</div>
                                            <div>{bucket_summary(ui_locale_diagnostics.as_deref(), &bootstrap.metrics.execution_target_totals)}</div>
                                        </div>
                                        <div>
                                            <div class="font-medium text-foreground">{t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.taskProfiles", "Task profiles")}</div>
                                            <div>{bucket_summary(ui_locale_diagnostics.as_deref(), &bootstrap.metrics.task_profile_totals)}</div>
                                        </div>
                                        <div>
                                            <div class="font-medium text-foreground">{t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.resolvedLocales", "Resolved locales")}</div>
                                            <div>{bucket_summary(ui_locale_diagnostics.as_deref(), &bootstrap.metrics.resolved_locale_totals)}</div>
                                        </div>
                                        <div>
                                            <div class="font-medium text-foreground">{t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.recentRuns", "Recent runs")}</div>
                                            <div>{recent_run_summary(ui_locale_diagnostics.as_deref(), &bootstrap.recent_runs)}</div>
                                        </div>
                                        <div class="space-y-2">
                                            {bootstrap
                                                .recent_runs
                                                .iter()
                                                .take(8)
                                                .cloned()
                                                .map(|run| {
                                                    let error_message = run.error_message.clone().unwrap_or_default();
                                                    let has_error = !error_message.trim().is_empty();
                                                    view! {
                                                        <div class="rounded-lg border border-border px-3 py-3">
                                                            <div class="font-medium text-foreground">
                                                                {format!(
                                                                    "{} В· {} В· {} ms",
                                                                    run.session_title,
                                                                    run.status,
                                                                    run.duration_ms,
                                                                )}
                                                            </div>
                                                            <div class="text-xs text-muted-foreground">
                                                                {format!(
                                                                    "{} В· {} В· {} -> {}",
                                                                    run.provider_display_name,
                                                                    run
                                                                        .execution_target
                                                                        .clone()
                                                                        .unwrap_or_else(|| run.execution_path.clone()),
                                                                    run.requested_locale.clone().unwrap_or_else(|| "auto".to_string()),
                                                                    run.resolved_locale,
                                                                )}
                                                            </div>
                                                            <div class="mt-1 text-xs text-muted-foreground">
                                                                {format!(
                                                                    "{}{}",
                                                                    run.started_at,
                                                                    run.task_profile_slug
                                                                        .as_ref()
                                                                        .map(|slug| format!(" В· task {slug}"))
                                                                        .unwrap_or_default(),
                                                                )}
                                                            </div>
                                                            <Show when=move || has_error>
                                                                <div class="mt-1 text-sm text-destructive">
                                                                    {error_message.clone()}
                                                                </div>
                                                            </Show>
                                                        </div>
                                                    }
                                                })
                                                .collect_view()}
                                        </div>
                                        <div>
                                            <div class="font-medium text-foreground">{t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.recentStreamEvents", "Recent stream events")}</div>
                                            <div>
                                                {if bootstrap.recent_stream_events.is_empty() {
                                                    t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.noRecentEvents", "No recent events yet.")
                                                } else {
                                                    t(ui_locale_diagnostics.as_deref(), "ai.diagnostics.cachedEventsCount", "{count} cached event(s)")
                                                        .replace("{count}", bootstrap.recent_stream_events.len().to_string().as_str())
                                                }}
                                            </div>
                                        </div>
                                        <div class="space-y-2">
                                            {bootstrap
                                                .recent_stream_events
                                                .iter()
                                                .take(6)
                                                .cloned()
                                                .map(|event| {
                                                    let status = stream_event_kind_label(
                                                        ui_locale_diagnostics.as_deref(),
                                                        &event.event_kind,
                                                    );
                                                    let error_message = event.error_message.clone().unwrap_or_default();
                                                    let has_error = !error_message.trim().is_empty();
                                                    view! {
                                                        <div class="rounded-lg border border-border px-3 py-3">
                                                            <div class="font-medium text-foreground">
                                                                {format!("{status} · {}", event.run_id)}
                                                            </div>
                                                            <div class="text-xs text-muted-foreground">{event.created_at.clone()}</div>
                                                            <div class="mt-1 whitespace-pre-wrap text-foreground">
                                                                {event
                                                                    .accumulated_content
                                                                    .clone()
                                                                    .or(event.content_delta.clone())
                                                                    .unwrap_or_else(|| t(ui_locale_diagnostics.as_deref(), "ai.common.noTextualDelta", "(no textual delta)"))}
                                                            </div>
                                                            <Show when=move || has_error>
                                                                <div class="mt-1 text-sm text-destructive">
                                                                    {error_message.clone()}
                                                                </div>
                                                            </Show>
                                                        </div>
                                                    }
                                                })
                                                .collect_view()}
                                        </div>
                                    </div>
                                </Card>

                                {if !diagnostics_only.get() { view! {
                                <Card title=t(ui_locale_blog.as_deref(), "ai.card.blogDraft", "Blog Draft")>
                                    <form class="space-y-3" on:submit=on_run_blog_job.clone()>
                                        <TextField label=t(ui_locale_blog.as_deref(), "ai.field.jobTitle", "Job title") value=blog_title />
                                        <TextField
                                            label=t(ui_locale_blog.as_deref(), "ai.field.locale", "Locale")
                                            value=blog_locale
                                            placeholder=t(ui_locale_blog.as_deref(), "ai.field.localeAutoPlaceholder", "auto (request locale -> tenant default -> en)")
                                        />
                                        <TextField label=t(ui_locale_blog.as_deref(), "ai.field.existingPostId", "Existing post id") value=blog_post_id />
                                        <TextField label=t(ui_locale_blog.as_deref(), "ai.field.sourceLocale", "Source locale") value=blog_source_locale />
                                        <TextField label=t(ui_locale_blog.as_deref(), "ai.field.sourceTitleOverride", "Source title override") value=blog_source_title />
                                        <label class="block space-y-1">
                                            <span class="text-sm text-muted-foreground">{t(ui_locale_blog.as_deref(), "ai.field.sourceBodyOverride", "Source body override")}</span>
                                            <textarea
                                                class="min-h-28 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                                prop:value=blog_source_body
                                                on:input=move |ev| blog_source_body.set(event_target_value(&ev))
                                            />
                                        </label>
                                        <TextField label=t(ui_locale_blog.as_deref(), "ai.field.sourceExcerptOverride", "Source excerpt override") value=blog_source_excerpt />
                                        <TextField label=t(ui_locale_blog.as_deref(), "ai.field.sourceSeoTitleOverride", "Source SEO title override") value=blog_source_seo_title />
                                        <label class="block space-y-1">
                                            <span class="text-sm text-muted-foreground">{t(ui_locale_blog.as_deref(), "ai.field.sourceSeoDescriptionOverride", "Source SEO description override")}</span>
                                            <textarea
                                                class="min-h-20 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                                prop:value=blog_source_seo_description
                                                on:input=move |ev| blog_source_seo_description.set(event_target_value(&ev))
                                            />
                                        </label>
                                        <TextField label=t(ui_locale_blog.as_deref(), "ai.field.tagsCsv", "Tags (csv)") value=blog_tags />
                                        <TextField label=t(ui_locale_blog.as_deref(), "ai.field.categoryId", "Category id") value=blog_category_id />
                                        <TextField label=t(ui_locale_blog.as_deref(), "ai.field.featuredImageUrl", "Featured image URL") value=blog_featured_image_url />
                                        <label class="block space-y-1">
                                            <span class="text-sm text-muted-foreground">{t(ui_locale_blog.as_deref(), "ai.field.copyInstructions", "Copy instructions")}</span>
                                            <textarea
                                                class="min-h-20 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                                prop:value=blog_copy_instructions
                                                on:input=move |ev| blog_copy_instructions.set(event_target_value(&ev))
                                            />
                                        </label>
                                        <TextField label=t(ui_locale_blog.as_deref(), "ai.field.assistantPrompt", "Assistant prompt") value=blog_assistant_prompt />
                                        <div class="rounded-lg border border-border px-3 py-2 text-sm text-muted-foreground">
                                            {move || direct_transport_summary(
                                                blog_transport_locale.as_deref(),
                                                selected_provider.get().as_str(),
                                                selected_task_profile.get().as_str(),
                                            )}
                                        </div>
                                        <button type="submit" class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground">{t(ui_locale_blog.as_deref(), "ai.action.generateBlogDraft", "Generate blog draft")}</button>
                                    </form>
                                </Card>

                                <Card title=t(ui_locale_product.as_deref(), "ai.card.productCopy", "Product Copy")>
                                    <form class="space-y-3" on:submit=on_run_product_job.clone()>
                                        <TextField label=t(ui_locale_product.as_deref(), "ai.field.jobTitle", "Job title") value=product_title />
                                        <TextField
                                            label=t(ui_locale_product.as_deref(), "ai.field.locale", "Locale")
                                            value=product_locale
                                            placeholder=t(ui_locale_product.as_deref(), "ai.field.localeAutoPlaceholder", "auto (request locale -> tenant default -> en)")
                                        />
                                        <TextField label=t(ui_locale_product.as_deref(), "ai.field.productId", "Product id") value=product_id />
                                        <TextField label=t(ui_locale_product.as_deref(), "ai.field.sourceLocale", "Source locale") value=product_source_locale />
                                        <TextField label=t(ui_locale_product.as_deref(), "ai.field.sourceTitleOverride", "Source title override") value=product_source_title />
                                        <label class="block space-y-1">
                                            <span class="text-sm text-muted-foreground">{t(ui_locale_product.as_deref(), "ai.field.sourceDescriptionOverride", "Source description override")}</span>
                                            <textarea
                                                class="min-h-24 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                                prop:value=product_source_description
                                                on:input=move |ev| product_source_description.set(event_target_value(&ev))
                                            />
                                        </label>
                                        <TextField label=t(ui_locale_product.as_deref(), "ai.field.sourceMetaTitleOverride", "Source meta title override") value=product_source_meta_title />
                                        <label class="block space-y-1">
                                            <span class="text-sm text-muted-foreground">{t(ui_locale_product.as_deref(), "ai.field.sourceMetaDescriptionOverride", "Source meta description override")}</span>
                                            <textarea
                                                class="min-h-20 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                                prop:value=product_source_meta_description
                                                on:input=move |ev| product_source_meta_description.set(event_target_value(&ev))
                                            />
                                        </label>
                                        <label class="block space-y-1">
                                            <span class="text-sm text-muted-foreground">{t(ui_locale_product.as_deref(), "ai.field.copyInstructions", "Copy instructions")}</span>
                                            <textarea
                                                class="min-h-20 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                                prop:value=product_copy_instructions
                                                on:input=move |ev| product_copy_instructions.set(event_target_value(&ev))
                                            />
                                        </label>
                                        <TextField label=t(ui_locale_product.as_deref(), "ai.field.assistantPrompt", "Assistant prompt") value=product_assistant_prompt />
                                        <div class="rounded-lg border border-border px-3 py-2 text-sm text-muted-foreground">
                                            {move || direct_transport_summary(
                                                product_transport_locale.as_deref(),
                                                selected_provider.get().as_str(),
                                                selected_task_profile.get().as_str(),
                                            )}
                                        </div>
                                        <button type="submit" class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground">{t(ui_locale_product.as_deref(), "ai.action.generateProductCopy", "Generate product copy")}</button>
                                    </form>
                                </Card>

                                <Card title=t(ui_locale_image.as_deref(), "ai.card.mediaImage", "Media Image")>
                                    <form class="space-y-3" on:submit=on_run_image_job.clone()>
                                        <TextField label=t(ui_locale_image.as_deref(), "ai.field.jobTitle", "Job title") value=image_title />
                                        <TextField
                                            label=t(ui_locale_image.as_deref(), "ai.field.locale", "Locale")
                                            value=image_locale
                                            placeholder=t(ui_locale_image.as_deref(), "ai.field.localeAutoPlaceholder", "auto (request locale -> tenant default -> en)")
                                        />
                                        <TextField label=t(ui_locale_image.as_deref(), "ai.field.prompt", "Prompt") value=image_prompt />
                                        <TextField label=t(ui_locale_image.as_deref(), "ai.field.negativePrompt", "Negative prompt") value=image_negative_prompt />
                                        <TextField label=t(ui_locale_image.as_deref(), "ai.field.fileName", "File name") value=image_file_name />
                                        <TextField label=t(ui_locale_image.as_deref(), "ai.field.mediaTitle", "Media title") value=image_asset_title />
                                        <TextField label=t(ui_locale_image.as_deref(), "ai.field.altText", "Alt text") value=image_alt_text />
                                        <TextField label=t(ui_locale_image.as_deref(), "ai.field.caption", "Caption") value=image_caption />
                                        <TextField label=t(ui_locale_image.as_deref(), "ai.field.size", "Size") value=image_size />
                                        <TextField label=t(ui_locale_image.as_deref(), "ai.field.assistantPrompt", "Assistant prompt") value=image_assistant_prompt />
                                        <div class="rounded-lg border border-border px-3 py-2 text-sm text-muted-foreground">
                                            {move || direct_transport_summary(
                                                image_transport_locale.as_deref(),
                                                selected_provider.get().as_str(),
                                                selected_task_profile.get().as_str(),
                                            )}
                                        </div>
                                        <button type="submit" class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground">{t(ui_locale_image.as_deref(), "ai.action.generateMediaImage", "Generate media image")}</button>
                                    </form>
                                </Card>

                                <Card title=t(ui_locale_alloy.as_deref(), "ai.card.alloyAssist", "Alloy Assist")>
                                    <form class="space-y-3" on:submit=on_run_alloy_job.clone()>
                                        <TextField label=t(ui_locale_alloy.as_deref(), "ai.field.jobTitle", "Job title") value=alloy_title />
                                        <TextField
                                            label=t(ui_locale_alloy.as_deref(), "ai.field.locale", "Locale")
                                            value=alloy_locale
                                            placeholder=t(ui_locale_alloy.as_deref(), "ai.field.localeAutoPlaceholder", "auto (request locale -> tenant default -> en)")
                                        />
                                        <TextField label=t(ui_locale_alloy.as_deref(), "ai.field.operation", "Operation") value=alloy_operation />
                                        <TextField label=t(ui_locale_alloy.as_deref(), "ai.field.scriptId", "Script id") value=alloy_script_id />
                                        <TextField label=t(ui_locale_alloy.as_deref(), "ai.field.scriptName", "Script name") value=alloy_script_name />
                                        <TextField label=t(ui_locale_alloy.as_deref(), "ai.field.assistantPrompt", "Assistant prompt") value=alloy_prompt />
                                        <label class="block space-y-1">
                                            <span class="text-sm text-muted-foreground">{t(ui_locale_alloy.as_deref(), "ai.field.scriptSource", "Script source")}</span>
                                            <textarea
                                                class="min-h-28 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                                prop:value=alloy_script_source
                                                on:input=move |ev| alloy_script_source.set(event_target_value(&ev))
                                            />
                                        </label>
                                        <label class="block space-y-1">
                                            <span class="text-sm text-muted-foreground">{t(ui_locale_alloy.as_deref(), "ai.field.runtimePayloadJson", "Runtime payload JSON")}</span>
                                            <textarea
                                                class="min-h-24 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                                prop:value=alloy_runtime_payload
                                                on:input=move |ev| alloy_runtime_payload.set(event_target_value(&ev))
                                            />
                                        </label>
                                        <div class="rounded-lg border border-border px-3 py-2 text-sm text-muted-foreground">
                                            {move || direct_transport_summary(
                                                alloy_transport_locale.as_deref(),
                                                selected_provider.get().as_str(),
                                                selected_task_profile.get().as_str(),
                                            )}
                                        </div>
                                        <button type="submit" class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground">{t(ui_locale_alloy.as_deref(), "ai.action.runAlloyJob", "Run Alloy job")}</button>
                                    </form>
                                </Card>

                                <Card title=t(ui_locale_new_session.as_deref(), "ai.card.newSession", "New Session")>
                                    <form class="space-y-3" on:submit=on_start_session.clone()>
                                        <TextField label=t(ui_locale_new_session.as_deref(), "ai.field.title", "Title") value=session_title />
                                        <TextField
                                            label=t(ui_locale_new_session.as_deref(), "ai.field.locale", "Locale")
                                            value=session_locale
                                            placeholder=t(ui_locale_new_session.as_deref(), "ai.field.localeAutoPlaceholder", "auto (request locale -> tenant default -> en)")
                                        />
                                        <TextField label=t(ui_locale_new_session.as_deref(), "ai.field.initialMessage", "Initial message") value=session_message />
                                        <div class="rounded-lg border border-border px-3 py-2 text-sm text-muted-foreground">
                                            {move || session_transport_summary(
                                                session_transport_locale.as_deref(),
                                                selected_provider.get().as_str(),
                                                selected_task_profile.get().as_str(),
                                                selected_tool_profile.get().as_str(),
                                            )}
                                        </div>
                                        <button type="submit" class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground">{t(ui_locale_new_session.as_deref(), "ai.action.startSession", "Start session")}</button>
                                    </form>
                                </Card>
                                }.into_any() } else { ().into_any() }}

                                <Card title=t(ui_locale_sessions.as_deref(), "ai.card.sessions", "Sessions")>
                                    <div class="space-y-2">
                                            {bootstrap.sessions.into_iter().map(|session| {
                                                let session_id = session.id.clone();
                                                let item_query_writer = select_session_query_writer.clone();
                                                view! {
                                                <button
                                                    class="w-full rounded-lg border border-border px-3 py-3 text-left text-sm hover:bg-muted"
                                                    on:click=move |_| {
                                                        item_query_writer.replace_value(
                                                            AdminQueryKey::SessionId.as_str(),
                                                            session_id.clone(),
                                                        );
                                                    }
                                                >
                                                    <div class="font-medium">{session.title}</div>
                                                    <div class="text-muted-foreground">
                                                        {session_list_summary(
                                                            ui_locale_sessions.as_deref(),
                                                            session.status.as_str(),
                                                            session.execution_mode.as_str(),
                                                            session.latest_run_status.as_deref(),
                                                            session.pending_approvals,
                                                        )}
                                                    </div>
                                                </button>
                                            }
                                        }).collect_view()}
                                    </div>
                                </Card>
                            </section>

                            <section>
                                <Card title=t(ui_locale_operator.as_deref(), "ai.card.operatorChat", "Operator Chat")>
                                    <Suspense fallback=move || view! { <div class="h-64 animate-pulse rounded-xl bg-muted"></div> }>
                                        {move || {
                                            let ui_locale = ui_locale_operator.clone();
                                            let on_send_message = on_send_message.clone();
                                            session_detail.get().map(|result| match result {
                                            Ok(Some(detail)) => {
                                                let ui_locale_form = ui_locale.clone();
                                                let ui_locale_approvals = ui_locale.clone();
                                                let ui_locale_runs = ui_locale.clone();
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
                                                                {session_profile_summary(
                                                                    ui_locale.as_deref(),
                                                                    detail.provider_profile.display_name.as_str(),
                                                                    detail.provider_profile.model.as_str(),
                                                                    detail.session.execution_mode.as_str(),
                                                                )}
                                                            </div>
                                                            <div class="text-muted-foreground">
                                                                {locale_flow_summary(
                                                                    ui_locale.as_deref(),
                                                                    detail.session.requested_locale.as_deref(),
                                                                    detail.session.resolved_locale.as_str(),
                                                                )}
                                                            </div>
                                                        </div>

                                                        <div class="max-h-[380px] space-y-3 overflow-y-auto rounded-xl border border-border p-3">
                                                            {detail.messages.into_iter().map(|message| view! {
                                                                <div class="rounded-lg border border-border px-3 py-3 text-sm">
                                                                    <div class="mb-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                                                                        {message.role.clone()}
                                                                    </div>
                                                                    <div>{message.content.unwrap_or_else(|| t(ui_locale.as_deref(), "ai.common.noTextualContent", "(no textual content)"))}</div>
                                                                </div>
                                                            }).collect_view()}
                                                        </div>

                                                        {move || live_stream.get().map(|stream| {
                                                            let content = if stream.content.trim().is_empty() {
                                                                t(ui_locale.as_deref(), "ai.session.waitingForAssistant", "Waiting for assistant output...")
                                                            } else {
                                                                stream.content.clone()
                                                            };
                                                            let error_message = stream.error_message.clone().unwrap_or_default();
                                                            let has_error = !error_message.trim().is_empty();
                                                            view! {
                                                                <div class="rounded-lg border border-sky-300 bg-sky-50 px-4 py-3 text-sm text-sky-950">
                                                                    <div class="flex items-center justify-between gap-3">
                                                                        <div class="font-medium">{t(ui_locale.as_deref(), "ai.session.liveStream", "Live stream")}</div>
                                                                        <div class="text-xs uppercase tracking-wide text-sky-800">
                                                                            {stream_status_summary(
                                                                                ui_locale.as_deref(),
                                                                                stream.connected,
                                                                                stream.status.as_str(),
                                                                            )}
                                                                        </div>
                                                                    </div>
                                                                    <div class="mt-2 whitespace-pre-wrap text-sky-950">{content}</div>
                                                                    <Show when=move || has_error>
                                                                        <div class="mt-2 text-sm text-destructive">{error_message.clone()}</div>
                                                                    </Show>
                                                                </div>
                                                            }
                                                        })}

                                                        <form class="space-y-3" on:submit=on_send_message.clone()>
                                                            <textarea
                                                                class="min-h-28 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                                                prop:value=reply_message
                                                                on:input=move |ev| reply_message.set(event_target_value(&ev))
                                                            />
                                                            <button type="submit" class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground">{t(ui_locale_form.as_deref(), "ai.action.send", "Send")}</button>
                                                        </form>

                                                        {if pending_approvals.is_empty() {
                                                            ().into_any()
                                                        } else {
                                                            view! {
                                                                <div class="space-y-3">
                                                                    <div class="text-sm font-semibold">{t(ui_locale_approvals.as_deref(), "ai.session.pendingApprovals", "Pending approvals")}</div>
                                                                    {pending_approvals.into_iter().map(|approval| {
                                                                    let approve_id = approval.id.clone();
                                                                    let reject_id = approval.id.clone();
                                                                    let approval_reason = approval.reason.unwrap_or_else(|| t(ui_locale_approvals.as_deref(), "ai.session.operatorApprovalRequired", "Operator approval required"));
                                                                    let approve_label = t(ui_locale_approvals.as_deref(), "ai.action.approve", "Approve");
                                                                    let reject_label = t(ui_locale_approvals.as_deref(), "ai.action.reject", "Reject");
                                                                    let reject_reason = t(ui_locale_approvals.as_deref(), "ai.session.rejectedInAdminUi", "Rejected in admin UI");
                                                                    view! {
                                                                        <div class="rounded-lg border border-amber-300 bg-amber-50 px-4 py-3 text-sm text-amber-900">
                                                                            <div class="font-medium">{approval.tool_name.clone()}</div>
                                                                            <div class="mt-1 text-amber-800">{approval_reason}</div>
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
                                                                                    {approve_label}
                                                                                </button>
                                                                                <button
                                                                                    class="rounded-md border border-amber-900 px-3 py-2 text-xs font-semibold text-amber-900"
                                                                                    on:click=move |_| {
                                                                                        let approval_id = reject_id.clone();
                                                                                        let reject_reason = reject_reason.clone();
                                                                                        spawn_local(async move {
                                                                                            let _ = api::resume_approval(approval_id, false, Some(reject_reason)).await;
                                                                                            set_refresh_nonce.update(|value| *value += 1);
                                                                                        });
                                                                                    }
                                                                                >
                                                                                    {reject_label}
                                                                                </button>
                                                                            </div>
                                                                        </div>
                                                                    }
                                                                    }).collect_view()}
                                                                </div>
                                                            }.into_any()
                                                        }}

                                                        <div class="space-y-3">
                                                            <div class="text-sm font-semibold">{t(ui_locale_runs.as_deref(), "ai.session.runs", "Runs")}</div>
                                                            {detail.runs.into_iter().map(|run| {
                                                                let error_message = run.error_message.clone().unwrap_or_default();
                                                                let has_error = !error_message.is_empty();
                                                                view! {
                                                                    <div class="rounded-lg border border-border px-3 py-3 text-sm">
                                                                        <div class="font-medium">{run.model.clone()}</div>
                                                                        <div class="text-muted-foreground">
                                                                            {run_path_summary(
                                                                                ui_locale_runs.as_deref(),
                                                                                run.status.as_str(),
                                                                                run.execution_mode.as_str(),
                                                                                run.execution_path.as_str(),
                                                                            )}
                                                                        </div>
                                                                        <div class="text-muted-foreground">
                                                                            {locale_flow_summary(
                                                                                ui_locale_runs.as_deref(),
                                                                                run.requested_locale.as_deref(),
                                                                                run.resolved_locale.as_str(),
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
                                                            <div class="text-sm font-semibold">{t(ui_locale_runs.as_deref(), "ai.session.toolTrace", "Tool trace")}</div>
                                                            {detail.tool_traces.into_iter().map(|trace| view! {
                                                                <div class="rounded-lg border border-border px-3 py-3 text-sm">
                                                                    <div class="font-medium">{trace.tool_name}</div>
                                                                    <div class="text-muted-foreground">{tool_trace_summary(ui_locale_runs.as_deref(), trace.status.as_str(), trace.duration_ms)}</div>
                                                                </div>
                                                            }).collect_view()}
                                                        </div>

                                                        <div class="space-y-3">
                                                            <div class="text-sm font-semibold">{t(ui_locale_runs.as_deref(), "ai.diagnostics.recentStreamEvents", "Recent stream events")}</div>
                                                            {if detail.recent_stream_events.is_empty() {
                                                                view! {
                                                                    <div class="rounded-lg border border-dashed border-border px-4 py-6 text-sm text-muted-foreground">
                                                                        {t(ui_locale_runs.as_deref(), "ai.session.noCachedStreamEvents", "No cached stream events for this session yet.")}
                                                                    </div>
                                                                }.into_any()
                                                            } else {
                                                                view! {
                                                                    {detail.recent_stream_events.into_iter().take(10).map(|event| {
                                                                        let status = stream_event_kind_label(
                                                                            ui_locale_runs.as_deref(),
                                                                            &event.event_kind,
                                                                        );
                                                                        let error_message = event.error_message.clone().unwrap_or_default();
                                                                        let has_error = !error_message.trim().is_empty();
                                                                        view! {
                                                                            <div class="rounded-lg border border-border px-3 py-3 text-sm">
                                                                                <div class="font-medium">{format!("{status} · {}", event.run_id)}</div>
                                                                                <div class="text-xs text-muted-foreground">{event.created_at}</div>
                                                                                <div class="mt-1 whitespace-pre-wrap">
                                                                                    {event
                                                                                        .accumulated_content
                                                                                        .or(event.content_delta)
                                                                                        .unwrap_or_else(|| t(ui_locale_runs.as_deref(), "ai.common.noTextualDelta", "(no textual delta)"))}
                                                                                </div>
                                                                                <Show when=move || has_error>
                                                                                    <div class="mt-1 text-destructive">{error_message.clone()}</div>
                                                                                </Show>
                                                                            </div>
                                                                        }
                                                                    }).collect_view()}
                                                                }.into_any()
                                                            }}
                                                        </div>
                                                    </div>
                                                }.into_any()
                                            }
                                            Ok(None) => view! {
                                                <div class="rounded-lg border border-dashed border-border px-4 py-8 text-sm text-muted-foreground">
                                                    {t(ui_locale.as_deref(), "ai.session.selectPrompt", "Select a session to inspect chat history, traces, and approvals.")}
                                                </div>
                                            }.into_any(),
                                            Err(err) => view! {
                                                <div class="rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                                    {t(ui_locale.as_deref(), "ai.session.loadSession", "Failed to load session: {error}")
                                                        .replace("{error}", err.to_string().as_str())}
                                                </div>
                                            }.into_any(),
                                            })
                                        }}
                                    </Suspense>
                                </Card>
                            </section>
                        </div>
                    }.into_any(),
                    Err(err) => view! {
                        <div class="rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                            {t(ui_locale_operator.as_deref(), "ai.session.loadBootstrap", "Failed to load AI bootstrap: {error}")
                                .replace("{error}", err.to_string().as_str())}
                        </div>
                    }.into_any(),
                    })
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn Card(#[prop(into)] title: String, children: Children) -> impl IntoView {
    view! {
        <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
            <h2 class="mb-4 text-lg font-semibold text-card-foreground">{title}</h2>
            {children()}
        </section>
    }
}

#[component]
fn TextField(
    #[prop(into)] label: String,
    value: RwSignal<String>,
    #[prop(optional, into)] placeholder: Option<String>,
) -> impl IntoView {
    view! {
        <label class="block space-y-1">
            <span class="text-sm text-muted-foreground">{label}</span>
            <input
                type="text"
                class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                prop:value=value
                placeholder=placeholder.clone().unwrap_or_default()
                on:input=move |ev| value.set(event_target_value(&ev))
            />
        </label>
    }
}

#[component]
fn InfoItem(#[prop(into)] label: String, value: String) -> impl IntoView {
    view! {
        <div class="rounded-lg border border-border px-3 py-3">
            <div class="text-xs uppercase tracking-wide text-muted-foreground">{label}</div>
            <div class="mt-1 text-lg font-semibold text-card-foreground">{value}</div>
        </div>
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

fn bucket_summary(locale: Option<&str>, buckets: &[AiMetricBucketPayload]) -> String {
    if buckets.is_empty() {
        t(locale, "ai.summary.bucketNoData", "no data")
    } else {
        buckets
            .iter()
            .map(|bucket| format!("{}={}", bucket.label, bucket.total))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn recent_run_summary(locale: Option<&str>, runs: &[model::AiRecentRunPayload]) -> String {
    if runs.is_empty() {
        return t(
            locale,
            "ai.diagnostics.noRecentEvents",
            "No recent events yet.",
        );
    }

    let failed = runs.iter().filter(|run| run.status == "failed").count();
    let waiting = runs
        .iter()
        .filter(|run| run.status == "waiting_approval")
        .count();
    let avg_latency = runs
        .iter()
        .map(|run| run.duration_ms.max(0) as u64)
        .sum::<u64>()
        / runs.len() as u64;

    t(
        locale,
        "ai.summary.recentRuns",
        "{count} run(s), {failed} failed, {waiting} waiting approval, avg {latency} ms",
    )
    .replace("{count}", runs.len().to_string().as_str())
    .replace("{failed}", failed.to_string().as_str())
    .replace("{waiting}", waiting.to_string().as_str())
    .replace("{latency}", avg_latency.to_string().as_str())
}

fn stream_event_kind_label(
    locale: Option<&str>,
    value: &model::AiRunStreamEventKindPayload,
) -> String {
    match value {
        model::AiRunStreamEventKindPayload::Started => t(locale, "ai.status.started", "STARTED"),
        model::AiRunStreamEventKindPayload::Delta => t(locale, "ai.status.delta", "DELTA"),
        model::AiRunStreamEventKindPayload::Completed => {
            t(locale, "ai.status.completed", "COMPLETED")
        }
        model::AiRunStreamEventKindPayload::Failed => t(locale, "ai.status.failed", "FAILED"),
        model::AiRunStreamEventKindPayload::WaitingApproval => {
            t(locale, "ai.status.waitingApproval", "WAITING_APPROVAL")
        }
    }
}

fn average_run_latency_summary(locale: Option<&str>, latency_ms: u64) -> String {
    t(
        locale,
        "ai.diagnostics.averageRunLatency",
        "Average run latency: {value} ms",
    )
    .replace("{value}", latency_ms.to_string().as_str())
}

fn provider_profile_summary(
    locale: Option<&str>,
    kind: &str,
    model: &str,
    capabilities: usize,
    active: bool,
) -> String {
    t(
        locale,
        "ai.summary.providerList",
        "{kind} · {model} · {count} capabilities · {state}",
    )
    .replace("{kind}", kind)
    .replace("{model}", model)
    .replace("{count}", capabilities.to_string().as_str())
    .replace("{state}", active_state_label(locale, active).as_str())
}

fn tool_profile_summary(
    locale: Option<&str>,
    allowed_count: usize,
    sensitive_count: usize,
    active: bool,
) -> String {
    t(
        locale,
        "ai.summary.toolProfileList",
        "allowed: {allowed} · sensitive: {sensitive} · {state}",
    )
    .replace("{allowed}", allowed_count.to_string().as_str())
    .replace("{sensitive}", sensitive_count.to_string().as_str())
    .replace("{state}", active_state_label(locale, active).as_str())
}

fn task_profile_summary(
    locale: Option<&str>,
    capability: &str,
    mode: &str,
    active: bool,
) -> String {
    t(
        locale,
        "ai.summary.taskProfileList",
        "{capability} · {mode} · {state}",
    )
    .replace("{capability}", capability)
    .replace("{mode}", mode)
    .replace("{state}", active_state_label(locale, active).as_str())
}

fn direct_transport_summary(locale: Option<&str>, provider: &str, task_profile: &str) -> String {
    t(
        locale,
        "ai.summary.transportDirect",
        "Provider: {provider} | Task profile: {task_profile} | Mode: {mode}",
    )
    .replace("{provider}", provider)
    .replace("{task_profile}", task_profile)
    .replace("{mode}", t(locale, "ai.common.direct", "direct").as_str())
}

fn session_transport_summary(
    locale: Option<&str>,
    provider: &str,
    task_profile: &str,
    tool_profile: &str,
) -> String {
    t(
        locale,
        "ai.summary.transportSession",
        "Provider: {provider} | Task profile: {task_profile} | Tool profile: {tool_profile}",
    )
    .replace("{provider}", provider)
    .replace("{task_profile}", task_profile)
    .replace("{tool_profile}", tool_profile)
}

fn session_list_summary(
    locale: Option<&str>,
    status: &str,
    mode: &str,
    latest: Option<&str>,
    approvals: i32,
) -> String {
    let latest_value = latest
        .map(ToString::to_string)
        .unwrap_or_else(|| t(locale, "ai.common.idle", "idle"));
    t(
        locale,
        "ai.summary.sessionList",
        "status: {status} · mode: {mode} · latest: {latest} · approvals: {approvals}",
    )
    .replace("{status}", status)
    .replace("{mode}", mode)
    .replace("{latest}", latest_value.as_str())
    .replace("{approvals}", approvals.to_string().as_str())
}

fn session_profile_summary(
    locale: Option<&str>,
    provider: &str,
    model: &str,
    mode: &str,
) -> String {
    t(
        locale,
        "ai.summary.sessionProfile",
        "provider: {provider} · model: {model} · mode: {mode}",
    )
    .replace("{provider}", provider)
    .replace("{model}", model)
    .replace("{mode}", mode)
}

fn locale_flow_summary(locale: Option<&str>, requested: Option<&str>, resolved: &str) -> String {
    let requested_value = requested
        .map(ToString::to_string)
        .unwrap_or_else(|| t(locale, "ai.common.auto", "auto"));
    t(
        locale,
        "ai.summary.localeFlow",
        "locale: {requested} -> {resolved}",
    )
    .replace("{requested}", requested_value.as_str())
    .replace("{resolved}", resolved)
}

fn run_path_summary(locale: Option<&str>, status: &str, mode: &str, path: &str) -> String {
    t(
        locale,
        "ai.summary.runPath",
        "{status} · {mode} · path {path}",
    )
    .replace("{status}", status)
    .replace("{mode}", mode)
    .replace("{path}", path)
}

fn tool_trace_summary(locale: Option<&str>, status: &str, duration_ms: i64) -> String {
    t(locale, "ai.summary.toolTrace", "{status} · {duration} ms")
        .replace("{status}", status)
        .replace("{duration}", duration_ms.to_string().as_str())
}

fn stream_status_summary(locale: Option<&str>, connected: bool, status: &str) -> String {
    let connection_label = if connected {
        t(locale, "ai.common.connected", "connected")
    } else {
        t(locale, "ai.common.disconnected", "disconnected")
    };
    t(locale, "ai.summary.streamStatus", "{connection} · {status}")
        .replace("{connection}", connection_label.as_str())
        .replace("{status}", status)
}

fn active_state_label(locale: Option<&str>, active: bool) -> String {
    if active {
        t(locale, "ai.common.active", "active")
    } else {
        t(locale, "ai.common.inactive", "inactive")
    }
}

fn apply_provider_profile(
    selected_provider: RwSignal<String>,
    provider_slug: RwSignal<String>,
    provider_name: RwSignal<String>,
    provider_kind: RwSignal<String>,
    provider_base_url: RwSignal<String>,
    provider_model: RwSignal<String>,
    provider_api_key: RwSignal<String>,
    provider_temperature: RwSignal<String>,
    provider_max_tokens: RwSignal<String>,
    provider_capabilities: RwSignal<String>,
    provider_allowed_tasks: RwSignal<String>,
    provider_denied_tasks: RwSignal<String>,
    provider_restricted_roles: RwSignal<String>,
    provider_active: RwSignal<bool>,
    profile: &AiProviderProfilePayload,
) {
    selected_provider.set(profile.id.clone());
    provider_slug.set(profile.slug.clone());
    provider_name.set(profile.display_name.clone());
    provider_kind.set(profile.provider_kind.clone());
    provider_base_url.set(profile.base_url.clone());
    provider_model.set(profile.model.clone());
    provider_api_key.set(String::new());
    provider_temperature.set(
        profile
            .temperature
            .map(|value| value.to_string())
            .unwrap_or_default(),
    );
    provider_max_tokens.set(
        profile
            .max_tokens
            .map(|value| value.to_string())
            .unwrap_or_default(),
    );
    provider_capabilities.set(profile.capabilities.join(","));
    provider_allowed_tasks.set(profile.allowed_task_profiles.join(","));
    provider_denied_tasks.set(profile.denied_task_profiles.join(","));
    provider_restricted_roles.set(profile.restricted_role_slugs.join(","));
    provider_active.set(profile.is_active);
}

fn clear_provider_profile(
    selected_provider: RwSignal<String>,
    provider_slug: RwSignal<String>,
    provider_name: RwSignal<String>,
    provider_kind: RwSignal<String>,
    provider_base_url: RwSignal<String>,
    provider_model: RwSignal<String>,
    provider_api_key: RwSignal<String>,
    provider_temperature: RwSignal<String>,
    provider_max_tokens: RwSignal<String>,
    provider_capabilities: RwSignal<String>,
    provider_allowed_tasks: RwSignal<String>,
    provider_denied_tasks: RwSignal<String>,
    provider_restricted_roles: RwSignal<String>,
    provider_active: RwSignal<bool>,
) {
    selected_provider.set(String::new());
    provider_slug.set(String::new());
    provider_name.set(String::new());
    provider_kind.set("openai_compatible".to_string());
    provider_base_url.set("http://localhost:11434".to_string());
    provider_model.set("gpt-4.1-mini".to_string());
    provider_api_key.set(String::new());
    provider_temperature.set("0.2".to_string());
    provider_max_tokens.set("1024".to_string());
    provider_capabilities
        .set("text_generation,structured_generation,image_generation,code_generation".to_string());
    provider_allowed_tasks.set(String::new());
    provider_denied_tasks.set(String::new());
    provider_restricted_roles.set(String::new());
    provider_active.set(true);
}

fn apply_tool_profile(
    selected_tool_profile: RwSignal<String>,
    tool_slug: RwSignal<String>,
    tool_name: RwSignal<String>,
    tool_description: RwSignal<String>,
    tool_allowed: RwSignal<String>,
    tool_denied: RwSignal<String>,
    tool_sensitive: RwSignal<String>,
    tool_active: RwSignal<bool>,
    profile: &AiToolProfilePayload,
) {
    selected_tool_profile.set(profile.id.clone());
    tool_slug.set(profile.slug.clone());
    tool_name.set(profile.display_name.clone());
    tool_description.set(profile.description.clone().unwrap_or_default());
    tool_allowed.set(profile.allowed_tools.join(","));
    tool_denied.set(profile.denied_tools.join(","));
    tool_sensitive.set(profile.sensitive_tools.join(","));
    tool_active.set(profile.is_active);
}

fn clear_tool_profile(
    selected_tool_profile: RwSignal<String>,
    tool_slug: RwSignal<String>,
    tool_name: RwSignal<String>,
    tool_description: RwSignal<String>,
    tool_allowed: RwSignal<String>,
    tool_denied: RwSignal<String>,
    tool_sensitive: RwSignal<String>,
    tool_active: RwSignal<bool>,
) {
    selected_tool_profile.set(String::new());
    tool_slug.set(String::new());
    tool_name.set(String::new());
    tool_description.set(String::new());
    tool_allowed.set("list_modules,query_modules,module_details,mcp_health,mcp_whoami".to_string());
    tool_denied.set(String::new());
    tool_sensitive.set(
        "alloy_create_script,alloy_update_script,alloy_delete_script,alloy_apply_module_scaffold"
            .to_string(),
    );
    tool_active.set(true);
}

fn apply_task_profile(
    selected_task_profile: RwSignal<String>,
    task_slug: RwSignal<String>,
    task_name: RwSignal<String>,
    task_description: RwSignal<String>,
    task_capability: RwSignal<String>,
    task_system_prompt: RwSignal<String>,
    task_allowed_providers: RwSignal<String>,
    task_preferred_providers: RwSignal<String>,
    task_execution_mode: RwSignal<String>,
    task_active: RwSignal<bool>,
    profile: &AiTaskProfilePayload,
) {
    selected_task_profile.set(profile.id.clone());
    task_slug.set(profile.slug.clone());
    task_name.set(profile.display_name.clone());
    task_description.set(profile.description.clone().unwrap_or_default());
    task_capability.set(profile.target_capability.clone());
    task_system_prompt.set(profile.system_prompt.clone().unwrap_or_default());
    task_allowed_providers.set(profile.allowed_provider_profile_ids.join(","));
    task_preferred_providers.set(profile.preferred_provider_profile_ids.join(","));
    task_execution_mode.set(profile.default_execution_mode.clone());
    task_active.set(profile.is_active);
}

fn clear_task_profile(
    selected_task_profile: RwSignal<String>,
    task_slug: RwSignal<String>,
    task_name: RwSignal<String>,
    task_description: RwSignal<String>,
    task_capability: RwSignal<String>,
    task_system_prompt: RwSignal<String>,
    task_allowed_providers: RwSignal<String>,
    task_preferred_providers: RwSignal<String>,
    task_execution_mode: RwSignal<String>,
    task_active: RwSignal<bool>,
) {
    selected_task_profile.set(String::new());
    task_slug.set(String::new());
    task_name.set(String::new());
    task_description.set(String::new());
    task_capability.set("text_generation".to_string());
    task_system_prompt.set(String::new());
    task_allowed_providers.set(String::new());
    task_preferred_providers.set(String::new());
    task_execution_mode.set("auto".to_string());
    task_active.set(true);
}

#[cfg(target_arch = "wasm32")]
const AI_SESSION_EVENTS_SUBSCRIPTION: &str = r#"
subscription AiSessionEvents($sessionId: UUID!) {
  aiSessionEvents(sessionId: $sessionId) {
    sessionId
    runId
    eventKind
    contentDelta
    accumulatedContent
    errorMessage
    createdAt
  }
}
"#;

#[cfg(target_arch = "wasm32")]
struct AiLiveSubscriptionHandle {
    generation: u64,
    ws: WebSocket,
    on_open: Closure<dyn FnMut(Event)>,
    on_message: Closure<dyn FnMut(MessageEvent)>,
    on_error: Closure<dyn FnMut(ErrorEvent)>,
    on_close: Closure<dyn FnMut(CloseEvent)>,
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static AI_LIVE_SUBSCRIPTION_HANDLE: RefCell<Option<AiLiveSubscriptionHandle>> = const { RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
static AI_LIVE_SUBSCRIPTION_GENERATION: AtomicU64 = AtomicU64::new(1);

#[cfg(target_arch = "wasm32")]
fn next_live_subscription_generation() -> u64 {
    AI_LIVE_SUBSCRIPTION_GENERATION.fetch_add(1, Ordering::Relaxed)
}

#[cfg(target_arch = "wasm32")]
fn close_live_subscription_handle(handle: AiLiveSubscriptionHandle) {
    handle.ws.set_onopen(None);
    handle.ws.set_onmessage(None);
    handle.ws.set_onerror(None);
    handle.ws.set_onclose(None);
    let _ = handle.ws.close();
    drop(handle.on_open);
    drop(handle.on_message);
    drop(handle.on_error);
    drop(handle.on_close);
}

#[cfg(target_arch = "wasm32")]
fn replace_live_subscription(handle: Option<AiLiveSubscriptionHandle>) {
    AI_LIVE_SUBSCRIPTION_HANDLE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if let Some(previous) = slot.take() {
            close_live_subscription_handle(previous);
        }
        *slot = handle;
    });
}

#[cfg(target_arch = "wasm32")]
fn clear_live_subscription_generation(generation: u64) {
    AI_LIVE_SUBSCRIPTION_HANDLE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let should_clear = slot
            .as_ref()
            .map(|handle| handle.generation == generation)
            .unwrap_or(false);
        if should_clear {
            if let Some(handle) = slot.take() {
                close_live_subscription_handle(handle);
            }
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn graphql_ws_url() -> String {
    let window = web_sys::window().expect("window should exist in browser");
    let location = window.location();
    let protocol = location
        .protocol()
        .ok()
        .unwrap_or_else(|| "http:".to_string());
    let host = location
        .host()
        .ok()
        .unwrap_or_else(|| "localhost:5150".to_string());
    let ws_scheme = if protocol.eq_ignore_ascii_case("https:") {
        "wss"
    } else {
        "ws"
    };
    format!("{ws_scheme}://{host}/api/graphql/ws")
}

#[cfg(target_arch = "wasm32")]
fn browser_admin_locale(preferred: Option<&str>) -> Option<String> {
    if let Some(preferred) = preferred.map(str::trim).filter(|value| !value.is_empty()) {
        return Some(preferred.to_string());
    }
    let window = web_sys::window()?;
    let storage = window.local_storage().ok().flatten()?;
    storage
        .get_item("rustok-admin-locale")
        .ok()
        .flatten()
        .filter(|value| !value.trim().is_empty())
}

fn alloy_task_payload(
    operation: String,
    script_id: Option<String>,
    script_name: Option<String>,
    script_source: Option<String>,
    runtime_payload_json: Option<String>,
    assistant_prompt: Option<String>,
) -> Result<String, serde_json::Error> {
    let payload = serde_json::json!({
        "operation": operation,
        "script_id": script_id,
        "script_name": script_name,
        "script_source": script_source,
        "runtime_payload_json": runtime_payload_json,
        "assistant_prompt": assistant_prompt,
    });
    serde_json::to_string(&payload)
}

fn image_task_payload(
    prompt: String,
    negative_prompt: Option<String>,
    title: Option<String>,
    alt_text: Option<String>,
    caption: Option<String>,
    file_name: Option<String>,
    size: Option<String>,
    assistant_prompt: Option<String>,
) -> Result<String, serde_json::Error> {
    let payload = serde_json::json!({
        "prompt": prompt,
        "negative_prompt": negative_prompt,
        "title": title,
        "alt_text": alt_text,
        "caption": caption,
        "file_name": file_name,
        "size": size,
        "assistant_prompt": assistant_prompt,
    });
    serde_json::to_string(&payload)
}

fn product_task_payload(
    product_id: String,
    source_locale: Option<String>,
    source_title: Option<String>,
    source_description: Option<String>,
    source_meta_title: Option<String>,
    source_meta_description: Option<String>,
    copy_instructions: Option<String>,
    assistant_prompt: Option<String>,
) -> Result<String, serde_json::Error> {
    let product_id = uuid::Uuid::parse_str(product_id.trim()).map_err(|error| {
        serde_json::Error::io(std::io::Error::new(std::io::ErrorKind::InvalidInput, error))
    })?;
    let payload = serde_json::json!({
        "product_id": product_id,
        "source_locale": source_locale,
        "source_title": source_title,
        "source_description": source_description,
        "source_meta_title": source_meta_title,
        "source_meta_description": source_meta_description,
        "copy_instructions": copy_instructions,
        "assistant_prompt": assistant_prompt,
    });
    serde_json::to_string(&payload)
}

fn blog_task_payload(
    post_id: Option<String>,
    source_locale: Option<String>,
    source_title: Option<String>,
    source_body: Option<String>,
    source_excerpt: Option<String>,
    source_seo_title: Option<String>,
    source_seo_description: Option<String>,
    tags: Vec<String>,
    category_id: Option<String>,
    featured_image_url: Option<String>,
    copy_instructions: Option<String>,
    assistant_prompt: Option<String>,
) -> Result<String, serde_json::Error> {
    let post_id = post_id
        .map(|value| {
            uuid::Uuid::parse_str(value.trim()).map_err(|error| {
                serde_json::Error::io(std::io::Error::new(std::io::ErrorKind::InvalidInput, error))
            })
        })
        .transpose()?;
    let category_id = category_id
        .map(|value| {
            uuid::Uuid::parse_str(value.trim()).map_err(|error| {
                serde_json::Error::io(std::io::Error::new(std::io::ErrorKind::InvalidInput, error))
            })
        })
        .transpose()?;
    let payload = serde_json::json!({
        "post_id": post_id,
        "source_locale": source_locale,
        "source_title": source_title,
        "source_body": source_body,
        "source_excerpt": source_excerpt,
        "source_seo_title": source_seo_title,
        "source_seo_description": source_seo_description,
        "tags": tags,
        "category_id": category_id,
        "featured_image_url": featured_image_url,
        "copy_instructions": copy_instructions,
        "assistant_prompt": assistant_prompt,
    });
    serde_json::to_string(&payload)
}
