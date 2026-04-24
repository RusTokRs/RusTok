use super::module_card::ModuleCard;
use super::module_detail_panel::ModuleDetailPanel;
use super::module_update_card::ModuleUpdateCard;
use crate::app::providers::enabled_modules::use_enabled_modules_context;
use crate::entities::module::{
    BuildJob, InstalledModule, MarketplaceModule, ModuleInfo, ModuleSettingField, ReleaseInfo,
    TenantModule,
};
use crate::features::modules::api;
#[cfg(target_arch = "wasm32")]
use crate::shared::api as shared_api;
use crate::shared::ui::ui_success_message as UiSuccessMessage;
use crate::{t, t_string, use_i18n};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_hook_form::FormState;
use leptos_router::hooks::{use_navigate, use_query_map};
use leptos_use::use_interval_fn;
use std::collections::{HashMap, HashSet};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{closure::Closure, JsCast};
#[cfg(target_arch = "wasm32")]
use web_sys::{CloseEvent, ErrorEvent, Event, MessageEvent, WebSocket};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ModulesTab {
    Installed,
    Marketplace,
    Updates,
}

#[derive(Clone, PartialEq, Eq)]
struct CatalogFilters {
    search: String,
    category: String,
    tag: String,
    source: String,
    trust_level: String,
    only_compatible: bool,
    installed_only: bool,
}

impl Default for CatalogFilters {
    fn default() -> Self {
        Self {
            search: String::new(),
            category: "all".to_string(),
            tag: "all".to_string(),
            source: "all".to_string(),
            trust_level: "all".to_string(),
            only_compatible: false,
            installed_only: false,
        }
    }
}

fn humanize_label(value: &str) -> String {
    value
        .to_lowercase()
        .split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn pretty_json(value: &str) -> String {
    serde_json::from_str::<serde_json::Value>(value)
        .ok()
        .and_then(|json| serde_json::to_string_pretty(&json).ok())
        .unwrap_or_else(|| value.to_string())
}

fn parse_settings_object(value: &str) -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_str::<serde_json::Value>(value)
        .ok()
        .and_then(|json| json.as_object().cloned())
        .unwrap_or_default()
}

fn schema_supports_generated_form(schema: &[ModuleSettingField]) -> bool {
    !schema.is_empty()
}

fn setting_field_draft_value(
    field: &ModuleSettingField,
    value: Option<&serde_json::Value>,
) -> String {
    let value = value.or(field.default_value.as_ref());
    match field.value_type.as_str() {
        "string" => value
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        "integer" => value
            .and_then(|value| {
                value
                    .as_i64()
                    .map(|number| number.to_string())
                    .or_else(|| value.as_u64().map(|number| number.to_string()))
            })
            .unwrap_or_default(),
        "number" => value
            .and_then(serde_json::Value::as_f64)
            .map(|number| {
                let mut rendered = number.to_string();
                if rendered.ends_with(".0") {
                    rendered.truncate(rendered.len() - 2);
                }
                rendered
            })
            .unwrap_or_default(),
        "boolean" => value
            .and_then(serde_json::Value::as_bool)
            .map(|flag| flag.to_string())
            .unwrap_or_else(|| "false".to_string()),
        "object" | "array" | "json" | "any" => value
            .and_then(|value| serde_json::to_string_pretty(value).ok())
            .unwrap_or_default(),
        _ => value.map(ToString::to_string).unwrap_or_default(),
    }
}

fn build_settings_form_draft(
    schema: &[ModuleSettingField],
    tenant_module: Option<&TenantModule>,
) -> HashMap<String, String> {
    let existing = tenant_module
        .map(|module| parse_settings_object(&module.settings))
        .unwrap_or_default();

    schema
        .iter()
        .map(|field| {
            (
                field.key.clone(),
                setting_field_draft_value(field, existing.get(&field.key)),
            )
        })
        .collect()
}

fn schema_settings_payload(
    schema: &[ModuleSettingField],
    draft: &HashMap<String, String>,
) -> Result<String, String> {
    let mut payload = serde_json::Map::new();

    for field in schema {
        let raw = draft.get(&field.key).cloned().unwrap_or_default();
        let trimmed = raw.trim();

        let value = match field.value_type.as_str() {
            "string" => serde_json::Value::String(raw),
            "integer" => {
                if trimmed.is_empty() {
                    continue;
                }
                let parsed = trimmed
                    .parse::<i64>()
                    .map_err(|_| format!("{} must be an integer", humanize_label(&field.key)))?;
                serde_json::json!(parsed)
            }
            "number" => {
                if trimmed.is_empty() {
                    continue;
                }
                let parsed = trimmed
                    .parse::<f64>()
                    .map_err(|_| format!("{} must be a number", humanize_label(&field.key)))?;
                serde_json::Number::from_f64(parsed)
                    .map(serde_json::Value::Number)
                    .ok_or_else(|| {
                        format!("{} must be a finite number", humanize_label(&field.key))
                    })?
            }
            "boolean" => serde_json::Value::Bool(trimmed.eq_ignore_ascii_case("true")),
            "object" => {
                if trimmed.is_empty() {
                    continue;
                }
                let parsed = serde_json::from_str::<serde_json::Value>(trimmed).map_err(|err| {
                    format!("{} must be valid JSON: {}", humanize_label(&field.key), err)
                })?;
                if !parsed.is_object() {
                    return Err(format!(
                        "{} must be a JSON object",
                        humanize_label(&field.key)
                    ));
                }
                parsed
            }
            "array" => {
                if trimmed.is_empty() {
                    continue;
                }
                let parsed = serde_json::from_str::<serde_json::Value>(trimmed).map_err(|err| {
                    format!("{} must be valid JSON: {}", humanize_label(&field.key), err)
                })?;
                if !parsed.is_array() {
                    return Err(format!(
                        "{} must be a JSON array",
                        humanize_label(&field.key)
                    ));
                }
                parsed
            }
            "json" | "any" => {
                if trimmed.is_empty() {
                    continue;
                }
                serde_json::from_str::<serde_json::Value>(trimmed).map_err(|err| {
                    format!("{} must be valid JSON: {}", humanize_label(&field.key), err)
                })?
            }
            _ => continue,
        };

        payload.insert(field.key.clone(), value);
    }

    serde_json::to_string(&serde_json::Value::Object(payload))
        .map_err(|err| format!("Failed to serialize module settings: {err}"))
}

fn tab_button_class(active: bool) -> &'static str {
    if active {
        "inline-flex items-center justify-center rounded-md bg-background px-3 py-2 text-sm font-medium text-foreground shadow-sm transition-colors"
    } else {
        "inline-flex items-center justify-center rounded-md px-3 py-2 text-sm font-medium text-muted-foreground transition-colors hover:text-foreground"
    }
}

fn catalog_entry_to_module_info(module: &MarketplaceModule) -> ModuleInfo {
    ModuleInfo {
        module_slug: module.slug.clone(),
        name: module.name.clone(),
        description: module.description.clone(),
        version: module.latest_version.clone(),
        kind: module.kind.clone(),
        dependencies: module.dependencies.clone(),
        enabled: false,
        ownership: module.ownership.clone(),
        trust_level: module.trust_level.clone(),
        has_admin_ui: module.has_admin_ui,
        has_storefront_ui: module.has_storefront_ui,
        ui_classification: module.ui_classification.clone(),
        recommended_admin_surfaces: module.recommended_admin_surfaces.clone(),
        showcase_admin_surfaces: module.showcase_admin_surfaces.clone(),
    }
}

fn is_build_active(build: &BuildJob) -> bool {
    matches!(build.status.as_str(), "QUEUED" | "RUNNING")
}

#[cfg(target_arch = "wasm32")]
fn is_build_terminal_status(status: &str) -> bool {
    matches!(status, "SUCCESS" | "FAILED" | "CANCELLED")
}

