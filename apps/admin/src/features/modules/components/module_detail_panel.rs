#![allow(
    clippy::clone_on_copy,
    clippy::if_same_then_else,
    clippy::needless_return,
    clippy::redundant_locals,
    clippy::too_many_arguments,
    clippy::useless_format
)]

use super::detail::governance_form::GovernanceForm;
use super::detail::metadata_checklist_view::MetadataChecklistView;
use super::detail::version_trail::VersionTrailView;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::collections::HashMap;

use crate::entities::module::model::{MarketplaceModuleVersion, RegistryReleaseLifecycle};
use crate::entities::module::{MarketplaceModule, ModuleSettingField, TenantModule};
use crate::features::modules::transport::{
    self, RegistryMutationResult, RegistryPublishStatusContract,
};
use crate::shared::ui::Button;
use crate::{Locale, use_i18n};

use super::detail::{
    governance::{
        RegistryAutomatedCheckItem, automated_check_label, curl_snippet_for_live_api_action,
        destructive_governance_confirmation_message, follow_up_gate_label,
        follow_up_gate_status_summary, governance_action_reason_code_required,
        governance_action_reason_code_validation_message, governance_action_reason_required,
        governance_event_summary, governance_event_title, latest_validation_event,
        latest_validation_job_event, lifecycle_detail_lines, merge_governance_actions,
        moderation_history_badge_label, moderation_history_badge_status,
        moderation_history_context_lines, moderation_history_events, registry_governance_hint,
        registry_live_api_action_lines, registry_mutation_result_summary,
        registry_next_action_lines, registry_operator_command_lines,
        registry_request_is_review_ready, registry_request_status_badge_classes,
        registry_review_policy_lines, registry_validation_outcome_summary, status_eq,
        validation_feedback_badge_classes, validation_job_event_context_lines,
        validation_stage_recent_history, validation_stage_status_summary,
    },
    humanize_setting_key, humanize_token,
    json_editor::{ComplexSettingEditor, setting_option_draft_value, setting_option_label},
    metadata::marketplace_metadata_checklist,
};

fn tr(locale: Locale, en: &'static str, ru: &'static str) -> &'static str {
    match locale {
        Locale::ru => ru,
        _ => en,
    }
}

fn short_checksum(value: Option<&str>) -> Option<String> {
    let value = value?;
    if value.len() > 16 {
        Some(format!("{}...", &value[..12]))
    } else {
        Some(value.to_string())
    }
}

