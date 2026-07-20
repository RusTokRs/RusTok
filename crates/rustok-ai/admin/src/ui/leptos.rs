#![allow(clippy::too_many_arguments)]
#![allow(clippy::single_match)]
#![allow(clippy::manual_checked_ops)]
#![allow(clippy::redundant_iter_cloned)]

use crate::model;
#[cfg(not(target_arch = "wasm32"))]
use crate::model::AiLiveStreamStatePayload;
#[cfg(target_arch = "wasm32")]
use crate::model::{
    AiLiveStreamStatePayload, AiRunStreamEventKindPayload, AiSessionSubscriptionEnvelope,
};
use crate::model::{
    AiMetricBucketPayload, AiProviderProfilePayload, AiTaskProfilePayload, AiToolProfilePayload,
};
use crate::transport;
use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_ui_core::{AdminQueryKey, UiRouteContext};
#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, closure::Closure};
#[cfg(target_arch = "wasm32")]
use web_sys::{CloseEvent, ErrorEvent, Event, MessageEvent, WebSocket};

use super::components::agent_panel::{
    AiAgentModelAssignmentCreateForm, AiAgentModelAssignmentUpdateForm, AiAgentPanel,
    AiAgentPrincipalCreateForm, AiAgentPrincipalUpdateForm,
};
use super::components::chat_session_panel::AiChatSessionPanel;
use super::components::diagnostics_panel::AiDiagnosticsPanel;
use super::components::jobs_panel::AiJobsPanel;
use super::components::provider_panel::AiProviderPanel;
use super::components::task_panel::AiTaskPanel;
use super::components::tool_panel::AiToolPanel;
use crate::core::{
    BlogTaskPayloadInput, ImageTaskPayloadInput, OrderAnalyticsTaskPayloadInput,
    OrderOpsAssistantTaskPayloadInput, ProductAttributesTaskPayloadInput, ProductTaskPayloadInput,
    alloy_task_payload, blog_task_payload, image_task_payload, optional_text,
    order_analytics_task_payload, order_ops_assistant_task_payload, parse_csv,
    product_attributes_task_payload, product_task_payload, summarize_recent_runs,
};
use crate::i18n::t;
#[cfg(target_arch = "wasm32")]
use crate::transport::graphql_adapter::{
    connection_init_message, graphql_ws_url_from_location, session_events_subscribe_message,
};

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
    let provider_integration = RwSignal::new(String::new());
    let provider_credential_refs = RwSignal::new(Vec::new());
    let provider_model = RwSignal::new("gpt-4.1-mini".to_string());
    let provider_temperature = RwSignal::new("0.2".to_string());
    let provider_max_tokens = RwSignal::new("1024".to_string());
    let provider_capabilities = RwSignal::new(
        "text_generation,structured_generation,image_generation,code_generation".to_string(),
    );
    let provider_allowed_tasks = RwSignal::new(String::new());
    let provider_denied_tasks = RwSignal::new(String::new());
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

    let principal_slug = RwSignal::new(String::new());
    let selected_agent_descriptor = RwSignal::new(String::new());
    let selected_agent_principal = RwSignal::new(String::new());
    let selected_agent_roles = RwSignal::new(Vec::<String>::new());
    let agent_principal_active = RwSignal::new(true);
    let assignment_principal_id = RwSignal::new(String::new());
    let assignment_provider_profile_id = RwSignal::new(String::new());
    let assignment_model_override = RwSignal::new(String::new());
    let assignment_execution_mode = RwSignal::new("auto".to_string());
    let assignment_active = RwSignal::new(true);
    let selected_assignment_id = RwSignal::new(String::new());

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
    let product_attributes_title = RwSignal::new(t(
        ui_locale.as_deref(),
        "ai.job.productAttributesTitle",
        "Product Attributes",
    ));
    let product_attributes_locale = RwSignal::new(String::new());
    let product_attributes_product_id = RwSignal::new(String::new());
    let product_attributes_category_slug = RwSignal::new(String::new());
    let product_attributes_source_locale = RwSignal::new(String::new());
    let product_attributes_source_title = RwSignal::new(String::new());
    let product_attributes_source_description = RwSignal::new(String::new());
    let product_attributes_image_urls = RwSignal::new(String::new());
    let product_attributes_copy_instructions = RwSignal::new(String::new());
    let product_attributes_assistant_prompt = RwSignal::new(String::new());
    let order_analytics_title = RwSignal::new(t(
        ui_locale.as_deref(),
        "ai.job.orderAnalyticsTitle",
        "Order Analytics",
    ));
    let order_analytics_locale = RwSignal::new(String::new());
    let order_analytics_order_ids = RwSignal::new(String::new());
    let order_analytics_date_from = RwSignal::new(String::new());
    let order_analytics_date_to = RwSignal::new(String::new());
    let order_analytics_focus = RwSignal::new(String::new());
    let order_analytics_assistant_prompt = RwSignal::new(String::new());
    let order_ops_title = RwSignal::new(t(
        ui_locale.as_deref(),
        "ai.job.orderOpsAssistantTitle",
        "Order Operations Assistant",
    ));
    let order_ops_locale = RwSignal::new(String::new());
    let order_ops_order_id = RwSignal::new(String::new());
    let order_ops_recommended_action = RwSignal::new(String::new());
    let order_ops_context = RwSignal::new(String::new());
    let order_ops_assistant_prompt = RwSignal::new(String::new());
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
        move |_| async move { transport::fetch_bootstrap().await },
    );

    let session_detail = local_resource(
        move || (selected_session.get(), refresh_nonce.get()),
        move |(session_id, _)| async move {
            match session_id {
                Some(session_id) => transport::fetch_session(session_id).await,
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
    let agent_principal_created_template = t(
        ui_locale.as_deref(),
        "ai.feedback.agentPrincipalCreated",
        "Agent principal `{slug}` created.",
    );
    let agent_principal_updated_template = t(
        ui_locale.as_deref(),
        "ai.feedback.agentPrincipalUpdated",
        "Agent principal `{slug}` updated.",
    );
    let model_assignment_created_template = t(
        ui_locale.as_deref(),
        "ai.feedback.modelAssignmentCreated",
        "Model assignment created.",
    );
    let model_assignment_updated_template = t(
        ui_locale.as_deref(),
        "ai.feedback.modelAssignmentUpdated",
        "Model assignment updated.",
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
    let product_attributes_completed_template = t(
        ui_locale.as_deref(),
        "ai.feedback.productAttributesCompleted",
        "Product attributes job `{title}` completed.",
    );
    let order_analytics_completed_template = t(
        ui_locale.as_deref(),
        "ai.feedback.orderAnalyticsCompleted",
        "Order analytics job `{title}` completed.",
    );
    let order_ops_completed_template = t(
        ui_locale.as_deref(),
        "ai.feedback.orderOpsAssistantCompleted",
        "Order operations assistant job `{title}` completed.",
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
    let err_select_product_attributes_task = t(
        ui_locale.as_deref(),
        "ai.error.selectProductAttributesTaskProfile",
        "Select the `product_attributes` task profile before generating product attributes.",
    );
    let err_select_order_analytics_task = t(
        ui_locale.as_deref(),
        "ai.error.selectOrderAnalyticsTaskProfile",
        "Select the `order_analytics` task profile before generating order analytics.",
    );
    let err_select_order_ops_task = t(
        ui_locale.as_deref(),
        "ai.error.selectOrderOpsAssistantTaskProfile",
        "Select the `order_ops_assistant` task profile before running the order operations assistant.",
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
    let err_product_attributes_payload = t(
        ui_locale.as_deref(),
        "ai.error.assembleProductAttributesPayload",
        "Failed to assemble product attributes payload. Check product id and seed fields.",
    );
    let err_order_analytics_payload = t(
        ui_locale.as_deref(),
        "ai.error.assembleOrderAnalyticsPayload",
        "Failed to assemble order analytics payload. Check order ids and RFC 3339 dates.",
    );
    let err_order_ops_payload = t(
        ui_locale.as_deref(),
        "ai.error.assembleOrderOpsAssistantPayload",
        "Failed to assemble order operations payload. Check the order id.",
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
    let product_attributes_session_query_writer = query_writer.clone();
    let order_analytics_session_query_writer = query_writer.clone();
    let order_ops_session_query_writer = query_writer.clone();
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
                                provider_integration,
                                provider_credential_refs,
                                provider_model,
                                provider_temperature,
                                provider_max_tokens,
                                provider_capabilities,
                                provider_allowed_tasks,
                                provider_denied_tasks,
                                provider_active,
                                profile,
                            );
                        } else {
                            clear_provider_profile(
                                selected_provider,
                                provider_slug,
                                provider_name,
                                provider_integration,
                                provider_credential_refs,
                                provider_model,
                                provider_temperature,
                                provider_max_tokens,
                                provider_capabilities,
                                provider_allowed_tasks,
                                provider_denied_tasks,
                                provider_active,
                            );
                            provider_query_writer.clear_key(AdminQueryKey::ProviderSlug.as_str());
                        }
                    }
                    None => clear_provider_profile(
                        selected_provider,
                        provider_slug,
                        provider_name,
                        provider_integration,
                        provider_credential_refs,
                        provider_model,
                        provider_temperature,
                        provider_max_tokens,
                        provider_capabilities,
                        provider_allowed_tasks,
                        provider_denied_tasks,
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
                sequence: 0,
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

            let init_message = serde_json::to_string(&connection_init_message(
                token_value,
                tenant_value,
                host_admin_locale(ui_locale_value.as_deref()),
            ))
            .unwrap_or_default();
            let subscribe_message = serde_json::to_string(&session_events_subscribe_message(
                "ai-session-events",
                session_id,
            ))
            .unwrap_or_default();

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
                                AiRunStreamEventKindPayload::ToolCall => "TOOL_CALL",
                                AiRunStreamEventKindPayload::Usage => "USAGE",
                                AiRunStreamEventKindPayload::Completed => "COMPLETED",
                                AiRunStreamEventKindPayload::Failed => "FAILED",
                                AiRunStreamEventKindPayload::Cancelled => "CANCELLED",
                                AiRunStreamEventKindPayload::WaitingApproval => "WAITING_APPROVAL",
                            }
                            .to_string();
                            let usage = event.usage.map(|usage| {
                                format!(
                                    "tokens: input {}, output {}, total {}",
                                    usage.input_tokens, usage.output_tokens, usage.total_tokens
                                )
                            });
                            let content = event
                                .accumulated_content
                                .or(event.content_delta)
                                .or(event.tool_call.map(|tool_call| {
                                    format!("{}({})", tool_call.name, tool_call.arguments)
                                }))
                                .or(usage.clone())
                                .unwrap_or_default();
                            let is_terminal = matches!(
                                event.event_kind,
                                AiRunStreamEventKindPayload::Completed
                                    | AiRunStreamEventKindPayload::Failed
                                    | AiRunStreamEventKindPayload::Cancelled
                                    | AiRunStreamEventKindPayload::WaitingApproval
                            );

                            let is_duplicate = live_stream.get_untracked().is_some_and(|current| {
                                current.run_id == event.run_id && event.sequence <= current.sequence
                            });
                            if is_duplicate {
                                return;
                            }
                            set_live_stream.set(Some(AiLiveStreamStatePayload {
                                run_id: event.run_id,
                                status,
                                content,
                                error_message: event.error_message,
                                sequence: event.sequence,
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
                                    sequence: 0,
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
            let result = transport::create_provider(
                provider_slug.get_untracked(),
                provider_name.get_untracked(),
                provider_integration.get_untracked(),
                provider_model.get_untracked(),
                provider_credential_refs.get_untracked(),
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

    let reset_provider_form = Callback::new(move |_| {
        reset_provider_query_writer.clear_key(AdminQueryKey::ProviderSlug.as_str());
        clear_provider_profile(
            selected_provider,
            provider_slug,
            provider_name,
            provider_integration,
            provider_credential_refs,
            provider_model,
            provider_temperature,
            provider_max_tokens,
            provider_capabilities,
            provider_allowed_tasks,
            provider_denied_tasks,
            provider_active,
        );
    });

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
            let result = transport::update_provider(
                provider_id,
                provider_name.get_untracked(),
                provider_integration.get_untracked(),
                provider_model.get_untracked(),
                provider_credential_refs.get_untracked(),
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
            match transport::test_provider(provider_id).await {
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
            match transport::deactivate_provider(provider_id).await {
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
            let result = transport::create_tool_profile(
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

    let reset_tool_form = Callback::new(move |_| {
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
    });

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
            let result = transport::update_tool_profile(
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

    let on_create_agent_principal = move |input: AiAgentPrincipalCreateForm| {
        set_feedback.set(None);
        set_error.set(None);
        let agent_principal_created_template = agent_principal_created_template.clone();
        spawn_local(async move {
            match transport::create_agent_principal(
                input.slug,
                input.descriptor_owner,
                input.descriptor_slug,
                input.role_slugs,
            )
            .await
            {
                Ok(principal) => {
                    selected_agent_principal.set(principal.id.clone());
                    principal_slug.set(principal.slug.clone());
                    selected_agent_descriptor.set(principal.descriptor_slug.clone());
                    selected_agent_roles.set(principal.role_slugs.clone());
                    agent_principal_active.set(principal.is_active);
                    set_feedback.set(Some(
                        agent_principal_created_template.replace("{slug}", principal.slug.as_str()),
                    ));
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(error) => set_error.set(Some(error.to_string())),
            }
        });
    };

    let on_update_agent_principal = move |input: AiAgentPrincipalUpdateForm| {
        set_feedback.set(None);
        set_error.set(None);
        let agent_principal_updated_template = agent_principal_updated_template.clone();
        spawn_local(async move {
            match transport::update_agent_principal(input.id, input.role_slugs, input.is_active)
                .await
            {
                Ok(principal) => {
                    selected_agent_principal.set(principal.id.clone());
                    principal_slug.set(principal.slug.clone());
                    selected_agent_descriptor.set(principal.descriptor_slug.clone());
                    selected_agent_roles.set(principal.role_slugs.clone());
                    agent_principal_active.set(principal.is_active);
                    set_feedback.set(Some(
                        agent_principal_updated_template.replace("{slug}", principal.slug.as_str()),
                    ));
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(error) => set_error.set(Some(error.to_string())),
            }
        });
    };

    let on_create_model_assignment = move |input: AiAgentModelAssignmentCreateForm| {
        set_feedback.set(None);
        set_error.set(None);
        let model_assignment_created_template = model_assignment_created_template.clone();
        spawn_local(async move {
            match transport::create_agent_model_assignment(
                input.agent_principal_id,
                input.provider_profile_id,
                input.model_override,
                input.execution_mode,
            )
            .await
            {
                Ok(assignment) => {
                    selected_assignment_id.set(assignment.id.clone());
                    assignment_principal_id.set(assignment.agent_principal_id.clone());
                    assignment_provider_profile_id.set(assignment.provider_profile_id.clone());
                    assignment_model_override
                        .set(assignment.model_override.clone().unwrap_or_default());
                    assignment_execution_mode.set(assignment.execution_mode.clone());
                    assignment_active.set(assignment.is_active);
                    set_feedback.set(Some(model_assignment_created_template.clone()));
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(error) => set_error.set(Some(error.to_string())),
            }
        });
    };

    let on_update_model_assignment = move |input: AiAgentModelAssignmentUpdateForm| {
        set_feedback.set(None);
        set_error.set(None);
        let model_assignment_updated_template = model_assignment_updated_template.clone();
        spawn_local(async move {
            match transport::update_agent_model_assignment(
                input.id,
                input.model_override,
                input.execution_mode,
                input.is_active,
            )
            .await
            {
                Ok(assignment) => {
                    selected_assignment_id.set(assignment.id.clone());
                    assignment_model_override
                        .set(assignment.model_override.clone().unwrap_or_default());
                    assignment_execution_mode.set(assignment.execution_mode.clone());
                    assignment_active.set(assignment.is_active);
                    set_feedback.set(Some(model_assignment_updated_template.clone()));
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(error) => set_error.set(Some(error.to_string())),
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
            let result = transport::start_session(
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
            let result = transport::run_task_job(
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

        let payload = image_task_payload(ImageTaskPayloadInput {
            prompt: image_prompt.get_untracked(),
            negative_prompt: optional_text(image_negative_prompt.get_untracked()),
            title: optional_text(image_asset_title.get_untracked()),
            alt_text: optional_text(image_alt_text.get_untracked()),
            caption: optional_text(image_caption.get_untracked()),
            file_name: optional_text(image_file_name.get_untracked()),
            size: optional_text(image_size.get_untracked()),
            assistant_prompt: optional_text(image_assistant_prompt.get_untracked()),
        });
        let Ok(payload) = payload else {
            set_error.set(Some(err_image_payload.clone()));
            return;
        };

        set_feedback.set(None);
        set_error.set(None);
        let image_completed_template = image_completed_template.clone();
        let image_session_query_writer = image_session_query_writer.clone();
        spawn_local(async move {
            let result = transport::run_task_job(
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

        let payload = product_task_payload(ProductTaskPayloadInput {
            product_id: product_id.get_untracked(),
            source_locale: optional_text(product_source_locale.get_untracked()),
            source_title: optional_text(product_source_title.get_untracked()),
            source_description: optional_text(product_source_description.get_untracked()),
            source_meta_title: optional_text(product_source_meta_title.get_untracked()),
            source_meta_description: optional_text(product_source_meta_description.get_untracked()),
            copy_instructions: optional_text(product_copy_instructions.get_untracked()),
            assistant_prompt: optional_text(product_assistant_prompt.get_untracked()),
        });
        let Ok(payload) = payload else {
            set_error.set(Some(err_product_payload.clone()));
            return;
        };

        set_feedback.set(None);
        set_error.set(None);
        let product_completed_template = product_completed_template.clone();
        let product_session_query_writer = product_session_query_writer.clone();
        spawn_local(async move {
            let result = transport::run_task_job(
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

    let can_submit_product_attributes = move || {
        let task_profile_id = selected_task_profile.get();
        let has_product_id = !product_attributes_product_id.get().trim().is_empty();
        let matches_product_attributes = bootstrap
            .get()
            .and_then(Result::ok)
            .map(|payload| {
                payload.task_profiles.iter().any(|profile| {
                    profile.id == task_profile_id
                        && profile.slug == "product_attributes"
                        && profile.is_active
                })
            })
            .unwrap_or(false);

        has_product_id && matches_product_attributes
    };

    let can_submit_product_attributes_signal = Signal::derive(can_submit_product_attributes);

    let can_submit_order_analytics = move || {
        let task_profile_id = selected_task_profile.get();
        let has_order_ids = !parse_csv(order_analytics_order_ids.get()).is_empty();
        let matches_order_analytics = bootstrap
            .get()
            .and_then(Result::ok)
            .map(|payload| {
                payload.task_profiles.iter().any(|profile| {
                    profile.id == task_profile_id
                        && profile.slug == "order_analytics"
                        && profile.is_active
                })
            })
            .unwrap_or(false);

        has_order_ids && matches_order_analytics
    };
    let can_submit_order_analytics_signal = Signal::derive(can_submit_order_analytics);

    let can_submit_order_ops = move || {
        let task_profile_id = selected_task_profile.get();
        let has_order_id = !order_ops_order_id.get().trim().is_empty();
        let matches_order_ops = bootstrap
            .get()
            .and_then(Result::ok)
            .map(|payload| {
                payload.task_profiles.iter().any(|profile| {
                    profile.id == task_profile_id
                        && profile.slug == "order_ops_assistant"
                        && profile.is_active
                })
            })
            .unwrap_or(false);

        has_order_id && matches_order_ops
    };
    let can_submit_order_ops_signal = Signal::derive(can_submit_order_ops);

    let on_run_product_attributes_job = move |ev: SubmitEvent| {
        ev.prevent_default();
        set_feedback.set(None);
        set_error.set(None);
        let task_profile_id = selected_task_profile.get_untracked();
        if task_profile_id.trim().is_empty() {
            set_error.set(Some(err_select_product_attributes_task.clone()));
            return;
        }
        let selected_profile_is_product_attributes = bootstrap
            .get_untracked()
            .and_then(Result::ok)
            .map(|payload| {
                payload.task_profiles.iter().any(|profile| {
                    profile.id == task_profile_id
                        && profile.slug == "product_attributes"
                        && profile.is_active
                })
            })
            .unwrap_or(false);
        if !selected_profile_is_product_attributes {
            set_error.set(Some(err_select_product_attributes_task.clone()));
            return;
        }

        let payload = product_attributes_task_payload(ProductAttributesTaskPayloadInput {
            product_id: product_attributes_product_id.get_untracked(),
            category_slug: optional_text(product_attributes_category_slug.get_untracked()),
            source_locale: optional_text(product_attributes_source_locale.get_untracked()),
            source_title: optional_text(product_attributes_source_title.get_untracked()),
            source_description: optional_text(
                product_attributes_source_description.get_untracked(),
            ),
            image_urls_csv: product_attributes_image_urls.get_untracked(),
            copy_instructions: optional_text(product_attributes_copy_instructions.get_untracked()),
            assistant_prompt: optional_text(product_attributes_assistant_prompt.get_untracked()),
        });
        let Ok(payload) = payload else {
            set_error.set(Some(err_product_attributes_payload.clone()));
            return;
        };

        let product_completed_template = product_attributes_completed_template.clone();
        let product_attributes_session_query_writer =
            product_attributes_session_query_writer.clone();
        spawn_local(async move {
            let result = transport::run_task_job(
                product_attributes_title.get_untracked(),
                optional_text(selected_provider.get_untracked()),
                task_profile_id,
                Some("direct".to_string()),
                optional_text(product_attributes_locale.get_untracked()),
                payload,
            )
            .await;
            match result {
                Ok(result) => {
                    let session_id = result.session.session.id.clone();
                    set_selected_session.set(Some(session_id.clone()));
                    product_attributes_session_query_writer
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

    let on_run_order_analytics_job = move |ev: SubmitEvent| {
        ev.prevent_default();
        set_feedback.set(None);
        set_error.set(None);
        let task_profile_id = selected_task_profile.get_untracked();
        let selected_profile_is_order_analytics = bootstrap
            .get_untracked()
            .and_then(Result::ok)
            .map(|payload| {
                payload.task_profiles.iter().any(|profile| {
                    profile.id == task_profile_id
                        && profile.slug == "order_analytics"
                        && profile.is_active
                })
            })
            .unwrap_or(false);
        if !selected_profile_is_order_analytics {
            set_error.set(Some(err_select_order_analytics_task.clone()));
            return;
        }

        let payload = order_analytics_task_payload(OrderAnalyticsTaskPayloadInput {
            order_ids_csv: order_analytics_order_ids.get_untracked(),
            date_from: optional_text(order_analytics_date_from.get_untracked()),
            date_to: optional_text(order_analytics_date_to.get_untracked()),
            focus: optional_text(order_analytics_focus.get_untracked()),
            assistant_prompt: optional_text(order_analytics_assistant_prompt.get_untracked()),
        });
        let Ok(payload) = payload else {
            set_error.set(Some(err_order_analytics_payload.clone()));
            return;
        };

        let completed_template = order_analytics_completed_template.clone();
        let session_query_writer = order_analytics_session_query_writer.clone();
        spawn_local(async move {
            let result = transport::run_task_job(
                order_analytics_title.get_untracked(),
                optional_text(selected_provider.get_untracked()),
                task_profile_id,
                Some("direct".to_string()),
                optional_text(order_analytics_locale.get_untracked()),
                payload,
            )
            .await;
            match result {
                Ok(result) => {
                    let session_id = result.session.session.id.clone();
                    set_selected_session.set(Some(session_id.clone()));
                    session_query_writer
                        .replace_value(AdminQueryKey::SessionId.as_str(), session_id);
                    set_feedback.set(Some(
                        completed_template
                            .replace("{title}", result.session.session.title.as_str()),
                    ));
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(err.to_string())),
            }
        });
    };

    let on_run_order_ops_job = move |ev: SubmitEvent| {
        ev.prevent_default();
        set_feedback.set(None);
        set_error.set(None);
        let task_profile_id = selected_task_profile.get_untracked();
        let selected_profile_is_order_ops = bootstrap
            .get_untracked()
            .and_then(Result::ok)
            .map(|payload| {
                payload.task_profiles.iter().any(|profile| {
                    profile.id == task_profile_id
                        && profile.slug == "order_ops_assistant"
                        && profile.is_active
                })
            })
            .unwrap_or(false);
        if !selected_profile_is_order_ops {
            set_error.set(Some(err_select_order_ops_task.clone()));
            return;
        }

        let payload = order_ops_assistant_task_payload(OrderOpsAssistantTaskPayloadInput {
            order_id: order_ops_order_id.get_untracked(),
            recommended_action: optional_text(order_ops_recommended_action.get_untracked()),
            context: optional_text(order_ops_context.get_untracked()),
            assistant_prompt: optional_text(order_ops_assistant_prompt.get_untracked()),
        });
        let Ok(payload) = payload else {
            set_error.set(Some(err_order_ops_payload.clone()));
            return;
        };

        let completed_template = order_ops_completed_template.clone();
        let session_query_writer = order_ops_session_query_writer.clone();
        spawn_local(async move {
            let result = transport::run_task_job(
                order_ops_title.get_untracked(),
                optional_text(selected_provider.get_untracked()),
                task_profile_id,
                Some("direct".to_string()),
                optional_text(order_ops_locale.get_untracked()),
                payload,
            )
            .await;
            match result {
                Ok(result) => {
                    let session_id = result.session.session.id.clone();
                    set_selected_session.set(Some(session_id.clone()));
                    session_query_writer
                        .replace_value(AdminQueryKey::SessionId.as_str(), session_id);
                    set_feedback.set(Some(
                        completed_template
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

        let payload = blog_task_payload(BlogTaskPayloadInput {
            post_id: optional_text(blog_post_id.get_untracked()),
            source_locale: optional_text(blog_source_locale.get_untracked()),
            source_title: optional_text(blog_source_title.get_untracked()),
            source_body: optional_text(blog_source_body.get_untracked()),
            source_excerpt: optional_text(blog_source_excerpt.get_untracked()),
            source_seo_title: optional_text(blog_source_seo_title.get_untracked()),
            source_seo_description: optional_text(blog_source_seo_description.get_untracked()),
            tags: parse_csv(blog_tags.get_untracked()),
            category_id: optional_text(blog_category_id.get_untracked()),
            featured_image_url: optional_text(blog_featured_image_url.get_untracked()),
            copy_instructions: optional_text(blog_copy_instructions.get_untracked()),
            assistant_prompt: optional_text(blog_assistant_prompt.get_untracked()),
        });
        let Ok(payload) = payload else {
            set_error.set(Some(err_blog_payload.clone()));
            return;
        };

        set_feedback.set(None);
        set_error.set(None);
        let blog_completed_template = blog_completed_template.clone();
        let blog_session_query_writer = blog_session_query_writer.clone();
        spawn_local(async move {
            let result = transport::run_task_job(
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

    let reset_task_form = Callback::new(move |_| {
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
    });

    let on_create_task_profile = move |ev: SubmitEvent| {
        ev.prevent_default();
        set_feedback.set(None);
        set_error.set(None);
        let task_created_template = task_created_template.clone();
        let create_task_query_writer = create_task_query_writer.clone();
        spawn_local(async move {
            let result = transport::create_task_profile(
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
            let result = transport::update_task_profile(
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
            let result = transport::send_message(session_id, content).await;
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
                    let ui_locale_operator = ui_locale.clone();
                    let on_create_provider = on_create_provider.clone();
                    let on_update_provider = on_update_provider.clone();
                    let on_test_provider = on_test_provider.clone();
                    let on_deactivate_provider = on_deactivate_provider.clone();
                    let reset_provider_form = reset_provider_form;
                    let on_create_tool_profile = on_create_tool_profile.clone();
                    let on_update_tool_profile = on_update_tool_profile.clone();
                    let on_create_agent_principal = on_create_agent_principal.clone();
                    let on_update_agent_principal = on_update_agent_principal.clone();
                    let on_create_model_assignment = on_create_model_assignment.clone();
                    let on_update_model_assignment = on_update_model_assignment.clone();
                    let reset_tool_form = reset_tool_form;
                    let on_create_task_profile = on_create_task_profile.clone();
                    let on_update_task_profile = on_update_task_profile.clone();
                    let reset_task_form = reset_task_form;
                    let on_run_blog_job = on_run_blog_job.clone();
                    let on_run_product_job = on_run_product_job.clone();
                    let on_run_product_attributes_job = on_run_product_attributes_job.clone();
                    let on_run_order_analytics_job = on_run_order_analytics_job.clone();
                    let on_run_order_ops_job = on_run_order_ops_job.clone();
                    let on_run_image_job = on_run_image_job.clone();
                    let on_run_alloy_job = on_run_alloy_job.clone();
                    let on_start_session = on_start_session.clone();
                    let on_send_message = on_send_message.clone();
                    let select_provider_query_writer = query_writer.clone();
                    let select_tool_query_writer = query_writer.clone();
                    let select_task_query_writer = query_writer.clone();
                    let select_session_query_writer = query_writer.clone();
                    bootstrap.get().map(|result| match result {
                    Ok(bootstrap) => {
                        let bootstrap_left = bootstrap.clone();
                        let bootstrap_diagnostics = bootstrap.clone();
                        let bootstrap_chat = bootstrap.clone();

                        let ui_locale_left = ui_locale.clone();
                        let ui_locale_diagnostics = ui_locale.clone();
                        let ui_locale_jobs = ui_locale.clone();
                        let ui_locale_chat = ui_locale.clone();

                        view! {
                            <div class=if diagnostics_only.get() {
                                "grid gap-6 xl:grid-cols-[1fr_1.6fr]".to_string()
                            } else {
                                "grid gap-6 xl:grid-cols-[1.2fr_1fr_1.6fr]".to_string()
                            }>
                                <Show when=move || !diagnostics_only.get()>
                                    <section class="space-y-6">
                                        <AiProviderPanel
                                            ui_locale=ui_locale_left.clone()
                                            provider_catalog=bootstrap_left.provider_catalog.clone()
                                            provider_targets=bootstrap_left.provider_targets.clone()
                                            providers=bootstrap_left.providers.clone()
                                            provider_slug=provider_slug
                                            provider_name=provider_name
                                            provider_integration=provider_integration
                                            provider_credential_refs=provider_credential_refs
                                            provider_model=provider_model
                                            provider_temperature=provider_temperature
                                            provider_max_tokens=provider_max_tokens
                                            provider_capabilities=provider_capabilities
                                            provider_allowed_tasks=provider_allowed_tasks
                                            provider_denied_tasks=provider_denied_tasks
                                            provider_active=provider_active
                                            on_create_provider=Callback::new(on_create_provider.clone())
                                            on_update_provider=Callback::new(on_update_provider.clone())
                                            on_test_provider=Callback::new(on_test_provider.clone())
                                            on_deactivate_provider=Callback::new(on_deactivate_provider.clone())
                                            on_reset=reset_provider_form
                                            select_provider_query_writer=select_provider_query_writer.clone()
                                        />

                                        <AiAgentPanel
                                            ui_locale=ui_locale_left.clone()
                                            catalog=bootstrap_left.agent_catalog.clone()
                                            workflows=bootstrap_left.agent_workflows.clone()
                                            principals=bootstrap_left.agent_principals.clone()
                                            assignments=bootstrap_left.agent_model_assignments.clone()
                                            providers=bootstrap_left.providers.clone()
                                            tenant_rbac_roles=bootstrap_left.tenant_rbac_roles.clone()
                                            tenant_rbac_permissions=bootstrap_left.tenant_rbac_permissions.clone()
                                            principal_slug=principal_slug
                                            selected_descriptor_slug=selected_agent_descriptor
                                            selected_principal_id=selected_agent_principal
                                            selected_role_slugs=selected_agent_roles
                                            principal_active=agent_principal_active
                                            on_create_principal=Callback::new(on_create_agent_principal.clone())
                                            on_update_principal=Callback::new(on_update_agent_principal.clone())
                                            assignment_principal_id
                                            assignment_provider_profile_id
                                            assignment_model_override
                                            assignment_execution_mode
                                            assignment_active
                                            selected_assignment_id
                                            on_create_assignment=Callback::new(on_create_model_assignment.clone())
                                            on_update_assignment=Callback::new(on_update_model_assignment.clone())
                                        />

                                        <AiToolPanel
                                            ui_locale=ui_locale_left.clone()
                                            tool_profiles=bootstrap_left.tool_profiles.clone()
                                            tool_slug=tool_slug
                                            tool_name=tool_name
                                            tool_description=tool_description
                                            tool_allowed=tool_allowed
                                            tool_denied=tool_denied
                                            tool_sensitive=tool_sensitive
                                            tool_active=tool_active
                                            on_create_tool_profile=Callback::new(on_create_tool_profile.clone())
                                            on_update_tool_profile=Callback::new(on_update_tool_profile.clone())
                                            on_reset=reset_tool_form
                                            select_tool_query_writer=select_tool_query_writer.clone()
                                        />

                                        <AiTaskPanel
                                            ui_locale=ui_locale_left.clone()
                                            task_profiles=bootstrap_left.task_profiles.clone()
                                            task_slug=task_slug
                                            task_name=task_name
                                            task_description=task_description
                                            task_capability=task_capability
                                            task_system_prompt=task_system_prompt
                                            task_allowed_providers=task_allowed_providers
                                            task_preferred_providers=task_preferred_providers
                                            task_execution_mode=task_execution_mode
                                            task_active=task_active
                                            on_create_task_profile=Callback::new(on_create_task_profile.clone())
                                            on_update_task_profile=Callback::new(on_update_task_profile.clone())
                                            on_reset=reset_task_form
                                            select_task_query_writer=select_task_query_writer.clone()
                                        />
                                    </section>
                                </Show>

                                <section class="space-y-6">
                                    <Show when=move || diagnostics_only.get()>
                                        <AiDiagnosticsPanel
                                            ui_locale=ui_locale_diagnostics.clone()
                                            bootstrap=bootstrap_diagnostics.clone()
                                        />
                                    </Show>

                                    <Show when=move || !diagnostics_only.get()>
                                        <AiJobsPanel
                                            ui_locale=ui_locale_jobs.clone()

                                            blog_title=blog_title
                                            blog_locale=blog_locale
                                            blog_post_id=blog_post_id
                                            blog_source_locale=blog_source_locale
                                            blog_source_title=blog_source_title
                                            blog_source_body=blog_source_body
                                            blog_source_excerpt=blog_source_excerpt
                                            blog_source_seo_title=blog_source_seo_title
                                            blog_source_seo_description=blog_source_seo_description
                                            blog_tags=blog_tags
                                            blog_category_id=blog_category_id
                                            blog_featured_image_url=blog_featured_image_url
                                            blog_copy_instructions=blog_copy_instructions
                                            blog_assistant_prompt=blog_assistant_prompt
                                            on_run_blog_job=Callback::new(on_run_blog_job.clone())

                                            product_title=product_title
                                            product_locale=product_locale
                                            product_id=product_id
                                            product_source_locale=product_source_locale
                                            product_source_title=product_source_title
                                            product_source_description=product_source_description
                                            product_source_meta_title=product_source_meta_title
                                            product_source_meta_description=product_source_meta_description
                                            product_copy_instructions=product_copy_instructions
                                            product_assistant_prompt=product_assistant_prompt
                                            on_run_product_job=Callback::new(on_run_product_job.clone())

                                            product_attributes_title=product_attributes_title
                                            product_attributes_locale=product_attributes_locale
                                            product_attributes_product_id=product_attributes_product_id
                                            product_attributes_category_slug=product_attributes_category_slug
                                            product_attributes_source_locale=product_attributes_source_locale
                                            product_attributes_source_title=product_attributes_source_title
                                            product_attributes_source_description=product_attributes_source_description
                                            product_attributes_image_urls=product_attributes_image_urls
                                            product_attributes_copy_instructions=product_attributes_copy_instructions
                                            product_attributes_assistant_prompt=product_attributes_assistant_prompt
                                            on_run_product_attributes_job=Callback::new(on_run_product_attributes_job.clone())
                                            can_submit_product_attributes=can_submit_product_attributes_signal

                                            order_analytics_title=order_analytics_title
                                            order_analytics_locale=order_analytics_locale
                                            order_analytics_order_ids=order_analytics_order_ids
                                            order_analytics_date_from=order_analytics_date_from
                                            order_analytics_date_to=order_analytics_date_to
                                            order_analytics_focus=order_analytics_focus
                                            order_analytics_assistant_prompt=order_analytics_assistant_prompt
                                            on_run_order_analytics_job=Callback::new(on_run_order_analytics_job.clone())
                                            can_submit_order_analytics=can_submit_order_analytics_signal

                                            order_ops_title=order_ops_title
                                            order_ops_locale=order_ops_locale
                                            order_ops_order_id=order_ops_order_id
                                            order_ops_recommended_action=order_ops_recommended_action
                                            order_ops_context=order_ops_context
                                            order_ops_assistant_prompt=order_ops_assistant_prompt
                                            on_run_order_ops_job=Callback::new(on_run_order_ops_job.clone())
                                            can_submit_order_ops=can_submit_order_ops_signal

                                            image_title=image_title
                                            image_locale=image_locale
                                            image_prompt=image_prompt
                                            image_negative_prompt=image_negative_prompt
                                            image_file_name=image_file_name
                                            image_asset_title=image_asset_title
                                            image_alt_text=image_alt_text
                                            image_caption=image_caption
                                            image_size=image_size
                                            image_assistant_prompt=image_assistant_prompt
                                            on_run_image_job=Callback::new(on_run_image_job.clone())

                                            alloy_title=alloy_title
                                            alloy_locale=alloy_locale
                                            alloy_operation=alloy_operation
                                            alloy_script_id=alloy_script_id
                                            alloy_script_name=alloy_script_name
                                            alloy_script_source=alloy_script_source
                                            alloy_runtime_payload=alloy_runtime_payload
                                            alloy_prompt=alloy_prompt
                                            on_run_alloy_job=Callback::new(on_run_alloy_job.clone())

                                            session_title=session_title
                                            session_locale=session_locale
                                            session_message=session_message
                                            selected_provider=selected_provider
                                            selected_task_profile=selected_task_profile
                                            selected_tool_profile=selected_tool_profile
                                            on_start_session=Callback::new(on_start_session.clone())
                                        />
                                    </Show>

                                    <AiChatSessionPanel
                                        ui_locale=ui_locale_chat.clone()
                                        bootstrap=bootstrap_chat.clone()
                                        session_detail=session_detail
                                        live_stream=live_stream.into()
                                        reply_message=reply_message
                                        on_send_message=Callback::new(on_send_message.clone())
                                        select_session_query_writer=select_session_query_writer.clone()
                                        set_refresh_nonce=set_refresh_nonce
                                    />
                                </section>
                            </div>
                        }.into_any()
                    },
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
pub(crate) fn Card(#[prop(into)] title: String, children: Children) -> impl IntoView {
    view! {
        <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
            <h2 class="mb-4 text-lg font-semibold text-card-foreground">{title}</h2>
            {children()}
        </section>
    }
}

#[component]
pub(crate) fn TextField(
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
pub(crate) fn InfoItem(#[prop(into)] label: String, value: String) -> impl IntoView {
    view! {
        <div class="rounded-lg border border-border px-3 py-3">
            <div class="text-xs uppercase tracking-wide text-muted-foreground">{label}</div>
            <div class="mt-1 text-lg font-semibold text-card-foreground">{value}</div>
        </div>
    }
}

pub(crate) fn bucket_summary(locale: Option<&str>, buckets: &[AiMetricBucketPayload]) -> String {
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

pub(crate) fn recent_run_summary(
    locale: Option<&str>,
    runs: &[model::AiRecentRunPayload],
) -> String {
    if runs.is_empty() {
        return t(
            locale,
            "ai.diagnostics.noRecentEvents",
            "No recent events yet.",
        );
    }

    let stats = summarize_recent_runs(
        runs.iter()
            .map(|run| (run.status.as_str(), run.duration_ms)),
    );

    t(
        locale,
        "ai.summary.recentRuns",
        "{count} run(s), {failed} failed, {waiting} waiting approval, avg {latency} ms",
    )
    .replace("{count}", stats.total.to_string().as_str())
    .replace("{failed}", stats.failed.to_string().as_str())
    .replace("{waiting}", stats.waiting_approval.to_string().as_str())
    .replace("{latency}", stats.average_latency_ms.to_string().as_str())
}

pub(crate) fn stream_event_kind_label(
    locale: Option<&str>,
    value: &model::AiRunStreamEventKindPayload,
) -> String {
    match value {
        model::AiRunStreamEventKindPayload::Started => t(locale, "ai.status.started", "STARTED"),
        model::AiRunStreamEventKindPayload::Delta => t(locale, "ai.status.delta", "DELTA"),
        model::AiRunStreamEventKindPayload::ToolCall => {
            t(locale, "ai.status.toolCall", "TOOL CALL")
        }
        model::AiRunStreamEventKindPayload::Usage => t(locale, "ai.status.usage", "USAGE"),
        model::AiRunStreamEventKindPayload::Completed => {
            t(locale, "ai.status.completed", "COMPLETED")
        }
        model::AiRunStreamEventKindPayload::Failed => t(locale, "ai.status.failed", "FAILED"),
        model::AiRunStreamEventKindPayload::Cancelled => {
            t(locale, "ai.status.cancelled", "CANCELLED")
        }
        model::AiRunStreamEventKindPayload::WaitingApproval => {
            t(locale, "ai.status.waitingApproval", "WAITING_APPROVAL")
        }
    }
}

pub(crate) fn average_run_latency_summary(locale: Option<&str>, latency_ms: u64) -> String {
    t(
        locale,
        "ai.diagnostics.averageRunLatency",
        "Average run latency: {value} ms",
    )
    .replace("{value}", latency_ms.to_string().as_str())
}

pub(crate) fn provider_profile_summary(
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

pub(crate) fn tool_profile_summary(
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

pub(crate) fn task_profile_summary(
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

pub(crate) fn direct_transport_summary(
    locale: Option<&str>,
    provider: &str,
    task_profile: &str,
) -> String {
    t(
        locale,
        "ai.summary.transportDirect",
        "Provider: {provider} | Task profile: {task_profile} | Mode: {mode}",
    )
    .replace("{provider}", provider)
    .replace("{task_profile}", task_profile)
    .replace("{mode}", t(locale, "ai.common.direct", "direct").as_str())
}

pub(crate) fn session_transport_summary(
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

pub(crate) fn session_list_summary(
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

pub(crate) fn session_profile_summary(
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

pub(crate) fn locale_flow_summary(
    locale: Option<&str>,
    requested: Option<&str>,
    resolved: &str,
) -> String {
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

pub(crate) fn run_path_summary(
    locale: Option<&str>,
    status: &str,
    mode: &str,
    path: &str,
) -> String {
    t(
        locale,
        "ai.summary.runPath",
        "{status} · {mode} · path {path}",
    )
    .replace("{status}", status)
    .replace("{mode}", mode)
    .replace("{path}", path)
}

pub(crate) fn tool_trace_summary(locale: Option<&str>, status: &str, duration_ms: i64) -> String {
    t(locale, "ai.summary.toolTrace", "{status} · {duration} ms")
        .replace("{status}", status)
        .replace("{duration}", duration_ms.to_string().as_str())
}

pub(crate) fn stream_status_summary(locale: Option<&str>, connected: bool, status: &str) -> String {
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
    provider_integration: RwSignal<String>,
    provider_credential_refs: RwSignal<Vec<crate::model::AiCredentialRefPayload>>,
    provider_model: RwSignal<String>,
    provider_temperature: RwSignal<String>,
    provider_max_tokens: RwSignal<String>,
    provider_capabilities: RwSignal<String>,
    provider_allowed_tasks: RwSignal<String>,
    provider_denied_tasks: RwSignal<String>,
    provider_active: RwSignal<bool>,
    profile: &AiProviderProfilePayload,
) {
    selected_provider.set(profile.id.clone());
    provider_slug.set(profile.slug.clone());
    provider_name.set(profile.display_name.clone());
    provider_integration.set(profile.provider_target_id.clone());
    provider_credential_refs.set(profile.credential_refs.clone());
    provider_model.set(profile.model.clone());
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
    provider_active.set(profile.is_active);
}

fn clear_provider_profile(
    selected_provider: RwSignal<String>,
    provider_slug: RwSignal<String>,
    provider_name: RwSignal<String>,
    provider_integration: RwSignal<String>,
    provider_credential_refs: RwSignal<Vec<crate::model::AiCredentialRefPayload>>,
    provider_model: RwSignal<String>,
    provider_temperature: RwSignal<String>,
    provider_max_tokens: RwSignal<String>,
    provider_capabilities: RwSignal<String>,
    provider_allowed_tasks: RwSignal<String>,
    provider_denied_tasks: RwSignal<String>,
    provider_active: RwSignal<bool>,
) {
    selected_provider.set(String::new());
    provider_slug.set(String::new());
    provider_name.set(String::new());
    provider_integration.set(String::new());
    provider_credential_refs.set(Vec::new());
    provider_model.set("gpt-4.1-mini".to_string());
    provider_temperature.set("0.2".to_string());
    provider_max_tokens.set("1024".to_string());
    provider_capabilities
        .set("text_generation,structured_generation,image_generation,code_generation".to_string());
    provider_allowed_tasks.set(String::new());
    provider_denied_tasks.set(String::new());
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
    let protocol = location.protocol().ok();
    let host = location.host().ok();
    graphql_ws_url_from_location(protocol.as_deref(), host.as_deref())
}

#[cfg(target_arch = "wasm32")]
fn host_admin_locale(preferred: Option<&str>) -> Option<String> {
    preferred
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}