#[cfg(target_arch = "wasm32")]
fn apply_build_progress_to_job(job: &mut BuildJob, event: &api::BuildProgressEvent) {
    job.status = event.status.clone();
    job.stage = event.stage.clone();
    job.progress = event.progress;
    job.release_id = event.release_id.clone();
    job.error_message = event.error_message.clone();
}

fn normalize_catalog_filters(filters: &CatalogFilters) -> api::MarketplaceVariables {
    api::MarketplaceVariables {
        search: (!filters.search.trim().is_empty()).then(|| filters.search.trim().to_string()),
        category: (filters.category != "all").then(|| filters.category.clone()),
        tag: (filters.tag != "all").then(|| filters.tag.clone()),
        source: (filters.source != "all").then(|| filters.source.clone()),
        trust_level: (filters.trust_level != "all").then(|| filters.trust_level.clone()),
        only_compatible: filters.only_compatible.then_some(true),
        installed_only: filters.installed_only.then_some(true),
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum BuildProgressSubscriptionMessage {
    ConnectionAck,
    Next {
        payload: BuildProgressSubscriptionPayload,
    },
    Error {
        payload: Vec<BuildProgressSubscriptionError>,
    },
    Complete,
    Ping {
        payload: Option<serde_json::Value>,
    },
    Pong,
}

#[cfg(target_arch = "wasm32")]
#[derive(serde::Deserialize)]
struct BuildProgressSubscriptionPayload {
    data: Option<BuildProgressSubscriptionData>,
    errors: Option<Vec<BuildProgressSubscriptionError>>,
}

#[cfg(target_arch = "wasm32")]
#[derive(serde::Deserialize)]
struct BuildProgressSubscriptionData {
    #[serde(rename = "buildProgress")]
    build_progress: Option<api::BuildProgressEvent>,
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, serde::Deserialize)]
struct BuildProgressSubscriptionError {
    message: String,
}

#[component]
pub fn ModulesList(
    admin_surface: String,
    modules: Vec<ModuleInfo>,
    marketplace_modules: Vec<MarketplaceModule>,
    installed_modules: Vec<InstalledModule>,
    tenant_modules: Vec<TenantModule>,
    active_build: Option<BuildJob>,
    active_release: Option<ReleaseInfo>,
    build_history: Vec<BuildJob>,
) -> impl IntoView {
    let i18n = use_i18n();
    let (module_list, set_module_list) = signal(modules);
    let (marketplace_catalog, set_marketplace_catalog) = signal(marketplace_modules);
    let (installed_module_list, set_installed_module_list) = signal(installed_modules);
    let (tenant_module_list, set_tenant_module_list) = signal(tenant_modules);
    let (active_build_state, set_active_build_state) = signal(active_build);
    let (active_release_state, set_active_release_state) = signal(active_release);
    let (build_history_state, set_build_history_state) = signal(build_history);
    let (selected_module_slug, set_selected_module_slug) = signal::<Option<String>>(None);
    let (selected_module_detail, set_selected_module_detail) =
        signal::<Option<MarketplaceModule>>(None);
    let (module_detail_loading, set_module_detail_loading) = signal(false);
    let (catalog_filter_draft, set_catalog_filter_draft) = signal(CatalogFilters::default());
    let (applied_catalog_filters, set_applied_catalog_filters) = signal(CatalogFilters::default());
    let (catalog_refreshing, set_catalog_refreshing) = signal(false);
    let (known_categories, set_known_categories) = signal(
        marketplace_catalog
            .get_untracked()
            .into_iter()
            .map(|module| module.category)
            .filter(|category| !category.is_empty())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>(),
    );
    let (known_tags, set_known_tags) = signal(
        marketplace_catalog
            .get_untracked()
            .into_iter()
            .flat_map(|module| module.tags)
            .filter(|tag| !tag.is_empty())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>(),
    );
    let (known_sources, set_known_sources) = signal(
        marketplace_catalog
            .get_untracked()
            .into_iter()
            .map(|module| module.source)
            .filter(|source| !source.is_empty())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>(),
    );
    let (selected_tab, set_selected_tab) = signal(ModulesTab::Installed);
    let (loading_slug, set_loading_slug) = signal::<Option<String>>(None);
    let (platform_loading_slug, set_platform_loading_slug) = signal::<Option<String>>(None);
    let (settings_loading_slug, set_settings_loading_slug) = signal::<Option<String>>(None);
    let (rollback_loading_build_id, set_rollback_loading_build_id) = signal::<Option<String>>(None);
    let (module_settings_draft, set_module_settings_draft) = signal("{}".to_string());
    let (module_settings_form_draft, set_module_settings_form_draft) =
        signal(HashMap::<String, String>::new());
    let (form_state, set_form_state) = signal(FormState::idle());
    let (success_message, set_success_message) = signal::<Option<String>>(None);
    let (live_subscription_connected, set_live_subscription_connected) = signal(false);
    let token = use_token();
    let tenant = use_tenant();
    let navigate = use_navigate();
    let query = use_query_map();
    let enabled_modules = use_enabled_modules_context();
    let is_showcase_surface = admin_surface == "next-admin";

    let push_build_job = move |build: BuildJob| {
        set_active_build_state.set(Some(build.clone()));
        set_build_history_state.update(|history| {
            history.retain(|existing| existing.id != build.id);
            history.insert(0, build.clone());
            if history.len() > 10 {
                history.truncate(10);
            }
        });
    };

    let refresh_marketplace_catalog = move |token_value: Option<String>,
                                            tenant_value: Option<String>,
                                            filters: CatalogFilters,
                                            silent: bool| {
        if !silent {
            set_catalog_refreshing.set(true);
        }
        spawn_local(async move {
            let filters = normalize_catalog_filters(&filters);
            let result = api::fetch_marketplace_modules(token_value, tenant_value, filters).await;
            if let Ok(marketplace) = result {
                set_known_categories.update(|categories| {
                    for category in marketplace
                        .iter()
                        .map(|module| module.category.clone())
                        .filter(|category| !category.is_empty())
                    {
                        if !categories.contains(&category) {
                            categories.push(category);
                        }
                    }
                    categories.sort();
                    categories.dedup();
                });
                set_known_tags.update(|tags| {
                    for tag in marketplace
                        .iter()
                        .flat_map(|module| module.tags.iter().cloned())
                        .filter(|tag| !tag.is_empty())
                    {
                        if !tags.contains(&tag) {
                            tags.push(tag);
                        }
                    }
                    tags.sort();
                    tags.dedup();
                });
                set_known_sources.update(|sources| {
                    for source in marketplace
                        .iter()
                        .map(|module| module.source.clone())
                        .filter(|source| !source.is_empty())
                    {
                        if !sources.contains(&source) {
                            sources.push(source);
                        }
                    }
                    sources.sort();
                    sources.dedup();
                });
                set_marketplace_catalog.set(marketplace);
            }
            if !silent {
                set_catalog_refreshing.set(false);
            }
        });
    };
    let refresh_orchestration_state =
        move |token_value: Option<String>,
              tenant_value: Option<String>,
              filters: CatalogFilters| {
            spawn_local(async move {
                if let Ok(build) =
                    api::fetch_active_build(token_value.clone(), tenant_value.clone()).await
                {
                    set_active_build_state.set(build);
                }
                if let Ok(release) =
                    api::fetch_active_release(token_value.clone(), tenant_value.clone()).await
                {
                    set_active_release_state.set(release);
                }
                if let Ok(history) =
                    api::fetch_build_history(token_value.clone(), tenant_value.clone(), 10, 0).await
                {
                    set_build_history_state.set(history);
                }
                let filters = normalize_catalog_filters(&filters);
                if let Ok(marketplace) =
                    api::fetch_marketplace_modules(token_value, tenant_value, filters).await
                {
                    set_known_categories.update(|categories| {
                        for category in marketplace
                            .iter()
                            .map(|module| module.category.clone())
                            .filter(|category| !category.is_empty())
                        {
                            if !categories.contains(&category) {
                                categories.push(category);
                            }
                        }
                        categories.sort();
                        categories.dedup();
                    });
                    set_known_tags.update(|tags| {
                        for tag in marketplace
                            .iter()
                            .flat_map(|module| module.tags.iter().cloned())
                            .filter(|tag| !tag.is_empty())
                        {
                            if !tags.contains(&tag) {
                                tags.push(tag);
                            }
                        }
                        tags.sort();
                        tags.dedup();
                    });
                    set_known_sources.update(|sources| {
                        for source in marketplace
                            .iter()
                            .map(|module| module.source.clone())
                            .filter(|source| !source.is_empty())
                        {
                            if !sources.contains(&source) {
                                sources.push(source);
                            }
                        }
                        sources.sort();
                        sources.dedup();
                    });
                    set_marketplace_catalog.set(marketplace);
                }
            });
        };
    let refresh_live_state = move || {
        let token_value = token.get();
        let tenant_value = tenant.get();
        refresh_orchestration_state(token_value, tenant_value, applied_catalog_filters.get());
    };
    let live_polling = use_interval_fn(refresh_live_state, 5000);
    (live_polling.pause)();
    let pause_live_polling = live_polling.pause.clone();
    let resume_live_polling = live_polling.resume.clone();
    #[cfg(target_arch = "wasm32")]
    let apply_build_progress_event =
        move |event: api::BuildProgressEvent,
              token_value: Option<String>,
              tenant_value: Option<String>| {
            let mut matched_active_build = false;
            set_active_build_state.update(|active_build| {
                if let Some(build) = active_build.as_mut() {
                    if build.id == event.build_id {
                        apply_build_progress_to_job(build, &event);
                        matched_active_build = true;
                    }
                }
            });

            set_build_history_state.update(|history| {
                if let Some(build) = history.iter_mut().find(|build| build.id == event.build_id) {
                    apply_build_progress_to_job(build, &event);
                }
            });

            if !matched_active_build || is_build_terminal_status(&event.status) {
                refresh_orchestration_state(
                    token_value,
                    tenant_value,
                    applied_catalog_filters.get_untracked(),
                );
            }
        };

    Effect::new(move |_| {
        let module_from_query = query.get().get("module");
        if module_from_query != selected_module_slug.get() {
            set_selected_module_slug.set(module_from_query.clone());
            if module_from_query.is_none() {
                set_selected_module_detail.set(None);
                set_module_detail_loading.set(false);
            }
        }
    });

    Effect::new(move |_| {
        if active_build_state
            .get()
            .as_ref()
            .is_some_and(is_build_active)
        {
            if live_subscription_connected.get() {
                pause_live_polling();
            } else {
                refresh_live_state();
                resume_live_polling();
            }
        } else {
            pause_live_polling();
        }
    });

    Effect::new(move |_| {
        let token_value = token.get();
        let tenant_value = tenant.get();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = (token_value, tenant_value);
            set_live_subscription_connected.set(false);
        }

        #[cfg(target_arch = "wasm32")]
        {
            let Some(token_value) = token_value else {
                set_live_subscription_connected.set(false);
                return;
            };
            let Some(tenant_value) = tenant_value else {
                set_live_subscription_connected.set(false);
                return;
            };

            set_live_subscription_connected.set(false);

            let ws = match WebSocket::new_with_str(
                &shared_api::get_graphql_ws_url(),
                "graphql-transport-ws",
            ) {
                Ok(ws) => ws,
                Err(_) => return,
            };

            let init_message = serde_json::json!({
                "type": "connection_init",
                "payload": {
                    "token": token_value.clone(),
                    "tenantSlug": tenant_value.clone(),
                    "locale": shared_api::get_stored_locale(),
                }
            })
            .to_string();
            let subscribe_message = serde_json::json!({
                "id": "modules-build-progress",
                "type": "subscribe",
                "payload": {
                    "query": api::BUILD_PROGRESS_SUBSCRIPTION,
                }
            })
            .to_string();

            let subscribed = std::rc::Rc::new(std::cell::Cell::new(false));

            let ws_for_open = ws.clone();
            let on_open = Closure::<dyn FnMut(Event)>::new(move |_| {
                let _ = ws_for_open.send_with_str(&init_message);
            });

            let ws_for_message = ws.clone();
            let subscribed_for_message = subscribed.clone();
            let token_for_message = token_value.clone();
            let tenant_for_message = tenant_value.clone();
            let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
                let Some(text) = event.data().as_string() else {
                    return;
                };

                let Ok(message) = serde_json::from_str::<BuildProgressSubscriptionMessage>(&text)
                else {
                    return;
                };

                match message {
                    BuildProgressSubscriptionMessage::ConnectionAck => {
                        set_live_subscription_connected.set(true);
                        if !subscribed_for_message.get() {
                            subscribed_for_message.set(true);
                            let _ = ws_for_message.send_with_str(&subscribe_message);
                        }
                    }
                    BuildProgressSubscriptionMessage::Next { payload } => {
                        if let Some(errors) = payload.errors {
                            if !errors.is_empty() {
                                set_live_subscription_connected.set(false);
                                refresh_orchestration_state(
                                    Some(token_for_message.clone()),
                                    Some(tenant_for_message.clone()),
                                    applied_catalog_filters.get_untracked(),
                                );
                                return;
                            }
                        }

                        if let Some(build_progress) =
                            payload.data.and_then(|data| data.build_progress)
                        {
                            apply_build_progress_event(
                                build_progress,
                                Some(token_for_message.clone()),
                                Some(tenant_for_message.clone()),
                            );
                        }
                    }
                    BuildProgressSubscriptionMessage::Error { payload } => {
                        if payload.iter().any(|error| !error.message.trim().is_empty()) {
                            set_live_subscription_connected.set(false);
                        }
                    }
                    BuildProgressSubscriptionMessage::Ping { payload } => {
                        let pong = serde_json::json!({
                            "type": "pong",
                            "payload": payload,
                        })
                        .to_string();
                        let _ = ws_for_message.send_with_str(&pong);
                    }
                    BuildProgressSubscriptionMessage::Complete
                    | BuildProgressSubscriptionMessage::Pong => {}
                }
            });

            let on_error = Closure::<dyn FnMut(ErrorEvent)>::new(move |_| {
                set_live_subscription_connected.set(false);
            });

            let on_close = Closure::<dyn FnMut(CloseEvent)>::new(move |_| {
                set_live_subscription_connected.set(false);
            });

            ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
            ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
            ws.set_onerror(Some(on_error.as_ref().unchecked_ref()));
            ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));

            on_open.forget();
            on_message.forget();
            on_error.forget();
            on_close.forget();
        }
    });

    let upsert_installed_module = move |slug: String, version: String| {
        let registry_module = module_list
            .get()
            .iter()
            .find(|module| module.module_slug == slug)
            .cloned();
        let catalog_module = marketplace_catalog
            .get()
            .into_iter()
            .find(|module| module.slug == slug);
        set_installed_module_list.update(|installed| {
            if let Some(existing) = installed.iter_mut().find(|module| module.slug == slug) {
                existing.version = Some(version.clone());
                return;
            }
            installed.push(InstalledModule {
                slug: slug.clone(),
                source: catalog_module
                    .as_ref()
                    .map(|module| module.source.clone())
                    .unwrap_or_else(|| "path".to_string()),
                crate_name: catalog_module
                    .as_ref()
                    .map(|module| module.crate_name.clone())
                    .or_else(|| {
                        registry_module
                            .as_ref()
                            .map(|module| module.module_slug.clone())
                    })
                    .unwrap_or_else(|| slug.clone()),
                version: Some(version),
                required: false,
                dependencies: catalog_module
                    .as_ref()
                    .map(|module| module.dependencies.clone())
                    .or_else(|| {
                        registry_module
                            .as_ref()
                            .map(|module| module.dependencies.clone())
                    })
                    .unwrap_or_default(),
            });
            installed.sort_by(|left, right| left.slug.cmp(&right.slug));
        });
    };

    let enabled_modules_for_toggle = enabled_modules.clone();
    let on_toggle = Callback::new(move |(slug, enabled): (String, bool)| {
        let slug_clone = slug.clone();
        set_loading_slug.set(Some(slug.clone()));
        set_form_state.set(FormState::idle());
        set_success_message.set(None);
        let token_val = token.get();
        let tenant_val = tenant.get();
        let enabled_modules_for_toggle_async = enabled_modules_for_toggle.clone();
        spawn_local(async move {
            set_form_state.set(FormState::submitting());
            match api::toggle_module(slug_clone.clone(), enabled, token_val, tenant_val).await {
                Ok(result) => {
                    set_module_list.update(|modules| {
                        if let Some(module) = modules
                            .iter_mut()
                            .find(|module| module.module_slug == slug_clone)
                        {
                            module.enabled = result.enabled;
                        }
                    });
                    set_tenant_module_list.update(|modules: &mut Vec<TenantModule>| {
                        let module = TenantModule {
                            module_slug: result.module_slug.clone(),
                            enabled: result.enabled,
                            settings: result.settings.clone(),
                        };
                        if let Some(existing) = modules
                            .iter_mut()
                            .find(|existing| existing.module_slug == module.module_slug)
                        {
                            *existing = module;
                        } else {
                            modules.push(module);
                            modules.sort_by(|left, right| left.module_slug.cmp(&right.module_slug));
                        }
                    });
                    enabled_modules_for_toggle_async
                        .set_module_enabled(&slug_clone, result.enabled);
                    let status = if result.enabled {
                        t_string!(i18n, modules.toast.enabled)
                    } else {
                        t_string!(i18n, modules.toast.disabled)
                    };
                    set_success_message.set(Some(status.to_string()));
                }
                Err(err) => set_form_state.set(FormState::with_form_error(format!("{}", err))),
            }
            set_loading_slug.set(None);
        });
    });

    let on_install = Callback::new(move |(slug, version): (String, String)| {
        let slug_clone = slug.clone();
        let version_clone = version.clone();
        set_platform_loading_slug.set(Some(slug.clone()));
        set_form_state.set(FormState::idle());
        set_success_message.set(None);
        let token_val = token.get();
        let tenant_val = tenant.get();
        let refresh_token = token_val.clone();
        let refresh_tenant = tenant_val.clone();
        spawn_local(async move {
            set_form_state.set(FormState::submitting());
            match api::install_module(slug_clone.clone(), version, token_val, tenant_val).await {
                Ok(build) => {
                    upsert_installed_module(slug_clone.clone(), version_clone);
                    push_build_job(build);
                    set_selected_tab.set(ModulesTab::Installed);
                    set_success_message.set(Some(format!("Install queued for {}", slug_clone)));
                    refresh_orchestration_state(
                        refresh_token,
                        refresh_tenant,
                        applied_catalog_filters.get(),
                    );
                }
                Err(err) => set_form_state.set(FormState::with_form_error(format!("{}", err))),
            }
            set_platform_loading_slug.set(None);
        });
    });
    let enabled_modules_for_uninstall = enabled_modules.clone();
    let on_uninstall = Callback::new(move |slug: String| {
        let slug_clone = slug.clone();
        set_platform_loading_slug.set(Some(slug.clone()));
        set_form_state.set(FormState::idle());
        set_success_message.set(None);
        let token_val = token.get();
        let tenant_val = tenant.get();
        let refresh_token = token_val.clone();
        let refresh_tenant = tenant_val.clone();
        let enabled_modules_for_uninstall_async = enabled_modules_for_uninstall.clone();
        spawn_local(async move {
            set_form_state.set(FormState::submitting());
            match api::uninstall_module(slug_clone.clone(), token_val, tenant_val).await {
                Ok(build) => {
                    set_installed_module_list
                        .update(|modules| modules.retain(|module| module.slug != slug_clone));
                    set_tenant_module_list.update(|modules: &mut Vec<TenantModule>| {
                        if let Some(module) = modules
                            .iter_mut()
                            .find(|module| module.module_slug == slug_clone)
                        {
                            module.enabled = false;
                        }
                    });
                    set_module_list.update(|modules| {
                        if let Some(module) = modules
                            .iter_mut()
                            .find(|module| module.module_slug == slug_clone)
                        {
                            module.enabled = false;
                        }
                    });
                    enabled_modules_for_uninstall_async.set_module_enabled(&slug_clone, false);
                    push_build_job(build);
                    set_success_message.set(Some(format!("Uninstall queued for {}", slug_clone)));
                    refresh_orchestration_state(
                        refresh_token,
                        refresh_tenant,
                        applied_catalog_filters.get(),
                    );
                }
                Err(err) => set_form_state.set(FormState::with_form_error(format!("{}", err))),
            }
            set_platform_loading_slug.set(None);
        });
    });

    let on_upgrade = Callback::new(move |(slug, version): (String, String)| {
        let slug_clone = slug.clone();
        let version_clone = version.clone();
        set_platform_loading_slug.set(Some(slug.clone()));
        set_form_state.set(FormState::idle());
        set_success_message.set(None);
        let token_val = token.get();
        let tenant_val = tenant.get();
        let refresh_token = token_val.clone();
        let refresh_tenant = tenant_val.clone();
        spawn_local(async move {
            set_form_state.set(FormState::submitting());
            match api::upgrade_module(slug_clone.clone(), version, token_val, tenant_val).await {
                Ok(build) => {
                    upsert_installed_module(slug_clone.clone(), version_clone);
                    push_build_job(build);
                    set_success_message.set(Some(format!("Upgrade queued for {}", slug_clone)));
                    refresh_orchestration_state(
                        refresh_token,
                        refresh_tenant,
                        applied_catalog_filters.get(),
                    );
                }
                Err(err) => set_form_state.set(FormState::with_form_error(format!("{}", err))),
            }
            set_platform_loading_slug.set(None);
        });
    });

    let on_rollback = Callback::new(move |build_id: String| {
        let build_id_clone = build_id.clone();
        set_rollback_loading_build_id.set(Some(build_id.clone()));
        set_form_state.set(FormState::idle());
        set_success_message.set(None);
        let token_val = token.get();
        let tenant_val = tenant.get();
        let refresh_token = token_val.clone();
        let refresh_tenant = tenant_val.clone();
        spawn_local(async move {
            set_form_state.set(FormState::submitting());
            match api::rollback_build(build_id_clone.clone(), token_val, tenant_val).await {
                Ok(build) => {
                    set_active_build_state.set(None);
                    push_build_job(build);
                    set_success_message
                        .set(Some(format!("Rollback completed for {}", build_id_clone)));
                    refresh_orchestration_state(
                        refresh_token,
                        refresh_tenant,
                        applied_catalog_filters.get(),
                    );
                }
                Err(err) => set_form_state.set(FormState::with_form_error(format!("{}", err))),
            }
            set_rollback_loading_build_id.set(None);
        });
    });

    let selected_tenant_module = Signal::derive(move || {
        selected_module_slug.get().and_then(|slug| {
            tenant_module_list
                .get()
                .into_iter()
                .find(|module| module.module_slug == slug)
        })
    });
    let settings_editable = Signal::derive(move || {
        let Some(slug) = selected_module_slug.get() else {
            return false;
        };

        module_list
            .get()
            .into_iter()
            .find(|module| module.module_slug == slug)
            .is_some_and(|module| module.enabled || module.is_core())
    });
    let selected_settings_schema = Signal::derive(move || {
        selected_module_detail
            .get()
            .map(|module| module.settings_schema)
            .unwrap_or_default()
    });
    let settings_form_supported =
        Signal::derive(move || schema_supports_generated_form(&selected_settings_schema.get()));
    let settings_saving = Signal::derive(move || settings_loading_slug.get().is_some());
    Effect::new(move |_| {
        let tenant_module = selected_tenant_module.get();
        let schema = selected_settings_schema.get();
        set_module_settings_form_draft
            .set(build_settings_form_draft(&schema, tenant_module.as_ref()));
        let draft = tenant_module
            .as_ref()
            .map(|module| pretty_json(&module.settings))
            .or_else(|| {
                if schema.is_empty() {
                    None
                } else {
                    schema_settings_payload(
                        &schema,
                        &build_settings_form_draft(&schema, tenant_module.as_ref()),
                    )
                    .ok()
                    .map(|payload| pretty_json(&payload))
                }
            })
            .unwrap_or_else(|| "{}".to_string());
        set_module_settings_draft.set(draft);
    });
    let on_settings_input = Callback::new(move |value: String| {
        set_module_settings_draft.set(value);
    });
    let on_settings_field_input = Callback::new(move |(key, value): (String, String)| {
        set_module_settings_form_draft.update(|draft: &mut HashMap<String, String>| {
            draft.insert(key, value);
        });
    });
    let on_save_settings = Callback::new(move |_| {
        let Some(slug) = selected_module_slug.get() else {
            return;
        };

        set_form_state.set(FormState::idle());
        set_success_message.set(None);
        let token_val = token.get();
        let tenant_val = tenant.get();
        let schema = selected_settings_schema.get_untracked();
        let settings_payload = if schema_supports_generated_form(&schema) {
            match schema_settings_payload(&schema, &module_settings_form_draft.get_untracked()) {
                Ok(payload) => payload,
                Err(err) => {
                    set_form_state.set(FormState::with_form_error(err));
                    return;
                }
            }
        } else {
            module_settings_draft.get_untracked()
        };
        set_settings_loading_slug.set(Some(slug.clone()));
        spawn_local(async move {
            set_form_state.set(FormState::submitting());
            match api::update_module_settings(slug.clone(), settings_payload, token_val, tenant_val)
                .await
            {
                Ok(module) => {
                    set_tenant_module_list.update(|modules: &mut Vec<TenantModule>| {
                        if let Some(existing) = modules
                            .iter_mut()
                            .find(|existing| existing.module_slug == module.module_slug)
                        {
                            *existing = module.clone();
                        } else {
                            modules.push(module.clone());
                            modules.sort_by(|left, right| left.module_slug.cmp(&right.module_slug));
                        }
                    });
                    set_module_settings_draft.set(pretty_json(&module.settings));
                    set_success_message.set(Some(format!("Saved tenant settings for {}", slug)));
                }
                Err(err) => set_form_state.set(FormState::with_form_error(format!("{}", err))),
            }
            set_settings_loading_slug.set(None);
        });
    });
    let navigate_for_inspect = navigate.clone();
    let on_inspect = Callback::new(move |slug: String| {
        set_selected_module_detail.set(
            marketplace_catalog
                .get()
                .into_iter()
                .find(|module| module.slug == slug),
        );
        navigate_for_inspect(&format!("/modules?module={}", slug), Default::default());
    });
    let navigate_for_close = navigate.clone();
    let on_close_detail = Callback::new(move |_| {
        navigate_for_close("/modules", Default::default());
    });
    let on_refresh_detail = Callback::new(move |_| {
        let Some(slug) = selected_module_slug.get() else {
            return;
        };
        let token_value = token.get();
        let tenant_value = tenant.get();
        set_module_detail_loading.set(true);
        spawn_local(async move {
            match api::fetch_marketplace_module(slug, token_value, tenant_value).await {
                Ok(module) => set_selected_module_detail.set(module),
                Err(err) => set_form_state.set(FormState::with_form_error(format!("{}", err))),
            }
            set_module_detail_loading.set(false);
        });
    });
    let on_apply_filters = Callback::new(move |_| {
        set_form_state.set(FormState::idle());
        let token_value = token.get();
        let tenant_value = tenant.get();
        let filters = catalog_filter_draft.get();
        set_applied_catalog_filters.set(filters.clone());
        refresh_marketplace_catalog(token_value, tenant_value, filters, false);
    });
    let on_reset_filters = Callback::new(move |_| {
        let defaults = CatalogFilters::default();
        set_catalog_filter_draft.set(defaults.clone());
        set_applied_catalog_filters.set(defaults.clone());
        set_form_state.set(FormState::idle());
        let token_value = token.get();
        let tenant_value = tenant.get();
        refresh_marketplace_catalog(token_value, tenant_value, defaults, false);
    });

    let core_modules = move || {
        module_list
            .get()
            .into_iter()
            .filter(|module| module.is_core())
            .collect::<Vec<_>>()
    };
    let installed_optional_modules = move || {
        let installed: HashSet<String> = installed_module_list
            .get()
            .into_iter()
            .map(|module| module.slug)
            .collect();
        module_list
            .get()
            .into_iter()
            .filter(|module| !module.is_core() && installed.contains(&module.module_slug))
            .collect::<Vec<_>>()
    };
    let marketplace_modules = move || {
        let installed: HashSet<String> = installed_module_list
            .get()
            .into_iter()
            .map(|module| module.slug)
            .collect();
        marketplace_catalog
            .get()
            .into_iter()
            .filter(|module| !installed.contains(&module.slug))
            .collect::<Vec<_>>()
    };
    let catalog_for_slug = move |slug: &str| {
        marketplace_catalog
            .get()
            .into_iter()
            .find(|module| module.slug == slug)
    };
    Effect::new(move |_| {
        if let Some(slug) = selected_module_slug.get() {
            if let Some(module) = marketplace_catalog
                .get()
                .into_iter()
                .find(|module| module.slug == slug)
            {
                set_selected_module_detail.set(Some(module));
            }
        }
    });
    Effect::new(move |_| {
        if let Some(slug) = selected_module_slug.get() {
            let token_value = token.get();
            let tenant_value = tenant.get();
            set_module_detail_loading.set(true);
            spawn_local(async move {
                match api::fetch_marketplace_module(slug, token_value, tenant_value).await {
                    Ok(module) => set_selected_module_detail.set(module),
                    Err(err) => set_form_state.set(FormState::with_form_error(format!("{}", err))),
                }
                set_module_detail_loading.set(false);
            });
        }
    });
    let update_candidates = move || {
        let installed = installed_module_list.get();
        marketplace_catalog
            .get()
            .into_iter()
            .filter_map(|module| {
                let installed_module = installed
                    .iter()
                    .find(|item| item.slug == module.slug)
                    .cloned()?;
                match installed_module.version.clone() {
                    Some(current_version) if current_version != module.latest_version => {
                        Some((module, installed_module))
                    }
                    _ => None,
                }
            })
            .collect::<Vec<_>>()
    };
    let visible_installed_count = move || core_modules().len() + installed_optional_modules().len();
    let latest_build = move || {
        active_build_state
            .get()
            .or_else(|| build_history_state.get().into_iter().next())
    };

    let admin_surface_for_detail = StoredValue::new(admin_surface.clone());

    view! {
        <div class="space-y-8">
            <div class="rounded-xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-2">
                    <h3 class="text-sm font-medium text-card-foreground">"Admin surface policy"</h3>
                    <p class="text-sm text-muted-foreground">
                        {if is_showcase_surface {
                            "Next admin is a showcase surface. Dedicated module UI appears only where a module is explicitly marked with Next showcase support."
                        } else {
                            "Leptos admin is the canonical operator surface for module UI and ongoing module parity."
                        }}
                    </p>
                </div>
                <div class="mt-4 flex flex-wrap items-center gap-2 text-xs">
                    <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 font-semibold text-secondary-foreground">
                        {if is_showcase_surface { "Current: Next showcase" } else { "Current: Leptos canonical" }}
                    </span>
                    <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                        "Primary modules target Leptos first"
                    </span>
                </div>
            </div>

            <Show when=move || form_state.get().form_error.is_some()>
                <div class="rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive">{move || form_state.get().form_error.unwrap_or_default()}</div>
            </Show>
            <Show when=move || success_message.get().is_some()>
                <UiSuccessMessage message=success_message.get().unwrap_or_default() />
            </Show>
            <div class="grid gap-4 xl:grid-cols-[minmax(0,1.4fr)_repeat(3,minmax(0,0.8fr))]">
                <div class="rounded-xl border border-border bg-card p-6 shadow-sm xl:col-span-2">
                    <div class="space-y-2">
                        <h3 class="text-base font-semibold text-card-foreground">"Build orchestration"</h3>
                        <p class="text-sm text-muted-foreground">"Install, uninstall, and upgrade actions queue a shared rebuild for both admin stacks."</p>
                    </div>
                    <div class="mt-4">
                        <Show when=move || latest_build().is_some() fallback=move || view! { <p class="text-sm text-muted-foreground">"No platform builds yet. The first install, uninstall, or upgrade will queue one."</p> }>
                            {move || latest_build().map(|build| {
                                let progress_width = format!("width: {}%;", build.progress);
                                view! {
                                    <div class="space-y-4">
                                        <div class="flex items-center justify-between gap-3">
                                            <div class="space-y-1">
                                                <div class="flex items-center gap-2">
                                                    <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">{humanize_label(&build.status)}</span>
                                                    <span class="text-xs text-muted-foreground">{humanize_label(&build.stage)}</span>
                                                </div>
                                                <p class="text-sm font-medium text-card-foreground">{if build.modules_delta.is_empty() { build.reason.clone().unwrap_or_else(|| "Platform module rebuild".to_string()) } else { build.modules_delta.clone() }}</p>
                                                <p class="text-xs text-muted-foreground">{format!("Updated {}", build.updated_at)}</p>
                                                <Show when=move || active_release_state.get().is_some()>
                                                    <p class="text-xs text-muted-foreground">
                                                        {move || {
                                                            active_release_state.get().map(|release| {
                                                                format!(
                                                                    "Active release {} in {}",
                                                                    release.id, release.environment
                                                                )
                                                            }).unwrap_or_default()
                                                        }}
                                                    </p>
                                                </Show>
                                            </div>
                                            <span class="text-sm font-semibold text-card-foreground">{format!("{}%", build.progress)}</span>
                                        </div>
                                        <div class="h-2 overflow-hidden rounded-full bg-primary/15"><div class="h-full rounded-full bg-primary transition-all" style=progress_width></div></div>
                                        <p class="text-xs text-muted-foreground">{move || if active_build_state.get().is_some() { "Platform actions stay locked until the current build finishes.".to_string() } else { "No active build. The latest completed job is shown for context.".to_string() }}</p>
                                        <Show when=move || active_build_state.get().as_ref().is_some_and(is_build_active)>
                                            <p class="text-xs text-muted-foreground">
                                                "Live refresh is active every 5 seconds while this build is running."
                                            </p>
                                        </Show>
                                    </div>
                                }
                            })}
                        </Show>
                    </div>
                </div>
                <div class="rounded-xl border border-border bg-card p-6 shadow-sm"><h3 class="text-sm font-medium text-card-foreground">"Installed"</h3><p class="mt-3 text-3xl font-semibold text-card-foreground">{visible_installed_count}</p><p class="mt-2 text-xs text-muted-foreground">"Core and optional modules visible to this admin workspace."</p></div>
                <div class="rounded-xl border border-border bg-card p-6 shadow-sm"><h3 class="text-sm font-medium text-card-foreground">"Marketplace"</h3><p class="mt-3 text-3xl font-semibold text-card-foreground">{move || marketplace_modules().len()}</p><p class="mt-2 text-xs text-muted-foreground">"Optional modules available to add into modules.toml."</p></div>
                <div class="rounded-xl border border-border bg-card p-6 shadow-sm"><h3 class="text-sm font-medium text-card-foreground">"Updates"</h3><p class="mt-3 text-3xl font-semibold text-card-foreground">{move || update_candidates().len()}</p><p class="mt-2 text-xs text-muted-foreground">"Version-pinned modules that can be upgraded from this screen."</p></div>
            </div>
            <div class="rounded-xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-2">
                    <h3 class="text-base font-semibold text-card-foreground">"Catalog filters"</h3>
                    <p class="text-sm text-muted-foreground">
                        "Narrow Marketplace and Updates to the modules you actually want to review."
                    </p>
                </div>
                <div class="mt-4 grid gap-3 lg:grid-cols-[minmax(0,2fr)_repeat(4,minmax(0,1fr))_auto_auto]">
                    <input
                        class="h-9 rounded-md border border-input bg-background px-3 text-sm text-foreground shadow-xs outline-none transition focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/40"
                        prop:value=move || catalog_filter_draft.get().search
                        placeholder="Search by name, slug, or description"
                        on:input=move |event| {
                            let value = event_target_value(&event);
                            set_catalog_filter_draft.update(|filters| filters.search = value);
                        }
                    />
                    <select
                        class="h-9 rounded-md border border-input bg-background px-3 text-sm text-foreground shadow-xs outline-none transition focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/40"
                        prop:value=move || catalog_filter_draft.get().category
                        on:change=move |event| {
                            let value = event_target_value(&event);
                            set_catalog_filter_draft.update(|filters| filters.category = value);
                        }
                    >
                        <option value="all">"All categories"</option>
                        {move || known_categories.get().into_iter().map(|category| {
                            view! {
                                <option value=category.clone()>{humanize_label(&category)}</option>
                            }
                        }).collect_view()}
                    </select>
                    <select
                        class="h-9 rounded-md border border-input bg-background px-3 text-sm text-foreground shadow-xs outline-none transition focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/40"
                        prop:value=move || catalog_filter_draft.get().tag
                        on:change=move |event| {
                            let value = event_target_value(&event);
                            set_catalog_filter_draft.update(|filters| filters.tag = value);
                        }
                    >
                        <option value="all">"All tags"</option>
                        {move || known_tags.get().into_iter().map(|tag| {
                            view! {
                                <option value=tag.clone()>{humanize_label(&tag)}</option>
                            }
                        }).collect_view()}
                    </select>
                    <select
                        class="h-9 rounded-md border border-input bg-background px-3 text-sm text-foreground shadow-xs outline-none transition focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/40"
                        prop:value=move || catalog_filter_draft.get().source
                        on:change=move |event| {
                            let value = event_target_value(&event);
                            set_catalog_filter_draft.update(|filters| filters.source = value);
                        }
                    >
                        <option value="all">"All sources"</option>
                        {move || known_sources.get().into_iter().map(|source| {
                            view! {
                                <option value=source.clone()>{humanize_label(&source)}</option>
                            }
                        }).collect_view()}
                    </select>
                    <select
                        class="h-9 rounded-md border border-input bg-background px-3 text-sm text-foreground shadow-xs outline-none transition focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/40"
                        prop:value=move || catalog_filter_draft.get().trust_level
                        on:change=move |event| {
                            let value = event_target_value(&event);
                            set_catalog_filter_draft.update(|filters| filters.trust_level = value);
                        }
                    >
                        <option value="all">"All trust levels"</option>
                        <option value="core">"Core"</option>
                        <option value="verified">"Verified"</option>
                        <option value="unverified">"Unverified"</option>
                        <option value="private">"Private"</option>
                    </select>
                    <button
                        type="button"
                        class=move || {
                            if catalog_filter_draft.get().only_compatible {
                                "inline-flex items-center justify-center rounded-md bg-primary px-3 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
                            } else {
                                "inline-flex items-center justify-center rounded-md border border-border bg-background px-3 py-2 text-sm font-medium text-foreground transition-colors hover:bg-accent"
                            }
                        }
                        on:click=move |_| {
                            set_catalog_filter_draft.update(|filters| {
                                filters.only_compatible = !filters.only_compatible;
                            });
                        }
                    >
                        {move || if catalog_filter_draft.get().only_compatible {
                            "Compatible only"
                        } else {
                            "Include risks"
                        }}
                    </button>
                    <button
                        type="button"
                        class=move || {
                            if catalog_filter_draft.get().installed_only {
                                "inline-flex items-center justify-center rounded-md bg-primary px-3 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
                            } else {
                                "inline-flex items-center justify-center rounded-md border border-border bg-background px-3 py-2 text-sm font-medium text-foreground transition-colors hover:bg-accent"
                            }
                        }
                        on:click=move |_| {
                            set_catalog_filter_draft.update(|filters| {
                                filters.installed_only = !filters.installed_only;
                            });
                        }
                    >
                        {move || if catalog_filter_draft.get().installed_only {
                            "Installed only"
                        } else {
                            "All install states"
                        }}
                    </button>
                </div>
                <div class="mt-4 flex flex-wrap items-center justify-between gap-3">
                    <div class="flex flex-wrap items-center gap-2 text-xs">
                        <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 font-semibold text-secondary-foreground">
                            {move || if catalog_refreshing.get() { "Refreshing catalog" } else { "Catalog ready" }}
                        </span>
                        <Show when=move || catalog_filter_draft.get() != applied_catalog_filters.get()>
                            <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                "Pending changes"
                            </span>
                        </Show>
                        <Show when=move || !applied_catalog_filters.get().search.is_empty()>
                            <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                {move || format!("Search: {}", applied_catalog_filters.get().search)}
                            </span>
                        </Show>
                        <Show when=move || applied_catalog_filters.get().category != "all">
                            <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                {move || format!("Category: {}", humanize_label(&applied_catalog_filters.get().category))}
                            </span>
                        </Show>
                        <Show when=move || applied_catalog_filters.get().tag != "all">
                            <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                {move || format!("Tag: {}", humanize_label(&applied_catalog_filters.get().tag))}
                            </span>
                        </Show>
                        <Show when=move || applied_catalog_filters.get().source != "all">
                            <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                {move || format!("Source: {}", humanize_label(&applied_catalog_filters.get().source))}
                            </span>
                        </Show>
                        <Show when=move || applied_catalog_filters.get().trust_level != "all">
                            <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                {move || format!("Trust: {}", humanize_label(&applied_catalog_filters.get().trust_level))}
                            </span>
                        </Show>
                        <Show when=move || applied_catalog_filters.get().only_compatible>
                            <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                "Compatible only"
                            </span>
                        </Show>
                        <Show when=move || applied_catalog_filters.get().installed_only>
                            <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                "Installed only"
                            </span>
                        </Show>
                    </div>
                    <div class="flex items-center gap-2">
                        <button
                            type="button"
                            class="inline-flex items-center justify-center rounded-md border border-border bg-background px-3 py-2 text-sm font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                            disabled=move || catalog_refreshing.get()
                            on:click=move |_| on_reset_filters.run(())
                        >
                            "Reset"
                        </button>
                        <button
                            type="button"
                            class="inline-flex items-center justify-center rounded-md bg-primary px-3 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50"
                            disabled=move || catalog_refreshing.get()
                            on:click=move |_| on_apply_filters.run(())
                        >
                            {move || if catalog_refreshing.get() { "Refreshing..." } else { "Apply filters" }}
                        </button>
                    </div>
                </div>
            </div>
            <Show when=move || selected_module_slug.get().is_some()>
                {move || {
                    let admin_surface_value = admin_surface_for_detail.get_value();
                    selected_module_slug.get().map(|slug| {
                        view! {
                            <ModuleDetailPanel
                                admin_surface=admin_surface_value.clone()
                                selected_slug=slug
                                module=selected_module_detail.get()
                                tenant_module=selected_tenant_module.get()
                                settings_schema=selected_settings_schema.get()
                                settings_form_supported=settings_form_supported
                                settings_form_draft=Signal::derive(move || module_settings_form_draft.get())
                                settings_draft=Signal::derive(move || module_settings_draft.get())
                                settings_editable=settings_editable
                                settings_saving=settings_saving
                                loading=Signal::derive(move || module_detail_loading.get())
                                access_token=Signal::derive(move || token.get())
                                tenant_slug=Signal::derive(move || tenant.get())
                                on_settings_field_input=on_settings_field_input
                                on_settings_input=on_settings_input
                                on_save_settings=on_save_settings
                                on_refresh_detail=on_refresh_detail
                                on_close=on_close_detail
                            />
                        }
                    })
                }}
            </Show>
            <div class="space-y-6">
                <div class="inline-flex w-fit items-center rounded-lg bg-muted p-1">
                    <button type="button" class=move || tab_button_class(selected_tab.get() == ModulesTab::Installed) on:click=move |_| set_selected_tab.set(ModulesTab::Installed)>"Installed"</button>
                    <button type="button" class=move || tab_button_class(selected_tab.get() == ModulesTab::Marketplace) on:click=move |_| set_selected_tab.set(ModulesTab::Marketplace)>"Marketplace"</button>
                    <button type="button" class=move || tab_button_class(selected_tab.get() == ModulesTab::Updates) on:click=move |_| set_selected_tab.set(ModulesTab::Updates)>"Updates"</button>
                </div>                <Show when=move || selected_tab.get() == ModulesTab::Installed>
                    <div class="space-y-6">
                        <div class="rounded-xl border border-border bg-card p-6 shadow-sm">
                            <div class="space-y-2"><h3 class="text-base font-semibold text-card-foreground">"Build history"</h3><p class="text-sm text-muted-foreground">"Recent rebuild jobs visible to both admin applications."</p></div>
                            <div class="mt-4 space-y-3">
                                <Show when=move || !build_history_state.get().is_empty() fallback=move || view! { <p class="text-sm text-muted-foreground">"No builds yet."</p> }>
                                    {move || build_history_state.get().into_iter().map(|build| {
                                        let primary = if build.modules_delta.is_empty() { build.reason.clone().unwrap_or_else(|| build.id.clone()) } else { build.modules_delta.clone() };
                                        let release_id = build.release_id.clone();
                                        let release_id_for_badge = release_id.clone();
                                        let release_id_for_badge_text = release_id.clone();
                                        let release_id_for_active = release_id.clone();
                                        let release_id_for_rollback = release_id.clone();
                                        let logs_url = build.logs_url.clone();
                                        let logs_url_for_when = logs_url.clone();
                                        let error_message = build.error_message.clone();
                                        let error_message_for_when = error_message.clone();
                                        let build_id = StoredValue::new(build.id.clone());
                                        let build_id_for_loading = build_id;
                                        let build_id_for_label = build_id;
                                        view! {
                                            <div class="flex items-center justify-between gap-3 rounded-lg border border-border px-3 py-2">
                                                <div class="space-y-1">
                                                    <p class="text-sm font-medium text-card-foreground">{primary}</p>
                                                    <p class="text-xs text-muted-foreground">{format!("{} / {} / {}%", humanize_label(&build.status), humanize_label(&build.stage), build.progress)}</p>
                                                    <Show when=move || error_message_for_when.is_some()>
                                                        <p class="text-xs text-destructive">{error_message.clone().unwrap_or_default()}</p>
                                                    </Show>
                                                    <div class="flex flex-wrap items-center gap-2 text-xs">
                                                        <Show when=move || release_id_for_badge.is_some()>
                                                            <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 font-semibold text-secondary-foreground">
                                                                {format!("Release {}", release_id_for_badge_text.clone().unwrap_or_default())}
                                                            </span>
                                                        </Show>
                                                        <Show when=move || active_release_state.get().as_ref().and_then(|release| release_id_for_active.as_ref().map(|id| release.id == *id)).unwrap_or(false)>
                                                            <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                                                "Active release"
                                                            </span>
                                                        </Show>
                                                        <Show when=move || logs_url_for_when.is_some()>
                                                            <a class="text-primary underline-offset-4 hover:underline" href=logs_url.clone().unwrap_or_default() target="_blank" rel="noreferrer">
                                                                "Open logs"
                                                            </a>
                                                        </Show>
                                                    </div>
                                                </div>
                                                <div class="space-y-2 text-right">
                                                    <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">{humanize_label(&build.status)}</span>
                                                    <p class="text-xs text-muted-foreground">{build.created_at.clone()}</p>
                                                    <Show when=move || active_release_state.get().as_ref().is_some_and(|release| release.previous_release_id.is_some() && release_id_for_rollback.as_ref().is_some_and(|id| release.id == *id))>
                                                        <button
                                                            type="button"
                                                            class="text-primary text-xs font-medium underline-offset-4 hover:underline disabled:no-underline disabled:opacity-50"
                                                            disabled=move || rollback_loading_build_id.get().as_deref() == Some(build_id_for_loading.get_value().as_str()) || active_build_state.get().is_some()
                                                            on:click=move |_| on_rollback.run(build_id.get_value())
                                                        >
                                                            {move || {
                                                                if rollback_loading_build_id.get().as_deref() == Some(build_id_for_label.get_value().as_str()) {
                                                                    "Rolling back...".to_string()
                                                                } else {
                                                                    "Rollback".to_string()
                                                                }
                                                            }}
                                                        </button>
                                                    </Show>
                                                </div>
                                            </div>
                                        }
                                    }).collect_view()}
                                </Show>
                            </div>
                        </div>
                        <div class="space-y-3">
                            <div class="flex items-center gap-2"><svg class="h-5 w-5 text-muted-foreground" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" /></svg><h3 class="text-lg font-semibold text-foreground">{t!(i18n, modules.section.core)}</h3><span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground">{t!(i18n, modules.always_active)}</span></div>
                            <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
                                {move || core_modules().into_iter().map(|module| {
                                    let slug = module.module_slug.clone();
                                    let slug_for_loading = slug.clone();
                                    let slug_for_version = slug.clone();
                                    let slug_for_catalog = slug_for_version.clone();
                                    let recommended = module.version.clone();
                                    let tenant_loading = Signal::derive(move || loading_slug.get().as_deref() == Some(&slug_for_loading));
                                    let platform_loading = Signal::derive(move || platform_loading_slug.get().as_deref() == Some(&slug));
                                    let platform_version = Signal::derive(move || installed_module_list.get().iter().find(|item| item.slug == slug_for_version).and_then(|item| item.version.clone()));
                                    let recommended_version = Signal::derive(move || Some(recommended.clone()));
                                    let catalog_module = catalog_for_slug(&slug_for_catalog);
                                    view! { <ModuleCard module=module catalog_module=catalog_module tenant_loading=tenant_loading platform_loading=platform_loading platform_installed=Signal::derive(|| true) platform_busy=Signal::derive(move || active_build_state.get().is_some()) platform_version=platform_version recommended_version=recommended_version on_inspect=Some(on_inspect) /> }
                                }).collect_view()}
                            </div>
                        </div>
                        <div class="space-y-3">
                            <div class="flex items-center gap-2"><svg class="h-5 w-5 text-muted-foreground" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4" /></svg><h3 class="text-lg font-semibold text-foreground">{t!(i18n, modules.section.optional)}</h3><span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">{move || format!("{} installed", installed_optional_modules().len())}</span></div>
                            <Show when=move || !installed_optional_modules().is_empty() fallback=move || view! { <div class="rounded-xl border border-border bg-card p-6 shadow-sm"><p class="text-sm text-muted-foreground">"No optional modules are installed yet. Use the Marketplace tab to queue the first install."</p></div> }>
                                <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
                                    {move || installed_optional_modules().into_iter().map(|module| {
                                        let slug = module.module_slug.clone();
                                        let slug_for_loading = slug.clone();
                                        let slug_for_installed = slug.clone();
                                        let slug_for_version = slug.clone();
                                        let slug_for_catalog = slug_for_version.clone();
                                        let recommended = module.version.clone();
                                        let tenant_loading = Signal::derive(move || loading_slug.get().as_deref() == Some(&slug_for_loading));
                                        let platform_loading = Signal::derive(move || platform_loading_slug.get().as_deref() == Some(&slug));
                                        let platform_installed = Signal::derive(move || installed_module_list.get().iter().any(|item| item.slug == slug_for_installed));
                                        let platform_version = Signal::derive(move || installed_module_list.get().iter().find(|item| item.slug == slug_for_version).and_then(|item| item.version.clone()));
                                        let recommended_version = Signal::derive(move || Some(recommended.clone()));
                                        let catalog_module = catalog_for_slug(&slug_for_catalog);
                                        view! { <ModuleCard module=module catalog_module=catalog_module tenant_loading=tenant_loading platform_loading=platform_loading platform_installed=platform_installed platform_busy=Signal::derive(move || active_build_state.get().is_some()) platform_version=platform_version recommended_version=recommended_version on_toggle=Some(on_toggle) on_inspect=Some(on_inspect) on_uninstall=Some(on_uninstall) /> }
                                    }).collect_view()}
                                </div>
                            </Show>
                        </div>
                    </div>
                </Show>
                <Show when=move || selected_tab.get() == ModulesTab::Marketplace>
                    <div class="space-y-6">
                        <div class="rounded-xl border border-border bg-card p-6 shadow-sm"><div class="space-y-2"><h3 class="text-base font-semibold text-card-foreground">"Catalog workspace"</h3><p class="text-sm text-muted-foreground">"Modules here are known to the platform registry but not yet present in modules.toml."</p></div></div>
                        <Show when=move || !marketplace_modules().is_empty() fallback=move || view! { <div class="rounded-xl border border-border bg-card p-6 shadow-sm"><p class="text-sm text-muted-foreground">"All optional registry modules are already installed in the platform manifest."</p></div> }>
                            <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
                                {move || marketplace_modules().into_iter().map(|module| {
                                    let module_info = catalog_entry_to_module_info(&module);
                                    let slug = module.slug.clone();
                                    let slug_for_loading = slug.clone();
                                    let recommended = module.latest_version.clone();
                                    let tenant_loading = Signal::derive(move || loading_slug.get().as_deref() == Some(&slug_for_loading));
                                    let platform_loading = Signal::derive(move || platform_loading_slug.get().as_deref() == Some(&slug));
                                    let recommended_version = Signal::derive(move || Some(recommended.clone()));
                                    view! { <ModuleCard module=module_info catalog_module=Some(module.clone()) tenant_loading=tenant_loading platform_loading=platform_loading platform_installed=Signal::derive(|| false) platform_busy=Signal::derive(move || active_build_state.get().is_some()) platform_version=Signal::derive(|| None) recommended_version=recommended_version on_install=Some(on_install) on_inspect=Some(on_inspect) /> }
                                }).collect_view()}
                            </div>
                        </Show>
                    </div>
                </Show>
                <Show when=move || selected_tab.get() == ModulesTab::Updates>
                    <div class="space-y-6">
                        <div class="rounded-xl border border-border bg-card p-6 shadow-sm"><div class="space-y-2"><h3 class="text-base font-semibold text-card-foreground">"Versioned updates"</h3><p class="text-sm text-muted-foreground">"Only modules with an explicit installed version and a newer registry version appear here."</p></div></div>
                        <Show when=move || !update_candidates().is_empty() fallback=move || view! { <div class="rounded-xl border border-border bg-card p-6 shadow-sm"><p class="text-sm font-medium text-card-foreground">"No pinned module updates detected."</p><p class="mt-2 text-sm text-muted-foreground">"Path-based local modules follow the current repository state and therefore do not show a separate version upgrade action."</p></div> }>
                            <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
                                {move || update_candidates().into_iter().map(|(module, installed_module)| {
                                    let slug = module.slug.clone();
                                    let platform_loading = Signal::derive(move || platform_loading_slug.get().as_deref() == Some(&slug));
                                    view! { <ModuleUpdateCard module=module installed_module=installed_module platform_loading=platform_loading platform_busy=Signal::derive(move || active_build_state.get().is_some()) on_inspect=on_inspect on_upgrade=on_upgrade /> }
                                }).collect_view()}
                            </div>
                        </Show>
                    </div>
                </Show>
            </div>
        </div>
    }
}