#[cfg(target_arch = "wasm32")]
fn copy_text_to_clipboard(text: &str) {
    if let Some(window) = web_sys::window() {
        let navigator = window.navigator();
        let clipboard = navigator.clipboard();
        let _ = clipboard.write_text(text);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn copy_text_to_clipboard(_text: &str) {}

fn latest_active_registry_version(module: &MarketplaceModule) -> Option<&MarketplaceModuleVersion> {
    module.versions.iter().find(|version| !version.yanked)
}

fn setting_field_hint(field: &ModuleSettingField, locale: Locale) -> Option<String> {
    let mut parts = Vec::new();
    if field.required {
        parts.push(tr(locale, "Required", "РћР±СЏР·Р°С‚РµР»СЊРЅРѕ").to_string());
    }
    if let Some(default) = &field.default_value {
        parts.push(format!(
            "{}: {}",
            tr(locale, "Default", "РџРѕ СѓРјРѕР»С‡Р°РЅРёСЋ"),
            default
        ));
    }
    match (field.min, field.max) {
        (Some(min), Some(max)) => parts.push(format!(
            "{}: {}..{}",
            tr(locale, "Range", "Р”РёР°РїР°Р·РѕРЅ"),
            min,
            max
        )),
        (Some(min), None) => {
            parts.push(format!("{}: {}", tr(locale, "Min", "РњРёРЅРёРјСѓРј"), min))
        }
        (None, Some(max)) => parts.push(format!(
            "{}: {}",
            tr(locale, "Max", "РњР°РєСЃРёРјСѓРј"),
            max
        )),
        (None, None) => {}
    }
    if !field.options.is_empty() {
        parts.push(format!(
            "{}: {}",
            tr(locale, "Options", "РћРїС†РёРё"),
            field
                .options
                .iter()
                .map(setting_option_label)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !field.object_keys.is_empty() {
        parts.push(format!(
            "{}: {}",
            tr(locale, "Object keys", "РљР»СЋС‡Рё РѕР±СЉРµРєС‚Р°"),
            field
                .object_keys
                .iter()
                .map(|key| humanize_setting_key(key))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if let Some(item_type) = field.item_type.as_deref() {
        parts.push(format!(
            "{}: {}",
            tr(locale, "Array items", "Array items"),
            humanize_token(item_type)
        ));
    }

    (!parts.is_empty()).then(|| parts.join(" В· "))
}

fn setting_field_placeholder(field: &ModuleSettingField) -> Option<&'static str> {
    match field.value_type.as_str() {
        "object" => Some("{\n  \"key\": \"value\"\n}"),
        "array" => Some("[\n  \"item\"\n]"),
        "json" | "any" => Some("{\n  \"any\": true\n}"),
        _ => None,
    }
}

#[component]
pub fn ModuleDetailPanel(
    admin_surface: String,
    selected_slug: String,
    module: Option<MarketplaceModule>,
    tenant_module: Option<TenantModule>,
    settings_schema: Vec<ModuleSettingField>,
    #[prop(into)] settings_form_supported: Signal<bool>,
    #[prop(into)] settings_form_draft: Signal<HashMap<String, String>>,
    #[prop(into)] settings_draft: Signal<String>,
    #[prop(into)] settings_editable: Signal<bool>,
    #[prop(into)] settings_saving: Signal<bool>,
    #[prop(into)] loading: Signal<bool>,
    #[prop(into)] access_token: Signal<Option<String>>,
    #[prop(into)] tenant_slug: Signal<Option<String>>,
    on_settings_field_input: Callback<(String, String)>,
    on_settings_input: Callback<String>,
    on_save_settings: Callback<()>,
    on_refresh_detail: Callback<()>,
    on_close: Callback<()>,
) -> impl IntoView {
    let locale = use_i18n().get_locale();
    let detail = module.clone();
    let detail_for_body = StoredValue::new(module.clone());
    let admin_surface_for_body = StoredValue::new(admin_surface.clone());
    let selected_slug_for_body = StoredValue::new(selected_slug.clone());
    let tenant_module_for_body = StoredValue::new(tenant_module.clone());
    let settings_schema_for_body = StoredValue::new(settings_schema.clone());
    let (governance_reason, set_governance_reason) = signal(String::new());
    let (governance_reason_code, set_governance_reason_code) = signal(String::new());
    let (governance_new_owner_user_id, set_governance_new_owner_user_id) = signal(String::new());
    let (governance_dry_run, set_governance_dry_run) = signal(false);
    let (governance_submitting, set_governance_submitting) = signal(false);
    let (governance_feedback, set_governance_feedback) = signal(None::<String>);
    let (governance_error, set_governance_error) = signal(None::<String>);
    let (governance_result, set_governance_result) = signal(None::<RegistryMutationResult>);
    let (governance_confirmation_action, set_governance_confirmation_action) =
        signal(None::<String>);
    let (governance_intent_action, set_governance_intent_action) = signal(None::<String>);
    let (governance_status_contract, set_governance_status_contract) =
        signal(None::<RegistryPublishStatusContract>);
    let (governance_status_contract_loading, set_governance_status_contract_loading) =
        signal(false);
    let (governance_status_contract_error, set_governance_status_contract_error) =
        signal(None::<String>);
    let (governance_contract_refresh_nonce, set_governance_contract_refresh_nonce) = signal(0u32);
    let status_request_id = module.as_ref().and_then(|module| {
        module
            .registry_lifecycle
            .as_ref()
            .and_then(|lifecycle| lifecycle.latest_request.as_ref())
            .map(|request| request.id.clone())
    });

    Effect::new(move |_| {
        let Some(request_id) = status_request_id.clone() else {
            set_governance_status_contract.set(None);
            set_governance_status_contract_loading.set(false);
            set_governance_status_contract_error.set(None);
            return;
        };

        let requested_refresh_nonce = governance_contract_refresh_nonce.get();
        let token = access_token.get();
        let tenant = tenant_slug.get();

        if token.is_none() {
            set_governance_status_contract.set(None);
            set_governance_status_contract_loading.set(false);
            set_governance_status_contract_error.set(None);
            return;
        }

        set_governance_status_contract_loading.set(true);
        set_governance_status_contract_error.set(None);

        spawn_local(async move {
            match transport::fetch_registry_publish_request_status(request_id, token, tenant).await
            {
                Ok(status) => {
                    if governance_contract_refresh_nonce.get_untracked() == requested_refresh_nonce
                    {
                        set_governance_status_contract.set(Some(status));
                        set_governance_status_contract_error.set(None);
                    }
                }
                Err(error) => {
                    if governance_contract_refresh_nonce.get_untracked() == requested_refresh_nonce
                    {
                        set_governance_status_contract.set(None);
                        set_governance_status_contract_error.set(Some(error.to_string()));
                    }
                }
            }

            if governance_contract_refresh_nonce.get_untracked() == requested_refresh_nonce {
                set_governance_status_contract_loading.set(false);
            }
        });
    });

    view! {
        <div class="rounded-xl border border-primary/20 bg-primary/5 p-6 shadow-sm">
            <div class="flex items-start justify-between gap-3">
                <div class="space-y-1">
                    <h3 class="text-base font-semibold text-card-foreground">
                        {tr(locale, "Module detail", "Р”РµС‚Р°Р»Рё РјРѕРґСѓР»СЏ")}
                    </h3>
                    <p class="text-sm text-muted-foreground">
                        {match detail.as_ref() {
                            Some(module) => format!(
                                "{} {}",
                                module.name
                                ,
                                tr(
                                    locale,
                                    "metadata from the internal marketplace catalog.",
                                    "вЂ” РјРµС‚Р°РґР°РЅРЅС‹Рµ РёР· РІРЅСѓС‚СЂРµРЅРЅРµРіРѕ marketplace-РєР°С‚Р°Р»РѕРіР°.",
                                )
                            ),
                            None if loading.get() => format!(
                                "{} {}",
                                tr(locale, "Loading", "Р—Р°РіСЂСѓР·РєР°"),
                                selected_slug
                            ),
                            None => format!(
                                "{} {}.",
                                tr(locale, "No catalog entry resolved for", "РќРµ СѓРґР°Р»РѕСЃСЊ РЅР°Р№С‚Рё Р·Р°РїРёСЃСЊ РєР°С‚Р°Р»РѕРіР° РґР»СЏ"),
                                selected_slug
                            ),
                        }}
                    </p>
                </div>
                <button
                    type="button"
                    class="inline-flex items-center justify-center rounded-md border border-border bg-background px-3 py-2 text-sm font-medium text-foreground transition-colors hover:bg-accent"
                    on:click=move |_| on_close.run(())
                >
                    {tr(locale, "Close", "Р—Р°РєСЂС‹С‚СЊ")}
                </button>
            </div>

            <Show
                when=move || detail.is_some()
                fallback=move || view! {
                    <p class="mt-4 text-sm text-muted-foreground">
                        {tr(
                            locale,
                            "The selected module is not available in the current catalog snapshot.",
                            "Р’С‹Р±СЂР°РЅРЅС‹Р№ РјРѕРґСѓР»СЊ РЅРµРґРѕСЃС‚СѓРїРµРЅ РІ С‚РµРєСѓС‰РµРј СЃРЅРёРјРєРµ РєР°С‚Р°Р»РѕРіР°.",
                        )}
                    </p>
                }
            >
                {move || {
                    detail_for_body.get_value().as_ref().map(|module| {
                        let module = module.clone();
                        let module_name = module.name.clone();
                        let module_tags = module.tags.clone();
                        let module_tags_for_show = module_tags.clone();
                        let module_icon_url = module.icon_url.clone();
                        let module_banner_url = module.banner_url.clone();
                        let module_banner_url_for_body = module_banner_url.clone();
                        let module_screenshots = module.screenshots.clone();
                        let module_screenshots_for_body = module_screenshots.clone();
                        let has_marketplace_visuals = module_banner_url.is_some() || !module_screenshots.is_empty();
                        let has_marketplace_screenshots = !module_screenshots.is_empty();
                        let metadata_checklist = marketplace_metadata_checklist(&module, locale);
                        let metadata_checklist_for_show = metadata_checklist.clone();
                        let metadata_required_issues = metadata_checklist
                            .iter()
                            .filter(|item| item.state == "warn" && item.priority == "required")
                            .count();
                        let metadata_recommended_gaps = metadata_checklist
                            .iter()
                            .filter(|item| item.state == "warn" && item.priority == "recommended")
                            .count();
                        let metadata_ready_count = metadata_checklist
                            .iter()
                            .filter(|item| item.state == "ready")
                            .count();
                        let version_trail = module.versions.clone().into_iter().take(5).collect::<Vec<_>>();
                        let latest_release = latest_active_registry_version(&module).cloned();
                        let latest_registry_request = module
                            .registry_lifecycle
                            .as_ref()
                            .and_then(|lifecycle| lifecycle.latest_request.clone());
                        let registry_owner_binding = module
                            .registry_lifecycle
                            .as_ref()
                            .and_then(|lifecycle| lifecycle.owner_binding.clone());
                        let latest_registry_release = module
                            .registry_lifecycle
                            .as_ref()
                            .and_then(|lifecycle| lifecycle.latest_release.clone());
                        let lifecycle_note_lines =
                            lifecycle_detail_lines(
                                latest_registry_request.as_ref(),
                                latest_registry_release.as_ref(),
                                registry_owner_binding.as_ref(),
                                locale,
                            );
                        let lifecycle_note_lines_for_show = lifecycle_note_lines.clone();
                        let review_policy_lines = registry_review_policy_lines(
                            latest_registry_request.as_ref(),
                            latest_registry_release.as_ref(),
                            registry_owner_binding.as_ref(),
                            locale,
                        );
                        let review_policy_lines_for_show = review_policy_lines.clone();
                        let next_action_lines = registry_next_action_lines(
                            &module,
                            latest_registry_request.as_ref(),
                            latest_registry_release.as_ref(),
                            registry_owner_binding.as_ref(),
                            module
                                .registry_lifecycle
                                .as_ref()
                                .map(|lifecycle| lifecycle.validation_stages.as_slice())
                                .unwrap_or(&[]),
                            locale,
                        );
                        let next_action_lines_for_show = next_action_lines.clone();
                        let operator_command_lines = registry_operator_command_lines(
                            &module,
                            latest_registry_request.as_ref(),
                            latest_registry_release.as_ref(),
                            registry_owner_binding.as_ref(),
                            module
                                .registry_lifecycle
                                .as_ref()
                                .map(|lifecycle| lifecycle.validation_stages.as_slice())
                                .unwrap_or(&[]),
                        );
                        let operator_command_lines_for_show = operator_command_lines.clone();
                        let live_api_action_lines = registry_live_api_action_lines(
                            &module,
                            latest_registry_request.as_ref(),
                            latest_registry_release.as_ref(),
                            registry_owner_binding.as_ref(),
                            module
                                .registry_lifecycle
                                .as_ref()
                                .map(|lifecycle| lifecycle.validation_stages.as_slice())
                                .unwrap_or(&[]),
                            locale,
                        );
                        let live_api_action_lines_for_show = live_api_action_lines.clone();
                        let summary_governance_actions = module
                            .registry_lifecycle
                            .as_ref()
                            .map(|lifecycle| lifecycle.governance_actions.clone())
                            .unwrap_or_default();
                        let summary_release_management_actions = summary_governance_actions
                            .iter()
                            .filter(|action| {
                                action.key.eq_ignore_ascii_case("owner_transfer")
                                    || action.key.eq_ignore_ascii_case("yank")
                            })
                            .cloned()
                            .collect::<Vec<_>>();
                        let recent_governance_events = module
                            .registry_lifecycle
                            .as_ref()
                            .map(|lifecycle| lifecycle.recent_events.clone())
                            .unwrap_or_default();
                        let recent_moderation_history =
                            moderation_history_events(&recent_governance_events, 6);
                        let validation_stages = module
                            .registry_lifecycle
                            .as_ref()
                            .map(|lifecycle| lifecycle.validation_stages.clone())
                            .unwrap_or_default();
                        let validation_stages_for_show =
                            StoredValue::new(validation_stages.clone());
                        let follow_up_gates = module
                            .registry_lifecycle
                            .as_ref()
                            .map(|lifecycle| lifecycle.follow_up_gates.clone())
                            .unwrap_or_default();
                        let follow_up_gates_for_show = StoredValue::new(follow_up_gates.clone());
                        let recent_governance_events_for_show =
                            StoredValue::new(recent_governance_events.clone());
                        let recent_moderation_history_for_show =
                            StoredValue::new(recent_moderation_history.clone());
                        let validation_warning_items = latest_registry_request
                            .as_ref()
                            .map(|request| request.warnings.clone())
                            .unwrap_or_default();
                        let validation_error_items = latest_registry_request
                            .as_ref()
                            .map(|request| request.errors.clone())
                            .unwrap_or_default();
                        let validation_rejection_reason = latest_registry_request
                            .as_ref()
                            .and_then(|request| request.rejection_reason.clone())
                            .filter(|value| !value.trim().is_empty());
                        let validation_outcome_summary = latest_registry_request
                            .as_ref()
                            .and_then(|request| {
                                registry_validation_outcome_summary(
                                    request,
                                    &recent_governance_events,
                                    locale,
                                )
                            });
                        let review_ready = latest_registry_request
                            .as_ref()
                            .is_some_and(registry_request_is_review_ready);
                        let latest_validation_event_summary = latest_validation_event(&recent_governance_events)
                            .map(|event| {
                                (
                                    governance_event_title(&event.event_type, locale),
                                    governance_event_summary(event, locale),
                                    event.created_at.clone(),
                                    event.actor.clone(),
                                )
                            });
                        let automated_check_items: Vec<RegistryAutomatedCheckItem> = Vec::new();
                        let automated_check_items_for_show =
                            StoredValue::new(automated_check_items.clone());
                        let latest_validation_job_summary = latest_validation_job_event(
                            &recent_governance_events,
                        )
                        .map(|event| {
                            (
                                governance_event_title(&event.event_type, locale),
                                governance_event_summary(event, locale),
                                event.created_at.clone(),
                                event.actor.clone(),
                                validation_job_event_context_lines(event, locale),
                            )
                        });
                        let follow_up_gate_summary =
                            follow_up_gate_status_summary(&follow_up_gates, locale);
                        let validation_stage_summary =
                            validation_stage_status_summary(&validation_stages, locale);
                        let validation_warning_items_for_show =
                            StoredValue::new(validation_warning_items.clone());
                        let validation_error_items_for_show =
                            StoredValue::new(validation_error_items.clone());
                        let has_validation_warnings = !validation_warning_items.is_empty();
                        let has_validation_errors = !validation_error_items.is_empty();
                        let has_automated_check_items = !automated_check_items.is_empty();
                        let show_validation_summary = has_validation_warnings
                            || has_validation_errors
                            || validation_rejection_reason.is_some()
                            || validation_outcome_summary.is_some()
                            || review_ready
                            || latest_validation_event_summary.is_some()
                            || latest_validation_job_summary.is_some()
                            || has_automated_check_items;
                        let show_follow_up_gates = !follow_up_gates.is_empty();
                        let show_validation_stages = !validation_stages.is_empty();
                        let governance_hint = registry_governance_hint(&module, locale);
                        let checksum = short_checksum(module.checksum_sha256.as_deref());
                        let request_id = latest_registry_request.as_ref().map(|request| request.id.clone());
                        let has_request_status_contract = latest_registry_request.is_some();
                        let summary_release_management_actions_for_form =
                            summary_release_management_actions.clone();
                        let governance_actions_for_form = Memo::new(move |_| {
                            let request_level_actions = governance_status_contract
                                .get()
                                .map(|status| status.governance_actions)
                                .unwrap_or_default();
                            merge_governance_actions(
                                &request_level_actions,
                                &summary_release_management_actions_for_form,
                            )
                        });
                        let show_interactive_governance_form = latest_registry_request.is_some()
                            || !summary_release_management_actions.is_empty();
                        let release_version = latest_registry_release
                            .as_ref()
                            .map(|release| release.version.clone())
                            .unwrap_or_else(|| module.latest_version.clone());
                        let module_slug_for_actions = module.slug.clone();
                        let admin_surface = admin_surface_for_body.get_value();
                        let primary_here = module
                            .recommended_admin_surfaces
                            .iter()
                            .any(|surface| surface == &admin_surface);
                        let showcase_here = module
                            .showcase_admin_surfaces
                            .iter()
                            .any(|surface| surface == &admin_surface);
                        let refresh_detail_after_validate = on_refresh_detail.clone();
                        let refresh_detail_after_approve = on_refresh_detail.clone();
                        let refresh_detail_after_request_changes = on_refresh_detail.clone();
                        let refresh_detail_after_hold = on_refresh_detail.clone();
                        let refresh_detail_after_resume = on_refresh_detail.clone();
                        let refresh_detail_after_reject = on_refresh_detail.clone();
                        let refresh_detail_after_transfer = on_refresh_detail.clone();
                        let refresh_detail_after_yank = on_refresh_detail.clone();
                        let on_validate_request = {
                            let request_id = request_id.clone();
                            let access_token = access_token;
                            let tenant_slug = tenant_slug;
                            Callback::new(move |_| {
                                set_governance_intent_action.set(Some("validate".to_string()));
                                set_governance_confirmation_action.set(None);
                                let Some(request_id) = request_id.clone() else {
                                    set_governance_error.set(Some(
                                        tr(locale, "No publish request available.", "РќРµС‚ РґРѕСЃС‚СѓРїРЅРѕРіРѕ publish-Р·Р°РїСЂРѕСЃР°.")
                                            .to_string(),
                                    ));
                                    return;
                                };
                                set_governance_submitting.set(true);
                                set_governance_feedback.set(None);
                                set_governance_error.set(None);
                                let dry_run = governance_dry_run.get_untracked();
                                let token = access_token.get_untracked();
                                let tenant = tenant_slug.get_untracked();
                                spawn_local(async move {
                                    match transport::validate_registry_publish_request(
                                        request_id,
                                        dry_run,
                                        token,
                                        tenant,
                                    )
                                    .await
                                    {
                                        Ok(result) => {
                                            set_governance_feedback.set(Some(
                                                registry_mutation_result_summary(&result, locale),
                                            ));
                                            set_governance_result.set(Some(result));
                                            set_governance_contract_refresh_nonce.update(|value| *value += 1);
                                            refresh_detail_after_validate.run(());
                                        }
                                        Err(error) => {
                                            set_governance_error
                                                .set(Some(error.to_string()));
                                        }
                                    }
                                    set_governance_submitting.set(false);
                                });
                            })
                        };
                        let on_approve_request = {
                            let request_id = request_id.clone();
                            let access_token = access_token;
                            let tenant_slug = tenant_slug;
                            Callback::new(move |_| {
                                set_governance_intent_action.set(Some("approve".to_string()));
                                set_governance_confirmation_action.set(None);
                                let Some(request_id) = request_id.clone() else {
                                    set_governance_error.set(Some(
                                        tr(locale, "No publish request available.", "РќРµС‚ РґРѕСЃС‚СѓРїРЅРѕРіРѕ publish-Р·Р°РїСЂРѕСЃР°.")
                                            .to_string(),
                                    ));
                                    return;
                                };
                                set_governance_submitting.set(true);
                                set_governance_feedback.set(None);
                                set_governance_error.set(None);
                                let dry_run = governance_dry_run.get_untracked();
                                let reason =
                                    governance_reason.get_untracked().trim().to_string();
                                let reason_code =
                                    governance_reason_code.get_untracked().trim().to_string();
                                let governance_actions =
                                    governance_actions_for_form.get_untracked();
                                if !dry_run
                                    && governance_action_reason_required(
                                        &governance_actions,
                                        "approve",
                                    )
                                    && reason.is_empty()
                                {
                                    set_governance_error.set(Some(
                                        tr(locale, "Reason is required.", "РќСѓР¶РЅРѕ СѓРєР°Р·Р°С‚СЊ РїСЂРёС‡РёРЅСѓ.")
                                            .to_string(),
                                    ));
                                    return;
                                }
                                if !dry_run
                                    && governance_action_reason_code_required(
                                        &governance_actions,
                                        "approve",
                                    )
                                    && reason_code.is_empty()
                                {
                                    set_governance_error.set(Some(
                                        tr(locale, "Reason code is required.", "РќСѓР¶РЅРѕ СѓРєР°Р·Р°С‚СЊ reason code.")
                                            .to_string(),
                                    ));
                                    return;
                                }
                                if let Some(message) = governance_action_reason_code_validation_message(
                                    &governance_actions,
                                    "approve",
                                    &reason_code,
                                    locale,
                                ) {
                                    set_governance_error.set(Some(message));
                                    return;
                                }
                                let token = access_token.get_untracked();
                                let tenant = tenant_slug.get_untracked();
                                spawn_local(async move {
                                    match transport::approve_registry_publish_request(
                                        request_id,
                                        (!reason.is_empty()).then_some(reason),
                                        (!reason_code.is_empty()).then_some(reason_code),
                                        dry_run,
                                        token,
                                        tenant,
                                    )
                                    .await
                                    {
                                        Ok(result) => {
                                            set_governance_feedback.set(Some(
                                                registry_mutation_result_summary(&result, locale),
                                            ));
                                            set_governance_result.set(Some(result));
                                            set_governance_contract_refresh_nonce.update(|value| *value += 1);
                                            refresh_detail_after_approve.run(());
                                        }
                                        Err(error) => {
                                            set_governance_error
                                                .set(Some(error.to_string()));
                                        }
                                    }
                                    set_governance_submitting.set(false);
                                });
                            })
                        };
                        let on_request_changes_request = {
                            let request_id = request_id.clone();
                            let access_token = access_token;
                            let tenant_slug = tenant_slug;
                            Callback::new(move |_| {
                                set_governance_intent_action
                                    .set(Some("request_changes".to_string()));
                                set_governance_confirmation_action.set(None);
                                let Some(request_id) = request_id.clone() else {
                                    set_governance_error.set(Some(
                                        tr(locale, "No publish request available.", "РќРµС‚ РґРѕСЃС‚СѓРїРЅРѕРіРѕ publish-Р·Р°РїСЂРѕСЃР°.")
                                            .to_string(),
                                    ));
                                    return;
                                };
                                let reason = governance_reason.get_untracked().trim().to_string();
                                let reason_code =
                                    governance_reason_code.get_untracked().trim().to_string();
                                let dry_run = governance_dry_run.get_untracked();
                                let governance_actions =
                                    governance_actions_for_form.get_untracked();
                                if !dry_run
                                    && governance_action_reason_required(
                                        &governance_actions,
                                        "request_changes",
                                    )
                                    && reason.is_empty()
                                {
                                    set_governance_error.set(Some(
                                        tr(locale, "Reason is required.", "РќСѓР¶РЅРѕ СѓРєР°Р·Р°С‚СЊ РїСЂРёС‡РёРЅСѓ.")
                                            .to_string(),
                                    ));
                                    return;
                                }
                                if !dry_run
                                    && governance_action_reason_code_required(
                                        &governance_actions,
                                        "request_changes",
                                    )
                                    && reason_code.is_empty()
                                {
                                    set_governance_error.set(Some(
                                        tr(locale, "Reason code is required.", "РќСѓР¶РЅРѕ СѓРєР°Р·Р°С‚СЊ reason code.")
                                            .to_string(),
                                    ));
                                    return;
                                }
                                if let Some(message) = governance_action_reason_code_validation_message(
                                    &governance_actions,
                                    "request_changes",
                                    &reason_code,
                                    locale,
                                ) {
                                    set_governance_error.set(Some(message));
                                    return;
                                }
                                set_governance_submitting.set(true);
                                set_governance_feedback.set(None);
                                set_governance_error.set(None);
                                let token = access_token.get_untracked();
                                let tenant = tenant_slug.get_untracked();
                                spawn_local(async move {
                                    match transport::request_changes_registry_publish_request(
                                        request_id,
                                        reason,
                                        reason_code,
                                        dry_run,
                                        token,
                                        tenant,
                                    )
                                    .await
                                    {
                                        Ok(result) => {
                                            set_governance_feedback.set(Some(
                                                registry_mutation_result_summary(&result, locale),
                                            ));
                                            set_governance_result.set(Some(result));
                                            set_governance_contract_refresh_nonce.update(|value| *value += 1);
                                            refresh_detail_after_request_changes.run(());
                                        }
                                        Err(error) => {
                                            set_governance_error
                                                .set(Some(error.to_string()));
                                        }
                                    }
                                    set_governance_submitting.set(false);
                                });
                            })
                        };
                        let on_hold_request = {
                            let request_id = request_id.clone();
                            let access_token = access_token;
                            let tenant_slug = tenant_slug;
                            Callback::new(move |_| {
                                set_governance_intent_action.set(Some("hold".to_string()));
                                set_governance_confirmation_action.set(None);
                                let Some(request_id) = request_id.clone() else {
                                    set_governance_error.set(Some(
                                        tr(locale, "No publish request available.", "РќРµС‚ РґРѕСЃС‚СѓРїРЅРѕРіРѕ publish-Р·Р°РїСЂРѕСЃР°.")
                                            .to_string(),
                                    ));
                                    return;
                                };
                                let reason = governance_reason.get_untracked().trim().to_string();
                                let reason_code =
                                    governance_reason_code.get_untracked().trim().to_string();
                                let dry_run = governance_dry_run.get_untracked();
                                let governance_actions =
                                    governance_actions_for_form.get_untracked();
                                if !dry_run
                                    && governance_action_reason_required(
                                        &governance_actions,
                                        "hold",
                                    )
                                    && reason.is_empty()
                                {
                                    set_governance_error.set(Some(
                                        tr(locale, "Reason is required.", "РќСѓР¶РЅРѕ СѓРєР°Р·Р°С‚СЊ РїСЂРёС‡РёРЅСѓ.")
                                            .to_string(),
                                    ));
                                    return;
                                }
                                if !dry_run
                                    && governance_action_reason_code_required(
                                        &governance_actions,
                                        "hold",
                                    )
                                    && reason_code.is_empty()
                                {
                                    set_governance_error.set(Some(
                                        tr(locale, "Reason code is required.", "РќСѓР¶РЅРѕ СѓРєР°Р·Р°С‚СЊ reason code.")
                                            .to_string(),
                                    ));
                                    return;
                                }
                                if let Some(message) = governance_action_reason_code_validation_message(
                                    &governance_actions,
                                    "hold",
                                    &reason_code,
                                    locale,
                                ) {
                                    set_governance_error.set(Some(message));
                                    return;
                                }
                                set_governance_submitting.set(true);
                                set_governance_feedback.set(None);
                                set_governance_error.set(None);
                                let token = access_token.get_untracked();
                                let tenant = tenant_slug.get_untracked();
                                spawn_local(async move {
                                    match transport::hold_registry_publish_request(
                                        request_id,
                                        reason,
                                        reason_code,
                                        dry_run,
                                        token,
                                        tenant,
                                    )
                                    .await
                                    {
                                        Ok(result) => {
                                            set_governance_feedback.set(Some(
                                                registry_mutation_result_summary(&result, locale),
                                            ));
                                            set_governance_result.set(Some(result));
                                            set_governance_contract_refresh_nonce.update(|value| *value += 1);
                                            refresh_detail_after_hold.run(());
                                        }
                                        Err(error) => {
                                            set_governance_error
                                                .set(Some(error.to_string()));
                                        }
                                    }
                                    set_governance_submitting.set(false);
                                });
                            })
                        };
                        let on_resume_request = {
                            let request_id = request_id.clone();
                            let access_token = access_token;
                            let tenant_slug = tenant_slug;
                            Callback::new(move |_| {
                                set_governance_intent_action.set(Some("resume".to_string()));
                                set_governance_confirmation_action.set(None);
                                let Some(request_id) = request_id.clone() else {
                                    set_governance_error.set(Some(
                                        tr(locale, "No publish request available.", "РќРµС‚ РґРѕСЃС‚СѓРїРЅРѕРіРѕ publish-Р·Р°РїСЂРѕСЃР°.")
                                            .to_string(),
                                    ));
                                    return;
                                };
                                let reason = governance_reason.get_untracked().trim().to_string();
                                let reason_code =
                                    governance_reason_code.get_untracked().trim().to_string();
                                let dry_run = governance_dry_run.get_untracked();
                                let governance_actions =
                                    governance_actions_for_form.get_untracked();
                                if !dry_run
                                    && governance_action_reason_required(
                                        &governance_actions,
                                        "resume",
                                    )
                                    && reason.is_empty()
                                {
                                    set_governance_error.set(Some(
                                        tr(locale, "Reason is required.", "РќСѓР¶РЅРѕ СѓРєР°Р·Р°С‚СЊ РїСЂРёС‡РёРЅСѓ.")
                                            .to_string(),
                                    ));
                                    return;
                                }
                                if !dry_run
                                    && governance_action_reason_code_required(
                                        &governance_actions,
                                        "resume",
                                    )
                                    && reason_code.is_empty()
                                {
                                    set_governance_error.set(Some(
                                        tr(locale, "Reason code is required.", "РќСѓР¶РЅРѕ СѓРєР°Р·Р°С‚СЊ reason code.")
                                            .to_string(),
                                    ));
                                    return;
                                }
                                if let Some(message) = governance_action_reason_code_validation_message(
                                    &governance_actions,
                                    "resume",
                                    &reason_code,
                                    locale,
                                ) {
                                    set_governance_error.set(Some(message));
                                    return;
                                }
                                set_governance_submitting.set(true);
                                set_governance_feedback.set(None);
                                set_governance_error.set(None);
                                let token = access_token.get_untracked();
                                let tenant = tenant_slug.get_untracked();
                                spawn_local(async move {
                                    match transport::resume_registry_publish_request(
                                        request_id,
                                        reason,
                                        reason_code,
                                        dry_run,
                                        token,
                                        tenant,
                                    )
                                    .await
                                    {
                                        Ok(result) => {
                                            set_governance_feedback.set(Some(
                                                registry_mutation_result_summary(&result, locale),
                                            ));
                                            set_governance_result.set(Some(result));
                                            set_governance_contract_refresh_nonce.update(|value| *value += 1);
                                            refresh_detail_after_resume.run(());
                                        }
                                        Err(error) => {
                                            set_governance_error
                                                .set(Some(error.to_string()));
                                        }
                                    }
                                    set_governance_submitting.set(false);
                                });
                            })
                        };
                        let on_reject_request = {
                            let request_id = request_id.clone();
                            let access_token = access_token;
                            let tenant_slug = tenant_slug;
                            let module_slug_for_actions = module_slug_for_actions.clone();
                            Callback::new(move |_| {
                                set_governance_intent_action.set(Some("reject".to_string()));
                                let Some(request_id) = request_id.clone() else {
                                    set_governance_error.set(Some(
                                        tr(locale, "No publish request available.", "РќРµС‚ РґРѕСЃС‚СѓРїРЅРѕРіРѕ publish-Р·Р°РїСЂРѕСЃР°.")
                                            .to_string(),
                                    ));
                                    return;
                                };
                                let reason = governance_reason.get_untracked().trim().to_string();
                                let reason_code =
                                    governance_reason_code.get_untracked().trim().to_string();
                                let dry_run = governance_dry_run.get_untracked();
                                let governance_actions =
                                    governance_actions_for_form.get_untracked();
                                if !dry_run
                                    && governance_action_reason_required(
                                        &governance_actions,
                                        "reject",
                                    )
                                    && reason.is_empty()
                                {
                                    set_governance_error.set(Some(
                                        tr(locale, "Reason is required.", "РќСѓР¶РЅРѕ СѓРєР°Р·Р°С‚СЊ РїСЂРёС‡РёРЅСѓ.")
                                            .to_string(),
                                    ));
                                    return;
                                }
                                if !dry_run
                                    && governance_action_reason_code_required(
                                        &governance_actions,
                                        "reject",
                                    )
                                    && reason_code.is_empty()
                                {
                                    set_governance_error.set(Some(
                                        tr(locale, "Reason code is required.", "РќСѓР¶РЅРѕ СѓРєР°Р·Р°С‚СЊ reason code.")
                                            .to_string(),
                                    ));
                                    return;
                                }
                                if let Some(message) = governance_action_reason_code_validation_message(
                                    &governance_actions,
                                    "reject",
                                    &reason_code,
                                    locale,
                                ) {
                                    set_governance_error.set(Some(message));
                                    return;
                                }
                                if !dry_run
                                    && governance_confirmation_action.get_untracked().as_deref()
                                        != Some("reject")
                                {
                                    set_governance_confirmation_action
                                        .set(Some("reject".to_string()));
                                    set_governance_feedback.set(Some(
                                        destructive_governance_confirmation_message(
                                            "reject",
                                            &module_slug_for_actions,
                                            None,
                                            None,
                                            locale,
                                        ),
                                    ));
                                    set_governance_error.set(None);
                                    set_governance_result.set(None);
                                    return;
                                }
                                set_governance_confirmation_action.set(None);
                                set_governance_submitting.set(true);
                                set_governance_feedback.set(None);
                                set_governance_error.set(None);
                                let token = access_token.get_untracked();
                                let tenant = tenant_slug.get_untracked();
                                spawn_local(async move {
                                    match transport::reject_registry_publish_request(
                                        request_id,
                                        reason,
                                        reason_code,
                                        dry_run,
                                        token,
                                        tenant,
                                    )
                                    .await
                                    {
                                        Ok(result) => {
                                            set_governance_feedback.set(Some(
                                                registry_mutation_result_summary(&result, locale),
                                            ));
                                            set_governance_result.set(Some(result));
                                            set_governance_contract_refresh_nonce.update(|value| *value += 1);
                                            refresh_detail_after_reject.run(());
                                        }
                                        Err(error) => {
                                            set_governance_error
                                                .set(Some(error.to_string()));
                                        }
                                    }
                                    set_governance_submitting.set(false);
                                });
                            })
                        };
                        let on_transfer_owner = {
                            let module_slug_for_actions = module_slug_for_actions.clone();
                            let access_token = access_token;
                            let tenant_slug = tenant_slug;
                            Callback::new(move |_| {
                                set_governance_intent_action
                                    .set(Some("owner_transfer".to_string()));
                                let new_owner_user_id = governance_new_owner_user_id
                                    .get_untracked()
                                    .trim()
                                    .to_string();
                                let reason = governance_reason.get_untracked().trim().to_string();
                                let reason_code =
                                    governance_reason_code.get_untracked().trim().to_string();
                                let dry_run = governance_dry_run.get_untracked();
                                let governance_actions =
                                    governance_actions_for_form.get_untracked();
                                if new_owner_user_id.is_empty() {
                                    set_governance_error.set(Some(
                                        tr(
                                            locale,
                                            "New owner user id is required.",
                                            "РќСѓР¶РЅРѕ СѓРєР°Р·Р°С‚СЊ user id РЅРѕРІРѕРіРѕ РІР»Р°РґРµР»СЊС†Р°."
                                        )
                                        .to_string(),
                                    ));
                                    return;
                                }
                                if !dry_run
                                    && governance_action_reason_required(
                                        &governance_actions,
                                        "owner_transfer",
                                    )
                                    && reason.is_empty()
                                {
                                    set_governance_error.set(Some(
                                        tr(locale, "Reason is required.", "РќСѓР¶РЅРѕ СѓРєР°Р·Р°С‚СЊ РїСЂРёС‡РёРЅСѓ.")
                                            .to_string(),
                                    ));
                                    return;
                                }
                                if !dry_run
                                    && governance_action_reason_code_required(
                                        &governance_actions,
                                        "owner_transfer",
                                    )
                                    && reason_code.is_empty()
                                {
                                    set_governance_error.set(Some(
                                        tr(locale, "Reason code is required.", "РќСѓР¶РЅРѕ СѓРєР°Р·Р°С‚СЊ reason code.")
                                            .to_string(),
                                    ));
                                    return;
                                }
                                if let Some(message) = governance_action_reason_code_validation_message(
                                    &governance_actions,
                                    "owner_transfer",
                                    &reason_code,
                                    locale,
                                ) {
                                    set_governance_error.set(Some(message));
                                    return;
                                }
                                if !dry_run
                                    && governance_confirmation_action.get_untracked().as_deref()
                                        != Some("owner-transfer")
                                {
                                    set_governance_confirmation_action
                                        .set(Some("owner-transfer".to_string()));
                                    set_governance_feedback.set(Some(
                                        destructive_governance_confirmation_message(
                                            "owner-transfer",
                                            &module_slug_for_actions,
                                            None,
                                            Some(&new_owner_user_id),
                                            locale,
                                        ),
                                    ));
                                    set_governance_error.set(None);
                                    set_governance_result.set(None);
                                    return;
                                }
                                set_governance_confirmation_action.set(None);
                                set_governance_submitting.set(true);
                                set_governance_feedback.set(None);
                                set_governance_error.set(None);
                                let token = access_token.get_untracked();
                                let tenant = tenant_slug.get_untracked();
                                let module_slug_for_actions = module_slug_for_actions.clone();
                                spawn_local(async move {
                                    match transport::transfer_registry_owner(
                                        module_slug_for_actions.clone(),
                                        new_owner_user_id,
                                        reason,
                                        reason_code,
                                        dry_run,
                                        token,
                                        tenant,
                                    )
                                    .await
                                    {
                                        Ok(result) => {
                                            set_governance_feedback.set(Some(
                                                registry_mutation_result_summary(&result, locale),
                                            ));
                                            set_governance_result.set(Some(result));
                                            set_governance_contract_refresh_nonce.update(|value| *value += 1);
                                            refresh_detail_after_transfer.run(());
                                        }
                                        Err(error) => {
                                            set_governance_error
                                                .set(Some(error.to_string()));
                                        }
                                    }
                                    set_governance_submitting.set(false);
                                });
                            })
                        };
                        let on_yank_release = {
                            let module_slug_for_actions = module_slug_for_actions.clone();
                            let release_version = release_version.clone();
                            let access_token = access_token;
                            let tenant_slug = tenant_slug;
                            Callback::new(move |_| {
                                set_governance_intent_action.set(Some("yank".to_string()));
                                let reason = governance_reason.get_untracked().trim().to_string();
                                let reason_code =
                                    governance_reason_code.get_untracked().trim().to_string();
                                let dry_run = governance_dry_run.get_untracked();
                                let governance_actions =
                                    governance_actions_for_form.get_untracked();
                                if !dry_run
                                    && governance_action_reason_required(
                                        &governance_actions,
                                        "yank",
                                    )
                                    && reason.is_empty()
                                {
                                    set_governance_error.set(Some(
                                        tr(locale, "Reason is required.", "РќСѓР¶РЅРѕ СѓРєР°Р·Р°С‚СЊ РїСЂРёС‡РёРЅСѓ.")
                                            .to_string(),
                                    ));
                                    return;
                                }
                                if !dry_run
                                    && governance_action_reason_code_required(
                                        &governance_actions,
                                        "yank",
                                    )
                                    && reason_code.is_empty()
                                {
                                    set_governance_error.set(Some(
                                        tr(locale, "Reason code is required.", "РќСѓР¶РЅРѕ СѓРєР°Р·Р°С‚СЊ reason code.")
                                            .to_string(),
                                    ));
                                    return;
                                }
                                if let Some(message) = governance_action_reason_code_validation_message(
                                    &governance_actions,
                                    "yank",
                                    &reason_code,
                                    locale,
                                ) {
                                    set_governance_error.set(Some(message));
                                    return;
                                }
                                if !dry_run
                                    && governance_confirmation_action.get_untracked().as_deref()
                                        != Some("yank")
                                {
                                    set_governance_confirmation_action
                                        .set(Some("yank".to_string()));
                                    set_governance_feedback.set(Some(
                                        destructive_governance_confirmation_message(
                                            "yank",
                                            &module_slug_for_actions,
                                            Some(&release_version),
                                            None,
                                            locale,
                                        ),
                                    ));
                                    set_governance_error.set(None);
                                    set_governance_result.set(None);
                                    return;
                                }
                                set_governance_confirmation_action.set(None);
                                set_governance_submitting.set(true);
                                set_governance_feedback.set(None);
                                set_governance_error.set(None);
                                let token = access_token.get_untracked();
                                let tenant = tenant_slug.get_untracked();
                                let module_slug_for_actions = module_slug_for_actions.clone();
                                let release_version = release_version.clone();
                                spawn_local(async move {
                                    match transport::yank_registry_release(
                                        module_slug_for_actions.clone(),
                                        release_version.clone(),
                                        reason,
                                        reason_code,
                                        dry_run,
                                        token,
                                        tenant,
                                    )
                                    .await
                                    {
                                        Ok(result) => {
                                            set_governance_feedback.set(Some(
                                                registry_mutation_result_summary(&result, locale),
                                            ));
                                            set_governance_result.set(Some(result));
                                            set_governance_contract_refresh_nonce.update(|value| *value += 1);
                                            refresh_detail_after_yank.run(());
                                        }
                                        Err(error) => {
                                            set_governance_error
                                                .set(Some(error.to_string()));
                                        }
                                    }
                                    set_governance_submitting.set(false);
                                });
                            })
                        };
                        let on_governance_refresh = {
                            let on_refresh_detail = on_refresh_detail.clone();
                            Callback::new(move |_| {
                                set_governance_intent_action.set(None);
                                set_governance_confirmation_action.set(None);
                                set_governance_feedback.set(None);
                                set_governance_contract_refresh_nonce.update(|value| *value += 1);
                                on_refresh_detail.run(());
                            })
                        };
                        view! {
                            <div class="mt-4 space-y-4">
                                <div class="space-y-2">
                                    <div class="flex flex-wrap items-center gap-2">
                                        {module_icon_url.clone().map(|icon_url| {
                                            let module_name = module_name.clone();
                                            view! {
                                                <img
                                                    class="h-10 w-10 rounded-lg border border-border bg-background object-cover"
                                                    src=icon_url
                                                    alt=format!("{} icon", module_name)
                                                />
                                            }
                                        })}
                                        <h4 class="text-lg font-semibold text-card-foreground">{module.name.clone()}</h4>
                                        <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                            {format!("v{}", module.latest_version)}
                                        </span>
                                        <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground">
                                            {humanize_token(&module.source)}
                                        </span>
                                        <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                            {humanize_token(&module.category)}
                                        </span>
                                        <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                            {if module.compatible {
                                                tr(locale, "Compatible", "РЎРѕРІРјРµСЃС‚РёРј")
                                            } else {
                                                tr(locale, "Compatibility risk", "Р РёСЃРє СЃРѕРІРјРµСЃС‚РёРјРѕСЃС‚Рё")
                                            }}
                                        </span>
                                        {module.signature_present.then(|| view! {
                                            <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground">
                                                {tr(locale, "Signed", "РџРѕРґРїРёСЃР°РЅ")}
                                            </span>
                                        })}
                                        {module.installed.then(|| view! {
                                            <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground">
                                                {format!(
                                                    "{}{}",
                                                    tr(locale, "Installed", "РЈСЃС‚Р°РЅРѕРІР»РµРЅ"),
                                                    module
                                                        .installed_version
                                                        .as_ref()
                                                        .map(|value| format!(" v{}", value))
                                                        .unwrap_or_default()
                                                )}
                                            </span>
                                        })}
                                        {module.update_available.then(|| view! {
                                            <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                                {tr(locale, "Update available", "Р”РѕСЃС‚СѓРїРЅРѕ РѕР±РЅРѕРІР»РµРЅРёРµ")}
                                            </span>
                                        })}
                                    </div>
                                    <Show when=move || !module_tags_for_show.is_empty()>
                                        <div class="flex flex-wrap items-center gap-2 text-xs">
                                            {module_tags.clone().into_iter().map(|tag| {
                                                view! {
                                                    <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                                        {format!("#{}", tag)}
                                                    </span>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </Show>
                                    <p class="text-sm text-muted-foreground">{module.description.clone()}</p>
                                </div>

                                <div class="flex flex-wrap items-center gap-2 text-xs">
                                    <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 font-semibold text-secondary-foreground">
                                        {humanize_token(&module.ownership)}
                                    </span>
                                    <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                        {humanize_token(&module.trust_level)}
                                    </span>
                                    <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                        {if primary_here {
                                            tr(locale, "Primary for this admin", "Primary РґР»СЏ СЌС‚РѕР№ admin-РїРѕРІРµСЂС…РЅРѕСЃС‚Рё")
                                        } else if showcase_here {
                                            tr(locale, "Showcase for this admin", "Showcase РґР»СЏ СЌС‚РѕР№ admin-РїРѕРІРµСЂС…РЅРѕСЃС‚Рё")
                                        } else {
                                            tr(locale, "No dedicated UI for this admin", "Р”Р»СЏ СЌС‚РѕР№ admin-РїРѕРІРµСЂС…РЅРѕСЃС‚Рё РЅРµС‚ РІС‹РґРµР»РµРЅРЅРѕРіРѕ UI")
                                        }}
                                    </span>
                                </div>

                                <div class="grid gap-4 lg:grid-cols-2">
                                    <div class="rounded-lg border border-border bg-background/70 p-4 text-sm">
                                        <p class="text-xs uppercase tracking-wide text-muted-foreground">
                                            {tr(locale, "Package metadata", "РњРµС‚Р°РґР°РЅРЅС‹Рµ РїР°РєРµС‚Р°")}
                                        </p>
                                        <dl class="mt-3 space-y-2">
                                            <div class="flex items-start justify-between gap-3">
                                                <dt class="text-muted-foreground">{tr(locale, "Slug", "Slug")}</dt>
                                                <dd class="font-mono text-right">{module.slug.clone()}</dd>
                                            </div>
                                            <div class="flex items-start justify-between gap-3">
                                                <dt class="text-muted-foreground">{tr(locale, "Crate", "Crate")}</dt>
                                                <dd class="font-mono text-right">{module.crate_name.clone()}</dd>
                                            </div>
                                            <div class="flex items-start justify-between gap-3">
                                                <dt class="text-muted-foreground">{tr(locale, "Publisher", "РР·РґР°С‚РµР»СЊ")}</dt>
                                                <dd class="text-right">{module.publisher.clone().unwrap_or_else(|| tr(locale, "Workspace / unknown", "Workspace / РЅРµРёР·РІРµСЃС‚РЅРѕ").to_string())}</dd>
                                            </div>
                                            <div class="flex items-start justify-between gap-3">
                                                <dt class="text-muted-foreground">{tr(locale, "RusTok range", "Р”РёР°РїР°Р·РѕРЅ RusTok")}</dt>
                                                <dd class="text-right">
                                                    {format!(
                                                        "{}{}",
                                                        module
                                                            .rustok_min_version
                                                            .as_ref()
                                                            .map(|value| format!(">= {}", value))
                                                            .unwrap_or_else(|| tr(locale, "no min", "Р±РµР· min").to_string()),
                                                        module
                                                            .rustok_max_version
                                                            .as_ref()
                                                            .map(|value| format!(", <= {}", value))
                                                            .unwrap_or_else(|| format!(", {}", tr(locale, "no max", "Р±РµР· max")))
                                                    )}
                                                </dd>
                                            </div>
                                            <div class="flex items-start justify-between gap-3">
                                                <dt class="text-muted-foreground">{tr(locale, "Checksum", "РљРѕРЅС‚СЂРѕР»СЊРЅР°СЏ СЃСѓРјРјР°")}</dt>
                                                <dd class="font-mono text-right">{checksum.unwrap_or_else(|| tr(locale, "Not published", "РќРµ РѕРїСѓР±Р»РёРєРѕРІР°РЅ").to_string())}</dd>
                                            </div>
                                            <div class="flex items-start justify-between gap-3">
                                                <dt class="text-muted-foreground">{tr(locale, "Latest published", "РџРѕСЃР»РµРґРЅСЏСЏ РїСѓР±Р»РёРєР°С†РёСЏ")}</dt>
                                                <dd class="text-right">
                                                    {latest_release
                                                        .as_ref()
                                                        .map(|version| format!(
                                                            "v{}{}",
                                                            version.version,
                                                            version
                                                                .published_at
                                                                .as_ref()
                                                                .map(|value| format!(" В· {}", value))
                                                                .unwrap_or_default()
                                                        ))
                                                        .unwrap_or_else(|| tr(locale, "No active release", "РќРµС‚ Р°РєС‚РёРІРЅРѕРіРѕ СЂРµР»РёР·Р°").to_string())}
                                                </dd>
                                            </div>
                                        </dl>
                                    </div>

                                    <div class="rounded-lg border border-border bg-background/70 p-4 text-sm">
                                        <p class="text-xs uppercase tracking-wide text-muted-foreground">
                                            {tr(locale, "Surface policy", "РџРѕР»РёС‚РёРєР° РїРѕРІРµСЂС…РЅРѕСЃС‚РµР№")}
                                        </p>
                                        <div class="mt-3 space-y-3">
                                            <div class="flex flex-wrap gap-2">
                                                {if module.recommended_admin_surfaces.is_empty() {
                                                    view! {
                                                        <span class="text-xs text-muted-foreground">
                                                            {tr(locale, "No primary admin surface declared.", "Primary admin-РїРѕРІРµСЂС…РЅРѕСЃС‚СЊ РЅРµ РѕР±СЉСЏРІР»РµРЅР°.")}
                                                        </span>
                                                    }
                                                        .into_any()
                                                } else {
                                                    module
                                                        .recommended_admin_surfaces
                                                        .clone()
                                                        .into_iter()
                                                        .map(|surface| {
                                                            view! {
                                                                <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                                                    {format!("{}: {}", tr(locale, "Primary", "Primary"), humanize_token(&surface))}
                                                                </span>
                                                            }
                                                        })
                                                        .collect_view()
                                                        .into_any()
                                                }}
                                            </div>
                                            <div class="flex flex-wrap gap-2">
                                                {if module.showcase_admin_surfaces.is_empty() {
                                                    view! {
                                                        <span class="text-xs text-muted-foreground">
                                                            {tr(locale, "No showcase admin surface declared.", "Showcase admin-РїРѕРІРµСЂС…РЅРѕСЃС‚СЊ РЅРµ РѕР±СЉСЏРІР»РµРЅР°.")}
                                                        </span>
                                                    }
                                                        .into_any()
                                                } else {
                                                    module
                                                        .showcase_admin_surfaces
                                                        .clone()
                                                        .into_iter()
                                                        .map(|surface| {
                                                            view! {
                                                                <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                                                    {format!("{}: {}", tr(locale, "Showcase", "Showcase"), humanize_token(&surface))}
                                                                </span>
                                                            }
                                                        })
                                                        .collect_view()
                                                        .into_any()
                                                }}
                                            </div>
                                            <div class="text-xs text-muted-foreground">
                                                {if module.dependencies.is_empty() {
                                                    tr(locale, "No module dependencies declared.", "Р—Р°РІРёСЃРёРјРѕСЃС‚Рё РјРѕРґСѓР»СЏ РЅРµ РѕР±СЉСЏРІР»РµРЅС‹.").to_string()
                                                } else {
                                                    format!("{}: {}", tr(locale, "Depends on", "Р—Р°РІРёСЃРёС‚ РѕС‚"), module.dependencies.join(", "))
                                                }}
                                            </div>
                                        </div>
                                    </div>
                                </div>

                                <div class="rounded-lg border border-border bg-background/70 p-4 text-sm">
                                    <div class="flex flex-wrap items-center gap-2">
                                        <p class="text-xs uppercase tracking-wide text-muted-foreground">
                                            {tr(locale, "Publish lifecycle", "Р–РёР·РЅРµРЅРЅС‹Р№ С†РёРєР» РїСѓР±Р»РёРєР°С†РёРё")}
                                        </p>
                                        {latest_registry_request.as_ref().map(|request| {
                                            view! {
                                                <span class=registry_request_status_badge_classes(&request.status)>
                                                    {format!("{}: {}", tr(locale, "Request", "Р—Р°РїСЂРѕСЃ"), humanize_token(&request.status))}
                                                </span>
                                            }
                                        })}
                                        {latest_registry_release.as_ref().map(|release| {
                                            view! {
                                                <span class=registry_request_status_badge_classes(&release.status)>
                                                    {format!("{}: {}", tr(locale, "Release", "Р РµР»РёР·"), humanize_token(&release.status))}
                                                </span>
                                            }
                                        })}
                                        {if latest_registry_request.is_none() && latest_registry_release.is_none() {
                                            view! {
                                                <span class=registry_request_status_badge_classes("info")>
                                                    {tr(locale, "No V2 activity yet", "РђРєС‚РёРІРЅРѕСЃС‚Рё V2 РїРѕРєР° РЅРµС‚")}
                                                </span>
                                            }.into_any()
                                        } else {
                                            ().into_any()
                                        }}
                                    </div>
                                    <dl class="mt-3 space-y-2">
                                        <div class="flex items-start justify-between gap-3">
                                            <dt class="text-muted-foreground">{tr(locale, "Owner binding", "РЎРІСЏР·РєР° РІР»Р°РґРµР»СЊС†Р°")}</dt>
                                            <dd class="text-right">
                                                {registry_owner_binding
                                                    .as_ref()
                                                    .map(|owner| owner.owner.clone())
                                                    .unwrap_or_else(|| tr(locale, "No persisted owner binding", "РќРµС‚ СЃРѕС…СЂР°РЅС‘РЅРЅРѕР№ СЃРІСЏР·РєРё РІР»Р°РґРµР»СЊС†Р°").to_string())}
                                            </dd>
                                        </div>
                                        <div class="flex items-start justify-between gap-3">
                                            <dt class="text-muted-foreground">{tr(locale, "Owner bound by", "РљРµРј РїСЂРёРІСЏР·Р°РЅ РІР»Р°РґРµР»РµС†")}</dt>
                                            <dd class="text-right">
                                                {registry_owner_binding
                                                    .as_ref()
                                                    .map(|owner| owner.bound_by.clone())
                                                    .unwrap_or_else(|| tr(locale, "No owner transfer history", "РСЃС‚РѕСЂРёРё РїСЂРёРІСЏР·РєРё РІР»Р°РґРµР»СЊС†Р° РЅРµС‚").to_string())}
                                            </dd>
                                        </div>
                                        <div class="flex items-start justify-between gap-3">
                                            <dt class="text-muted-foreground">{tr(locale, "Owner updated", "Р’Р»Р°РґРµР»РµС† РѕР±РЅРѕРІР»С‘РЅ")}</dt>
                                            <dd class="text-right">
                                                {registry_owner_binding
                                                    .as_ref()
                                                    .map(|owner| owner.updated_at.clone())
                                                    .unwrap_or_else(|| tr(locale, "No owner activity", "РђРєС‚РёРІРЅРѕСЃС‚Рё РїРѕ РІР»Р°РґРµР»СЊС†Сѓ РЅРµС‚").to_string())}
                                            </dd>
                                        </div>
                                        <div class="flex items-start justify-between gap-3">
                                            <dt class="text-muted-foreground">{tr(locale, "Latest request", "РџРѕСЃР»РµРґРЅРёР№ Р·Р°РїСЂРѕСЃ")}</dt>
                                            <dd class="text-right">
                                                {latest_registry_request
                                                    .as_ref()
                                                    .map(|request| format!("{} В· {}", request.id, humanize_token(&request.status)))
                                                    .unwrap_or_else(|| tr(locale, "No publish request recorded", "Р—Р°РїСЂРѕСЃРѕРІ РЅР° РїСѓР±Р»РёРєР°С†РёСЋ РїРѕРєР° РЅРµС‚").to_string())}
                                            </dd>
                                        </div>
                                        <div class="flex items-start justify-between gap-3">
                                            <dt class="text-muted-foreground">{tr(locale, "Request principal", "РџСЂРёРЅС†РёРїР°Р» Р·Р°РїСЂРѕСЃР°")}</dt>
                                            <dd class="text-right">
                                                {latest_registry_request
                                                    .as_ref()
                                                    .map(|request| request.requested_by.clone())
                                                    .unwrap_or_else(|| tr(locale, "Unknown", "РќРµРёР·РІРµСЃС‚РЅРѕ").to_string())}
                                            </dd>
                                        </div>
                                        <div class="flex items-start justify-between gap-3">
                                            <dt class="text-muted-foreground">{tr(locale, "Request publisher", "РР·РґР°С‚РµР»СЊ Р·Р°РїСЂРѕСЃР°")}</dt>
                                            <dd class="text-right">
                                                {latest_registry_request
                                                    .as_ref()
                                                    .and_then(|request| request.publisher.clone())
                                                    .unwrap_or_else(|| tr(locale, "Not persisted", "РќРµ СЃРѕС…СЂР°РЅС‘РЅ").to_string())}
                                            </dd>
                                        </div>
                                        <div class="flex items-start justify-between gap-3">
                                            <dt class="text-muted-foreground">{tr(locale, "Request updated", "Р—Р°РїСЂРѕСЃ РѕР±РЅРѕРІР»С‘РЅ")}</dt>
                                            <dd class="text-right">
                                                {latest_registry_request
                                                    .as_ref()
                                                    .map(|request| request.updated_at.clone())
                                                    .unwrap_or_else(|| tr(locale, "No request activity", "РђРєС‚РёРІРЅРѕСЃС‚Рё РїРѕ Р·Р°РїСЂРѕСЃСѓ РЅРµС‚").to_string())}
                                            </dd>
                                        </div>
                                        <div class="flex items-start justify-between gap-3">
                                            <dt class="text-muted-foreground">{tr(locale, "Latest release state", "РЎРѕСЃС‚РѕСЏРЅРёРµ РїРѕСЃР»РµРґРЅРµРіРѕ СЂРµР»РёР·Р°")}</dt>
                                            <dd class="text-right">
                                                {latest_registry_release
                                                    .as_ref()
                                                    .map(|release: &RegistryReleaseLifecycle| format!(
                                                        "v{} В· {}{}",
                                                        release.version,
                                                        humanize_token(&release.status),
                                                        if status_eq(&release.status, "yanked") {
                                                            release
                                                                .yanked_at
                                                                .as_ref()
                                                                .map(|value| format!(" В· {}", value))
                                                                .unwrap_or_default()
                                                        } else {
                                                            format!(" В· {}", release.published_at)
                                                        }
                                                    ))
                                                    .unwrap_or_else(|| tr(locale, "No persisted release state", "РЎРѕС…СЂР°РЅС‘РЅРЅРѕРіРѕ СЃРѕСЃС‚РѕСЏРЅРёСЏ СЂРµР»РёР·Р° РЅРµС‚").to_string())}
                                            </dd>
                                        </div>
                                    </dl>
                                    <p class="mt-3 text-xs text-muted-foreground">{governance_hint}</p>
                                    <Show when=move || show_validation_summary>
                                        <div class="mt-3 space-y-2">
                                            <p class="text-xs uppercase tracking-wide text-muted-foreground">
                                                {tr(locale, "Validation summary", "РЎРІРѕРґРєР° РІР°Р»РёРґР°С†РёРё")}
                                            </p>
                                            <div class="flex flex-wrap gap-2 text-xs">
                                                {validation_outcome_summary.as_ref().map(|outcome| {
                                                    view! {
                                                        <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                                            {format!("{}: {}", tr(locale, "Outcome", "РС‚РѕРі"), outcome)}
                                                        </span>
                                                    }
                                                })}
                                                <Show when=move || review_ready>
                                                    <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground">
                                                        {tr(locale, "Ready for review", "Р“РѕС‚РѕРІ Рє review")}
                                                    </span>
                                                </Show>
                                                <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                                    {format!("{}: {}", tr(locale, "Warnings", "РџСЂРµРґСѓРїСЂРµР¶РґРµРЅРёСЏ"), validation_warning_items.len())}
                                                </span>
                                                <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                                    {format!("{}: {}", tr(locale, "Errors", "РћС€РёР±РєРё"), validation_error_items.len())}
                                                </span>
                                                {latest_validation_event_summary.as_ref().map(|(title, _, created_at, _)| {
                                                    view! {
                                                        <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                                            {format!("{}: {} В· {}", tr(locale, "Last event", "РџРѕСЃР»РµРґРЅРµРµ СЃРѕР±С‹С‚РёРµ"), title, created_at)}
                                                        </span>
                                                    }
                                                })}
                                            </div>
                                            {latest_validation_event_summary.as_ref().map(|(title, summary, created_at, actor)| {
                                                view! {
                                                    <div class="rounded-lg border border-border bg-background px-3 py-2 text-xs text-muted-foreground">
                                                        <p class="font-medium text-card-foreground">{title.clone()}</p>
                                                        <p class="mt-1">{summary.clone()}</p>
                                                        <p class="mt-1 text-[11px] text-muted-foreground">
                                                            {format!("{}: {} В· {}", tr(locale, "Principal", "РџСЂРёРЅС†РёРїР°Р»"), actor, created_at)}
                                                        </p>
                                                    </div>
                                                }
                                            })}
                                            {latest_validation_job_summary.as_ref().map(|(title, summary, created_at, actor, context_lines)| {
                                                let has_context_lines = !context_lines.is_empty();
                                                let context_lines_for_show = StoredValue::new(context_lines.clone());
                                                view! {
                                                    <div class="rounded-lg border border-border bg-background px-3 py-2 text-xs text-muted-foreground">
                                                        <div class="flex flex-wrap items-start justify-between gap-2">
                                                            <div class="space-y-1">
                                                                <span class=validation_feedback_badge_classes(
                                                                    if title.contains("failed") || title.contains("Failed") {
                                                                        "failed"
                                                                    } else if title.contains("succeeded") || title.contains("Succeeded") {
                                                                        "succeeded"
                                                                    } else {
                                                                        "running"
                                                                    }
                                                                )>
                                                                    {tr(locale, "Validation job trace", "Validation job trace")}
                                                                </span>
                                                                <p class="font-medium text-card-foreground">{title.clone()}</p>
                                                            </div>
                                                            <span class="text-[11px] text-muted-foreground">{created_at.clone()}</span>
                                                        </div>
                                                        <p class="mt-1">{summary.clone()}</p>
                                                        <Show when=move || has_context_lines>
                                                            <div class="mt-2 flex flex-wrap gap-2">
                                                                {context_lines_for_show.get_value().into_iter().map(|line| {
                                                                    view! {
                                                                        <span class="inline-flex items-center rounded-full border border-border/70 bg-background/80 px-2 py-1 text-[11px] text-muted-foreground">
                                                                            {line}
                                                                        </span>
                                                                    }
                                                                }).collect_view()}
                                                            </div>
                                                        </Show>
                                                        <p class="mt-2 text-[11px] text-muted-foreground">
                                                            {format!("{}: {}", tr(locale, "Principal", "РџСЂРёРЅС†РёРїР°Р»"), actor)}
                                                        </p>
                                                    </div>
                                                }
                                            })}
                                            <Show when=move || has_automated_check_items>
                                                <div class="rounded-lg border border-border bg-background px-3 py-2 text-xs text-muted-foreground">
                                                    <p class="font-medium text-card-foreground">
                                                        {tr(locale, "Automated checks", "Automated checks")}
                                                    </p>
                                                    <div class="mt-2 space-y-2">
                                                        {automated_check_items_for_show.get_value().into_iter().map(|check| {
                                                            view! {
                                                                <div class="rounded border border-border/70 bg-background/80 px-2 py-2">
                                                                    <div class="flex flex-wrap items-center justify-between gap-2">
                                                                        <span class="font-medium text-card-foreground">
                                                                            {automated_check_label(&check.key, locale)}
                                                                        </span>
                                                                        <span class=validation_feedback_badge_classes(&check.status)>
                                                                            {humanize_token(&check.status)}
                                                                        </span>
                                                                    </div>
                                                                    <p class="mt-1">{check.detail}</p>
                                                                </div>
                                                            }
                                                        }).collect_view()}
                                                    </div>
                                                </div>
                                            </Show>
                                            <Show when=move || has_validation_warnings>
                                                <div class="rounded-lg border border-amber-300 bg-amber-50 px-3 py-2 text-xs text-amber-900">
                                                    <p class="font-medium">{tr(locale, "Warnings", "РџСЂРµРґСѓРїСЂРµР¶РґРµРЅРёСЏ")}</p>
                                                    <div class="mt-2 space-y-1">
                                                        {validation_warning_items_for_show.get_value().into_iter().map(|warning| {
                                                            view! { <p>{warning}</p> }
                                                        }).collect_view()}
                                                    </div>
                                                </div>
                                            </Show>
                                            <Show when=move || has_validation_errors>
                                                <div class="rounded-lg border border-red-300 bg-red-50 px-3 py-2 text-xs text-red-700">
                                                    <p class="font-medium">{tr(locale, "Errors", "РћС€РёР±РєРё")}</p>
                                                    <div class="mt-2 space-y-1">
                                                        {validation_error_items_for_show.get_value().into_iter().map(|error| {
                                                            view! { <p>{error}</p> }
                                                        }).collect_view()}
                                                    </div>
                                                </div>
                                            </Show>
                                            {validation_rejection_reason.as_ref().map(|reason| {
                                                view! {
                                                    <div class="rounded-lg border border-red-300 bg-red-50 px-3 py-2 text-xs text-red-700">
                                                        <p class="font-medium">{tr(locale, "Rejection reason", "РџСЂРёС‡РёРЅР° РѕС‚РєР»РѕРЅРµРЅРёСЏ")}</p>
                                                        <p class="mt-2">{reason.clone()}</p>
                                                    </div>
                                                }
                                            })}
                                        </div>
                                    </Show>
                                    <Show when=move || show_validation_stages>
                                        <div class="mt-3 space-y-2">
                                            <p class="text-xs uppercase tracking-wide text-muted-foreground">
                                                {tr(locale, "Validation stages", "Validation stages")}
                                            </p>
                                            {validation_stage_summary.as_ref().map(|summary| {
                                                view! {
                                                    <div class="rounded-lg border border-border bg-background px-3 py-2 text-xs text-muted-foreground">
                                                        {summary.clone()}
                                                    </div>
                                                }
                                            })}
                                            {validation_stages_for_show.get_value().into_iter().map(|stage| {
                                                let stage_status = stage.status.clone();
                                                let stage_history =
                                                    validation_stage_recent_history(
                                                        &recent_governance_events_for_show.get_value(),
                                                        &stage.key,
                                                        3,
                                                    );
                                                let has_stage_history = !stage_history.is_empty();
                                                let stage_history_for_show =
                                                    StoredValue::new(stage_history.clone());
                                                view! {
                                                    <div class="rounded-lg border border-border bg-background px-3 py-2 text-xs text-muted-foreground">
                                                        <div class="flex flex-wrap items-center justify-between gap-3">
                                                            <span class="font-medium text-card-foreground">
                                                                {follow_up_gate_label(&stage.key, locale)}
                                                            </span>
                                                            <span class=registry_request_status_badge_classes(&stage_status)>
                                                                {humanize_token(&stage.status)}
                                                            </span>
                                                        </div>
                                                        <p class="mt-1">{stage.detail.clone()}</p>
                                                        <p class="mt-1 text-[11px] text-muted-foreground">
                                                            {format!("{}: {}", tr(locale, "Attempt", "РџРѕРїС‹С‚РєР°"), stage.attempt_number)}
                                                        </p>
                                                        {stage.started_at.as_ref().map(|started_at| {
                                                            view! {
                                                                <p class="mt-1 text-[11px] text-muted-foreground">
                                                                    {format!("{}: {}", tr(locale, "Started", "РќР°С‡Р°С‚Рѕ"), started_at)}
                                                                </p>
                                                            }
                                                        })}
                                                        {stage.finished_at.as_ref().map(|finished_at| {
                                                            view! {
                                                                <p class="mt-1 text-[11px] text-muted-foreground">
                                                                    {format!("{}: {}", tr(locale, "Finished", "Р—Р°РІРµСЂС€РµРЅРѕ"), finished_at)}
                                                                </p>
                                                            }
                                                        })}
                                                        <p class="mt-1 text-[11px] text-muted-foreground">
                                                            {format!("{}: {}", tr(locale, "Updated", "РћР±РЅРѕРІР»РµРЅРѕ"), stage.updated_at)}
                                                        </p>
                                                        <Show when=move || has_stage_history>
                                                            <div class="mt-2 space-y-2 border-t border-border/70 pt-2">
                                                                <p class="text-[11px] uppercase tracking-wide text-muted-foreground">
                                                                    {tr(locale, "Recent stage history", "РќРµРґР°РІРЅСЏСЏ РёСЃС‚РѕСЂРёСЏ СЌС‚Р°РїР°")}
                                                                </p>
                                                                {stage_history_for_show.get_value().into_iter().map(|event| {
                                                                    let title = governance_event_title(&event.event_type, locale);
                                                                    let summary = governance_event_summary(&event, locale);
                                                                    view! {
                                                                        <div class="rounded border border-border/70 bg-background/80 px-2 py-2 text-[11px] text-muted-foreground">
                                                                            <p class="font-medium text-card-foreground">{title}</p>
                                                                            <p class="mt-1">{summary}</p>
                                                                            <p class="mt-1 text-[10px] text-muted-foreground">
                                                                                {format!("{} В· {}", event.actor, event.created_at)}
                                                                            </p>
                                                                        </div>
                                                                    }
                                                                }).collect_view()}
                                                            </div>
                                                        </Show>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </Show>
                                    <Show when=move || show_follow_up_gates>
                                        <div class="mt-3 space-y-2">
                                            <p class="text-xs uppercase tracking-wide text-muted-foreground">
                                                {tr(locale, "Follow-up gates", "Follow-up gates")}
                                            </p>
                                            {follow_up_gate_summary.as_ref().map(|summary| {
                                                view! {
                                                    <div class="rounded-lg border border-border bg-background px-3 py-2 text-xs text-muted-foreground">
                                                        {summary.clone()}
                                                    </div>
                                                }
                                            })}
                                            {follow_up_gates_for_show.get_value().into_iter().map(|gate| {
                                                let gate_status = gate.status.clone();
                                                view! {
                                                    <div class="rounded-lg border border-border bg-background px-3 py-2 text-xs text-muted-foreground">
                                                        <div class="flex flex-wrap items-center justify-between gap-3">
                                                            <span class="font-medium text-card-foreground">
                                                                {follow_up_gate_label(&gate.key, locale)}
                                                            </span>
                                                            <span class=registry_request_status_badge_classes(&gate_status)>
                                                                {humanize_token(&gate.status)}
                                                            </span>
                                                        </div>
                                                        <p class="mt-1">{gate.detail}</p>
                                                        <p class="mt-1 text-[11px] text-muted-foreground">
                                                            {format!("{}: {}", tr(locale, "Updated", "РћР±РЅРѕРІР»РµРЅРѕ"), gate.updated_at)}
                                                        </p>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </Show>
                                    <Show when=move || !review_policy_lines_for_show.is_empty()>
                                        <div class="mt-3 space-y-2">
                                            <p class="text-xs uppercase tracking-wide text-muted-foreground">
                                                {tr(locale, "Moderation policy", "РџРѕР»РёС‚РёРєР° РјРѕРґРµСЂР°С†РёРё")}
                                            </p>
                                            {review_policy_lines.clone().into_iter().map(|line| {
                                                view! {
                                                    <div class="rounded-lg border border-border bg-background px-3 py-2 text-xs text-muted-foreground">
                                                        {line}
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </Show>
                                    <Show when=move || !next_action_lines_for_show.is_empty()>
                                        <div class="mt-3 space-y-2">
                                            <p class="text-xs uppercase tracking-wide text-muted-foreground">
                                                {tr(locale, "Next actions", "РЎР»РµРґСѓСЋС‰РёРµ РґРµР№СЃС‚РІРёСЏ")}
                                            </p>
                                            {next_action_lines.clone().into_iter().map(|line| {
                                                view! {
                                                    <div class="rounded-lg border border-border bg-background px-3 py-2 text-xs text-muted-foreground">
                                                        {line}
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </Show>
                                    <Show when=move || !operator_command_lines_for_show.is_empty()>
                                        <div class="mt-3 space-y-2">
                                            <p class="text-xs uppercase tracking-wide text-muted-foreground">
                                                {tr(locale, "Operator commands", "РљРѕРјР°РЅРґС‹ РѕРїРµСЂР°С‚РѕСЂР°")}
                                            </p>
                                            {operator_command_lines.clone().into_iter().map(|line| {
                                                let copy_label = tr(locale, "Copy", "РљРѕРїРёСЂРѕРІР°С‚СЊ");
                                                let line_for_copy = line.clone();
                                                view! {
                                                    <div class="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-border bg-background px-3 py-2 text-xs text-card-foreground">
                                                        <code class="font-mono break-all">{line}</code>
                                                        <Button
                                                            class="h-7 px-3 py-1 text-xs"
                                                            on_click=Callback::new(move |_| copy_text_to_clipboard(&line_for_copy))
                                                        >
                                                            {copy_label}
                                                        </Button>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </Show>
                                    <Show when=move || !live_api_action_lines_for_show.is_empty()>
                                        <div class="mt-3 space-y-2">
                                            <p class="text-xs uppercase tracking-wide text-muted-foreground">
                                                {tr(locale, "Live API actions", "Live API-РґРµР№СЃС‚РІРёСЏ")}
                                            </p>
                                            {live_api_action_lines.clone().into_iter().map(|item| {
                                                let copy_label = tr(locale, "Copy", "РљРѕРїРёСЂРѕРІР°С‚СЊ");
                                                let copy_curl_label = tr(locale, "Copy cURL", "РљРѕРїРёСЂРѕРІР°С‚СЊ cURL");
                                                let copy_xtask_label = tr(locale, "Copy xtask", "РљРѕРїРёСЂРѕРІР°С‚СЊ xtask");
                                                let line_for_copy = item.endpoint.clone();
                                                let curl_snippet = curl_snippet_for_live_api_action(&item);
                                                let curl_for_copy = curl_snippet.clone();
                                                let xtask_for_copy = item.xtask_hint.clone();
                                                let authority_label = tr(locale, "Allowed actor", "РљС‚Рѕ РјРѕР¶РµС‚ РІС‹Р·С‹РІР°С‚СЊ");
                                                let body_label = tr(locale, "Request body", "РўРµР»Рѕ Р·Р°РїСЂРѕСЃР°");
                                                let headers_label = tr(locale, "Headers", "Р—Р°РіРѕР»РѕРІРєРё");
                                                let curl_label = tr(locale, "cURL", "cURL");
                                                let xtask_label = tr(locale, "xtask", "xtask");
                                                let action_kind_label = if item.write_path {
                                                    tr(locale, "Write-path", "Write-path")
                                                } else {
                                                    tr(locale, "Read-only", "Read-only")
                                                };
                                                view! {
                                                    <div class="rounded-lg border border-border bg-background px-3 py-2 text-xs text-card-foreground">
                                                        <div class="flex flex-wrap items-center justify-between gap-3">
                                                            <div class="flex min-w-0 flex-1 flex-wrap items-center gap-2">
                                                                <code class="font-mono break-all">{item.endpoint.clone()}</code>
                                                                <span class=if item.write_path {
                                                                    "inline-flex items-center rounded-full border border-amber-300 bg-amber-50 px-2 py-0.5 text-[11px] font-semibold text-amber-700"
                                                                } else {
                                                                    "inline-flex items-center rounded-full border border-border px-2 py-0.5 text-[11px] font-semibold text-muted-foreground"
                                                                }>
                                                                    {action_kind_label}
                                                                </span>
                                                            </div>
                                                            <Button
                                                                class="h-7 px-3 py-1 text-xs"
                                                                on_click=Callback::new(move |_| copy_text_to_clipboard(&line_for_copy))
                                                            >
                                                                {copy_label}
                                                            </Button>
                                                            {curl_for_copy.as_ref().map(|snippet| {
                                                                let snippet = snippet.clone();
                                                                view! {
                                                                    <Button
                                                                        class="h-7 px-3 py-1 text-xs"
                                                                        on_click=Callback::new(move |_| copy_text_to_clipboard(&snippet))
                                                                    >
                                                                        {copy_curl_label}
                                                                    </Button>
                                                                }
                                                            })}
                                                            {xtask_for_copy.as_ref().map(|snippet| {
                                                                let snippet = snippet.clone();
                                                                view! {
                                                                    <Button
                                                                        class="h-7 px-3 py-1 text-xs"
                                                                        on_click=Callback::new(move |_| copy_text_to_clipboard(&snippet))
                                                                    >
                                                                        {copy_xtask_label}
                                                                    </Button>
                                                                }
                                                            })}
                                                        </div>
                                                        <p class="mt-2 text-xs text-muted-foreground">
                                                            {format!("{}: {}", authority_label, item.authority)}
                                                        </p>
                                                        {item.note.map(|note| {
                                                            view! {
                                                                <p class="mt-1 text-xs text-muted-foreground">{note}</p>
                                                            }
                                                        })}
                                                        {item.body_hint.map(|body_hint| {
                                                            view! {
                                                                <div class="mt-2">
                                                                    <p class="text-xs text-muted-foreground">{body_label}</p>
                                                                    <code class="mt-1 block rounded-md border border-border bg-background/80 px-2 py-1 font-mono text-[11px] break-all text-muted-foreground">
                                                                        {body_hint}
                                                                    </code>
                                                                </div>
                                                            }
                                                        })}
                                                        {item.header_hint.map(|header_hint| {
                                                            view! {
                                                                <div class="mt-2">
                                                                    <p class="text-xs text-muted-foreground">{headers_label}</p>
                                                                    <code class="mt-1 block whitespace-pre-wrap rounded-md border border-border bg-background/80 px-2 py-1 font-mono text-[11px] break-all text-muted-foreground">
                                                                        {header_hint}
                                                                    </code>
                                                                </div>
                                                            }
                                                        })}
                                                        {item.xtask_hint.map(|xtask_hint| {
                                                            view! {
                                                                <div class="mt-2">
                                                                    <p class="text-xs text-muted-foreground">{xtask_label}</p>
                                                                    <code class="mt-1 block whitespace-pre-wrap rounded-md border border-border bg-background/80 px-2 py-1 font-mono text-[11px] break-all text-muted-foreground">
                                                                        {xtask_hint}
                                                                    </code>
                                                                </div>
                                                            }
                                                        })}
                                                        {curl_snippet.map(|snippet| {
                                                            view! {
                                                                <div class="mt-2">
                                                                    <p class="text-xs text-muted-foreground">{curl_label}</p>
                                                                    <code class="mt-1 block whitespace-pre-wrap rounded-md border border-border bg-background/80 px-2 py-1 font-mono text-[11px] break-all text-muted-foreground">
                                                                        {snippet}
                                                                    </code>
                                                                </div>
                                                            }
                                                        })}
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </Show>
                                    <Show when=move || show_interactive_governance_form>
                                        <GovernanceForm
                                            locale=locale
                                            governance_dry_run=governance_dry_run.into()
                                            set_governance_dry_run=set_governance_dry_run
                                            governance_status_contract=governance_status_contract.into()
                                            governance_status_contract_loading=governance_status_contract_loading.into()
                                            governance_status_contract_error=governance_status_contract_error.into()
                                            has_request_status_contract=has_request_status_contract
                                            governance_new_owner_user_id=governance_new_owner_user_id.into()
                                            set_governance_new_owner_user_id=set_governance_new_owner_user_id
                                            governance_reason_code=governance_reason_code.into()
                                            set_governance_reason_code=set_governance_reason_code
                                            governance_reason=governance_reason.into()
                                            set_governance_reason=set_governance_reason
                                            governance_intent_action=governance_intent_action.into()
                                            set_governance_intent_action=set_governance_intent_action
                                            governance_actions_for_form=governance_actions_for_form.into()
                                            governance_submitting=governance_submitting.into()
                                            governance_confirmation_action=governance_confirmation_action.into()
                                            set_governance_confirmation_action=set_governance_confirmation_action
                                            governance_feedback=governance_feedback.into()
                                            set_governance_feedback=set_governance_feedback
                                            governance_error=governance_error.into()
                                            governance_result=governance_result.into()
                                            on_validate=on_validate_request
                                            on_approve=on_approve_request
                                            on_request_changes=on_request_changes_request
                                            on_hold=on_hold_request
                                            on_resume=on_resume_request
                                            on_reject=on_reject_request
                                            on_transfer_owner=on_transfer_owner
                                            on_yank_release=on_yank_release
                                            on_refresh=on_governance_refresh
                                        />
                                    </Show>
                                    <Show when=move || !lifecycle_note_lines_for_show.is_empty()>
                                        <div class="mt-3 space-y-2">
                                            {lifecycle_note_lines.clone().into_iter().map(|line| {
                                                view! {
                                                    <div class="rounded-lg border border-border bg-background px-3 py-2 text-xs text-muted-foreground">
                                                        {line}
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </Show>
                                    <Show when=move || !recent_governance_events_for_show.get_value().is_empty()>
                                        <Show when=move || !recent_moderation_history_for_show.get_value().is_empty()>
                                            <div class="mt-4 space-y-2">
                                                <p class="text-xs uppercase tracking-wide text-muted-foreground">
                                                    {tr(locale, "Moderation history", "Moderation history")}
                                                </p>
                                                {recent_moderation_history_for_show.get_value().into_iter().map(|event| {
                                                    let title = governance_event_title(&event.event_type, locale);
                                                    let summary = governance_event_summary(&event, locale);
                                                    let actor = event.actor.clone();
                                                    let created_at = event.created_at.clone();
                                                    let context_lines =
                                                        moderation_history_context_lines(&event, locale);
                                                    let has_context_lines = !context_lines.is_empty();
                                                    let context_lines_for_show =
                                                        StoredValue::new(context_lines.clone());
                                                    view! {
                                                        <div class="rounded-lg border border-border bg-background px-3 py-3 text-sm">
                                                            <div class="flex flex-wrap items-start justify-between gap-2">
                                                                <div class="space-y-1">
                                                                    <span class=registry_request_status_badge_classes(
                                                                        moderation_history_badge_status(&event.event_type)
                                                                    )>
                                                                        {moderation_history_badge_label(&event.event_type, locale)}
                                                                    </span>
                                                                    <p class="font-medium text-card-foreground">{title}</p>
                                                                </div>
                                                                <span class="text-xs text-muted-foreground">{created_at}</span>
                                                            </div>
                                                            <p class="mt-2 text-sm text-muted-foreground">{summary}</p>
                                                            <Show when=move || has_context_lines>
                                                                <div class="mt-2 flex flex-wrap gap-2 text-xs text-muted-foreground">
                                                                    {context_lines_for_show.get_value().into_iter().map(|line| {
                                                                        view! {
                                                                            <span class="inline-flex items-center rounded-full border border-border/70 bg-background/80 px-2 py-1">
                                                                                {line}
                                                                            </span>
                                                                        }
                                                                    }).collect_view()}
                                                                </div>
                                                            </Show>
                                                            <p class="mt-2 text-xs text-muted-foreground">
                                                                {format!("{}: {}", tr(locale, "Principal", "РџСЂРёРЅС†РёРїР°Р»"), actor)}
                                                            </p>
                                                        </div>
                                                    }
                                                }).collect_view()}
                                            </div>
                                        </Show>
                                        <div class="mt-4 space-y-2">
                                            <p class="text-xs uppercase tracking-wide text-muted-foreground">
                                                {tr(locale, "Audit trail", "РђСѓРґРёС‚-СЃР»РµРґ")}
                                            </p>
                                            {recent_governance_events_for_show.get_value().into_iter().map(|event| {
                                                let title = governance_event_title(&event.event_type, locale);
                                                let summary = governance_event_summary(&event, locale);
                                                let actor = event.actor.clone();
                                                let created_at = event.created_at.clone();
                                                let publisher = event.publisher.clone();
                                                view! {
                                                    <div class="rounded-lg border border-border bg-background px-3 py-3 text-sm">
                                                        <div class="flex flex-wrap items-center justify-between gap-2">
                                                            <p class="font-medium text-card-foreground">{title}</p>
                                                            <span class="text-xs text-muted-foreground">{created_at}</span>
                                                        </div>
                                                        <p class="mt-2 text-sm text-muted-foreground">{summary}</p>
                                                        <div class="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
                                                            <span>{format!("{}: {}", tr(locale, "Principal", "РџСЂРёРЅС†РёРїР°Р»"), actor)}</span>
                                                            {publisher.map(|publisher| {
                                                                view! {
                                                                    <span>{format!("{}: {}", tr(locale, "Publisher", "РР·РґР°С‚РµР»СЊ"), publisher)}</span>
                                                                }
                                                            })}
                                                        </div>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </Show>
                                </div>

                                <Show when=move || !metadata_checklist_for_show.is_empty()>
                                    <MetadataChecklistView
                                        locale=locale
                                        module_source=module.source.clone()
                                        metadata_checklist=metadata_checklist.clone()
                                        metadata_required_issues=metadata_required_issues
                                        metadata_recommended_gaps=metadata_recommended_gaps
                                        metadata_ready_count=metadata_ready_count
                                    />
                                </Show>
                                {if has_marketplace_visuals {
                                    view! {
                                        <div class="rounded-lg border border-border bg-background/70 p-4">
                                            <p class="text-xs uppercase tracking-wide text-muted-foreground">
                                                {tr(locale, "Marketplace visuals", "Р’РёР·СѓР°Р»С‹ marketplace")}
                                            </p>
                                            <div class="mt-3 space-y-3">
                                                {module_banner_url_for_body.clone().map(|banner_url| {
                                                    let module_name = module_name.clone();
                                                    view! {
                                                        <div class="space-y-2">
                                                            <p class="text-xs text-muted-foreground">{tr(locale, "Banner", "Р‘Р°РЅРЅРµСЂ")}</p>
                                                            <img
                                                                class="max-h-48 w-full rounded-lg border border-border object-cover"
                                                                src=banner_url
                                                                alt=format!("{} banner", module_name)
                                                            />
                                                        </div>
                                                    }
                                                })}
                                                {if has_marketplace_screenshots {
                                                    view! {
                                                        <div class="space-y-2">
                                                            <p class="text-xs text-muted-foreground">{tr(locale, "Screenshots", "РЎРєСЂРёРЅС€РѕС‚С‹")}</p>
                                                            <div class="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
                                                                {module_screenshots_for_body.clone().into_iter().map(|screenshot_url| {
                                                                    let module_name = module_name.clone();
                                                                    view! {
                                                                        <img
                                                                            class="h-32 w-full rounded-lg border border-border object-cover"
                                                                            src=screenshot_url
                                                                            alt=format!("{} screenshot", module_name)
                                                                        />
                                                                    }
                                                                }).collect_view()}
                                                            </div>
                                                        </div>
                                                    }.into_any()
                                                } else {
                                                    ().into_any()
                                                }}
                                            </div>
                                        </div>
                                    }.into_any()
                                } else {
                                    ().into_any()
                                }}

                                <div class="rounded-lg border border-border bg-background/70 p-4">
                                    <div class="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
                                        <div class="space-y-1">
                                            <p class="text-xs uppercase tracking-wide text-muted-foreground">
                                                {tr(locale, "Tenant settings", "РќР°СЃС‚СЂРѕР№РєРё tenant")}
                                            </p>
                                            <p class="text-sm text-muted-foreground">
                                                {if settings_form_supported.get() {
                                                    tr(locale, "This module exposes schema-driven tenant settings from rustok-module.toml.", "This module exposes schema-driven tenant settings from rustok-module.toml.")
                                                } else if settings_editable.get() {
                                                    tr(locale, "Persist raw JSON settings for the current tenant. The payload is stored in tenant_modules.settings.", "РЎРѕС…СЂР°РЅСЏР№С‚Рµ raw JSON-РЅР°СЃС‚СЂРѕР№РєРё РґР»СЏ С‚РµРєСѓС‰РµРіРѕ tenant. Payload С…СЂР°РЅРёС‚СЃСЏ РІ tenant_modules.settings.")
                                                } else {
                                                    tr(locale, "Enable this module for the current tenant before saving settings.", "Р’РєР»СЋС‡РёС‚Рµ СЌС‚РѕС‚ РјРѕРґСѓР»СЊ РґР»СЏ С‚РµРєСѓС‰РµРіРѕ tenant РїРµСЂРµРґ СЃРѕС…СЂР°РЅРµРЅРёРµРј РЅР°СЃС‚СЂРѕРµРє.")
                                                }}
                                            </p>
                                        </div>
                                        <button
                                            type="button"
                                            class="inline-flex items-center justify-center rounded-md bg-primary px-3 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50"
                                            disabled=move || !settings_editable.get() || settings_saving.get()
                                            on:click=move |_| on_save_settings.run(())
                                        >
                                            {move || if settings_saving.get() { tr(locale, "Saving...", "РЎРѕС…СЂР°РЅРµРЅРёРµ...") } else { tr(locale, "Save settings", "РЎРѕС…СЂР°РЅРёС‚СЊ РЅР°СЃС‚СЂРѕР№РєРё") }}
                                        </button>
                                    </div>
                                    <div class="mt-3 flex flex-wrap items-center gap-2 text-xs">
                                        <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                            {move || match tenant_module_for_body.get_value().as_ref() {
                                                Some(module) if module.enabled => tr(locale, "Tenant-enabled", "Р’РєР»СЋС‡С‘РЅ РґР»СЏ tenant").to_string(),
                                                Some(_) => tr(locale, "Tenant-disabled", "Р’С‹РєР»СЋС‡РµРЅ РґР»СЏ tenant").to_string(),
                                                None if settings_editable.get() => tr(locale, "No tenant override yet", "РџРµСЂРµРѕРїСЂРµРґРµР»РµРЅРёСЏ tenant РїРѕРєР° РЅРµС‚").to_string(),
                                                None => tr(locale, "Unavailable until enabled", "РќРµРґРѕСЃС‚СѓРїРЅРѕ РґРѕ РІРєР»СЋС‡РµРЅРёСЏ").to_string(),
                                            }}
                                        </span>
                                        <Show when=move || settings_form_supported.get() && !settings_schema_for_body.get_value().is_empty()>
                                            <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                                {format!(
                                                    "{} {}",
                                                    settings_schema_for_body.get_value().len(),
                                                    tr(locale, "fields", "РїРѕР»РµР№")
                                                )}
                                            </span>
                                        </Show>
                                    </div>
                                    <Show
                                        when=move || settings_form_supported.get() && !settings_schema_for_body.get_value().is_empty()
                                        fallback=move || view! {
                                            <textarea
                                                class="mt-3 min-h-48 w-full rounded-lg border border-border bg-background px-3 py-3 font-mono text-sm text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70"
                                                prop:value=move || settings_draft.get()
                                                disabled=move || !settings_editable.get() || settings_saving.get()
                                                on:input=move |event| on_settings_input.run(event_target_value(&event))
                                            ></textarea>
                                        }
                                    >
                                        <div class="mt-4 grid gap-4 md:grid-cols-2">
                                            {move || {
                                                settings_schema_for_body
                                                    .get_value()
                                                    .into_iter()
                                                    .map(|field| {
                                                        let field_key = field.key.clone();
                                                        let field_label = humanize_setting_key(&field.key);
                                                        let field_hint = setting_field_hint(&field, locale);
                                                        let field_description = field.description.clone();
                                                        let field_type = field.value_type.clone();
                                                        let field_options = field.options.clone();
                                                        let value_for_text = {
                                                            let field_key = field_key.clone();
                                                            move || {
                                                                settings_form_draft
                                                                    .get()
                                                                    .get(&field_key)
                                                                    .cloned()
                                                                    .unwrap_or_default()
                                                            }
                                                        };
                                                        let disabled = Signal::derive(move || {
                                                            !settings_editable.get() || settings_saving.get()
                                                        });

                                                        view! {
                                                            <div class="space-y-2 rounded-lg border border-border bg-background px-4 py-3">
                                                                <div class="space-y-1">
                                                                    <div class="flex flex-wrap items-center gap-2">
                                                                        <label class="text-sm font-medium text-card-foreground">
                                                                            {field_label}
                                                                        </label>
                                                                        <span class="inline-flex items-center rounded-full border border-border px-2 py-0.5 text-[11px] font-medium text-muted-foreground">
                                                                            {field.value_type.clone()}
                                                                        </span>
                                                                    </div>
                                                                    {field_description.map(|description| view! {
                                                                        <p class="text-xs text-muted-foreground">{description}</p>
                                                                    })}
                                                                    {field_hint.map(|hint| view! {
                                                                        <p class="text-[11px] text-muted-foreground">{hint}</p>
                                                                    })}
                                                                </div>

                                                                {match field_type.as_str() {
                                                                    "boolean" => {
                                                                        if !field_options.is_empty() {
                                                                            let field_key_for_select = field_key.clone();
                                                                            let field_type_for_select = field_type.clone();
                                                                            let options_for_select = field_options.clone();
                                                                            view! {
                                                                                <select
                                                                                    class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70"
                                                                                    prop:value=value_for_text
                                                                                    disabled=move || disabled.get()
                                                                                    on:change=move |event| {
                                                                                        on_settings_field_input.run((
                                                                                            field_key_for_select.clone(),
                                                                                            event_target_value(&event),
                                                                                        ))
                                                                                    }
                                                                                >
                                                                                    {options_for_select.into_iter().map(|option| {
                                                                                        let option_value = setting_option_draft_value(&field_type_for_select, &option);
                                                                                        let option_label = setting_option_label(&option);
                                                                                        view! {
                                                                                            <option value=option_value>{option_label}</option>
                                                                                        }
                                                                                    }).collect_view()}
                                                                                </select>
                                                                            }.into_any()
                                                                        } else {
                                                                            let field_key_for_toggle = field_key.clone();
                                                                            view! {
                                                                                <label class="inline-flex items-center gap-3 text-sm text-card-foreground">
                                                                                    <input
                                                                                        type="checkbox"
                                                                                        class="h-4 w-4 rounded border-border text-primary focus:ring-primary/20"
                                                                                        prop:checked=move || value_for_text() == "true"
                                                                                        disabled=move || disabled.get()
                                                                                        on:change=move |event| {
                                                                                            on_settings_field_input.run((
                                                                                                field_key_for_toggle.clone(),
                                                                                                if event_target_checked(&event) {
                                                                                                    "true".to_string()
                                                                                                } else {
                                                                                                    "false".to_string()
                                                                                                },
                                                                                            ))
                                                                                        }
                                                                                    />
                                                                                    <span>{tr(locale, "Enabled", "Р’РєР»СЋС‡РµРЅРѕ")}</span>
                                                                                </label>
                                                                            }.into_any()
                                                                        }
                                                                    }
                                                                    "integer" | "number" => {
                                                                        if !field_options.is_empty() {
                                                                            let field_key_for_select = field_key.clone();
                                                                            let field_type_for_select = field_type.clone();
                                                                            let options_for_select = field_options.clone();
                                                                            view! {
                                                                                <select
                                                                                    class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70"
                                                                                    prop:value=value_for_text
                                                                                    disabled=move || disabled.get()
                                                                                    on:change=move |event| {
                                                                                        on_settings_field_input.run((
                                                                                            field_key_for_select.clone(),
                                                                                            event_target_value(&event),
                                                                                        ))
                                                                                    }
                                                                                >
                                                                                    {options_for_select.into_iter().map(|option| {
                                                                                        let option_value = setting_option_draft_value(&field_type_for_select, &option);
                                                                                        let option_label = setting_option_label(&option);
                                                                                        view! {
                                                                                            <option value=option_value>{option_label}</option>
                                                                                        }
                                                                                    }).collect_view()}
                                                                                </select>
                                                                            }.into_any()
                                                                        } else {
                                                                            let field_key_for_input = field_key.clone();
                                                                            let step = if field_type == "integer" { "1" } else { "any" };
                                                                            view! {
                                                                                <input
                                                                                    type="number"
                                                                                    step=step
                                                                                    class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70"
                                                                                    prop:value=value_for_text
                                                                                    disabled=move || disabled.get()
                                                                                    on:input=move |event| {
                                                                                        on_settings_field_input.run((
                                                                                            field_key_for_input.clone(),
                                                                                            event_target_value(&event),
                                                                                        ))
                                                                                    }
                                                                                />
                                                                            }.into_any()
                                                                        }
                                                                    }
                                                                    "object" | "array" | "json" | "any" => {
                                                                        let field_key_for_input = field_key.clone();
                                                                        let placeholder = setting_field_placeholder(&field).unwrap_or_default();
                                                                        view! {
                                                                            <ComplexSettingEditor
                                                                                field_type=field_type.clone()
                                                                                placeholder=placeholder
                                                                                array_item_type=field.item_type.clone()
                                                                                schema_shape=field.shape.clone()
                                                                                value=Signal::derive(value_for_text)
                                                                                disabled=disabled
                                                                                on_input=Callback::new(move |next| {
                                                                                    on_settings_field_input.run((
                                                                                        field_key_for_input.clone(),
                                                                                        next,
                                                                                    ))
                                                                                })
                                                                            />
                                                                        }.into_any()
                                                                    }
                                                                    _ => {
                                                                        if !field_options.is_empty() {
                                                                            let field_key_for_select = field_key.clone();
                                                                            let field_type_for_select = field_type.clone();
                                                                            let options_for_select = field_options.clone();
                                                                            view! {
                                                                                <select
                                                                                    class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70"
                                                                                    prop:value=value_for_text
                                                                                    disabled=move || disabled.get()
                                                                                    on:change=move |event| {
                                                                                        on_settings_field_input.run((
                                                                                            field_key_for_select.clone(),
                                                                                            event_target_value(&event),
                                                                                        ))
                                                                                    }
                                                                                >
                                                                                    {options_for_select.into_iter().map(|option| {
                                                                                        let option_value = setting_option_draft_value(&field_type_for_select, &option);
                                                                                        let option_label = setting_option_label(&option);
                                                                                        view! {
                                                                                            <option value=option_value>{option_label}</option>
                                                                                        }
                                                                                    }).collect_view()}
                                                                                </select>
                                                                            }.into_any()
                                                                        } else {
                                                                            let field_key_for_input = field_key.clone();
                                                                            view! {
                                                                                <input
                                                                                    type="text"
                                                                                    class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70"
                                                                                    prop:value=value_for_text
                                                                                    disabled=move || disabled.get()
                                                                                    on:input=move |event| {
                                                                                        on_settings_field_input.run((
                                                                                            field_key_for_input.clone(),
                                                                                            event_target_value(&event),
                                                                                        ))
                                                                                    }
                                                                                />
                                                                            }.into_any()
                                                                        }
                                                                    }
                                                                }}
                                                            </div>
                                                        }
                                                    })
                                                    .collect_view()
                                            }}
                                        </div>
                                    </Show>
                                    <p class="mt-2 text-xs text-muted-foreground">
                                        {move || {
                                            if settings_form_supported.get() && !settings_schema_for_body.get_value().is_empty() {
                                                format!(
                                                    "{} `{}`. {}",
                                                    tr(locale, "Editing schema-driven settings for", "Р РµРґР°РєС‚РёСЂРѕРІР°РЅРёРµ schema-driven РЅР°СЃС‚СЂРѕРµРє РґР»СЏ"),
                                                    selected_slug_for_body.get_value(),
                                                    tr(locale, "Complex fields accept JSON per field.", "РЎР»РѕР¶РЅС‹Рµ РїРѕР»СЏ РїСЂРёРЅРёРјР°СЋС‚ JSON РїРѕ РєР°Р¶РґРѕРјСѓ РїРѕР»СЋ.")
                                                )
                                            } else {
                                                format!(
                                                    "{} `{}`.",
                                                    tr(locale, "Editing raw JSON settings for", "Р РµРґР°РєС‚РёСЂРѕРІР°РЅРёРµ raw JSON-РЅР°СЃС‚СЂРѕРµРє РґР»СЏ"),
                                                    selected_slug_for_body.get_value()
                                                )
                                            }
                                        }}
                                    </p>
                                </div>

                                <VersionTrailView
                                    locale=locale
                                    version_trail=version_trail.clone()
                                    loading=loading
                                />
                            </div>
                        }
                    })
                }}
            </Show>
        </div>
    }
}
